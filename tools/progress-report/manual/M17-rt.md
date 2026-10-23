---
id: m-rt
n: M17
part: VI
title: The hosted runtime and the build to SHELL.EXE
summary: What a DOS extender and a DPMI host are, the runtime that lets a Rust program start under one, its heap, DOS calls, panic, clean-up, interrupt stubs and hardware layer, then the build that turns Rust into SHELL.EXE; with the first hosted skeleton and the DXE3 result as a marked legacy section.
date: 2026-10-23
reviewed: 44f0265
watches: [spikes/d0-rust-dos/rt/, spikes/d0-rust-dos/dos-elf.json, spikes/d0-rust-dos/link.ld, spikes/d0-rust-dos/build.sh, spikes/d0-rust-dos/check.sh, tools/elf2go32.py, tools/coff2go32.py, shell/dos/build.sh, shell/dos/check.sh, core/, c/]
log: [wheels, module, purerust, firstwindow]
anatomy: [dos:stubr, dos:start, dos:heap, dos:dosw, dos:dpmi, dos:irq, dos:rthal, dos:exit, dos:target, dos:lld, dos:pack, dos:stubf, dos:exe, dos:cws]
---
## What it is

A DOS program that does anything useful has to solve a problem the native kernel never meets: it starts in someone else's machine. DOS owns the first 640 KB and the files; a second program, the [DPMI](term:dpmi) host, owns protected mode and all the memory above 1 MiB; and a small loader called a [stub](term:stub) is what DOS actually runs when you type the name. A Rust program that wants a heap, a screen and a keyboard under those three has to learn what each one hands over and what it will not do. The crate that learns this is `rt`, in `spikes/d0-rust-dos/rt/`, a "Rust runtime for 32-bit DOS programs under a DPMI host: entry, DPMI, INT 21h, heap, panic, x87" (its `Cargo.toml`). This chapter is about `rt` and about the build that surrounds it, from `dos-elf.json` to the file `SHELL.EXE`.

On the map ([M1](chapter:m-map)) this is the [hosted track](term:hosted-track)'s platform: the DOS-side counterpart of the native kernel's start-up code, interrupt tables and memory manager ([M5](chapter:m-boot) to [M8](chapter:m-bioscalls)). The [hardware abstraction layer](term:hal) ([M9](chapter:m-hal)) is the contract; `rt` is one [platform](term:platform) that implements it, and its two implementing types, `DosBios` and `DosDisplay`, are the last part of the runtime below. The anatomy page of the hosted program draws fourteen boxes for this chapter, from the stub that starts first to CWSDPMI underneath; the front matter lists them. After it you should be able to say what happens between typing `SHELL` and the first line of Rust, explain why every DOS file call goes through a buffer, follow one memory request to the DPMI host and back, build `SHELL.EXE` and say which byte of it came from where, and tell the live pipeline from the legacy one that the last section describes.

A caution about names. The runtime lives under `spikes/` because it began as spike d0 (`purerust`, PR #7), and `docs/D0-RUST-DOS.md` recommends promoting it to a top-level `rt/` crate. That has not happened: `shell/dos/Cargo.toml` still depends on `../../spikes/d0-rust-dos/rt`, and `docs/FIRST-WINDOW.md` records the promotion as a separate, open decision. This chapter calls it `rt` as the code does.

## How it works

### What a DOS extender and a DPMI host are

DOS was written for a processor in real mode, where a program can address one megabyte and the operating system's calls take 16-bit segments and offsets in the first 640 KB. A 386 or 486 can address 4 GiB in [protected mode](term:protected-mode), but a program that switches mode loses the ability to call DOS directly, because DOS's code expects the other mode. A [DOS extender](term:dos-extender) is the software that bridges the two: it puts the processor in protected mode, gives the program memory above the first megabyte, and arranges for the program's DOS and BIOS calls to be carried across to real mode and back. [DPMI](term:dpmi), the DOS Protected Mode Interface, is the standard set of calls a program uses to ask such a host for those services: allocate memory, make [descriptors](term:descriptor), hook interrupts, run a real-mode interrupt. Programs call it with `INT 31h`.

> **From the stacks — Why DPMI exists.** The Wikipedia article on DOS extenders says they were developed in the 1980s after the 80286 and 80386 to cope with the memory limitations of DOS. It says DOS functions needed 16-bit addresses within the first 640 kilobytes while protected mode used an incompatible addressing method, and that switching modes was costly. It names VCPI as the first of two interfaces, rapidly overshadowed by DPMI, which it says grew out of Windows 3.0 development. Its references include a DPMI 1.0 specification dated 12 March 1991.
> Source: [DOS extender, Wikipedia](https://en.wikipedia.org/wiki/DOS_extender)

The Desk uses the combination that DJGPP uses. CWSDPMI is the host, a separate program (`CWSDPMI.EXE`, 21,325 bytes in the build recorded by `docs/D0-RUST-DOS.md`) that loads when a program needs it and unloads when the program ends. DJGPP's stub is a 2 KiB real-mode program at the front of every `.EXE` that finds a host, switches to protected mode, loads the image behind it and jumps in. The runtime's whole contract is with these two, and `rt` reports the host's version as 0.90 (`expected/hello.txt`). Nothing here has been tried under another host, under EMM386 or QEMM, or on real hardware (`docs/D0-RUST-DOS.md`, "What was and was not verified").

### The contract: what the stub hands over

The stub is not documented in a form the project could read, so the contract was found by disassembling it and by running a program, `ctx`, whose only job is to print what it received (`docs/D0-RUST-DOS.md`, section 2). The transcript of that program is `spikes/d0-rust-dos/expected/ctx.txt`; the values below are from it.

| At the first instruction | Value in the transcript | What it means |
|---|---|---|
| `CS` | `00a7`, base `4194304` (`0x400000`) | A 32-bit code selector whose base is the start of the memory block the stub allocated for the image; the limit, `0x4ffff`, is that block's size minus one, 320 KiB |
| `DS`, `ES` | `00af` | One 32-bit data selector with the same base as `CS`. The transcript prints its limit as `0xffffffff` because the program prints after `rt` has already raised it (below) |
| `FS` | `008f`, base `6672`, limit `0xffff` | A selector for the stub's own data in conventional memory. `FS:0` is DJGPP's information block, 0x54 bytes, magic `go32stub, v 2.05` |
| `SS:ESP` | `008f`, `ESP` about `0x760` per the document | The stub's own stack, a few hundred bytes; not usable by a program |
| `GS` | `0000` | Nothing |
| `EFLAGS` | `00003212` | Interrupts on, direction flag clear (do not rely on it) |
| x87 control word | `0x037f` | Whatever DOS left. Not promised, so the runtime resets the unit |
| The block | `initial_size 0x50000` | The stub rounds the end of `.bss` up to 64 KiB, and no lower than 128 KiB (`tools/elf2go32.py`) |

Nothing is passed on the stack and there is no return address worth using. The stub's information block also carries the memory handle, the selector for the program segment prefix (`psp_selector`, base the PSP's linear address), the environment size, and the name of the host to load if none is present (`dpmi_server`, here `CWSDPMI.EXE`).

### `_start`

The entry point is a block of `global_asm!` in `rt/src/start.rs`; it has no prologue and a long run of instructions that must not be reordered, so a plain label is clearer than a naked function, and `docs/D0-RUST-DOS.md` records that `#[unsafe(naked)]` also works on the pinned compiler. In order, it:

1. stores every register into a static `EntryRegs` (17 fields, including the segment registers and the x87's control and status words) before anything can disturb them, which is what the `ctx` program prints;
2. clears the direction flag, because the stub does not promise it;
3. copies the stub's information block, 21 dwords or 84 bytes, from `FS:0` into a static `StubInfo` in the image, since `FS` may be reused;
4. loads `SS` from `DS`, so the stack segment and the data segment are the same flat one, and points `ESP` at the top of `RT_STACK`, a 256 KiB array in [bss](term:bss) (it costs address space and no file size, and there is no guard page below it, so an overflow corrupts data and does not fault);
5. runs `FNINIT`, which leaves the [x87](term:x87) control word at `0x037F` (64-bit mantissa, round to nearest, exceptions masked), and calls `rt_entry`; the instruction after the call is `ud2`, since `rt_entry` never returns.

`rt_entry` calls `dpmi::init`, then the program's own function and then `dos::exit` with the status it returned. The program names its function with the macro `rt::entry!(main)`, where `main` is `fn() -> i32`; the macro defines a symbol `__rt_main`, and a program without one fails to link.

### Flat memory: `dpmi::init`

The stub's data selector covers only the image's own block. Memory from the host's allocator is elsewhere in the linear address space, so `init` asks the host for the selector's base (DPMI 0006h), stores it, then raises the selector's limit to 4 GiB (0008h) and reloads `DS`, `ES` and `SS`. From then on a pointer is the [linear address](term:linear-address) minus the base, wrapping, and the segment arithmetic (base plus offset, modulo 2^32) undoes the subtraction. `linear_to_ptr` and `ptr_to_linear` are the two conversions, and every address that is not the program's own goes through them. The first megabyte (the PSP, the environment, the BIOS data area, the VGA window at `0xA0000`) is mapped at linear 0 by CWSDPMI, so it is reachable the same way. `docs/D0-RUST-DOS.md` calls this "the same near-pointer arrangement DJGPP programs use".

Every DPMI call goes through one function, `int31`, which takes a block of registers and loads them, runs `INT 31h` and stores them back. The block exists because on 32-bit x86 the compiler will not let inline assembly name `EBX`, `ESI` or `EDI` as operands, so the routine saves them, loads from the block and restores them. A wrapper per service sits on top: segments (0000h to 0002h, 0006h to 0009h), memory (0501h and 0502h), locking (0600h and 0601h), vectors (0200h and 0201h in real mode, 0204h and 0205h in protected mode), conventional memory (0100h and 0101h), the real-mode call (0300h) and physical mapping (0800h and 0801h).

### The heap over 0501h

`rt/src/heap.rs` is the `#[global_allocator]`: a first-fit [free list](term:free-list) sorted by address, so neighbours merge when a block is freed. Each allocation carries an 8-byte header holding where its block starts and how long it is, so `dealloc` needs nothing from the `Layout`. When no free node fits, `grow` asks the host for memory with DPMI 0501h, a size of at least `CHUNK`, 256 KiB, rounded up to 4096, adds it to the free list and tries again. Memory is never returned before exit, when the host reclaims it all. The heap is single-threaded and not reentrant, so an interrupt handler must never allocate. It is the same idea as the kernel's heap ([M7](chapter:m-memory)) with a different supplier of memory: there, 4 MiB of frames taken once at boot; here, chunks asked of a host on demand. One call is followed through with real numbers in the worked example below.

### INT 21h and the transfer buffer

The most expensive fact of the spike was found by accident. The first `hello` ran, panicked and printed 460 NUL bytes. A probe that wrote 64 bytes from `DS:0x1000` got CWSDPMI's interrupt gates, and one from `DS:0x200` got the host's own error strings. The explanation (`docs/D0-RUST-DOS.md`, "What failed on the way") is that **CWSDPMI r7 does not translate buffer pointers for a protected-mode `INT 21h`**: it reflects the call to real mode with the registers as they are, so `DX` is an offset in whatever the real-mode `DS` happens to be, which is the host's. The runtime keeps the evidence in its own transcript: `dpmi: direct INT 21h AH=40h with a pointer: DOS reports 4 bytes, file holds the buffer: false`.

So every DOS call that takes a pointer goes through a [transfer buffer](term:transfer-buffer): a block of conventional memory, allocated once with DPMI 0100h (`TB_PARAGRAPHS` is `0x800`, so 32 KiB), reachable from both modes because conventional memory is linear 0. To write, `dos::write` copies up to 32 KiB into the buffer, loads a `RealRegs` block with `AX = 4000h`, the handle, the count and `DS` set to the buffer's real-mode segment, and asks the host to run the interrupt in real mode (0300h, `simulate_int`). The host supplies the real-mode stack when `SS:SP` are zero. The carry flag in the returned flags is the error, and `AX` the code. Reads copy the other way. Larger transfers loop, which is why a 300,000-byte file is written and read back in one call and in 4 KiB pieces in the transcript. Only the exit (`AH=4Ch`), which takes no pointer and never returns, is made directly; the close (`AH=3Eh`) takes no pointer either but still goes through `simulate_int` in the code, although the file's header comment says it is made directly. The set is small: `09h`, `3Ch`, `3Dh`, `3Eh`, `3Fh`, `40h`, `41h`, `42h` and `4Ch`. The same trick, with a smaller buffer, is how the BIOS is reached below; the native track's version is the [bounce buffer](term:bounce-buffer).

### Panic, exit and the clean-up list

A hooked interrupt vector left pointing into a program that has gone is a crash waiting for the next tick, and a screen left in a graphics mode is a user at a prompt they cannot read. `rt/src/cleanup.rs` is a list of eight function pointers (`SLOTS`). Whatever installs such a thing registers a function; `exit` and the panic handler run them, last registered first, once each, and empty the list. `register` returns `false` when the list is full, and the interrupt code refuses to install a vector if it cannot register its undo first. The list uses loads, stores and an exchange (`swap`, which is `XCHG`), not compare-and-swap, for a reason found the hard way: DOSBox's `cputype = 486` raises an exception on `CMPXCHG`, which Rust's `compare_exchange` emits for this target, and a real 486 from the B step on has it. Nothing in the runtime uses it, and the shell crates ban it by lint (`shell/clippy.toml`).

The panic handler, `rt/src/panic.rs`, sets a flag with a swap (a second panic inside the handler or a clean-up function exits at once), runs the list first, so that a hooked timer is off the machine before anything formats (formatting can allocate, and allocation can fail), prints `RT PANIC: ` and the panic message to standard output, and exits with status 3. Standard output is what the headless harness captures. Status 3 is the skeleton's choice, a status no ordinary run returns. The other ways a run ends:

| Status | Cause | Where it is recorded |
|---|---|---|
| the program's own (`0` and up) | `dos::exit`: the list runs, then `AH=4Ch` with the status in `AL` | `rt/src/dos.rs` |
| `3` | A panic, including an allocation failure | `expected/panicky.txt`; `check.sh` |
| `255` | `dos::die`, before the runtime is up, with no allocation and no formatting | `rt/src/dos.rs` |
| `110` | No `CWSDPMI.EXE` beside the program: the stub's own message, `Load error: no DPMI - Get csdpmi*b.zip` | `docs/D0-RUST-DOS.md`, section 6 |
| `1`, with no output | A CPU exception inside the program; the host reports it on the console, which the harness does not capture | `docs/D0-RUST-DOS.md`, "What failed" |
| `125` | DOSBox itself failed (`tools/dosrun.sh`'s own status) | `tools/dosrun.sh` |

The recommendation in the document, an exception handler (DPMI 0203h) that prints a register dump so that a crash is visible, is not built; `rt` has no such handler.

### The IRQ stubs

A hardware interrupt arrives while any code is running, on a stack and with segment registers the host chose. Rust cannot be the first instruction: the handler must first find its own data. `rt/src/irq.rs` generates sixteen assembly stubs, one for each [IRQ](term:irq) line, with a macro. Each one:

1. saves the registers it will clobber, loads `DS` and `ES` with the flat data selector (read through `CS`, because `DS` cannot be trusted yet), and **switches to an 8 KiB interrupt stack inside the program's own data segment**, so Rust code may dereference a pointer to a local through `DS`;
2. calls `rt_irq_dispatch(line)`, which looks the line up in a table of `IrqHandler`s, runs the handler and, for a line the Desk owns (`Chain::None`), sends the [end of interrupt](term:eoi) through the `desk-pic` driver. On a line the Desk owns, a spurious IRQ 7 or 15 runs no handler and is acknowledged as the driver says ([M10](chapter:m-sysdrivers));
3. switches back with `lss` and returns with `iretd`, or, for `Chain::Previous`, jumps to the handler it replaced, which acknowledges the interrupt itself (the BIOS keyboard handler and the DOS timer do).

The handler runs with interrupts off, so it is never re-entered and one stack is enough. Everything a stub touches must stay in memory, because a page fault in an interrupt handler is fatal ("lock all pages", says CWSDPMI's documentation), so `register` first locks the whole image, from `__image_start` to `__image_end` (symbols `link.ld` defines), with DPMI 0600h. Then it saves the old vector (0204h), stores the new stub's selector and offset (0205h), and unmasks the line if it was masked. The number of the vector is the host's own interrupt base for the controller the line is on, from DPMI 0400h; the transcript shows the bases `0x08` and `0x70`. A first, simpler hook for IRQ 0 alone is in `rt/src/timer.rs`: eight instructions that count ticks and jump to the old handler; it is the hardware proof that the idea works. The keyboard ([M11](chapter:m-input)) is the first real user of the general stubs.

### `DosBios` and `DosDisplay`

`rt/src/hal.rs` is where the HAL's traits meet DOS, and the file's own header states the rule: everything specific to DOS lives here and nowhere else. `desk-vga` is given port I/O, memory regions and a `Bios`, and never learns about INT 10h, a transfer buffer or a DPMI mapping.

`DosBios` implements the HAL's `Bios` trait by running a real-mode interrupt with 0300h. A call with a buffer (at most `BIOS_BUFFER_MAX`, 512 bytes) copies it into a block of conventional memory allocated once (32 paragraphs), points `ES:DI` at it, makes the call and copies the block back. `read_low` reads below 1 MiB through the flat segment, which is where the VBE strings and mode list land. It does for DOS what the kernel's trampoline does natively ([M8](chapter:m-bioscalls)), with the host doing the mode switch.

`DosDisplay` implements `Display`. Its mode list is the two VGA modes it always has, 13h (320 by 200, indexed, linear) and 12h (640 by 480, planar), plus every VBE mode the BIOS reports that the Desk has a layout for. In the recorded run the transcript says `display offers 37 modes, VBE present` and the chosen mode is `0x101`, depth 8, pitch 640. A mode set is `INT 10h` for 13h and 12h, or VBE function 4F02h with the linear frame buffer bit; the frame buffer is mapped with DPMI 0800h, rounded up to whole 4 KiB pages. For a VBE mode the return to text mode and the 0801h unmap are registered on the clean-up list before the mode is set, so a failure or a panic after it cannot leave graphics on the screen; for modes 13h and 12h the registration comes just after the BIOS call succeeds. ([M12](chapter:m-graphics) has the drivers underneath.)

### The build: from Rust to `SHELL.EXE`

The pipeline below is the build of the d0 programs and of `SHELL.EXE`, which differ only in which crate is built; the sizes it shows are those recorded in `docs/D0-RUST-DOS.md`.

<!-- explainer pipeline -->

1. **The target.** `spikes/d0-rust-dos/dos-elf.json` is a freestanding ELF target: `i486-unknown-none-elf`, x87 hard float, SIMD off, `panic-strategy: abort`, `rust-lld` as the linker. [M19](chapter:m-toolchain) compares it field by field with the other two. `.cargo/config.toml` makes it the default, asks for [build-std](term:build-std) with `compiler-builtins-mem` (which supplies `memcpy`, `memmove`, `memset` and `memcmp`), and passes `-Tlink.ld -nostdlib -static` to the linker. `RUSTC_BOOTSTRAP=1` is set by the build script.
2. **The link.** `rust-lld` links the objects with `link.ld`, a [linker script](term:linker-script): `_start` as the entry, two loadable segments (`text`, flags `R E`, and `data`, flags `RW`), the image from `0x1000`, every section on a 512-byte boundary, `.bss` in the data segment, and `__image_start` and `__image_end` as symbols. Its own comment gives the reasons: the stub copies `.text` and `.data` from the file to the image's block and clears `.bss`, it places the image at offset 0 of the block (so the addresses are offsets, not linear addresses), and it reads the file in sectors.
3. **The stub.** `stubify -g`, from DJGPP, writes DJGPP's `STUB.EXE`, 2 KiB, "and does nothing else". It is data now, and neither `gcc`, `ld`, `as`, DJGPP's C library nor `crt0` is run. The stub's location in `vendor/` comes from `tools/setup.sh` ([M19](chapter:m-toolchain)).
4. **The pack.** `tools/elf2go32.py` reads the ELF's program headers, not its sections: the first `PT_LOAD` becomes `.text` (code and read-only data) and the second becomes `.data` and `.bss`. It writes a go32 COFF image: a 20-byte file header, a 28-byte a.out header and exactly three 40-byte section headers (`.text`, `.data`, `.bss`), 168 bytes in all, padded to 512; then the sections, each padded to 512, behind the stub. The stub, as found by reading its code, checks only the magic `0x014C`; takes the entry point as a plain offset; copies each section from `scnptr` to `vaddr`; zeroes `.bss`; and sizes the block as the end of `.bss` rounded up to 64 KiB, at least 128 KiB. There are no symbols, relocations or line numbers in the image. The tool refuses what it cannot pack: a file that is not a 32-bit little-endian ELF, an executable that is not `ET_EXEC`, a text segment with a `.bss` tail, an entry outside the text. With `--dpmi NAME` it patches the host's name into the stub (the last field of the information block, at offset `0x244`).
5. **The host beside it.** `CWSDPMI.EXE` is copied next to the program. If it is missing, the stub says so and exits with status 110, which `spikes/d0-rust-dos/check.sh` tests (it requires the "no DPMI" message and a non-zero status). The d0 build also packs one program onto `CWSDSTUB.EXE`, the host's own stub with the host built in, giving a single file that runs in a directory with no `CWSDPMI.EXE`: 70,144 bytes in the document's build, against 49,664 for the plain `HELLO.EXE`.

`shell/dos/build.sh` is the same pipeline for the shell: `cargo build --release --locked` in `shell/dos`, `stubify -g`, `tools/elf2go32.py` on the `shell` ELF, and the copying of `CWSDPMI.EXE`, `UI.DFN`, `TITLE.DFN` and `OFL.txt` beside it. `docs/FIRST-WINDOW.md` records `SHELL.EXE` at 102,400 bytes; a build for this chapter is in the worked example.

> **From the stacks — Where CWSDPMI came from.** The documentation shipped with release 7 of CWSDPMI is copyrighted 1995 to 2010 by Charles W Sandmann. It says the server was written to provide DPMI services for version 2 of DJGPP, and thanks DJ Delorie, who wrote the original GO32 code on which it is based, and Morten Welinder, who improved much of it. It says CWSDSTUB.EXE is a stub that includes the server, so that a program can be distributed as a single executable. It says the server does not support 16-bit DPMI applications.
> Source: [CWSDPMI r7 (csdpmi7b.zip), cwsdpmi.doc](https://www.delorie.com/pub/djgpp/current/v2misc/csdpmi7b.zip)

### Legacy: the first hosted skeleton and the DXE3 result

**This section describes the first pipeline, which is not the live one.** It is kept because the roadmap's decision to retire it is still open (`docs/D0-RUST-DOS.md`: "The retirement of the C layer and `coff2go32.py` is a later decision; this branch deletes nothing of the skeleton"), because `tools/check.sh` still builds and runs it, and because spike s1 (the DXE3 modules) stands on it. Nothing in `SHELL.EXE` uses any of it. The log chapters are `wheels` and `module`.

The skeleton (PR #3, `docs/TOOLCHAIN.md`) is Rust and C linked into one program. `core/` is a `no_std` Rust library (`core/src/lib.rs`, 125 lines on this branch) with no entry point; `c/` holds the C layer that owns `main`, the C library and the hardware (`main.c`, `out.c`, `panic.c`, 88 lines together); DJGPP's `gcc` links them with DJGPP's libraries. The pipeline is: `cargo build` for `core/dos.json` makes a static library of PE-flavoured COFF objects; `tools/coff2go32.py` (112 lines) converts them for DJGPP's linker; `i586-pc-msdosdjgpp-gcc` links `c/main.c`, `c/out.c` and `c/panic.c` with the archive into `DESK.EXE`, and `CWSDPMI.EXE` is copied beside it. The Rust heap is DJGPP's `malloc`; the panic handler reports only the source line, through C, and exits with status 3. Only integers, pointers and lengths cross the boundary. `DESK.EXE` was 236,072 bytes (115,712 stripped) in `docs/D0-RUST-DOS.md`'s build, and the check requires the exact line `DESK-SKELETON ok: squares=10 sum=385 big=385001155 sqrt=19.621417`.

The converter exists because the target borrows the MinGW triple, so rustc writes PE-flavoured COFF that DJGPP's `ld` opens and links without a single error, and the program it produces is wrong. `docs/TOOLCHAIN.md` records that each of the script's three repairs was necessary with the pinned compiler: without the patch of PC-relative addends the program page-faults; without folding the many `.text$name` sections the program runs and prints nothing; and the `compiler_builtins` objects must be dropped because their weak externals are not understood. The d0 spike's freestanding ELF target needs none of them, which is the reason the live pipeline is the one above.

The DXE3 spike (PR #4, `docs/S1-DXE3.md`) asked whether a Rust crate could be an app that the core loads and unloads at run time. DJGPP's `dxe3gen` turns a converted object into a `.DXE` file, the host `dlopen`s it and finds `_app_start`, `_app_event` and `_app_stop` with `dlsym` (the name carries COFF's leading underscore). The answer is "yes, with limits". Modules are small only with link-time optimisation: `HELLO.DXE` 7,734 bytes, `COUNTER.DXE` 22,853 and `CRASH.DXE` 8,157, against about 193 KB each without it, and without it they fail to load. A panic in a module is recovered with `setjmp` and `longjmp` and the host unloads the module; a divide error likewise. A null write was recovered only in QEMU with FreeDOS, because DOSBox aborts itself on any page fault; a stack overflow cannot be recovered at all, since CWSDPMI ends the program before a handler runs. Load, run and unload 200 times leaves memory unchanged. The step-through below replays the transcripts that CI compares.

<!-- explainer modules -->

None of this carries over to the live pipeline as it stands. A module format for the native track, which has no DJGPP, is a wish on the shelf of the `module` chapter, and `docs/D0-RUST-DOS.md` says that owning the linker "reframes `s1-dxe3`".

## Why it is this way

- **Pure Rust, with two pieces of data.** Spike d0 (`purerust`, PR #7) asked how little of DJGPP a Rust program needs, and answered: the 2 KiB stub and CWSDPMI, both used as data (`docs/D0-RUST-DOS.md`, "Verdict and recommendation"). The pivot to a native kernel had made a C layer uncomfortable, and the same drivers must run on both tracks.
- **A freestanding ELF target, not the MinGW COFF one.** The recommendation lists what disappears: no section folding, no REL32 patching, no weak-external objects to drop, no `_alloca`, and `compiler_builtins` "links as is".
- **The packer is a Python script.** Python is already needed by the repository; a Rust host tool is for "if Python leaves the build" (`docs/D0-RUST-DOS.md`, the recommendation on the packer).
- **Everything that takes a pointer goes through a buffer.** Forced by the finding above, and what DJGPP's own C library does (`docs/D0-RUST-DOS.md`, section 3).
- **The runtime has its own stack and its own flat selector.** The stub's stack is a few hundred bytes; the data selector covers only the block. A 256 KiB stack in `.bss` costs no file space (`rt/src/start.rs`).
- **Locked images, interrupt stacks and stubs in assembly.** A handler cannot trust `DS`, cannot allocate and cannot be paged out. The design is `docs/HAL.md`, "The DOS interrupt stub", built for the keyboard in PR #15 and reused for the mouse driver and `SHELL.EXE`.
- **The platform code is one file.** `hal.rs` keeps every DOS fact out of the drivers; this is the contract of [M9](chapter:m-hal).
- **Known gaps, as the documents state them.** No exception handler; no guard page below the stack; no command line over 126 characters; nothing on real hardware, other hosts or other memory managers; whether CWSDPMI restores protected-mode vectors if a client ends while the host stays resident is not shown (`docs/D0-RUST-DOS.md`, "What was and was not verified").
- **Where the document and the code differ.** `docs/D0-RUST-DOS.md` describes `rt` as "about 1,600 lines in 13 files" and its module table lists no `irq`, `hal` or `bios_kbd`; on this branch `rt/src` is 2,304 lines in 16 files. It gives the skeleton's C as about 75 lines and the converter as 91; they are 88 and 112 on this branch. The sizes it records for the programs (`HELLO.EXE` 49,664 bytes and so on) differ from a rebuild today, as the next section shows. These are the later growth of the code, not errors in the spike's measurements, and the chapter quotes the document's numbers where it names the document.

## The code

| Where | What it holds |
|---|---|
| [`spikes/d0-rust-dos/rt/src/start.rs`](repo:spikes/d0-rust-dos/rt/src/start.rs) | `_start` ([`RT_STACK`](sym:spikes/d0-rust-dos/rt/src/start.rs::RT_STACK), [`EntryRegs`](sym:spikes/d0-rust-dos/rt/src/start.rs::EntryRegs), [`StubInfo`](sym:spikes/d0-rust-dos/rt/src/start.rs::StubInfo)) and [`rt_entry`](sym:spikes/d0-rust-dos/rt/src/start.rs::rt_entry) |
| [`spikes/d0-rust-dos/rt/src/dpmi.rs`](repo:spikes/d0-rust-dos/rt/src/dpmi.rs) | [`int31`](sym:spikes/d0-rust-dos/rt/src/dpmi.rs::int31), [`init`](sym:spikes/d0-rust-dos/rt/src/dpmi.rs::init), [`alloc_block`](sym:spikes/d0-rust-dos/rt/src/dpmi.rs::alloc_block), [`simulate_int`](sym:spikes/d0-rust-dos/rt/src/dpmi.rs::simulate_int), [`lock`](sym:spikes/d0-rust-dos/rt/src/dpmi.rs::lock), the vector and descriptor calls |
| [`spikes/d0-rust-dos/rt/src/heap.rs`](repo:spikes/d0-rust-dos/rt/src/heap.rs) | The [`Heap`](sym:spikes/d0-rust-dos/rt/src/heap.rs::Heap) allocator, [`CHUNK`](sym:spikes/d0-rust-dos/rt/src/heap.rs::CHUNK), `take`, `insert` and `grow` |
| [`spikes/d0-rust-dos/rt/src/dos.rs`](repo:spikes/d0-rust-dos/rt/src/dos.rs) | INT 21h through the transfer buffer ([`TB_SIZE`](sym:spikes/d0-rust-dos/rt/src/dos.rs::TB_SIZE), [`write`](sym:spikes/d0-rust-dos/rt/src/dos.rs::write), [`read`](sym:spikes/d0-rust-dos/rt/src/dos.rs::read), [`File`](sym:spikes/d0-rust-dos/rt/src/dos.rs::File)), [`exit`](sym:spikes/d0-rust-dos/rt/src/dos.rs::exit), [`exit_now`](sym:spikes/d0-rust-dos/rt/src/dos.rs::exit_now), [`die`](sym:spikes/d0-rust-dos/rt/src/dos.rs::die) |
| [`spikes/d0-rust-dos/rt/src/panic.rs`](repo:spikes/d0-rust-dos/rt/src/panic.rs) | The panic handler: clean-up, `RT PANIC:`, status 3 |
| [`spikes/d0-rust-dos/rt/src/cleanup.rs`](repo:spikes/d0-rust-dos/rt/src/cleanup.rs) | [`register`](sym:spikes/d0-rust-dos/rt/src/cleanup.rs::register), [`forget`](sym:spikes/d0-rust-dos/rt/src/cleanup.rs::forget), [`run_all`](sym:spikes/d0-rust-dos/rt/src/cleanup.rs::run_all) |
| [`spikes/d0-rust-dos/rt/src/irq.rs`](repo:spikes/d0-rust-dos/rt/src/irq.rs) | The sixteen stubs, [`rt_irq_dispatch`](sym:spikes/d0-rust-dos/rt/src/irq.rs::rt_irq_dispatch), [`DosIrq`](sym:spikes/d0-rust-dos/rt/src/irq.rs::DosIrq) (the HAL's `IrqRegistry`) and [`Slot`](sym:spikes/d0-rust-dos/rt/src/irq.rs::Slot) |
| [`spikes/d0-rust-dos/rt/src/hal.rs`](repo:spikes/d0-rust-dos/rt/src/hal.rs) | [`DosBios`](sym:spikes/d0-rust-dos/rt/src/hal.rs::DosBios) and [`DosDisplay`](sym:spikes/d0-rust-dos/rt/src/hal.rs::DosDisplay) |
| [`spikes/d0-rust-dos/rt/src/timer.rs`](repo:spikes/d0-rust-dos/rt/src/timer.rs), [`fpu.rs`](repo:spikes/d0-rust-dos/rt/src/fpu.rs), [`env.rs`](repo:spikes/d0-rust-dos/rt/src/env.rs), [`bios_kbd.rs`](repo:spikes/d0-rust-dos/rt/src/bios_kbd.rs) | The IRQ 0 proof, the x87 maths `core` lacks, the command tail and environment, and the BIOS keyboard state |
| [`spikes/d0-rust-dos/dos-elf.json`](repo:spikes/d0-rust-dos/dos-elf.json), [`link.ld`](repo:spikes/d0-rust-dos/link.ld) | The target and the linker script |
| [`tools/elf2go32.py`](repo:tools/elf2go32.py) | [`parse_elf`](sym:tools/elf2go32.py::parse_elf) and the writer of the go32 image behind a stub |
| [`shell/dos/build.sh`](repo:shell/dos/build.sh), [`shell/dos/check.sh`](repo:shell/dos/check.sh) | The build of `SHELL.EXE` and the checks that run it in DOSBox |
| [`spikes/d0-rust-dos/build.sh`](repo:spikes/d0-rust-dos/build.sh), [`check.sh`](repo:spikes/d0-rust-dos/check.sh) | The same pipeline for the spike's programs, and their checks |
| [`core/`](repo:core/), [`c/`](repo:c/), [`tools/coff2go32.py`](repo:tools/coff2go32.py) | Legacy: the first Rust core, the C layer and the converter |

For depth: [`docs/D0-RUST-DOS.md`](doc:docs/D0-RUST-DOS.md#2-the-runtime-contract), [`docs/TOOLCHAIN.md`](doc:docs/TOOLCHAIN.md#the-pipeline) (the legacy pipeline), [`docs/S1-DXE3.md`](doc:docs/S1-DXE3.md#answers) and [`docs/FIRST-WINDOW.md`](repo:docs/FIRST-WINDOW.md).

## Worked example

**One memory request, through DPMI 0501h.** The spike's `oom` run asks for more than the machine has: `hello oom` calls `Vec::<u8>::with_capacity(900 << 20)`. Every number below comes from the code or from the run.

1. The request is 900 × 1,048,576 = 943,718,400 bytes, alignment 1. The heap's `alloc` calls `take`, which walks the free list. At that point the list holds only what earlier allocations left, which is far smaller, so `take` returns null.
2. `grow` computes what it needs: the size, plus the alignment (`max(1, 8)` = 8), plus the 8-byte header, plus `MIN_NODE` (16): 943,718,400 + 8 + 8 + 16 = 943,718,432. That is above `CHUNK` (262,144), so the request is rounded up to a multiple of 4096. 943,718,400 is 230,400 pages exactly, and the 32 extra bytes need one more: 943,722,496 bytes, `0x38401000`.
3. `dpmi::alloc_block` loads the registers the way the service wants them: `AX = 0x0501`, `BX` the high half of the size, `CX` the low half. Here `BX = 0x3840`, `CX = 0x1000`. A smaller request shows the same shape: the first allocation of any program, which wants only a few bytes, is raised to the chunk and asks for 262,144 = `0x40000`, so `BX = 0x0004` and `CX = 0x0000`.
4. The host cannot give it. The carry flag comes back set and `AX` holds the error; `call` turns that into `Err`, `grow` returns `false`, `alloc` returns null. (When it succeeds, `BX:CX` is the new block's linear address and `SI:DI` its handle; `alloc_block` stores the first as `linear` and `linear_to_ptr` subtracts the image base to make a pointer.)
5. A null from the global allocator becomes the standard library's allocation-error panic. Its message names the size that was asked for, not the rounded one: `memory allocation of 943718400 bytes failed`. The panic handler runs the clean-up list, prints `RT PANIC: ` and the location and message, and exits with status 3.

This was run on 2026-10-23 in the CI image (`spikes/d0-rust-dos/build.sh`, then `tools/dosrun.sh dist/d0 HELLO.EXE oom`). The last two lines of its output, and the status, are in the "Try it yourself" block. The location in the first line is the standard library's own source path in the toolchain, which `check.sh` normalises to `alloc.rs:L:C:` before comparing, since line numbers move.

**The pieces of `SHELL.EXE`.** `shell/dos/build.sh` was run in the same image. Its ELF, `shell/dos/target/dos-elf/release/shell`, is 127,232 bytes with two loadable segments: `.text` at `0x1000`, 0x18200 (98,816) bytes, and `.data` at `0x19200`, 0x200 (512) bytes followed by a `.bss` of 0x42400 (271,360) bytes, of which the runtime's stack is 262,144. `tools/elf2go32.py --info` reports the result:

```text
elf2go32: dist/shell/x.exe: stub 2048 + coff 99840 = 101888 bytes; text 0x1000+0x18200, data 0x19200+0x200, bss 0x19400+0x42400, entry 0x1550, block 384 KiB
```

The pieces add up. 2,048 bytes of stub; 512 of COFF headers (168 used); 98,816 of text; 512 of data; 98,816 + 512 + 512 = 99,840 for the COFF image; 101,888 in all. `.bss` is not in the file, which is why a 256 KiB stack costs nothing in it. The block the stub allocates is the end of `.bss`, `0x19400 + 0x42400 = 0x5B800` (374,784), rounded up to a multiple of 64 KiB: 384 KiB. `CWSDPMI.EXE` beside it is 21,325 bytes, the figure the document records. `docs/FIRST-WINDOW.md` records `SHELL.EXE` at 102,400 bytes, a measurement of an earlier tree; this build is 512 bytes smaller, and the reason was not looked for. The entry point, `0x1550`, is `_start`; what the linker placed in the 0x550 bytes of text before it was not examined.

## State of this layer

```status
id: rt.entry
label: _start, the stub's hand-over and rt_entry: registers recorded, stack switched, x87 reset
state: built-verified
evidence: sym:spikes/d0-rust-dos/rt/src/start.rs::_start, test:spikes/d0-rust-dos/expected/ctx.txt::stubinfo: basename, test:spikes/d0-rust-dos/check.sh::CTX.EXE
note: In headless DOSBox with CWSDPMI r7. The d0 build and the OOM and panic runs were redone for this chapter in the CI image; the ctx transcript is the committed one.

id: rt.dpmi
label: The DPMI client: flat 4 GiB data segment, memory blocks, locking, vectors, real-mode calls
state: built-verified
evidence: sym:spikes/d0-rust-dos/rt/src/dpmi.rs::int31, test:spikes/d0-rust-dos/expected/hello.txt::dpmi: 1 MiB block write/read/free ok, test:spikes/d0-rust-dos/expected/hello.txt::dpmi: lock/unlock true
note: The d0 build ran, and shell/dos/check.sh exercises this client; the hello transcript is the committed one and spikes/d0-rust-dos/check.sh was not rerun.

id: rt.heap
label: The global allocator over DPMI 0501h, first fit, 256 KiB chunks
state: built-verified
evidence: sym:spikes/d0-rust-dos/rt/src/heap.rs::Heap, test:spikes/d0-rust-dos/expected/hello.txt::alloc: churn kept 142500 bytes, test:spikes/d0-rust-dos/check.sh::Allocation failure is a panic

id: rt.dos
label: INT 21h through the 32 KiB transfer buffer: console and files
state: built-verified
evidence: sym:spikes/d0-rust-dos/rt/src/dos.rs::TB_SIZE, test:spikes/d0-rust-dos/expected/hello.txt::file: one read call: 300000 bytes, test:spikes/d0-rust-dos/expected/hello.txt::file: missing file gives Some(DosError(2))
note: The file transcript is the committed one; spikes/d0-rust-dos/check.sh was not rerun.

id: rt.panic
label: The panic handler, exit status 3 and the clean-up list
state: built-verified
evidence: sym:spikes/d0-rust-dos/rt/src/cleanup.rs::run_all, test:spikes/d0-rust-dos/expected/panicky.txt::IRQ 0 vector restored by the clean-up list: true, test:spikes/d0-rust-dos/check.sh::A panic restores the hooked vector and exits with status 3
note: The out-of-memory run in the worked example ended in the panic handler with status 3; the panicky transcript is the committed one and was not rerun.

id: rt.irq
label: The sixteen IRQ stubs, the locked image and the interrupt stack
state: built-verified
evidence: sym:spikes/d0-rust-dos/rt/src/irq.rs::rt_irq_dispatch, test:shell/dos/check.sh::Interactive: the keys typed through IRQ 1 reach the focused field
note: Keys typed by DOSBox's AUTOTYPE reach the shell through IRQ 1. The check types keys only; its header says the mouse is tried by hand.

id: rt.hal
label: DosBios and DosDisplay: mode 13h, mode 12h and VBE with a linear frame buffer
state: built-verified
evidence: sym:spikes/d0-rust-dos/rt/src/hal.rs::DosDisplay, test:tests/shell/expected-dos-8.txt::shell: frame open 8 640x480 920263aa c310363f, test:shell/dos/check.sh::No VBE: depth8 reports the fallback
note: The frame buffer and the DAC are read back and compared with the goldens. That records what the emulated card holds, not what DOSBox draws.

id: rt.build
label: dos-elf.json, rust-lld, link.ld, elf2go32.py, the stub and CWSDPMI.EXE make SHELL.EXE
state: built-verified
evidence: sym:tools/elf2go32.py::parse_elf, test:shell/dos/check.sh::All shell on DOS checks passed, test:spikes/d0-rust-dos/check.sh::Self-contained executable
note: shell/dos/check.sh was run in the CI image on 2026-10-23 and printed that line.

id: rt.cws
label: CWSDPMI r7, the DPMI host
state: external
evidence: sym:tools/fetch-tools.sh::CWSDPMI_URL

id: rt.legacy
label: The first hosted skeleton: core, the C layer and coff2go32.py
state: built-verified
evidence: sym:tools/coff2go32.py::convert_object, test:tools/check.sh::DESK-SKELETON ok
note: Rests on the committed check in tools/check.sh, which was not rerun. Marked legacy; retiring it is an open decision.

id: rt.dxe3
label: Rust modules as DXE3 files, loaded, run, crashed and unloaded
state: built-verified
evidence: repo:spikes/s1-dxe3/expected/life.txt, test:spikes/s1-dxe3/run.sh::leak, doc:docs/S1-DXE3.md#verdict-and-recommendation
note: Thirteen scenarios in DOSBox; the two faults DOSBox cannot take (null write, stack overflow) are run in QEMU by spikes/s1-dxe3/qemu-faults.sh. None of them was rerun; the explainer replays the committed transcripts.

id: rt.exceptions
label: An exception handler (DPMI 0203h) that prints a register dump
state: planned
evidence: doc:docs/D0-RUST-DOS.md#verdict-and-recommendation

id: rt.promote
label: rt promoted to a top-level crate
state: planned
evidence: doc:docs/D0-RUST-DOS.md#verdict-and-recommendation

id: rt.hardware
label: Any of this on a real machine, under another DPMI host or another memory manager
state: planned
evidence: doc:docs/D0-RUST-DOS.md#what-was-and-was-not-verified
```

The runtime is {{state.rt.entry}} for its entry and {{state.rt.dpmi}} for its DPMI client, and the build is {{state.rt.build}}, all in DOSBox. CWSDPMI is {{state.rt.cws}}. The legacy skeleton is {{state.rt.legacy}} by a committed check, and the exception handler, the promotion of `rt` and a run on real hardware are {{state.rt.hardware}}. Nothing here ran on a real machine.

## Try it yourself

These need Docker for the DOS toolchain (DJGPP, DOSBox Staging, CWSDPMI), which is not installed on this host; the commands use the CI image by the digest in `.github/workflows/ci.yml`, with the repository mounted and a home directory under `dist/` so that nothing is written outside the tree and nothing is installed on the host. The last two commands run on the host once `dist/shell/obj/stub.exe` exists.

```try
needs: Docker with the image ghcr.io/liminal-hq/ci-dos (pinned by digest in .github/workflows/ci.yml), python3 and xxd on the host
ran: 2026-10-23, Docker for the DOS steps, the host for the last three; DOSBox Staging 0.83.0 and CWSDPMI r7 inside the image
$ docker run --rm --user "$(id -u):$(id -g)" -e HOME=/work/dist/h -e CARGO_HOME=/work/dist/h/cargo -e DOS_TOOLCHAIN=/opt/dos-toolchain -v "$PWD:/work" -w /work ghcr.io/liminal-hq/ci-dos@sha256:93d129b5c910b2a2d5de38ae7235e9dc8b311d302c1c57992f060b036a74dc4e bash -c 'tools/setup.sh > /dev/null && shell/dos/build.sh 2>&1 | tail -1'
build: dist/shell/SHELL.EXE (101888 bytes)
$ docker run --rm --user "$(id -u):$(id -g)" -e HOME=/work/dist/h -e CARGO_HOME=/work/dist/h/cargo -e DOS_TOOLCHAIN=/opt/dos-toolchain -v "$PWD:/work" -w /work ghcr.io/liminal-hq/ci-dos@sha256:93d129b5c910b2a2d5de38ae7235e9dc8b311d302c1c57992f060b036a74dc4e bash -c 'shell/dos/check.sh 2>&1 | tail -1'
All shell on DOS checks passed.
$ docker run --rm --user "$(id -u):$(id -g)" -e HOME=/work/dist/h -e CARGO_HOME=/work/dist/h/cargo -e DOS_TOOLCHAIN=/opt/dos-toolchain -v "$PWD:/work" -w /work ghcr.io/liminal-hq/ci-dos@sha256:93d129b5c910b2a2d5de38ae7235e9dc8b311d302c1c57992f060b036a74dc4e bash -c 'spikes/d0-rust-dos/build.sh 2>&1 | tail -1; tools/dosrun.sh dist/d0 HELLO.EXE oom | tail -2; echo "status ${PIPESTATUS[0]}"'
build: dist/d0/HELLO.EXE (50688 bytes), dist/d0/solo/HELLO.EXE (71168 bytes, host included)
RT PANIC: panicked at /usr/local/rustup/toolchains/1.98.0-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/alloc/src/alloc.rs:573:9:
memory allocation of 943718400 bytes failed
status 3
$ python3 tools/elf2go32.py --info dist/shell/obj/stub.exe shell/dos/target/dos-elf/release/shell dist/shell/x.exe
elf2go32: dist/shell/x.exe: stub 2048 + coff 99840 = 101888 bytes; text 0x1000+0x18200, data 0x19200+0x200, bss 0x19400+0x42400, entry 0x1550, block 384 KiB
$ xxd -s 0x200 -l 16 dist/shell/obj/stub.exe
00000200: 676f 3332 7374 7562 2c20 7620 322e 3035  go32stub, v 2.05
$ python3 tools/elf2go32.py dist/shell/obj/stub.exe dist/shell/obj/stub.exe dist/shell/y.exe; echo "exit $?"
elf2go32: not a 32-bit little-endian ELF file
exit 1
```

The rebuilt sizes (50,688 and 71,168 bytes) differ from the document's (49,664 and 70,144) because `rt` has grown since the spike. The `xxd` line is the stub's information block: the bytes at offset `0x200` that `elf2go32.py --dpmi` checks before it patches the host's name. The first two commands take about a minute when the image has to download the crates `-Zbuild-std` needs; the checks need network access for that reason.

**Exercises.** (1) The heap's first request is raised to a 256 KiB chunk. Predict the `BX` and `CX` of the second call to 0501h in a program that allocates 300,000 bytes after its first small allocation (hint: the free list still holds most of the first chunk), then confirm against `rt/src/heap.rs`. (2) The transfer buffer is 32 KiB and `File::read_to_vec` is told a chunk size. Say how many real-mode calls it takes to read the 300,000-byte file in one `read`, in 4 KiB pieces and in `TB_SIZE` pieces, and compare with `hello.txt`. (3) Change `SLOTS` in `rt/src/cleanup.rs` to 1 in a scratch branch and say which of the two things that register a clean-up function would fail first in `HW.EXE`, and what the failure looks like.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: DOS extender
def: Software that lets a DOS program run in 32-bit protected mode with memory above the first megabyte, and carries its calls to DOS and the BIOS across to real mode and back. Today's ones follow the DPMI standard.

term: DPMI
def: The DOS Protected Mode Interface: the calls, made with INT 31h, by which a protected-mode program asks its host to allocate memory, make descriptors, hook interrupts and run real-mode interrupts. The Desk's host is CWSDPMI.

term: Stub
def: The small real-mode program at the front of a DJGPP executable. It finds or loads a DPMI host, switches to protected mode, loads the image behind it and jumps to its entry point. The Desk uses DJGPP's 2 KiB stub as data, written out by stubify -g.

term: Transfer buffer
def: A block of conventional memory, 32 KiB in the Desk's runtime, through which every DOS call that takes a pointer passes, because the DPMI host does not translate pointers for a protected-mode INT 21h. The data is copied in, the call runs in real mode, and the result is copied out.
```

```figures
id: m17-pipeline
reuse: link-and-pack
at: explainer pipeline

id: m17-modules
reuse: modules
at: explainer modules
```

## Revisions

- 2026-10-23: first written, against `44f0265`. The runtime was read from the source and the contract from `expected/ctx.txt`; the SHELL.EXE and spike builds, `shell/dos/check.sh` and the out-of-memory run were done in the CI image, and the packer's output on the host. Found while writing: `docs/D0-RUST-DOS.md` is out of date about the size of `rt` (about 1,600 lines in 13 files against 2,304 in 16), the line counts of the skeleton's C and converter (about 75 and 91 against 88 and 112) and the programs' sizes; the exception handler it recommends does not exist; and `rt` has not been promoted out of `spikes/`.
