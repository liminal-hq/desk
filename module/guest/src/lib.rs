// The app side of the module loader: import slots the loader fills, the three entry points, a log line and the panic handler.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// This is the seed of the SDK crate (R79), not the SDK: it knows the spike's four services (`desk_log`, `desk_panic`, `desk_alloc`,
// `desk_free`) and the entry-point shapes of docs/LOADER.md. A module is a `no_std`, `no_main` binary linked with `module.ld`; the
// packer finds an import by its slot, a 4-byte static named `__desk_imp_<name>` in the section `.desk.imp`, which `import!` defines.
#![cfg_attr(not(test), no_std)]

use core::fmt;

#[doc(hidden)]
pub use core::sync::atomic::AtomicUsize as __Slot;
#[doc(hidden)]
pub use core::sync::atomic::Ordering as __Ordering;

/// Declares a service the core provides and defines how to call it.
///
/// `import!(fn desk_log(text: *const u8, len: usize));` defines the import slot `__desk_imp_desk_log`, a zero word in the section
/// `.desk.imp` that the loader fills with the address of the service named `desk_log`, and an `unsafe fn desk_log(text, len)` that
/// calls through it. The slot lives inside the function, so a service nobody calls is not imported (the linker script keeps every
/// slot that is linked, and `#[used]` keeps it linked).
///
/// The call is unsafe because the core is trusted to take the arguments as the service documents them, and the slot holds a
/// valid address only after the loader ran.
#[macro_export]
macro_rules! import {
    ($(#[$meta:meta])* $vis:vis fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)?) => {
        $(#[$meta])*
        $vis unsafe fn $name($($arg: $ty),*) $(-> $ret)? {
            #[export_name = concat!("__desk_imp_", stringify!($name))]
            #[used]
            #[link_section = ".desk.imp"]
            static SLOT: $crate::__Slot = $crate::__Slot::new(0);
            let address = SLOT.load($crate::__Ordering::Relaxed);
            // SAFETY: the loader stores the address of the service of this name in the slot before the module's first entry point
            // runs, and refuses the module if the core has no such service; the service has the signature written here.
            let service: extern "C" fn($($ty),*) $(-> $ret)? = unsafe { core::mem::transmute::<usize, _>(address) };
            service($($arg),*)
        }
    };
}

/// Defines the module's entry points over plain Rust functions.
///
/// `desk_app!(start = f)`, optionally followed by `event = g` and `stop = h`, defines `app_start`, `app_event` and `app_stop`
/// with the C ABI and the names the packer looks for. `f(app: u32, args: &[u8]) -> i32` returns 0 when the module started,
/// `g(app: u32, event: *const u8)` takes an event and `h(app: u32)` runs once before the module is unloaded.
/// `app_start` is `unsafe` to call, because it trusts the pointer and length the core passes. A module that leaves out `event` or
/// `stop` has no such entry point (its header says 0), and its build must not ask the linker to keep one.
#[macro_export]
macro_rules! desk_app {
    (start = $start:path $(, event = $event:path)? $(, stop = $stop:path)? $(,)?) => {
        #[no_mangle]
        pub unsafe extern "C" fn app_start(app: u32, args: *const u8, args_len: u32) -> i32 {
            let args: &[u8] = if args.is_null() || args_len == 0 {
                &[]
            } else {
                // SAFETY: the core passes `args_len` readable bytes at `args` for the duration of this call, which is what
                // calling this entry point requires.
                unsafe { core::slice::from_raw_parts(args, args_len as usize) }
            };
            $start(app, args)
        }
        $(
            #[no_mangle]
            pub extern "C" fn app_event(app: u32, event: *const u8) {
                $event(app, event)
            }
        )?
        $(
            #[no_mangle]
            pub extern "C" fn app_stop(app: u32) {
                $stop(app)
            }
        )?
    };
}

import!(
    /// Writes one line of text to the core's log. The core adds the newline.
    ///
    /// # Safety
    ///
    /// `text` must be readable for `len` bytes, and the module must have been loaded (the slot is bound).
    pub fn desk_log(text: *const u8, len: usize)
);
import!(
    /// Reports a panic with its message and ends the instance; the core does not return from it (the guard abandons the caller's
    /// frames).
    ///
    /// # Safety
    ///
    /// `text` must be readable for `len` bytes, and the module must have been loaded (the slot is bound).
    pub fn desk_panic(text: *const u8, len: usize)
);

/// Sends one line to the core's log. A line longer than the core accepts is the core's to cut.
pub fn log(text: &str) {
    to_core(text.as_bytes())
}

/// Where `Lines` sends a finished line: `desk_log` for a module.
type Sink = fn(&[u8]);

fn to_core(line: &[u8]) {
    // SAFETY: `line` is readable for its length and `desk_log` only reads it.
    unsafe { desk_log(line.as_ptr(), line.len()) }
}

/// The longest line `Lines` sends in one piece.
pub const LINE_BYTES: usize = 160;

/// A `core::fmt::Write` that sends each line to `desk_log`: it buffers up to `LINE_BYTES` bytes, sends them at each `\n` (without
/// it), when the buffer is full, and when the value is dropped if anything is pending. Formatting needs no allocator.
pub struct Lines {
    buf: [u8; LINE_BYTES],
    len: usize,
    sink: Sink,
}

impl Lines {
    pub const fn new() -> Lines {
        Lines::to(to_core)
    }

    const fn to(sink: Sink) -> Lines {
        Lines {
            buf: [0; LINE_BYTES],
            len: 0,
            sink,
        }
    }

    fn flush(&mut self) {
        if self.len > 0 {
            (self.sink)(&self.buf[..self.len]);
            self.len = 0;
        }
    }
}

impl Default for Lines {
    fn default() -> Lines {
        Lines::new()
    }
}

impl fmt::Write for Lines {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &b in s.as_bytes() {
            if b == b'\n' {
                self.flush();
            } else {
                if self.len == LINE_BYTES {
                    self.flush();
                }
                self.buf[self.len] = b;
                self.len += 1;
            }
        }
        Ok(())
    }
}

impl Drop for Lines {
    fn drop(&mut self) {
        self.flush();
    }
}

#[cfg(all(feature = "panic-handler", not(test)))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use core::fmt::Write;
    let mut text = Message::new();
    let _ = match info.location() {
        Some(at) => write!(text, "{} ({}:{})", info.message(), at.file(), at.line()),
        None => write!(text, "{}", info.message()),
    };
    // SAFETY: the buffer holds `len` initialised bytes and `desk_panic` only reads them.
    unsafe { desk_panic(text.buf.as_ptr(), text.len) };
    // The core does not return from `desk_panic`; a core that did would leave nothing safe to run.
    loop {
        core::hint::spin_loop();
    }
}

/// The panic message, cut at `LINE_BYTES` on a character boundary.
#[cfg(all(feature = "panic-handler", not(test)))]
struct Message {
    buf: [u8; LINE_BYTES],
    len: usize,
}

#[cfg(all(feature = "panic-handler", not(test)))]
impl Message {
    const fn new() -> Message {
        Message {
            buf: [0; LINE_BYTES],
            len: 0,
        }
    }
}

#[cfg(all(feature = "panic-handler", not(test)))]
impl fmt::Write for Message {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            let mut one = [0u8; 4];
            let bytes = c.encode_utf8(&mut one).as_bytes();
            if self.len + bytes.len() > LINE_BYTES {
                break;
            }
            self.buf[self.len..self.len + bytes.len()].copy_from_slice(bytes);
            self.len += bytes.len();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::fmt::Write;
    use std::cell::RefCell;
    use std::string::String;
    use std::vec::Vec;

    std::thread_local! {
        static SENT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    fn record(line: &[u8]) {
        SENT.with(|s| s.borrow_mut().push(String::from_utf8_lossy(line).into_owned()));
    }

    fn sent() -> Vec<String> {
        SENT.with(|s| core::mem::take(&mut *s.borrow_mut()))
    }

    fn lines() -> Lines {
        Lines::to(record)
    }

    #[test]
    fn a_line_is_sent_when_the_value_is_dropped() {
        sent();
        write!(lines(), "hello: app={} count={}", 1, 7).unwrap();
        assert_eq!(sent(), ["hello: app=1 count=7"]);
    }

    #[test]
    fn each_newline_ends_a_line_and_is_not_sent() {
        sent();
        write!(lines(), "one\ntwo\n\nthree").unwrap();
        assert_eq!(sent(), ["one", "two", "three"]);
        writeln!(lines(), "four").unwrap();
        assert_eq!(sent(), ["four"]);
    }

    #[test]
    fn nothing_pending_sends_nothing() {
        sent();
        drop(lines());
        assert!(sent().is_empty());
    }

    #[test]
    fn a_full_buffer_is_sent_in_pieces_of_the_buffer_size() {
        sent();
        let long: String = (0..LINE_BYTES * 2 + 5).map(|i| (b'a' + (i % 26) as u8) as char).collect();
        write!(lines(), "{long}").unwrap();
        let got = sent();
        assert_eq!(got.iter().map(|s| s.len()).collect::<Vec<_>>(), [LINE_BYTES, LINE_BYTES, 5]);
        assert_eq!(got.concat(), long);
    }

    fn start(app: u32, args: &[u8]) -> i32 {
        (app + args.len() as u32) as i32
    }

    static EVENTS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

    fn event(app: u32, _event: *const u8) {
        EVENTS.fetch_add(app, core::sync::atomic::Ordering::Relaxed);
    }

    fn stop(app: u32) {
        EVENTS.fetch_add(app * 100, core::sync::atomic::Ordering::Relaxed);
    }

    desk_app!(start = start, event = event, stop = stop);

    #[test]
    fn the_entry_points_call_the_plain_functions() {
        // SAFETY: each call passes as many readable bytes as it says, or none.
        unsafe {
            assert_eq!(app_start(2, b"abc".as_ptr(), 3), 5);
            // No arguments: a null pointer or a zero length is an empty slice, never a call to `from_raw_parts` with them.
            assert_eq!(app_start(2, core::ptr::null(), 9), 2);
            assert_eq!(app_start(2, b"abc".as_ptr(), 0), 2);
        }
        app_event(3, core::ptr::null());
        app_stop(4);
        assert_eq!(EVENTS.load(core::sync::atomic::Ordering::Relaxed), 403);
    }
}
