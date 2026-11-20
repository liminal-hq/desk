// The DOS platform's interrupt registry: a protected-mode stub per IRQ line that calls a Rust `IrqHandler`, and the end-of-interrupt.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `timer.rs` hooks IRQ 0 with a stub that never calls Rust. This is the general form `docs/HAL.md` ("The DOS interrupt stub")
// asks for, one stub for each of the 16 lines:
//
// 1. The host calls the stub on a stack of its own, with CS holding the code selector and DS, ES and SS whatever the interrupted
//    program had or the host chose. The stub saves what it clobbers, loads DS and ES with the flat data selector (read through CS,
//    because DS cannot be trusted yet), and **switches to a locked interrupt stack in the program's own data segment**, so Rust
//    code can dereference a pointer to a local through DS.
// 2. It calls `rt_irq_dispatch(line)`, which looks the line up in a table, runs the `IrqHandler` and, for a line Desk owns
//    (`Chain::None`), sends the end-of-interrupt through the HAL's `desk-pic` driver. A spurious IRQ 7 or 15 runs no handler and gets
//    the acknowledgement `eoi_spurious` says.
// 3. It switches back and returns with `iretd`, or, for `Chain::Previous`, jumps to the previous handler, which acknowledges the
//    interrupt itself (the BIOS keyboard handler and the DOS timer do).
//
// The handler runs with interrupts off (the host delivers a hardware interrupt through an interrupt gate and the stub never
// enables them), so it is never re-entered and the one interrupt stack is enough. Everything the stub and the handler touch is in
// the program image, which `register` locks with DPMI 0600h before the first vector is changed.
//
// The vectors are put back by `unregister`, and by a clean-up function on the runtime's list that runs at exit and in the panic
// handler, so a program cannot leave an interrupt pointing at code that has gone.
use crate::{cleanup, dpmi};
use core::arch::global_asm;
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::ptr::{addr_of, addr_of_mut};
use core::sync::atomic::{AtomicBool, Ordering};
use desk_hal::irq::{Chain, InterruptController, Irq, IrqHandler, IrqRegistry};
use desk_hal::x86::RawPorts;
use desk_hal::HalError;
use desk_pic::Pic8259;

/// Bytes of the interrupt stack. The handlers the HAL defines (a port read and a ring push) use a few hundred.
const STACK_SIZE: usize = 8192;

#[repr(C, align(16))]
struct IrqStack([u8; STACK_SIZE]);

#[no_mangle]
static mut RT_IRQ_STACK: IrqStack = IrqStack([0; STACK_SIZE]);
/// The interrupted stack: ESP, then SS (the layout `lss` loads).
#[no_mangle]
static mut RT_IRQ_SAVED: [u32; 2] = [0; 2];
/// The flat data selector, where the stub can read it through CS.
#[no_mangle]
static mut RT_IRQ_DSEL: u32 = 0;
/// Nonzero for a line whose previous handler runs after ours.
#[no_mangle]
static mut RT_IRQ_CHAIN: [u32; 16] = [0; 16];
/// The previous handler of each line as a far pointer: 32-bit offset, then the selector (the layout of `jmp fword ptr`).
#[no_mangle]
static mut RT_IRQ_OLD: [[u32; 2]; 16] = [[0; 2]; 16];
/// The registered handlers.
static mut TABLE: [Option<IrqHandler>; 16] = [None; 16];
/// The protected-mode vector number of each installed line, or 0.
static mut VECTORS: [u8; 16] = [0; 16];

global_asm!(
    ".text",
    ".macro RT_IRQ_STUB n",
    "rt_irq_stub_\\n:",
    "push eax",
    "push ecx",
    "push edx",
    "push ds",
    "push es",
    "mov eax, dword ptr cs:[{dsel}]",
    "mov ds, ax",
    "mov es, ax",
    "mov dword ptr [{saved}], esp",
    "mov word ptr [{saved} + 4], ss",
    "mov ss, ax",
    "mov esp, offset {stack} + {stack_size}",
    "cld",
    "sub esp, 12",
    "push \\n",
    "call {dispatch}",
    "add esp, 16",
    "lss esp, fword ptr [{saved}]",
    "pop es",
    "pop ds",
    "pop edx",
    "pop ecx",
    "pop eax",
    "cmp dword ptr cs:[{chain} + 4 * \\n], 0",
    "jne 2f",
    "iretd",
    "2:",
    "jmp fword ptr cs:[{old} + 8 * \\n]",
    ".endm",
    ".irp n, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15",
    "RT_IRQ_STUB \\n",
    ".endr",
    ".data",
    ".global rt_irq_stub_table",
    "rt_irq_stub_table:",
    ".irp n, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15",
    ".long rt_irq_stub_\\n",
    ".endr",
    ".text",
    dsel = sym RT_IRQ_DSEL,
    saved = sym RT_IRQ_SAVED,
    stack = sym RT_IRQ_STACK,
    stack_size = const STACK_SIZE,
    dispatch = sym rt_irq_dispatch,
    chain = sym RT_IRQ_CHAIN,
    old = sym RT_IRQ_OLD,
);

extern "C" {
    static rt_irq_stub_table: [u32; 16];
    static __image_start: u8;
    static __image_end: u8;
}

/// Called by the stub, on the interrupt stack, with the flat data selector loaded.
#[no_mangle]
extern "C" fn rt_irq_dispatch(line: u32) {
    let line = (line & 15) as usize;
    // SAFETY: the DPMI client may use the PIC ports (`RawPorts::new`'s contract; the first port access of the program is the VGA
    // palette in the checks and `desk-pic` is what DJGPP programs use as well).
    let pic = Pic8259::new(unsafe { RawPorts::new() });
    let irq = Irq::new(line as u8).unwrap_or(Irq::TIMER);
    // SAFETY: interrupts are off while a stub runs, so nothing else reads or writes these at the same time; `register` and
    // `unregister` change them with the line's vector not pointing at the stub (before the vector is set, after it is restored).
    let (handler, chained) = unsafe { ((*addr_of!(TABLE))[line], (*addr_of!(RT_IRQ_CHAIN))[line] != 0) };
    if !chained && (line == 7 || line == 15) && pic.is_spurious(irq) {
        pic.eoi_spurious(irq);
        return;
    }
    if let Some(h) = handler {
        // SAFETY: built by `from_static`, `from_shared` or `from_raw` under their contracts, and a stub never re-enters.
        unsafe { h.call() };
    }
    if !chained {
        pic.eoi(irq);
    }
}

static LOCKED: AtomicBool = AtomicBool::new(false);
static CLEANUP: AtomicBool = AtomicBool::new(false);

/// Locks the whole image (code, data, bss, both stacks) so the host never pages anything a handler touches.
fn lock_image() -> Result<(), HalError> {
    if LOCKED.load(Ordering::Relaxed) {
        return Ok(());
    }
    let start = dpmi::ptr_to_linear(addr_of!(__image_start));
    let end = dpmi::ptr_to_linear(addr_of!(__image_end));
    dpmi::lock(start, end - start).map_err(|e| HalError::Platform(e.0))?;
    LOCKED.store(true, Ordering::Relaxed);
    Ok(())
}

/// The protected-mode vector of an IRQ line: the DPMI host's interrupt base for the controller it is on.
fn vector_of(irq: Irq) -> u8 {
    let (_, _, _, _, master, slave) = dpmi::version();
    let (master, slave) = (if master == 0 { 0x08 } else { master }, if slave == 0 { 0x70 } else { slave });
    if irq.is_slave() {
        slave + (irq.line() - 8)
    } else {
        master + irq.line()
    }
}

/// The IRQ lines of the DOS machine, installed through DPMI 0204h and 0205h.
#[derive(Clone, Copy, Debug, Default)]
pub struct DosIrq;

impl DosIrq {
    /// Whether `irq` has a handler of ours installed.
    pub fn is_installed(irq: Irq) -> bool {
        // SAFETY: a single-threaded read of a byte that is only written outside interrupts.
        unsafe { (*addr_of!(VECTORS))[usize::from(irq.line())] != 0 }
    }

    /// The protected-mode vector `irq` is installed at, or 0.
    pub fn vector(irq: Irq) -> u8 {
        // SAFETY: as `is_installed`.
        unsafe { (*addr_of!(VECTORS))[usize::from(irq.line())] }
    }

    /// The protected-mode handler the host would run for `irq` right now, as (selector, offset): what `unregister` must restore.
    pub fn current(irq: Irq) -> Result<(u16, u32), HalError> {
        dpmi::get_pm_vector(vector_of(irq)).map_err(|e| HalError::Platform(e.0))
    }
}

impl IrqRegistry for DosIrq {
    fn register(&self, irq: Irq, handler: IrqHandler, chain: Chain) -> Result<(), HalError> {
        let line = usize::from(irq.line());
        if irq == Irq::CASCADE {
            return Err(HalError::InvalidArgument);
        }
        if Self::is_installed(irq) {
            return Err(HalError::Busy);
        }
        lock_image()?;
        // The clean-up entry goes first: a full list must refuse the registration, not leave a vector that nothing puts back.
        if !CLEANUP.load(Ordering::Relaxed) {
            if !cleanup::register(restore_all) {
                return Err(HalError::Busy);
            }
            CLEANUP.store(true, Ordering::Relaxed);
        }
        let vector = vector_of(irq);
        let old = dpmi::get_pm_vector(vector).map_err(|e| HalError::Platform(e.0))?;
        let cs: u16;
        // SAFETY: reading a segment register has no effect.
        unsafe { core::arch::asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags)) };
        // SAFETY: the vector still points at the old handler, so no stub can run while the tables are written. `rt_irq_stub_table`
        // is the assembler's table of the 16 stub offsets.
        let stub = unsafe {
            (*addr_of_mut!(TABLE))[line] = Some(handler);
            (*addr_of_mut!(RT_IRQ_CHAIN))[line] = u32::from(chain == Chain::Previous);
            (*addr_of_mut!(RT_IRQ_OLD))[line] = [old.1, u32::from(old.0)];
            RT_IRQ_DSEL = u32::from(dpmi::data_selector());
            (*addr_of!(rt_irq_stub_table))[line]
        };
        if let Err(e) = dpmi::set_pm_vector(vector, cs, stub) {
            // SAFETY: the vector was not changed, so no stub can be running.
            unsafe { (*addr_of_mut!(TABLE))[line] = None };
            return Err(HalError::Platform(e.0));
        }
        // SAFETY: the stub can now run, and it never touches `VECTORS`.
        unsafe { (*addr_of_mut!(VECTORS))[line] = vector };
        // SAFETY: as `rt_irq_dispatch`. The mask register is read and written without turning interrupts off, which a DPMI client
        // cannot do cheaply; the lines Desk takes (keyboard, mouse) are already unmasked by the BIOS, so this normally writes nothing.
        let pic = Pic8259::new(unsafe { RawPorts::new() });
        if pic.is_masked(irq) {
            pic.unmask(irq);
        }
        Ok(())
    }

    fn unregister(&self, irq: Irq) -> Result<(), HalError> {
        let line = usize::from(irq.line());
        if !Self::is_installed(irq) {
            return Err(HalError::InvalidArgument);
        }
        // SAFETY: only this function and `restore_all` write these, from the program's single thread.
        let (vector, old) = unsafe { ((*addr_of!(VECTORS))[line], (*addr_of!(RT_IRQ_OLD))[line]) };
        // If the host refuses, the vector still points at the stub, so the line stays registered: the handler keeps running (not an
        // empty table entry that swallows the interrupt) and `restore_all` can try again.
        dpmi::set_pm_vector(vector, old[1] as u16, old[0]).map_err(|e| HalError::Platform(e.0))?;
        // SAFETY: the vector no longer points at the stub.
        unsafe {
            (*addr_of_mut!(VECTORS))[line] = 0;
            (*addr_of_mut!(TABLE))[line] = None;
        }
        Ok(())
    }
}

/// Puts every vector back. Safe to call twice, and from the clean-up list.
fn restore_all() {
    for line in 0..16u8 {
        if let Ok(irq) = Irq::new(line) {
            if DosIrq::is_installed(irq) {
                let _ = DosIrq.unregister(irq);
            }
        }
    }
}

/// One place for interrupt-side state that must outlive `main`'s frame and sit in the locked image: a `static` the program declares
/// and fills once. `IrqHandler::from_static` wants a `&'static mut`; a `Box::leak` would put the state in heap memory, which is not
/// locked.
pub struct Slot<T> {
    used: AtomicBool,
    value: UnsafeCell<MaybeUninit<T>>,
}

// SAFETY: `place` hands the contents out once, as a `&'static mut`, to a single owner; the type is `Sync` so it can be a `static`.
unsafe impl<T: Send> Sync for Slot<T> {}

impl<T> Slot<T> {
    pub const fn new() -> Slot<T> {
        Slot {
            used: AtomicBool::new(false),
            value: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    /// Stores `value` and hands out the only reference to it, the first time; `None` after that.
    #[allow(clippy::mut_from_ref)] // sound: the `used` flag makes the first caller the only one that ever gets a reference
    pub fn place(&'static self, value: T) -> Option<&'static mut T> {
        if self.used.swap(true, Ordering::AcqRel) {
            return None;
        }
        // SAFETY: the flag makes this the only writer and the only reference ever made.
        Some(unsafe { (*self.value.get()).write(value) })
    }
}

impl<T> Default for Slot<T> {
    fn default() -> Slot<T> {
        Slot::new()
    }
}
