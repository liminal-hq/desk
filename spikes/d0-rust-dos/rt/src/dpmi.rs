// A client for the DPMI host (CWSDPMI): INT 31h services and the flat address space the runtime builds on them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The stub's data selector has a base (the start of the image's block) and a limit of just that block. Memory from DPMI function
// 0501h lives elsewhere in the linear address space, so `init` raises the limit of DS to 4 GiB. A pointer is then the linear
// address minus the base, wrapping, which is exactly what the segment arithmetic undoes: base + offset, modulo 2^32. The first
// megabyte (the PSP, the environment, the VGA frame buffer) is mapped at linear 0 by CWSDPMI, so it is reachable the same way.
use core::arch::asm;
use core::sync::atomic::{AtomicU32, Ordering};

/// The registers a DPMI call takes and returns. Only the general registers an INT 31h function uses.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Regs {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
    pub esi: u32,
    pub edi: u32,
    /// Out: 1 if the carry flag was set (the call failed and AX holds the error code)
    pub carry: u32,
}

/// An error code from a DPMI call (the AX the host returned).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error(pub u16);

/// `INT 31h` with every register loaded from and stored to `r`.
///
/// ESI, EDI and EBX cannot be named as operands of inline assembly on x86-32 (LLVM keeps them), so the registers go through
/// a structure the block loads from and stores to, saving and restoring what the caller expects preserved.
pub fn int31(r: &mut Regs) {
    // SAFETY: the block saves and restores every register it touches that the compiler did not give it (EBX, ESI, EDI), reads
    // and writes only `*r`, and leaves the stack balanced. INT 31h itself is a call to the host, which preserves the segment
    // registers.
    unsafe {
        asm!(
            "push ebx",
            "push esi",
            "push edi",
            "push edx",
            "mov eax, [edx]",
            "mov ebx, [edx + 4]",
            "mov ecx, [edx + 8]",
            "mov esi, [edx + 16]",
            "mov edi, [edx + 20]",
            "mov edx, [edx + 12]",
            "int 0x31",
            "pushfd",
            "push edx",
            "mov edx, [esp + 8]",
            "mov [edx], eax",
            "mov [edx + 4], ebx",
            "mov [edx + 8], ecx",
            "mov [edx + 16], esi",
            "mov [edx + 20], edi",
            "pop dword ptr [edx + 12]",
            "pop eax",
            "and eax, 1",
            "mov [edx + 24], eax",
            "add esp, 4",
            "pop edi",
            "pop esi",
            "pop ebx",
            inout("edx") r as *mut Regs => _,
            out("eax") _,
            out("ecx") _,
        );
    }
}

fn call(ax: u32, bx: u32, cx: u32, dx: u32, si: u32, di: u32) -> Result<Regs, Error> {
    let mut r = Regs {
        eax: ax,
        ebx: bx,
        ecx: cx,
        edx: dx,
        esi: si,
        edi: di,
        carry: 0,
    };
    int31(&mut r);
    if r.carry != 0 {
        Err(Error(r.eax as u16))
    } else {
        Ok(r)
    }
}

/// The linear address of the start of the image. Set by `init`.
static BASE: AtomicU32 = AtomicU32::new(0);
/// The flat data selector (DS at entry). Set by `init`.
static DATA_SELECTOR: AtomicU32 = AtomicU32::new(0);

/// The linear address of offset 0 of the program's data segment.
pub fn image_base() -> u32 {
    BASE.load(Ordering::Relaxed)
}

/// The data selector the program runs on, which interrupt handlers must load before touching their statics.
pub fn data_selector() -> u16 {
    DATA_SELECTOR.load(Ordering::Relaxed) as u16
}

/// Turns a linear address into a pointer through the 4 GiB data segment.
pub fn linear_to_ptr<T>(linear: u32) -> *mut T {
    linear.wrapping_sub(image_base()) as usize as *mut T
}

/// Turns a pointer into the linear address it names.
pub fn ptr_to_linear<T>(p: *const T) -> u32 {
    (p as usize as u32).wrapping_add(image_base())
}

/// Brings up the client: records the base, lifts the limit of DS to 4 GiB and reloads the segment registers.
///
/// # Safety
/// Call once, first, from `rt_entry`, with DS holding the stub's data selector.
pub unsafe fn init() {
    let ds: u16;
    // SAFETY: reading a segment register has no effect.
    unsafe { asm!("mov {0:x}, ds", out(reg) ds, options(nomem, nostack, preserves_flags)) };
    DATA_SELECTOR.store(ds as u32, Ordering::Relaxed);
    let base = get_segment_base(ds).unwrap_or_else(|_| crate::dos::die(b"dpmi: 0006h failed\r\n"));
    BASE.store(base, Ordering::Relaxed);
    if set_segment_limit(ds, 0xFFFF_FFFF).is_err() {
        crate::dos::die(b"dpmi: 0008h failed\r\n");
    }
    // SAFETY: reloading DS, ES and SS with the selector they already hold, so the CPU picks up the new limit. SS is reloaded
    // with a stack pointer already inside the old limit, so the instruction after cannot fault.
    unsafe {
        asm!("mov ds, {0:x}", "mov es, {0:x}", "mov ss, {0:x}", in(reg) ds, options(nostack, preserves_flags));
    }
}

/// DPMI 0400h: version. Returns (major, minor, flags, processor, master PIC base, slave PIC base).
pub fn version() -> (u8, u8, u16, u8, u8, u8) {
    let r = call(0x0400, 0, 0, 0, 0, 0).unwrap_or_default();
    (
        (r.eax >> 8) as u8,
        r.eax as u8,
        r.ebx as u16,
        r.ecx as u8,
        (r.edx >> 8) as u8,
        r.edx as u8,
    )
}

/// DPMI 0006h: the linear base address of a selector.
pub fn get_segment_base(sel: u16) -> Result<u32, Error> {
    let r = call(0x0006, sel as u32, 0, 0, 0, 0)?;
    Ok((r.ecx << 16) | (r.edx & 0xFFFF))
}

/// DPMI 0007h: set the base address of a selector.
pub fn set_segment_base(sel: u16, base: u32) -> Result<(), Error> {
    call(0x0007, sel as u32, base >> 16, base & 0xFFFF, 0, 0).map(|_| ())
}

/// DPMI 0008h: set the limit of a selector. Above 1 MiB the host needs the low 12 bits set (page granularity).
pub fn set_segment_limit(sel: u16, limit: u32) -> Result<(), Error> {
    call(0x0008, sel as u32, limit >> 16, limit & 0xFFFF, 0, 0).map(|_| ())
}

/// DPMI 0000h: allocate one LDT descriptor.
pub fn alloc_descriptor() -> Result<u16, Error> {
    call(0x0000, 0, 1, 0, 0, 0).map(|r| r.eax as u16)
}

/// DPMI 0001h: free an LDT descriptor.
pub fn free_descriptor(sel: u16) -> Result<(), Error> {
    call(0x0001, sel as u32, 0, 0, 0, 0).map(|_| ())
}

/// DPMI 0009h: set the access rights and the type/size bits of a descriptor.
pub fn set_descriptor_access(sel: u16, rights: u16) -> Result<(), Error> {
    call(0x0009, sel as u32, rights as u32, 0, 0, 0).map(|_| ())
}

/// DPMI 0002h: a descriptor for a real-mode segment (64 KiB limit).
pub fn segment_to_descriptor(segment: u16) -> Result<u16, Error> {
    call(0x0002, segment as u32, 0, 0, 0, 0).map(|r| r.eax as u16)
}

/// A block of extended memory from the host.
#[derive(Clone, Copy, Debug)]
pub struct Block {
    /// Linear address of the first byte
    pub linear: u32,
    /// The host's handle, for `free_block`
    pub handle: u32,
    pub size: u32,
}

/// DPMI 0501h: allocate a block of memory. The host returns page-aligned, committed memory.
pub fn alloc_block(size: u32) -> Result<Block, Error> {
    let r = call(0x0501, size >> 16, size & 0xFFFF, 0, 0, 0)?;
    Ok(Block {
        linear: (r.ebx << 16) | (r.ecx & 0xFFFF),
        handle: (r.esi << 16) | (r.edi & 0xFFFF),
        size,
    })
}

/// DPMI 0502h: free a block by its handle.
pub fn free_block(handle: u32) -> Result<(), Error> {
    call(0x0502, 0, 0, 0, handle >> 16, handle & 0xFFFF).map(|_| ())
}

/// DPMI 0600h: lock a linear region so the host never pages it out. Anything an interrupt handler touches must be locked.
pub fn lock(linear: u32, len: u32) -> Result<(), Error> {
    call(0x0600, linear >> 16, linear & 0xFFFF, 0, len >> 16, len & 0xFFFF).map(|_| ())
}

/// DPMI 0601h: unlock a linear region.
pub fn unlock(linear: u32, len: u32) -> Result<(), Error> {
    call(0x0601, linear >> 16, linear & 0xFFFF, 0, len >> 16, len & 0xFFFF).map(|_| ())
}

/// DPMI 0204h: the protected-mode vector for interrupt `n`, as (selector, offset).
pub fn get_pm_vector(n: u8) -> Result<(u16, u32), Error> {
    let r = call(0x0204, n as u32, 0, 0, 0, 0)?;
    Ok((r.ecx as u16, r.edx))
}

/// DPMI 0205h: set the protected-mode vector for interrupt `n`.
pub fn set_pm_vector(n: u8, selector: u16, offset: u32) -> Result<(), Error> {
    call(0x0205, n as u32, selector as u32, offset, 0, 0).map(|_| ())
}

/// DPMI 0200h: the real-mode vector for interrupt `n`, as (segment, offset).
pub fn get_rm_vector(n: u8) -> Result<(u16, u16), Error> {
    let r = call(0x0200, n as u32, 0, 0, 0, 0)?;
    Ok((r.ecx as u16, r.edx as u16))
}

/// DPMI 0201h: set the real-mode vector for interrupt `n`.
pub fn set_rm_vector(n: u8, segment: u16, offset: u16) -> Result<(), Error> {
    call(0x0201, n as u32, segment as u32, offset as u32, 0, 0).map(|_| ())
}

/// DPMI 0100h: allocate conventional memory. Returns (real-mode segment, protected-mode selector).
pub fn alloc_dos_memory(paragraphs: u16) -> Result<(u16, u16), Error> {
    call(0x0100, paragraphs as u32, 0, 0, 0, 0).map(|r| (r.eax as u16, r.edx as u16))
}

/// DPMI 0101h: free conventional memory by its selector.
pub fn free_dos_memory(selector: u16) -> Result<(), Error> {
    call(0x0101, 0, 0, selector as u32, 0, 0).map(|_| ())
}

/// The real-mode register block of DPMI 0300h: 0x32 bytes, in the order the host defines.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct RealRegs {
    pub edi: u32,
    pub esi: u32,
    pub ebp: u32,
    pub reserved: u32,
    pub ebx: u32,
    pub edx: u32,
    pub ecx: u32,
    pub eax: u32,
    pub flags: u16,
    pub es: u16,
    pub ds: u16,
    pub fs: u16,
    pub gs: u16,
    pub ip: u16,
    pub cs: u16,
    pub sp: u16,
    pub ss: u16,
}

/// DPMI 0300h: run a real-mode interrupt with the given register block (the host supplies the stack when SS:SP are zero).
pub fn simulate_int(int: u8, regs: &mut RealRegs) -> Result<(), Error> {
    let p = regs as *mut RealRegs as usize as u32;
    call(0x0300, int as u32, 0, 0, 0, p).map(|_| ())
}
