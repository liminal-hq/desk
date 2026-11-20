// The entry point and the contract with DJGPP's stub loader.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The stub (STUB.EXE, the first 2 KiB of the .EXE) finds a DPMI host, switches to protected mode, allocates one memory block for
// the image, makes two descriptors for it (a code and a data descriptor with the same base, both 32-bit), copies the .text and
// .data sections in, zeroes .bss, and far-jumps to the entry point. It hands over:
//
// - CS    the code selector; DS and ES the data selector. Base is the start of the image's block, limit is the block's size
// - FS    a selector for the stub's own data in conventional memory; FS:0 is the `_GO32_StubInfo` block (see `StubInfo`)
// - SS:ESP  still the stub's own protected-mode stack, a few hundred bytes, not usable for a program
// - GS    unspecified; the direction flag is whatever DOS left, so the entry clears it
// - the x87 is in whatever state DOS left it (not initialised by the stub), so the entry runs FNINIT
//
// The entry therefore snapshots every register for inspection, copies the stub information into the image (FS may be reused),
// switches to a stack in .bss and calls `rt_entry`. Nothing is passed on the stack and there is no return address to go back to.
use core::arch::global_asm;

/// The size of the program's stack. It lives in .bss, so it costs address space and no file size; the stub sizes the block to
/// the end of .bss. Large frames need no probing: there is no guard page below it, so keep the figure generous.
pub const STACK_SIZE: usize = 256 * 1024;

#[repr(C, align(16))]
pub struct Stack(pub [u8; STACK_SIZE]);

#[no_mangle]
pub static mut RT_STACK: Stack = Stack([0; STACK_SIZE]);

/// The registers at the first instruction of `_start`, in the order the stub left them.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct EntryRegs {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
    pub esi: u32,
    pub edi: u32,
    pub ebp: u32,
    pub esp: u32,
    pub eflags: u32,
    pub cs: u32,
    pub ds: u32,
    pub es: u32,
    pub fs: u32,
    pub gs: u32,
    pub ss: u32,
    /// x87 control word and status word, as found (before FNINIT).
    pub fcw: u32,
    pub fsw: u32,
}

#[no_mangle]
pub static mut RT_ENTRY_REGS: EntryRegs = EntryRegs {
    eax: 0,
    ebx: 0,
    ecx: 0,
    edx: 0,
    esi: 0,
    edi: 0,
    ebp: 0,
    esp: 0,
    eflags: 0,
    cs: 0,
    ds: 0,
    es: 0,
    fs: 0,
    gs: 0,
    ss: 0,
    fcw: 0,
    fsw: 0,
};

/// DJGPP's `_GO32_StubInfo` (`stubinfo.h`), copied out of the stub's data at entry.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct StubInfo {
    /// "go32stub, v 2.05" or similar
    pub magic: [u8; 16],
    /// Size of this structure (0x54)
    pub size: u32,
    /// Minimum stack the program asked for (`stubedit minstack=`); the runtime uses its own stack and ignores it
    pub minstack: u32,
    /// DPMI handle of the image's memory block
    pub memory_handle: u32,
    /// Size of the image's memory block in bytes, a multiple of 64 KiB
    pub initial_size: u32,
    pub minkeep: u16,
    /// Selector for the stub's own data segment (reachable through FS at entry)
    pub ds_selector: u16,
    /// Real-mode segment of the stub's data
    pub ds_segment: u16,
    /// Selector for the PSP: base is the PSP's linear address, limit 0xFF
    pub psp_selector: u16,
    pub cs_selector: u16,
    /// Size in bytes of the environment block
    pub env_size: u16,
    /// Base name of the program, 8 bytes, NUL-padded
    pub basename: [u8; 8],
    /// argv[0] file component, 16 bytes
    pub argv0: [u8; 16],
    /// Name of the DPMI server to load when none is present, 16 bytes
    pub dpmi_server: [u8; 16],
}

#[no_mangle]
pub static mut RT_STUB_INFO: StubInfo = StubInfo {
    magic: [0; 16],
    size: 0,
    minstack: 0,
    memory_handle: 0,
    initial_size: 0,
    minkeep: 0,
    ds_selector: 0,
    ds_segment: 0,
    psp_selector: 0,
    cs_selector: 0,
    env_size: 0,
    basename: [0; 8],
    argv0: [0; 16],
    dpmi_server: [0; 16],
};

/// The registers as the stub left them.
pub fn entry_regs() -> EntryRegs {
    // SAFETY: written once by `_start` before any Rust code runs, read-only afterwards.
    unsafe { *core::ptr::addr_of!(RT_ENTRY_REGS) }
}

/// The stub's information block.
pub fn stub_info() -> StubInfo {
    // SAFETY: as `entry_regs`.
    unsafe { *core::ptr::addr_of!(RT_STUB_INFO) }
}

global_asm!(
    ".text",
    ".global _start",
    "_start:",
    // Snapshot first: nothing below may disturb a register before it is stored.
    "mov [{regs} + 0], eax",
    "mov [{regs} + 4], ebx",
    "mov [{regs} + 8], ecx",
    "mov [{regs} + 12], edx",
    "mov [{regs} + 16], esi",
    "mov [{regs} + 20], edi",
    "mov [{regs} + 24], ebp",
    "mov [{regs} + 28], esp",
    "pushfd",
    "pop eax",
    "mov [{regs} + 32], eax",
    "mov word ptr [{regs} + 36], cs",
    "mov word ptr [{regs} + 40], ds",
    "mov word ptr [{regs} + 44], es",
    "mov word ptr [{regs} + 48], fs",
    "mov word ptr [{regs} + 52], gs",
    "mov word ptr [{regs} + 56], ss",
    "fnstcw word ptr [{regs} + 60]",
    "fnstsw word ptr [{regs} + 64]",
    // The direction flag is not guaranteed clear.
    "cld",
    // Copy the stub information block (0x54 bytes at FS:0) into the image.
    "xor esi, esi",
    "lea edi, [{info}]",
    "mov ecx, 21",
    "2:",
    "mov eax, fs:[esi]",
    "mov [edi], eax",
    "add esi, 4",
    "add edi, 4",
    "dec ecx",
    "jnz 2b",
    // The segment registers: ES and DS already hold the data selector; SS must too before the stack moves.
    "mov ax, ds",
    "mov es, ax",
    "mov ss, ax",
    "lea esp, [{stack} + {stack_size}]",
    "xor ebp, ebp",
    // x87 to its reset state: the stub leaves whatever DOS had.
    "fninit",
    "call {entry}",
    // `rt_entry` never returns.
    "ud2",
    regs = sym RT_ENTRY_REGS,
    info = sym RT_STUB_INFO,
    stack = sym RT_STACK,
    stack_size = const STACK_SIZE,
    entry = sym rt_entry,
);

/// First Rust code after `_start`. Brings up the DPMI client, then runs the program and exits with its status.
extern "C" fn rt_entry() -> ! {
    // SAFETY: the stub's selectors are valid and this is the first thing the program does; `dpmi::init` documents its needs.
    unsafe { crate::dpmi::init() };
    extern "Rust" {
        fn __rt_main() -> i32;
    }
    // SAFETY: `entry!` defines `__rt_main`; a program that does not define one fails to link.
    let code = unsafe { __rt_main() };
    crate::dos::exit(code)
}
