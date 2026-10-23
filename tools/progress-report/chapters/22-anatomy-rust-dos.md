---
id: anatomydos
n:
kicker: Anatomy
title: Anatomy of a Rust DOS application
date: 2026-10-23
summary: One real program, SHELL.EXE, followed from its Rust source to the hardware it touches and back to the DOS prompt, with every box named after the file or document that holds it.
prs: [7, 30]
section: reference
---
Most diagrams of "how a program runs" are drawn from memory. This one is drawn from the repository: the program is `SHELL.EXE`, the first window on DOS ([pull request #30](pr:30)), and every box below is a file, a crate or a document that exists. It stands on the pure-Rust DOS runtime of spike d0 ([#7](pr:7), `docs/D0-RUST-DOS.md`). The page is finished for now; it will be revised if the runtime moves (`rt` is still under `spikes/d0-rust-dos/`, and promoting it to the top level is an open decision).

<!-- figure anatomy-dos -->

**From source to executable.** Read the diagram from the top.

1. **The program** is one file, `shell/dos/src/main.rs`. It opens a display and an input source, makes a presenter, and runs the loop itself, calling the core's `Shell::handle` and `Shell::frame`. The scenario and two `.DFN` faces are compiled in.
2. **The shared core** is the `shell/` crates and the HAL drivers, all `no_std` and host-tested: `desk-shell`, `desk-wm`, `desk-ui` with `desk-font`, `desk-gfx` and `desk-present`, plus the VGA, 8042 and INT 33h drivers from `hal/`. None of them calls DOS; they reach the machine only through the HAL's traits.
3. **The runtime** (`rt`) is what a C program would get from `crt0` and a C library, and here it is Rust: the entry point `_start`, a panic handler (exit status 3), a clean-up list, a heap over DPMI memory blocks (function 0501h), `INT 21h` wrappers, a client for the DPMI host (`INT 31h`), a stub per IRQ line, and the DOS platform's implementations of the HAL's traits (`DosBios` and `DosDisplay`). Every `INT 21h` call that takes a pointer goes through a 32 KiB transfer buffer in conventional memory, because DOS cannot see the program's protected-mode memory; larger reads and writes loop.
4. **The build** uses a custom target, `spikes/d0-rust-dos/dos-elf.json` (a freestanding 32-bit ELF for an i486, SIMD off, `panic = "abort"`), with `core` and `alloc` built from source by `-Zbuild-std` (`RUSTC_BOOTSTRAP=1`, set by the build script). `rust-lld` links the crates with `link.ld` into one static ELF. Then `tools/elf2go32.py` rearranges the ELF's loadable segments into the COFF image DJGPP's stub expects, and the stub, written out by DJGPP's `stubify -g` (about 2 KiB), goes in front. The result is `SHELL.EXE`, 102,400 bytes as `docs/FIRST-WINDOW.md` records it; `CWSDPMI.EXE` is copied beside it. No C compiler is involved at any step.

**From DOS prompt to pixels and back.**

5. DOS runs `SHELL.EXE`. The stub at its front loads the COFF image and jumps to the entry point; the DPMI host, CWSDPMI, gives the program a 32-bit flat address space and the `INT 31h` services. Neither is ours.
6. The program asks the video BIOS for a mode through DPMI real-mode calls: VBE mode `0x101` (640 × 480 × 8) on DOSBox's S3 card at depth 8, or mode 12h at depth 4. The VBE linear frame buffer is at a physical address (`0xC0000000` on that card), which `DosDisplay` maps with DPMI function 0800h, so drawing is plain memory writes. At depth 4 the program writes the VGA's planes through its ports. The palette goes to the DAC through ports.
7. Input reaches the core in two ways: a protected-mode IRQ 1 handler pushes scancodes into a lock-free ring, and the mouse is read through the DOS mouse driver's `INT 33h`. In the check, keys are typed by DOSBox's `AUTOTYPE`; the mouse is not exercised.
8. On the way out the clean-up list restores every hooked vector and puts the screen back in text mode, and the exit call (`INT 21h`, `AH=4Ch`) returns to DOS. Exit status 0 is success; 1 is a failed comparison; 4 a timeout; 5 an argument the program does not know.

> **From the stacks — What an extender and a DPMI host are.** According to Wikipedia, the DOS Protected Mode Interface is a specification introduced in 1989 that lets a DOS program run in protected mode; Microsoft developed the first version for Windows 3.0, after which an industry group took it over, publishing version 0.9 in 1990 and 1.0 in 1991. A DOS extender checks whether a DPMI host is already present and installs its own only if there is none, which is why one program can run on bare DOS or under a multitasking system that supplies its own host. CWSDPMI is described there as a separate host that supports DPMI 0.9.
> Source: [DOS Protected Mode Interface, Wikipedia](https://en.wikipedia.org/wiki/DOS_Protected_Mode_Interface)

> **From the stacks — DJGPP.** DJGPP, "DJ's GNU Programming Platform", is a port of the GNU compiler collection and other GNU tools to DOS on 80386 and later processors. Wikipedia says DJ Delorie began the project in 1989, that it is based on a variant of the COFF executable format, and that CWSDPMI is one of the DPMI hosts that give it access to up to 4 GB of memory in pure DOS. That is why `elf2go32.py` emits COFF: the stub only knows how to load that.
> Source: [DJGPP, Wikipedia](https://en.wikipedia.org/wiki/DJGPP)

## The shelves

### Catalogued
- `SHELL.EXE` was built and run in headless DOSBox Staging by `shell/dos/check.sh` and the CI, and its frames equal the goldens at depth 8 and depth 4 ({{pr30.state}} in #30, CI `{{pr30.ci}}`); the keyboard path was exercised with `AUTOTYPE`, and the fallback to depth 4 when there is no VBE was run.
- The runtime, the target, the packer and the stub came from spike d0 ({{pr7.state}} in #7): programs built that way ran under CWSDPMI in DOSBox (`docs/D0-RUST-DOS.md`).
- Every box in the diagram has a file or document behind it; the generator refuses a built box with no evidence, and checks that each repository path exists.

### Uncatalogued
- The mouse path of this program: the driver is built and was tested in spike B, but the first-window check never moves a mouse.
- Real hardware, real VGA and VBE cards, and any DPMI host other than CWSDPMI (`docs/A-VGA-VBE.md` and `docs/B-DOS-INPUT.md` say so for their parts).
- The size of `SHELL.EXE`: it is the number the first-window Result records; I did not rebuild the DOS program for this page, because the DJGPP tools were not in this checkout.

### Wish list
- Promoting `rt` to a top-level crate and retiring the C layer (an owner decision, `docs/D0-RUST-DOS.md`).
- An own stub and extender (`docs/D0-RUST-DOS.md` calls it research, not planned).
- A run on a real DOS machine.

```figures
id: anatomy-dos
kind: diagram
track: hosted
title: Anatomy of SHELL.EXE
alt: A layered diagram of SHELL.EXE from the program and the shared core crates, through the rt runtime and the build steps from target to executable, to what happens at run time under DOS and CWSDPMI and the hardware it touches, ending at DOSBox and a planned real PC; a text table of every box follows it.
caption: Source to running program, each box a real file or document.
at: figure anatomy-dos
thumb: svg
