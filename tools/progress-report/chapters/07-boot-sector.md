---
id: boot
n: 7
title: Boot sector
date: 2026-10-22
summary: The native skeleton: 512 bytes of NASM that load a loader that load a Rust kernel, which prints its first lines on a serial port in QEMU.
prs: [6]
---
This is the chapter the pivot made possible, and the one I like best. PR #6 built a floppy image whose boot sector, written in NASM, reads a loader from the disk; the loader opens the A20 gate, reads the BIOS memory map, switches the processor to 32-bit protected mode and jumps into a Rust kernel. The kernel sets up its own tables, parses the memory map, allocates frames and a heap, counts timer ticks and does floating-point maths, and then prints `boot complete` and leaves QEMU through a debug port. There is no DOS underneath, nothing is borrowed from DJGPP, and the check compares the serial transcript with `tests/native/expected.txt`.

Step through the boot below. The transcript on the right is a real run: After the last merge I rebuilt the image from `main` with `tools/native-build.sh` and `tools/native-image.sh`, and the resulting image has the same SHA-256 (`8e5e6e19…0627`) as the one captured earlier in the day, and `tools/native-run.sh` printed exactly the same transcript on QEMU 11.1.2. The memory map fills in from the six `map` lines in it.

<!-- explainer boot -->

When the run ends, the VGA text screen shows the same lines. This is the screen as QEMU's monitor captured it.

<!-- figure qemu -->

The kernel itself is small and honest about it: {{m.kernelLines}} lines of Rust in `kernel/src/`, {{m.bootLines}} lines of boot assembly, and a flat binary of {{kernelBytes}} bytes (the kernel image, from the build output recorded in `snapshots/measured.json`). It has its own target file, `kernel/i486-desk-none.json`, which limits code generation to the 486 instruction set and builds with `-Zbuild-std`, and a {{loc.elf2binPy}}-line Python script, `tools/elf2bin.py`, stands in for `objcopy` so that the build needs no binutils.

<!-- facts kernelLines bootLines -->

## The shelves

### Catalogued
- The transcript matches `tests/native/expected.txt` in QEMU as a 486 (`-cpu 486 -m 16`), in CI on the merge commit `{{pr6.merge}}`, and again after the last merge on a fresh build of `main` (same image hash, identical transcript).
- Negative checks (PR #6): a wrong expected line fails with a clear message; a divide error, a bad segment load, `ud2`, a panic and a masked timer each exit non-zero with the expected dump; a hung kernel ends at the 5-second watchdog with status 124.
- Loader variants that force the fast-gate A20, the keyboard-controller A20 and the E801 memory fallback all boot.
- The six E820 entries and the arithmetic of the usable-RAM line are checked by the generator: 639 KiB plus 15,232 KiB is the 15,871 KiB the kernel prints.

### Uncatalogued
- Real hardware: not one real PC. `-cpu 486` makes QEMU reject instructions a 486 lacks, but it is an approximation of the chip and not a model of its quirks.
- 86Box and Bochs, IDE or hard-disk boot, and any BIOS other than QEMU's SeaBIOS.
- The wall-clock cost of the `apt-get` step that installs QEMU and NASM in CI: CI passed, but its time was never measured by me.
- Whether DOSBox could boot this image: not tried (see the note below).

### Wish list
- A PS/2 keyboard and mouse driver on IRQ 1 and IRQ 12; it needs no decision and is the next native step.
- Graphics: VBE before the switch, V86 calls or native drivers, an open question.
- Paging, scheduling, a floppy and IDE driver, a FAT filesystem, a module loader and the shared core on the native HAL.

> **From the stacks — Sector zero.** The BIOS reads the first 512-byte sector of the boot disk into memory at address `0x7C00` and will only start it if the last two bytes of the sector, at offsets `0x1FE` and `0x1FF`, are `0x55 0xAA`. The Desk's `boot/stage1.asm` is exactly one such sector, which is why stage 2 and the kernel need a loader of their own: a boot sector has no room for anything more.
> Source: [Boot sector, Wikipedia](https://en.wikipedia.org/wiki/Boot_sector)

> **From the stacks — The A20 gate.** The 8086 had 20 address lines, so an address past 1 MiB wrapped round to low memory. The 80286 in the IBM PC AT did not wrap, so IBM put a gate on the A20 line to restore the old behaviour for programs that relied on it, and the gate was driven from the keyboard controller. A loader that wants memory above 1 MiB has to open it first, which is why stage 2 tries the fast port `0x92` first and the keyboard controller second.
> Source: [A20 line, Wikipedia](https://en.wikipedia.org/wiki/A20_line)

> **From the stacks — Why not just boot it in DOSBox?** `docs/DIRECTION.md` says DOSBox and js-dos "emulate DOS, not PC hardware", so the native track is tested in QEMU instead. DOSBox's own wiki describes a `BOOT` command that starts floppy and hard-disk images outside its built-in DOS, so I will not claim that DOSBox cannot boot a custom kernel: nobody has tried it here. QEMU is chosen because it models a PC and has a `-cpu 486` setting, not because DOSBox was shown to fail.
> Source: [DOSBox wiki, BOOT](https://www.dosbox.com/wiki/BOOT) (as quoted by search results; the page itself refused an automated fetch)

```figures
id: boot-stepper
kind: explainer
track: native
title: The boot sequence and the memory map
alt: A step-through of the native boot: the BIOS loads the boot sector, the loader gets the memory map and switches to protected mode, and the kernel prints its lines, with the memory map filling in.
caption: From power on to boot complete, with the memory-map fill, from a recorded QEMU run.
at: explainer boot
thumb: steps

id: qemu-boot
kind: screenshot
track: native
title: The boot screen in QEMU
alt: A black 720 by 400 text screen showing the loader and kernel lines printed at the end of a headless QEMU boot.
caption: The VGA text screen after the native kernel booted in QEMU.
at: figure qemu
thumb: img:img/qemu-boot.png
```
