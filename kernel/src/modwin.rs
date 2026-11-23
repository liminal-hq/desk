// The visible-module spike's kernel half: three window services a loaded module may call while the shell is lent to it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `docs/LOADER.md` ("Visible-module spike") is the design. A module draws in a window of the native shell through `desk_window_new`,
// `desk_fill` and `desk_text`; it is never handed pixels or the frame buffer (`docs/PRESENT-API.md`), only a handle and what to draw,
// and the shell paints. The window services can be bound only by `module::resolve_window`, which the kernel uses while it is about to
// lend a shell, and they reach the shell through `SESSION`, which is set only for the length of `app_start` or `app_stop`.
// Everything here is throwaway until R78 (protection), R79 (the SDK's names) and M6 to M9 of the loader plan replace it. A service
// that refuses a call prints one line and does nothing else; `desk_fill` and `desk_text` return nothing, as `docs/SDK.md` has them.

use alloc::format;
use alloc::string::String;
use core::sync::atomic::{AtomicPtr, Ordering};

use desk_gfx::Rect;
use desk_module::{load_into, Placed, KIND_MODULE};
use desk_shell::{Shell, MAX_ITEMS};
use desk_ui::{ChromeLayout, Role, TITLE_HEIGHT};

use crate::module::{parse, resolve_base, resolve_window, Frames};
use crate::{memory, println};

static HELLOWIN: &[u8] = include_bytes!("../../dist/modules/HELLOWIN.DMD");

/// The windows one module instance may hold at once.
const SLOTS: usize = 4;
/// The longest title and the longest line of text a service takes, in bytes.
const TITLE_MAX: u32 = 64;
const TEXT_MAX: u32 = 160;
/// The largest content area, so a window fits the 640 x 480 screen with its chrome.
const WIDTH_MAX: i32 = 600;
const HEIGHT_MAX: i32 = 400;

/// One window of the running instance: whether the slot is used and how many items the instance has drawn in it.
#[derive(Clone, Copy)]
struct Slot {
    open: bool,
    items: usize,
}

/// What the services need while a module runs: the shell they draw in, the running instance and its windows.
struct Session {
    /// The shell lent for the call, set by `lend` from a `&mut` and used by nothing else until the call ends.
    shell: *mut Shell<'static>,
    app: u32,
    screen: (u32, u32),
    slots: [Slot; SLOTS],
}

/// The lent session: null except during `app_start` and `app_stop`. Only loads and stores, never compare-and-swap (a 486 has none
/// for the compiler to rely on).
static SESSION: AtomicPtr<Session> = AtomicPtr::new(core::ptr::null_mut());

/// Runs `call` with `shell` lent to the module through `session`.
///
/// The kernel stores the pointer, runs the call, stores null. It does not touch the shell or the session while the call runs, and no
/// service calls back into a module, so at most one `&mut` to either exists at a time.
fn lend<R>(session: &mut Session, shell: &mut Shell<'static>, call: impl FnOnce() -> R) -> R {
    session.shell = shell;
    SESSION.store(session, Ordering::SeqCst);
    let result = call();
    SESSION.store(core::ptr::null_mut(), Ordering::SeqCst);
    result
}

/// The session and the shell it lent, for a service. A service the module can bind is only called inside `lend`, so a null pointer
/// is a kernel bug and panics.
fn with_session<R>(f: impl FnOnce(&mut Session, &mut Shell<'static>) -> R) -> R {
    let p = SESSION.load(Ordering::SeqCst);
    assert!(!p.is_null(), "a window service was called with no shell lent");
    // SAFETY: `lend` stored the address of a live `Session` and does not use it until the call returns; its `shell` was set from a
    // `&mut Shell` that `lend`'s caller does not use during the call; this is the only reference to either.
    let (session, shell) = unsafe { (&mut *p, &mut *(*p).shell) };
    f(session, shell)
}

/// The name the shell knows a window by.
fn window_name(app: u32, handle: u32) -> String {
    format!("app{}-{}", app, handle)
}

/// Reads `len` bytes at `ptr` for a service, or says why not: longer than `max`, null with a length, or a range that wraps past the
/// end of the address space. Whether the bytes belong to the module is not checked (R78's job, as `svc_log` notes).
fn bytes<'a>(ptr: *const u8, len: u32, max: u32) -> Result<&'a [u8], String> {
    if len > max {
        return Err(format!("{} bytes is more than the {} allowed", len, max));
    }
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(String::from("a null pointer with a length"));
    }
    if (ptr as usize).checked_add(len as usize).is_none() {
        return Err(String::from("the pointer and length wrap past the end of memory"));
    }
    // SAFETY: the range does not wrap; the spike's modules are trusted for the rest, as in `svc_log`.
    Ok(unsafe { core::slice::from_raw_parts(ptr, len as usize) })
}

/// `desk_window_new(app, title, len, w, h, flags) -> handle`: opens a window of content `w` by `h`, centred and focused; 0 refuses.
pub(crate) extern "C" fn svc_window_new(app: u32, title: *const u8, len: u32, w: i32, h: i32, flags: u32) -> u32 {
    let refused = |why: String| {
        println!("module: desk_window_new refused: {}", why);
        0
    };
    with_session(|s, shell| {
        if app != s.app {
            return refused(format!("app {} is not the running instance ({})", app, s.app));
        }
        if flags != 0 {
            return refused(format!("flags {:#x} are not defined", flags));
        }
        let bytes = match bytes(title, len, TITLE_MAX) {
            Ok(b) => b,
            Err(why) => return refused(format!("the title: {}", why)),
        };
        let Ok(title) = core::str::from_utf8(bytes) else {
            return refused(String::from("the title is not UTF-8"));
        };
        if !(1..=WIDTH_MAX).contains(&w) {
            return refused(format!("content width {} is outside 1..={}", w, WIDTH_MAX));
        }
        if !(1..=HEIGHT_MAX).contains(&h) {
            return refused(format!("content height {} is outside 1..={}", h, HEIGHT_MAX));
        }
        let Some(slot) = s.slots.iter().position(|slot| !slot.open) else {
            return refused(format!("all {} windows are in use", SLOTS));
        };
        let handle = slot as u32 + 1;
        // The content is w by h; the outer box adds the title bar and its rule. A narrow window is widened by the chrome, and the
        // layout says by how much.
        let layout = ChromeLayout::new(Rect::new(0, 0, w as u32, h as u32 + TITLE_HEIGHT + 1));
        let (ow, oh) = (layout.outer.width() as i32, layout.outer.height() as i32);
        let (x, y) = ((s.screen.0 as i32 - ow) / 2, (s.screen.1 as i32 - oh) / 2);
        shell.open(&window_name(app, handle), Rect::new(x, y, ow as u32, oh as u32), title);
        s.slots[slot] = Slot { open: true, items: 0 };
        println!(
            "module: window {} for app {}: \"{}\", content {}x{}, outer at {},{}",
            handle,
            app,
            title,
            layout.content.width(),
            layout.content.height(),
            x,
            y
        );
        handle
    })
}

/// The slot of an open window of this session, or why the handle names none.
fn open_slot(s: &Session, shell: &mut Shell<'static>, win: u32) -> Result<usize, String> {
    let slot = (win as usize).checked_sub(1).filter(|i| *i < SLOTS && s.slots[*i].open);
    match slot {
        Some(i) if shell.wm().by_name(&window_name(s.app, win)).is_some() => Ok(i),
        _ => Err(format!("window {} is not open", win)),
    }
}

/// `desk_fill(win, x, y, w, h, role)`: a filled rectangle in content coordinates, clipped to the content area when painted.
pub(crate) extern "C" fn svc_fill(win: u32, x: i32, y: i32, w: i32, h: i32, role: u32) {
    with_session(|s, shell| {
        let refused = |why: String| println!("module: desk_fill refused: {}", why);
        let slot = match open_slot(s, shell, win) {
            Ok(i) => i,
            Err(why) => return refused(why),
        };
        if w <= 0 || h <= 0 {
            return refused(format!("the rectangle {}x{} is empty", w, h));
        }
        let Some(&role) = Role::ALL.get(role as usize) else {
            return refused(format!("role {} is not one of 0..{}", role, Role::COUNT));
        };
        if s.slots[slot].items >= MAX_ITEMS {
            return refused(format!("window {} already holds {} items", win, MAX_ITEMS));
        }
        if shell.fill(&window_name(s.app, win), Rect::new(x, y, w as u32, h as u32), role) {
            s.slots[slot].items += 1;
        } else {
            refused(format!("the shell would not take the rectangle for window {}", win));
        }
    })
}

/// `desk_text(win, x, y, s, len, font, role)`: a line of text at a position of the content area in a face and a role.
pub(crate) extern "C" fn svc_text(win: u32, x: i32, y: i32, text: *const u8, len: u32, font: u32, role: u32) {
    with_session(|s, shell| {
        let refused = |why: String| println!("module: desk_text refused: {}", why);
        let slot = match open_slot(s, shell, win) {
            Ok(i) => i,
            Err(why) => return refused(why),
        };
        let Some(&role) = Role::ALL.get(role as usize) else {
            return refused(format!("role {} is not one of 0..{}", role, Role::COUNT));
        };
        if s.slots[slot].items >= MAX_ITEMS {
            return refused(format!("window {} already holds {} items", win, MAX_ITEMS));
        }
        if font != 0 {
            return refused(format!("font {} does not exist (only 0)", font));
        }
        let bytes = match bytes(text, len, TEXT_MAX) {
            Ok(b) => b,
            Err(why) => return refused(format!("the text: {}", why)),
        };
        let Ok(text) = core::str::from_utf8(bytes) else {
            return refused(String::from("the text is not UTF-8"));
        };
        if text.chars().any(char::is_control) {
            return refused(String::from("the text has a control character"));
        }
        if shell.text(&window_name(s.app, win), x, y, text, role) {
            s.slots[slot].items += 1;
        } else {
            refused(format!("the shell would not take the text for window {}", win));
        }
    })
}

/// Loads HELLOWIN.DMD with the base services only, which must be refused: the window services cannot be bound outside a shell mode.
pub fn refuse_outside_shell() {
    let m = parse("HELLOWIN.DMD", HELLOWIN).expect("HELLOWIN.DMD parses");
    let mut frames = Frames::take(m.header().mem_size as usize).expect("no frames for the module");
    let base = frames.base;
    match load_into(&m, KIND_MODULE, frames.memory(), base, &resolve_base) {
        Err(e) => println!("module: refused HELLOWIN.DMD: {}", e),
        Ok(_) => panic!("HELLOWIN.DMD loaded with no window services"),
    }
}

/// HELLOWIN.DMD loaded into frames, with the windows it holds.
pub struct Instance {
    frames: Frames,
    placed: Placed,
    session: Session,
    free_before: usize,
}

/// The instance number HELLOWIN runs as (the module loader has no instance table yet).
const APP: u32 = 1;

impl Instance {
    /// Loads HELLOWIN.DMD for a shell of `screen` size with the window services bound, and prints what the loader did.
    pub fn load(screen: (u32, u32)) -> Instance {
        let m = parse("HELLOWIN.DMD", HELLOWIN).expect("HELLOWIN.DMD is refused");
        let h = *m.header();
        println!(
            "module: HELLOWIN.DMD {} bytes, image {}, {} relocations, {} imports",
            HELLOWIN.len(),
            h.mem_size,
            h.reloc_count,
            h.import_count
        );
        let free_before = memory::free_frames();
        let mut frames = Frames::take(h.mem_size as usize).expect("no frames for the module");
        let base = frames.base;
        let placed = match load_into(&m, KIND_MODULE, frames.memory(), base, &resolve_window) {
            Ok(p) => p,
            Err(e) => panic!("HELLOWIN.DMD did not load: {}", e),
        };
        println!(
            "module: HELLOWIN loaded away from its link base: {}",
            if placed.base != h.link_base { "yes" } else { "no" }
        );
        let session = Session {
            shell: core::ptr::null_mut(),
            app: APP,
            screen,
            slots: [Slot { open: false, items: 0 }; SLOTS],
        };
        Instance {
            frames,
            placed,
            session,
            free_before,
        }
    }

    /// Calls `app_start` with `shell` lent.
    pub fn start(&mut self, shell: &mut Shell<'static>) {
        // SAFETY: `start` is the address of `app_start` inside the image `load_into` placed and relocated; the guest crate defines
        // it as `extern "C" fn(app: u32, args: *const u8, args_len: u32) -> i32`, the signature transmuted to.
        let start: extern "C" fn(u32, *const u8, u32) -> i32 = unsafe { core::mem::transmute(self.placed.start as usize) };
        let status = lend(&mut self.session, shell, || start(APP, core::ptr::null(), 0));
        println!("module: HELLOWIN app_start returned {}", status);
    }

    /// Calls `app_stop` with `shell` lent, closes the windows the instance holds and frees its frames.
    pub fn unload(mut self, shell: &mut Shell<'static>) {
        if let Some(stop) = self.placed.stop {
            // SAFETY: as in `start`, for `app_stop(app: u32)`.
            let stop: extern "C" fn(u32) = unsafe { core::mem::transmute(stop as usize) };
            lend(&mut self.session, shell, || stop(APP));
        }
        let mut closed = 0;
        for (i, slot) in self.session.slots.iter().enumerate() {
            if slot.open && shell.close(&window_name(APP, i as u32 + 1)) {
                closed += 1;
            }
        }
        drop(self.frames);
        println!(
            "module: HELLOWIN unloaded, {} window{} closed, frames as before: {}",
            closed,
            if closed == 1 { "" } else { "s" },
            if memory::free_frames() == self.free_before {
                "yes"
            } else {
                "no"
            }
        );
    }
}
