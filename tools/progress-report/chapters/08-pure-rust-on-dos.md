---
id: purerust
n: 8
title: Pure Rust on DOS
date: 2026-10-22
summary: Spike d0 asked whether a DOS program could be Rust all the way down, with no C compiler, no libc and no start-up file. It can.
prs: [7]
---
The hosted skeleton of chapter 4 still leaned on a C layer, DJGPP's compiler and linker and its C library. The pivot made that uncomfortable: a native kernel has none of those, and the owner wanted shared machinery that a future DOS game could use without them. Spike `d0-rust-dos` (PR #7, stacked on #3) asked how little of DJGPP a Rust program really needs, and the answer is **two small pieces of data**: the 2 KiB DJGPP stub, and CWSDPMI.

The route is below. Rust objects are linked by the `rust-lld` that ships with the toolchain, and a {{m.packerLines}}-line Python script, `tools/elf2go32.py`, rearranges the linked ELF into the layout the stub expects. The sizes in the diagram are the ones the spike recorded in `docs/D0-RUST-DOS.md`, and the line counts are counted in the tree.

<!-- explainer pipeline -->

The runtime, `rt`, is {{m.rtLines}} lines in 13 files: its own `_start` (clears the direction flag, `FNINIT`, sets up a stack), a DPMI client for memory and real-mode calls, INT 21h console and file I/O, a heap so that `alloc` works, a panic handler that exits with status 3, and x87 floats. One finding shaped it more than any other: **CWSDPMI does not translate pointers for a direct protected-mode INT 21h**, so every DOS call has to go through a 32 KiB transfer buffer and DPMI function 0300h. The first `hello` printed 460 NUL bytes, and probes then found CWSDPMI's own data where the string should have been; `docs/D0-RUST-DOS.md` tells that story in full.

It is not only a hello. A hardware transcript in `spikes/d0-rust-dos/expected/hw.txt` shows the program hooking IRQ 0 through the DPMI, counting about a second of timer ticks that matched the BIOS counter, and then restoring the vector; and it switched to VGA mode 13h, wrote a pattern at `A0000h` through the flat data segment, read it back and matched the checksum, then returned to text mode. The `panicky` program shows that a panic puts the hooked vector back before it exits.

<!-- facts rtLines packerLines -->

## The shelves

### Catalogued
- Stage 1 (DJGPP's `ld` with `-nostdlib`, still needing `coff2go32.py`) and stage 2 (`rust-lld` and our packer) print an identical transcript, and every scenario passes in CI on the merge commit `{{pr7.merge}}`, in headless DOSBox Staging.
- The IRQ 0 hook, mode 13h write and read-back, a 300 KB file round trip and a 1 MB `Vec` are transcripts in `spikes/d0-rust-dos/expected/`, compared with each run.
- Negative checks (PR #7): a wrong transcript fails with a clear diff; a panic restores the IRQ 0 vector and exits with status 3; without `CWSDPMI.EXE` the stub says "no DPMI" and exits with status 110; an allocation failure is a panic.
- Sizes (recorded in `docs/D0-RUST-DOS.md`): `HELLO.EXE` is {{size.HELLO_EXE}} bytes against {{size.DESK_EXE}} for the skeleton's `DESK.EXE`, and packing on `CWSDSTUB.EXE` gives one {{size.solo_HELLO_EXE}}-byte file with the host inside.

### Uncatalogued
- Real hardware, other DPMI hosts, EMM386 or Windows: only CWSDPMI under DOSBox's own DOS was tried.
- The picture on a screen: no screenshot was taken, and mode 13h was verified by reading memory back.
- Sound, which cannot be checked headless.
- Whether CWSDPMI restores protected-mode vectors when a client ends while the host stays resident, and what a CPU exception does: it ends with status 1 and nothing the harness can see.
- The build time (about 7 s cold) and the sizes were measured by the spike's author, and the generator reads them back from the document; it did not rebuild them.

### Wish list
- A freestanding ELF target (`dos-elf.json`), and `rt` promoted to a top-level crate with an exception handler.
- Retiring the C layer and `coff2go32.py`: the recommendation, and still an owner decision.
- Rust drivers for VBE, the mouse, OPL and the Sound Blaster, shared with the native kernel; an own stub and extender (research, not planned); real hardware.
- Lieutenant Fizz Episode 2 as the first outside customer of the machinery.

> **From the stacks — Mode 13h.** BIOS video mode 13h is 320 by 200 pixels in 256 colours, introduced with the VGA on IBM's PS/2 line in 1987. Its frame buffer is a plain run of 64,000 bytes, one per pixel, at segment `0xA000` offset 0, and the palette lives in the DAC rather than in memory. That simplicity is why so many DOS games drew straight into it, and why d0 could check the whole mode with a checksum of memory.
> Source: [Mode 13h, Wikipedia](https://en.wikipedia.org/wiki/Mode_13h)

```figures
id: link-and-pack
kind: explainer
track: hosted
title: From Rust object to DOS program
alt: A step-through of the pipeline that links Rust objects with rust-lld and packs the result into a DOS executable with the DJGPP stub, with the sizes at each stage.
caption: The link-and-pack pipeline, with the sizes the spike recorded.
at: explainer pipeline
thumb: steps
```
