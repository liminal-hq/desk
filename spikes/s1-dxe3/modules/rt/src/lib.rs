// What every Desk module shares: the allocator, the panic handler, the callbacks into the core and the three entry points.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Only integers, pointers and lengths cross the boundary. The callbacks reach the core one of two ways: route a imports `desk_*`
// by name (dlopen resolves them from the table the core gave dlregsym), route b calls through a table of function pointers the
// core passes to `app_start`. The `route-b` feature picks one; the modules' own code is the same either way.
#![no_std]

extern crate alloc;

use alloc::alloc::{GlobalAlloc, Layout};
use core::fmt::Write;

/// One event, passed by pointer (docs/SDK.md).
#[repr(C)]
pub struct DeskEvent {
    pub kind: u32,
    pub window: u32,
    pub control: u32,
    pub x: i32,
    pub y: i32,
    pub key: u32,
    pub mods: u32,
    pub data: u32,
}

/// The table route b passes to `app_start`. `version` and `size` let the core grow it without breaking an older module.
#[repr(C)]
pub struct DeskApi {
    pub version: u32,
    pub size: u32,
    pub log: unsafe extern "C" fn(*const u8, usize),
    pub alloc: unsafe extern "C" fn(usize) -> *mut u8,
    pub free: unsafe extern "C" fn(*mut u8),
    pub window_new: unsafe extern "C" fn(u32, *const u8, u32, i32, i32, u32) -> u32,
    pub panic: unsafe extern "C" fn(*const u8, usize) -> !,
    pub memcpy: unsafe extern "C" fn(*mut u8, *const u8, usize) -> *mut u8,
    pub memset: unsafe extern "C" fn(*mut u8, i32, usize) -> *mut u8,
    pub memmove: unsafe extern "C" fn(*mut u8, *const u8, usize) -> *mut u8,
    pub memcmp: unsafe extern "C" fn(*const u8, *const u8, usize) -> i32,
}

#[cfg(not(feature = "route-b"))]
mod core_api {
    extern "C" {
        pub fn desk_log(text: *const u8, len: usize);
        pub fn desk_alloc(size: usize) -> *mut u8;
        pub fn desk_free(ptr: *mut u8);
        pub fn desk_window_new(app: u32, title: *const u8, len: u32, w: i32, h: i32, flags: u32) -> u32;
        pub fn desk_panic(msg: *const u8, len: usize) -> !;
    }
}

#[cfg(feature = "route-b")]
mod core_api {
    use super::DeskApi;
    use core::cell::UnsafeCell;
    use core::ffi::c_void;

    /// The table pointer, set once by `bind` in `app_start`. Modules run on one thread.
    struct Slot(UnsafeCell<*const DeskApi>);
    // SAFETY: a module is called from one thread only.
    unsafe impl Sync for Slot {}
    static API: Slot = Slot(UnsafeCell::new(core::ptr::null()));

    pub fn bind(api: *const DeskApi) {
        // SAFETY: single thread, and the core keeps the table alive for as long as the module is loaded.
        unsafe { *API.0.get() = api };
    }

    pub fn api() -> &'static DeskApi {
        // SAFETY: `bind` ran in `app_start` before any other entry point or callback.
        unsafe { &**API.0.get() }
    }

    // Route b modules import nothing, not even memcpy: the compiler calls these, so they go through the table (or `rep movs`).
    #[no_mangle]
    pub unsafe extern "C" fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
        (api().memcpy)(dst.cast(), src.cast(), n).cast()
    }
    #[no_mangle]
    pub unsafe extern "C" fn memset(dst: *mut c_void, c: i32, n: usize) -> *mut c_void {
        (api().memset)(dst.cast(), c, n).cast()
    }
    #[no_mangle]
    pub unsafe extern "C" fn memmove(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
        (api().memmove)(dst.cast(), src.cast(), n).cast()
    }
    #[no_mangle]
    pub unsafe extern "C" fn memcmp(a: *const c_void, b: *const c_void, n: usize) -> i32 {
        (api().memcmp)(a.cast(), b.cast(), n)
    }
    #[no_mangle]
    pub unsafe extern "C" fn bcmp(a: *const c_void, b: *const c_void, n: usize) -> i32 {
        (api().memcmp)(a.cast(), b.cast(), n)
    }
}

/// Route b only: stores the core's table. Route a has nothing to bind.
#[cfg(feature = "route-b")]
pub fn bind(api: *const DeskApi) {
    core_api::bind(api);
}

/// Writes a line to the core's log.
pub fn log(text: &str) {
    // SAFETY: the core copies `len` bytes at once and keeps no pointer.
    unsafe {
        #[cfg(not(feature = "route-b"))]
        core_api::desk_log(text.as_ptr(), text.len());
        #[cfg(feature = "route-b")]
        (core_api::api().log)(text.as_ptr(), text.len());
    }
}

/// Opens a window (a stub in the spike: the core returns a handle and logs it).
pub fn window_new(app: u32, title: &str, w: i32, h: i32) -> u32 {
    // SAFETY: as for `log`.
    unsafe {
        #[cfg(not(feature = "route-b"))]
        let handle = core_api::desk_window_new(app, title.as_ptr(), title.len() as u32, w, h, 0);
        #[cfg(feature = "route-b")]
        let handle = (core_api::api().window_new)(app, title.as_ptr(), title.len() as u32, w, h, 0);
        handle
    }
}

unsafe fn core_alloc(size: usize) -> *mut u8 {
    #[cfg(not(feature = "route-b"))]
    return core_api::desk_alloc(size);
    #[cfg(feature = "route-b")]
    return (core_api::api().alloc)(size);
}

unsafe fn core_free(ptr: *mut u8) {
    #[cfg(not(feature = "route-b"))]
    core_api::desk_free(ptr);
    #[cfg(feature = "route-b")]
    (core_api::api().free)(ptr);
}

/// The module's heap is the core's: every block goes through `desk_alloc`, so the core can count it and free what a crashed or
/// careless module left behind.
struct CoreHeap;

// The core's allocator returns 8-byte aligned blocks; a stricter alignment is refused with null.
unsafe impl GlobalAlloc for CoreHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() > 8 {
            return core::ptr::null_mut();
        }
        core_alloc(layout.size())
    }
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        core_free(ptr);
    }
}

#[global_allocator]
static HEAP: CoreHeap = CoreHeap;

/// A fixed buffer, so formatting the panic message allocates nothing.
struct Message {
    bytes: [u8; 160],
    len: usize,
}

impl Write for Message {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let room = self.bytes.len() - self.len;
        let n = s.len().min(room);
        self.bytes[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

/// A panic formats its message and location into a stack buffer and hands them to the core, which does not return here: it
/// `longjmp`s back to the frame that called the module.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let mut msg = Message { bytes: [0; 160], len: 0 };
    let line = info.location().map_or(0, |l| l.line());
    let _ = write!(msg, "{} (line {})", info.message(), line);
    // SAFETY: the core only reads `len` bytes of the buffer, which lives until the call ends.
    unsafe {
        #[cfg(not(feature = "route-b"))]
        core_api::desk_panic(msg.bytes.as_ptr(), msg.len);
        #[cfg(feature = "route-b")]
        (core_api::api().panic)(msg.bytes.as_ptr(), msg.len)
    }
}

/// Defines the three entry points the core looks up with `dlsym`, with the signature of the route in use.
///
/// `$start(app, args) -> i32`, `$event(app, &DeskEvent)` and `$stop(app)` are ordinary Rust functions.
#[macro_export]
macro_rules! desk_app {
    ($start:ident, $event:ident, $stop:ident) => {
        #[cfg(not(feature = "route-b"))]
        #[no_mangle]
        pub unsafe extern "C" fn app_start(app: u32, args: *const u8, args_len: u32) -> i32 {
            $start(app, $crate::args(args, args_len))
        }
        #[cfg(feature = "route-b")]
        #[no_mangle]
        pub unsafe extern "C" fn app_start(app: u32, args: *const u8, args_len: u32, api: *const $crate::DeskApi) -> i32 {
            $crate::bind(api);
            $start(app, $crate::args(args, args_len))
        }
        #[no_mangle]
        pub unsafe extern "C" fn app_event(app: u32, e: *const $crate::DeskEvent) {
            $event(app, &*e)
        }
        #[no_mangle]
        pub unsafe extern "C" fn app_stop(app: u32) {
            $stop(app)
        }
    };
}

/// The arguments the core passed, as text (the core promises UTF-8).
///
/// # Safety
///
/// `args` must be valid for `len` bytes of UTF-8, or null.
pub unsafe fn args<'a>(args: *const u8, len: u32) -> &'a str {
    if args.is_null() {
        return "";
    }
    core::str::from_utf8_unchecked(core::slice::from_raw_parts(args, len as usize))
}

/// A value in static storage for a module's own state, set in `app_start` and dropped in `app_stop`. A module runs on one
/// thread, and the core never calls into it re-entrantly, so there is no lock.
pub struct Slot<T>(core::cell::UnsafeCell<Option<T>>);

// SAFETY: a module is called from one thread only, one call at a time.
unsafe impl<T> Sync for Slot<T> {}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Slot<T> {
    pub const fn new() -> Self {
        Slot(core::cell::UnsafeCell::new(None))
    }
    pub fn set(&self, value: T) {
        // SAFETY: one call at a time (see the type's note); no reference into the slot outlives a call.
        unsafe { *self.0.get() = Some(value) };
    }
    pub fn take(&self) -> Option<T> {
        // SAFETY: as for `set`.
        unsafe { (*self.0.get()).take() }
    }
    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        // SAFETY: as for `set`; `f` cannot re-enter the slot because the module has no callbacks that reach it.
        unsafe { (*self.0.get()).as_mut().map(f) }
    }
}
