// BIOS calls from the protected-mode kernel, by a real-mode trampoline.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The loader leaves the machine in protected mode with no BIOS. To run one BIOS software interrupt this module turns interrupts off,
// puts the 8259s back on the BIOS vectors and masks, calls a small blob (`kernel/bios16.asm`) that drops to real mode, runs the
// handler and comes back, then undoes the PIC change and makes up for what the BIOS did meanwhile: the kernel ticks its handler did
// not see, and a keyboard or mouse byte whose interrupt could not be delivered. The design, the low-memory map and the reasons are in
// `docs/N3-BIOS-PATH.md`; read its "Contract" section before calling anything here.

use crate::hal::{CLOCK, IRQS, PIC, PORTS};
use crate::{gdt, sync};
use core::arch::asm;
use core::fmt;
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use desk_hal::bios::BIOS_BUFFER_MAX;
use desk_hal::irq::Irq;
use desk_hal::{Bios, BiosRegs, HalError, PortIo};
use desk_pic::{BIOS_BASES, REMAPPED_BASES};

// ---- The low-memory map (physical addresses; the first megabyte is reserved by the frame allocator)

/// Where the trampoline blob lives: the control block, the code, its tables and the 16-bit GDT. Must equal `BASE` in `bios16.asm`.
pub const LOW_BASE: u32 = 0x1_0000;
/// The blob's size: one page.
const BLOB_LEN: usize = 4096;
/// The bounce buffer: memory the BIOS can reach with a real-mode segment, for the structures and sectors a call reads and writes.
pub const BOUNCE_BASE: u32 = 0x1_1000;
pub const BOUNCE_LEN: usize = 8192;
/// The real-mode stack the BIOS runs on (4 KiB, top at 0x14000).
const STACK_BASE: u32 = 0x1_3000;
/// The real-mode segment of the bounce buffer; `es:di = BOUNCE_SEGMENT:0` addresses its first byte.
pub const BOUNCE_SEGMENT: u16 = (BOUNCE_BASE >> 4) as u16;

const ENTRY: u32 = LOW_BASE + 0x100;
const MAGIC: u32 = 0x3142_4B44; // "DKB1"

/// The blob, assembled by `tools/native-build.sh` from `kernel/bios16.asm`.
static BLOB: [u8; BLOB_LEN] = *include_bytes!("../../dist/native/bios16.bin");

/// The registers of a call, as the blob loads and stores them. The layout is shared with `bios16.asm` (`R_EAX` and the rest).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RealModeRegs {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
    pub esi: u32,
    pub edi: u32,
    pub ebp: u32,
    pub es: u16,
    pub ds: u16,
    pub fs: u16,
    pub gs: u16,
    /// In: the arithmetic flags and DF to enter with (the others are ignored; the handler is entered as an INT would, with IF
    /// and TF clear). Out: the flags the handler returned; bit 0 is the carry flag, which most BIOS functions use for failure.
    pub flags: u16,
    /// Pads the block to 40 bytes; keep it zero.
    pub reserved: u16,
}

impl RealModeRegs {
    pub const CARRY: u16 = 1;

    /// Registers with only `eax` set, the common start of a call.
    pub fn with_eax(eax: u32) -> RealModeRegs {
        RealModeRegs { eax, ..RealModeRegs::default() }
    }

    pub fn carry(&self) -> bool {
        self.flags & Self::CARRY != 0
    }

    pub fn ah(&self) -> u8 {
        (self.eax >> 8) as u8
    }
}

/// The control block at the start of the blob, as `bios16.asm` lays it out.
#[repr(C)]
struct Control {
    magic: u32,
    vector: u8,
    _pad: u8,
    flags_in: u16,
    regs: RealModeRegs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiosError {
    /// `init` has not run, or the blob in the kernel image is not the one this code was written for.
    NotReady,
    /// Another call is in progress (a handler tried to make a call while the main line was inside one).
    Busy,
    /// An interrupt handler is running (an 8259 in-service bit is set): the call would reprogram the controllers under it.
    InInterrupt,
    /// Paging is on. The trampoline runs at a physical address; see `docs/N3-BIOS-PATH.md`, "Paging".
    PagingEnabled,
    /// The vector table entry for the interrupt is 0000:0000, so there is no handler to call.
    NoHandler(u8),
    /// A buffer longer than the bounce buffer, or an address outside the first megabyte.
    BadAddress,
}

impl fmt::Display for BiosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BiosError::NotReady => f.write_str("the BIOS trampoline is not set up"),
            BiosError::Busy => f.write_str("a BIOS call is already in progress"),
            BiosError::InInterrupt => f.write_str("a BIOS call from an interrupt handler"),
            BiosError::PagingEnabled => f.write_str("a BIOS call with paging on"),
            BiosError::NoHandler(v) => write!(f, "no BIOS handler for interrupt {:#04x}", v),
            BiosError::BadAddress => f.write_str("an address or length outside the low-memory window"),
        }
    }
}

impl From<BiosError> for HalError {
    fn from(e: BiosError) -> HalError {
        match e {
            BiosError::NotReady | BiosError::PagingEnabled => HalError::Unsupported,
            BiosError::Busy | BiosError::InInterrupt => HalError::Busy,
            BiosError::NoHandler(_) => HalError::NoDevice,
            BiosError::BadAddress => HalError::InvalidArgument,
        }
    }
}

// ---- What a call changes and what it restores

/// The 8259 masks while the BIOS runs, bit n masking IRQ n (bits 8 to 15 the slave's). Open: the timer (the BIOS tick and the
/// floppy motor timeout), the cascade, the floppy (INT 13h waits for it), the RTC (INT 15h AH=83h and 86h wait for it) and the
/// two IDE lines. Closed: everything else, the keyboard and the mouse above all, because the BIOS handlers would swallow bytes
/// the kernel's rings should see.
const BIOS_PIC_MASK: u16 = !(1 << 0 | 1 << 2 | 1 << 6 | 1 << 8 | 1 << 14 | 1 << 15);

const CMOS_INDEX: u16 = 0x70;
const NMI_DISABLE: u8 = 0x80;
const KBC_STATUS: u16 = 0x64;
const KBC_OUTPUT_FULL: u8 = 1 << 0;
const KBC_FROM_MOUSE: u8 = 1 << 5;
/// The BIOS data area's timer tick count (a dword) and midnight flag (a byte).
const BDA_TICKS: u32 = 0x46C;
const BDA_MIDNIGHT: u32 = 0x470;
const CR0_PG: u32 = 1 << 31;

static READY: AtomicBool = AtomicBool::new(false);
static BUSY: AtomicBool = AtomicBool::new(false);
/// Whether NMI is enabled after a call. The BIOS leaves it masked after POST and the kernel has no NMI handler beyond the fatal
/// dump, so it stays masked; a kernel with a real handler changes this.
const NMI_ENABLED_AFTER: bool = false;

/// Counters for the tests and the transcript. Single writer (the one processor, inside a call), so plain adds suffice.
pub struct Stats {
    pub calls: AtomicU32,
    /// Kernel ticks added because the BIOS's timer handler took them (the BDA tick count advanced).
    pub ticks_added: AtomicU32,
    /// Ticks added because IRQ 0 was raised but undelivered when the controllers were reprogrammed.
    pub pending_ticks: AtomicU32,
    /// Keyboard or mouse bytes whose interrupt could not be delivered and were handed to the drivers by hand afterwards.
    pub input_bytes: AtomicU32,
}

pub static STATS: Stats = Stats { calls: AtomicU32::new(0), ticks_added: AtomicU32::new(0), pending_ticks: AtomicU32::new(0), input_bytes: AtomicU32::new(0) };

/// The lock-free re-entry guard: set with an `xchg`, cleared on drop.
struct Guard;

impl Guard {
    fn acquire() -> Result<Guard, BiosError> {
        if BUSY.swap(true, Ordering::Acquire) {
            Err(BiosError::Busy)
        } else {
            Ok(Guard)
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

/// Copies the trampoline below 1 MiB and checks it. The frame allocator never hands out the first megabyte, so the target is the
/// kernel's. Call once at boot, before the first call, with interrupts in any state.
pub fn init() -> Result<(), BiosError> {
    // The blob reloads the kernel's selectors by number.
    assert_eq!((gdt::KERNEL_CODE, gdt::KERNEL_DATA), (0x08, 0x10), "bios16.asm assumes the kernel's flat selectors are 0x08 and 0x10");
    let magic = u32::from_le_bytes([BLOB[0], BLOB[1], BLOB[2], BLOB[3]]);
    if magic != MAGIC {
        return Err(BiosError::NotReady);
    }
    // SAFETY: LOW_BASE..LOW_BASE + 4 KiB is in the first megabyte, reserved by the allocator and unused since the loader copied the
    // kernel up from there; the flat segments make the address a pointer, and nothing else refers to it.
    unsafe { core::ptr::copy_nonoverlapping(BLOB.as_ptr(), LOW_BASE as *mut u8, BLOB_LEN) };
    // SAFETY: as above.
    let back = unsafe { read_volatile(LOW_BASE as *const u32) };
    if back != MAGIC {
        return Err(BiosError::NotReady);
    }
    READY.store(true, Ordering::Release);
    Ok(())
}

fn cr0() -> u32 {
    let v: u32;
    // SAFETY: reading CR0 has no side effects.
    unsafe { asm!("mov {0}, cr0", out(reg) v, options(nomem, nostack, preserves_flags)) };
    v
}

/// Copies `out.len()` bytes from the physical address `addr`, one load at a time in assembly, so address 0 (the vector table) is
/// read without forming a Rust pointer to null.
fn copy_from_low(addr: u32, out: &mut [u8]) {
    for (i, slot) in out.iter_mut().enumerate() {
        let byte: u8;
        // SAFETY: the callers keep `addr + out.len()` inside the first megabyte, which the flat segments map one to one; a load
        // from there has no side effect beyond the read (the video and ROM windows are plain memory reads).
        unsafe { asm!("mov {0}, byte ptr [{1}]", out(reg_byte) byte, in(reg) addr as usize + i, options(nostack, preserves_flags, readonly)) };
        *slot = byte;
    }
}

/// Reads real-mode memory: `out.len()` bytes from `segment:offset`. `BadAddress` if the range leaves the first megabyte. The
/// BIOS ROM, the vector table, the BIOS data area and the bounce buffer all read the same way.
pub fn read_low(segment: u16, offset: u16, out: &mut [u8]) -> Result<(), BiosError> {
    let start = u32::from(segment) * 16 + u32::from(offset);
    if u64::from(start) + out.len() as u64 > 0x10_0000 {
        return Err(BiosError::BadAddress);
    }
    copy_from_low(start, out);
    Ok(())
}

/// The bounce buffer: the only low memory the kernel writes for the BIOS, and the only place a call may be given a buffer.
#[derive(Clone, Copy)]
pub struct Bounce;

/// The bounce buffer. The contents are the kernel's until a call, and the BIOS's during it; they stay as the BIOS left them until
/// the next call or the next `write_low`.
pub fn bounce_buffer() -> Bounce {
    Bounce
}

impl Bounce {
    /// Its real-mode segment (the offset is 0).
    pub const fn segment(&self) -> u16 {
        BOUNCE_SEGMENT
    }

    /// Copies `data` into the buffer at `offset`.
    pub fn write_low(&self, offset: usize, data: &[u8]) -> Result<(), BiosError> {
        if offset.checked_add(data.len()).is_none_or(|end| end > BOUNCE_LEN) {
            return Err(BiosError::BadAddress);
        }
        // SAFETY: the range is inside the bounce buffer, which only the kernel and (during a call) the BIOS use.
        unsafe { core::ptr::copy_nonoverlapping(data.as_ptr(), (BOUNCE_BASE as usize + offset) as *mut u8, data.len()) };
        Ok(())
    }

    /// Copies bytes out of the buffer starting at `offset`.
    pub fn read_low(&self, offset: usize, out: &mut [u8]) -> Result<(), BiosError> {
        if offset.checked_add(out.len()).is_none_or(|end| end > BOUNCE_LEN) {
            return Err(BiosError::BadAddress);
        }
        copy_from_low(BOUNCE_BASE + offset as u32, out);
        Ok(())
    }
}

/// The BIOS tick count and the midnight flag, as the timer handler keeps them.
fn bda_ticks() -> (u32, u8) {
    // SAFETY: the BIOS data area is ordinary memory in the first megabyte, never handed out by the allocator.
    unsafe { (read_volatile(BDA_TICKS as *const u32), read_volatile(BDA_MIDNIGHT as *const u8)) }
}

/// The vector table's entry for `vector` (offset in the low word, segment in the high word).
pub fn vector_entry(vector: u8) -> u32 {
    let mut b = [0u8; 4];
    copy_from_low(u32::from(vector) * 4, &mut b);
    u32::from_le_bytes(b)
}

/// Points the real-mode vector table entry for `vector` at `value` (offset in the low word, segment in the high word). The kernel
/// does not use the table, so this is for tests and for a future hook that wants the BIOS to call back into low memory.
pub fn set_vector_entry(vector: u8, value: u32) {
    // SAFETY: the table is the first kilobyte of memory, which the kernel owns and no Rust reference points into; a store in
    // assembly avoids forming a pointer to address 0.
    unsafe { asm!("mov dword ptr [{0}], {1}", in(reg) usize::from(vector) * 4, in(reg) value, options(nostack, preserves_flags)) };
}

/// Runs BIOS interrupt `vector` with `regs`, in real mode, and stores the registers it returns. The whole of the unsafe part:
/// no checks beyond the ones listed.
///
/// # Safety
///
/// - `init` has succeeded, and nothing else is in a call (`call` takes the guard that ensures it).
/// - Interrupts are off, and no interrupt handler is running (no 8259 in-service bit is set): the controllers are reprogrammed.
/// - Paging is off, or maps the first megabyte one to one (`docs/N3-BIOS-PATH.md`, "Paging").
/// - The vector table entry for `vector` is a handler (not 0000:0000), and the call is one the BIOS can serve with the machine as
///   the kernel has left it: a handler that never returns, or that corrupts low memory, takes the kernel with it.
pub unsafe fn bios_int(vector: u8, regs: &mut RealModeRegs) -> Result<(), BiosError> {
    let saved_mask = PIC.mask_bits();
    let (ticks_before, _) = bda_ticks();
    // A tick raised but not yet delivered would be thrown away by the controller reset below. The kernel owes it to its clock.
    let tick_waiting = PIC.pending_mask() & 1 != 0;

    // NMI: its handler would be the kernel's protected-mode one, which cannot run while the CPU is in real mode. Masked from here
    // until the kernel is back.
    PORTS.outb(CMOS_INDEX, NMI_DISABLE);
    PIC.init(BIOS_BASES, BIOS_PIC_MASK);

    let ctl = LOW_BASE as *mut Control;
    // SAFETY: the control block is in the copied blob; the blob only reads these fields after `entry` is called below.
    unsafe {
        write_volatile(addr_of_mut!((*ctl).vector), vector);
        write_volatile(addr_of_mut!((*ctl).flags_in), (regs.flags & 0x08D5) | 0x0002);
        write_volatile(addr_of_mut!((*ctl).regs), *regs);
        // The blob saves and restores everything it changes (pushad, pushfd, the descriptor tables, the segments, ESP and CR0),
        // so apart from the memory it writes the call is an ordinary function call.
        asm!("call {entry}", entry = in(reg) ENTRY, clobber_abi("C"));
        *regs = read_volatile(addr_of!((*ctl).regs));
    }

    // Whatever the timer raised in the last moments, after the BIOS handler could no longer take it, is the kernel's.
    let tick_late = PIC.pending_mask() & 1 != 0;
    PIC.init(REMAPPED_BASES, saved_mask);
    PORTS.outb(CMOS_INDEX, if NMI_ENABLED_AFTER { 0 } else { NMI_DISABLE });

    // The BIOS timer handler counted the ticks the kernel's handler did not see: the kernel keeps the PIT at its own rate, so one
    // BIOS tick is one kernel tick. A midnight rollover resets the count, so the delta is meaningless then and nothing is added.
    let (ticks_after, midnight) = bda_ticks();
    let counted = if midnight != 0 { 0 } else { ticks_after.wrapping_sub(ticks_before).min(10_000) };
    let pending = u32::from(tick_waiting) + u32::from(tick_late);
    for _ in 0..counted + pending {
        CLOCK.counter().tick();
    }
    STATS.ticks_added.fetch_add(counted, Ordering::Relaxed);
    STATS.pending_ticks.fetch_add(pending, Ordering::Relaxed);

    // A keyboard or mouse byte that arrived while its line was masked sits in the controller's output buffer, and the controller
    // raises no second edge for it. The drivers' handlers are run by hand, as the interrupt would have.
    for _ in 0..8 {
        let status = PORTS.inb(KBC_STATUS);
        if status & KBC_OUTPUT_FULL == 0 {
            break;
        }
        let line = if status & KBC_FROM_MOUSE != 0 { Irq::MOUSE } else { Irq::KEYBOARD };
        if !IRQS.run_handler(line) {
            break;
        }
        STATS.input_bytes.fetch_add(1, Ordering::Relaxed);
    }
    STATS.calls.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

/// The safe call: takes the re-entry guard, refuses the contexts `bios_int` cannot serve, and runs the call with interrupts off
/// (restoring the caller's interrupt flag afterwards).
///
/// Errors are about whether the call could be made; whether the BIOS function succeeded is in `regs` (usually the carry flag, and
/// for VBE `AX`).
pub fn call(vector: u8, regs: &mut RealModeRegs) -> Result<(), BiosError> {
    if !READY.load(Ordering::Acquire) {
        return Err(BiosError::NotReady);
    }
    let _guard = Guard::acquire()?;
    sync::without_interrupts(|| {
        if PIC.in_service_mask() != 0 {
            return Err(BiosError::InInterrupt);
        }
        if cr0() & CR0_PG != 0 {
            return Err(BiosError::PagingEnabled);
        }
        if vector_entry(vector) == 0 {
            return Err(BiosError::NoHandler(vector));
        }
        // SAFETY: the guard is held, interrupts are off, no handler is in service, paging is off and the vector has a handler.
        unsafe { bios_int(vector, regs) }
    })
}

/// Self-test of the re-entry guard: holds it and tries a call, which must come back `Busy`. `true` if it did.
pub fn guard_refuses_reentry() -> bool {
    let Ok(_held) = Guard::acquire() else { return false };
    let mut r = RealModeRegs::with_eax(0x0200);
    call(0x1A, &mut r) == Err(BiosError::Busy)
}

/// The native platform's `Bios`: the HAL's real-mode call over the trampoline, so `desk-vga`'s VBE layer runs unchanged.
pub struct NativeBios;

impl Bios for NativeBios {
    fn int(&self, vector: u8, regs: &mut BiosRegs, buffer: Option<&mut [u8]>) -> Result<(), HalError> {
        let mut r = RealModeRegs {
            eax: regs.eax,
            ebx: regs.ebx,
            ecx: regs.ecx,
            edx: regs.edx,
            esi: regs.esi,
            edi: regs.edi,
            ebp: regs.ebp,
            es: regs.es,
            ds: regs.ds,
            flags: regs.flags,
            ..RealModeRegs::default()
        };
        let bounce = bounce_buffer();
        let len = buffer.as_ref().map_or(0, |b| b.len());
        if let Some(b) = buffer.as_deref() {
            if b.len() > BIOS_BUFFER_MAX {
                return Err(HalError::InvalidArgument);
            }
            bounce.write_low(0, b)?;
            r.es = BOUNCE_SEGMENT;
            r.edi = 0;
        }
        call(vector, &mut r)?;
        regs.eax = r.eax;
        regs.ebx = r.ebx;
        regs.ecx = r.ecx;
        regs.edx = r.edx;
        regs.esi = r.esi;
        regs.edi = r.edi;
        regs.ebp = r.ebp;
        regs.es = r.es;
        regs.ds = r.ds;
        regs.flags = r.flags;
        if let Some(b) = buffer {
            bounce.read_low(0, &mut b[..len])?;
        }
        Ok(())
    }

    fn read_low(&self, segment: u16, offset: u16, out: &mut [u8]) -> Result<(), HalError> {
        read_low(segment, offset, out).map_err(HalError::from)
    }
}

/// Where the stack the BIOS runs on starts, for the transcript.
pub const fn stack_base() -> u32 {
    STACK_BASE
}
