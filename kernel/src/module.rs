// The module loader's native half: loads HELLO.DMD from a buffer embedded in the kernel, runs it and unloads it (docs/LOADER.md).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use desk_module::{load_into, FormatError, LoadError, Module, Placed, KIND_MODULE};

use crate::{heap, memory, modwin, println};

/// The newest service table this kernel provides: SDK 1 is the four spike services (`docs/LOADER.md`).
const CORE_SDK: u16 = 1;

/// The test modules, packed by `tools/build-modules.sh`, which `tools/native-build.sh` runs first.
static HELLO: &[u8] = include_bytes!("../../dist/modules/HELLO.DMD");
static APPMAIN: &[u8] = include_bytes!("../../dist/modules/APPMAIN.DKX");

/// `desk_log(text, len)`: one line from the module, printed behind a marker so the transcript shows who wrote it.
extern "C" fn svc_log(text: *const u8, len: usize) {
    // SAFETY: the module passes a pointer and a length it owns; the spike's modules are trusted, and a module's access is checked
    // by the protection that R78 adds, not here.
    let bytes = unsafe { core::slice::from_raw_parts(text, len) };
    println!("  log> {}", core::str::from_utf8(bytes).unwrap_or("<not UTF-8>"));
}

/// `desk_panic(text, len)`: a module panicked. Containing it is task M7's; until then it is a kernel panic that carries the text.
extern "C" fn svc_panic(text: *const u8, len: usize) {
    // SAFETY: as in `svc_log`.
    let bytes = unsafe { core::slice::from_raw_parts(text, len) };
    panic!("module panic: {}", core::str::from_utf8(bytes).unwrap_or("<not UTF-8>"));
}

/// A service's function-pointer type, by the shape of the call. The table holds function pointers because an address cannot be turned
/// into an integer in a `const`; `address` does it at load.
#[derive(Clone, Copy)]
pub(crate) enum Service {
    /// `desk_log` and `desk_panic`: a text and its length.
    Text(extern "C" fn(*const u8, usize)),
    /// `desk_window_new` (`modwin.rs`).
    WindowNew(extern "C" fn(u32, *const u8, u32, i32, i32, u32) -> u32),
    /// `desk_fill`.
    Fill(extern "C" fn(u32, i32, i32, i32, i32, u32)),
    /// `desk_text`.
    DrawText(extern "C" fn(u32, i32, i32, *const u8, u32, u32, u32)),
}

impl Service {
    fn address(self) -> u32 {
        match self {
            Service::Text(f) => f as usize as u32,
            Service::WindowNew(f) => f as usize as u32,
            Service::Fill(f) => f as usize as u32,
            Service::DrawText(f) => f as usize as u32,
        }
    }

    /// Whether the service is one of the window services, which only a shell mode may bind.
    fn is_window(self) -> bool {
        !matches!(self, Service::Text(_))
    }
}

/// The services by name, sorted. M4 binds `desk_log`, and `desk_panic` because the guest crate's panic handler links it into
/// every module, so `hello` imports both (the plan's table listed `desk_log` alone); `desk_alloc` and `desk_free` arrive with
/// M6. The visible-module spike adds the three window services (`docs/LOADER.md`), which `resolve_window` binds and `resolve_base`
/// does not.
const SERVICES: [(&str, Service); 5] = [
    ("desk_fill", Service::Fill(modwin::svc_fill)),
    ("desk_log", Service::Text(svc_log)),
    ("desk_panic", Service::Text(svc_panic)),
    ("desk_text", Service::DrawText(modwin::svc_text)),
    ("desk_window_new", Service::WindowNew(modwin::svc_window_new)),
];

/// The names must ascend strictly, so a table that is out of order or repeats a name does not build.
const _: () = {
    let mut i = 1;
    while i < SERVICES.len() {
        let (a, b) = (SERVICES[i - 1].0.as_bytes(), SERVICES[i].0.as_bytes());
        let mut j = 0;
        while j < a.len() && j < b.len() && a[j] == b[j] {
            j += 1;
        }
        // Ascending: the first difference is smaller in `a`, or `a` is a proper prefix of `b`.
        let ascends = if j < a.len() && j < b.len() { a[j] < b[j] } else { j == a.len() && j < b.len() };
        assert!(ascends, "SERVICES is not sorted");
        i += 1;
    }
};

fn find(name: &str) -> Option<Service> {
    SERVICES.iter().find(|(n, _)| *n == name).map(|&(_, service)| service)
}

/// The address of a service the earlier modes' modules may bind: `desk_log` and `desk_panic`.
pub(crate) fn resolve_base(name: &str) -> Option<u32> {
    find(name).filter(|s| !s.is_window()).map(Service::address)
}

/// The address of any service. Used only when a shell is about to be lent to the module (`modwin.rs`), so outside a shell mode a
/// window service cannot be bound at all.
pub(crate) fn resolve_window(name: &str) -> Option<u32> {
    find(name).map(Service::address)
}

/// Frames for an image, freed frame by frame when dropped.
pub(crate) struct Frames {
    pub(crate) base: u32,
    count: usize,
    bytes: usize,
}

impl Frames {
    pub(crate) fn take(bytes: usize) -> Option<Frames> {
        let count = bytes.div_ceil(memory::FRAME_SIZE as usize);
        let base = memory::alloc_contiguous(count)?;
        Some(Frames { base, count, bytes })
    }

    pub(crate) fn memory(&mut self) -> &mut [u8] {
        // SAFETY: the frames were just taken from the allocator, are identity-mapped (there is no paging) and nothing else refers
        // to them; the slice lives no longer than the borrow of `self`.
        unsafe { core::slice::from_raw_parts_mut(self.base as *mut u8, self.bytes) }
    }
}

impl Drop for Frames {
    fn drop(&mut self) {
        for i in 0..self.count as u32 {
            memory::free_frame(self.base + i * memory::FRAME_SIZE);
        }
    }
}

/// Says why a file was refused, in the transcript's words.
fn refuse(name: &str, error: &dyn core::fmt::Display) {
    println!("module: refused {}: {}", name, error);
}

/// Parses a file and prints the reason when it is refused.
pub(crate) fn parse<'a>(name: &str, bytes: &'a [u8]) -> Option<Module<'a>> {
    match Module::parse(bytes, CORE_SDK) {
        Ok(m) => Some(m),
        Err(FormatError::BadChecksum { stored, computed }) => {
            println!(
                "module: refused {}: bad checksum (stored {:#010x}, computed {:#010x})",
                name, stored, computed
            );
            None
        }
        Err(e) => {
            refuse(name, &e);
            None
        }
    }
}

/// Loads, runs and unloads HELLO.DMD, then shows that an application is refused. With `bad`, a copy of HELLO.DMD with one bit of
/// the image flipped is offered instead and must be refused.
pub fn run(bad: bool) {
    if bad {
        let mut copy: Vec<u8> = HELLO.to_vec();
        let at = Module::parse(HELLO, CORE_SDK)
            .expect("HELLO.DMD parses")
            .header()
            .image_offset as usize
            + 16;
        copy[at] ^= 0x01;
        assert!(parse("HELLO.DMD", &copy).is_none(), "a damaged module was accepted");
        return;
    }

    let m = parse("HELLO.DMD", HELLO).expect("HELLO.DMD is refused");
    let h = *m.header();
    println!(
        "module: HELLO.DMD {} bytes, image {}, {} relocations, {} imports",
        HELLO.len(),
        h.mem_size,
        h.reloc_count,
        h.import_count
    );
    let frames_before = memory::free_frames();
    let heap_before = heap::free_bytes();

    let mut frames = Frames::take(h.mem_size as usize).expect("no frames for the module");
    let base = frames.base;
    let placed: Placed = match load_into(&m, KIND_MODULE, frames.memory(), base, &resolve_base) {
        Ok(p) => p,
        Err(e) => {
            refuse("HELLO.DMD", &e);
            panic!("HELLO.DMD did not load");
        }
    };
    println!(
        "module: HELLO loaded away from its link base: {}",
        if placed.base != h.link_base { "yes" } else { "no" }
    );

    const APP: u32 = 1;
    // SAFETY: `start` is the address of `app_start` inside the image that `load_into` just placed and relocated; the guest crate
    // defines it as `extern "C" fn(app: u32, args: *const u8, args_len: u32) -> i32`, which is the signature transmuted to.
    let start: extern "C" fn(u32, *const u8, u32) -> i32 = unsafe { core::mem::transmute(placed.start as usize) };
    let status = start(APP, core::ptr::null(), 0);
    println!("module: HELLO app_start returned {}", status);
    if let Some(stop) = placed.stop {
        // SAFETY: as above, for `app_stop(app: u32)`.
        let stop: extern "C" fn(u32) = unsafe { core::mem::transmute(stop as usize) };
        stop(APP);
    }

    drop(frames);
    let same = memory::free_frames() == frames_before && heap::free_bytes() == heap_before;
    println!(
        "module: HELLO unloaded, memory as before: {}",
        if same { "yes" } else { "no" }
    );

    // An application needs a task (R77), so the module loader refuses it.
    let app = parse("APPMAIN.DKX", APPMAIN).expect("APPMAIN.DKX parses");
    let mut none: [u8; 0] = [];
    match load_into(&app, KIND_MODULE, &mut none, 0x1000, &resolve_base) {
        Err(LoadError::KindNotLoadable { kind: 2, .. }) => {
            println!("module: refused APPMAIN.DKX: an application (kind 2) is not loadable here")
        }
        Err(e) => refuse("APPMAIN.DKX", &e),
        Ok(_) => panic!("an application was loaded"),
    }
}
