---
id: findings
n:
kicker: Findings
title: Findings
date: 2026-10-23
summary: What the spikes and skeletons actually found, in one table, each with where it was written down. The first rows are merged work; the later ones come from open pull requests and say so.
prs: [3, 4, 6, 7, 8, 10, 11, 12, 13, 14, 15, 16, 17]
section: reference
---
A spike ends with a written result, and this is the index to them. Every row says what was found and where the finding lives, and each came from a run in an emulator: none has been tried on a real machine. If a row disagrees with the document it points to, the document wins and the generator's reader should say so.

| Source | Question | Finding | Evidence |
|---|---|---|---|
| Skeleton, PR #3 | Is the object converter still needed with Rust 1.98.0? | Yes. Without the REL32 addend patch the program crashes, and without section folding it prints nothing. | [`docs/TOOLCHAIN.md`](repo:docs/TOOLCHAIN.md) |
| Skeleton, PR #3 | Does a Rust panic surface to the host? | Yes: it reports through C and the program exits with status 3, which survives DOSBox. | `tools/check.sh` |
| s1-dxe3, PR #4 | Can Rust be a DXE3 module? | Yes, with one extra `dxe3gen` step; `coff2go32.py` works unchanged. Modules are 7.7 to 22.9 KB. | [`docs/S1-DXE3.md`](repo:docs/S1-DXE3.md) |
| s1-dxe3, PR #4 | Is link-time optimisation optional? | No. Without it a module is about 193 KB and fails to load (`unresolved symbol: ___register_frame_info`); the pull request says the cause was not found. | [`docs/S1-DXE3.md`](repo:docs/S1-DXE3.md) |
| s1-dxe3, PR #4 | Can a crash be contained? | A Rust panic and a divide error are recovered with `setjmp` and `longjmp`, and other modules keep working. | `spikes/s1-dxe3/expected/panic.txt` |
| s1-dxe3, PR #4 | What cannot be contained? | A stack overflow: CWSDPMI aborts the whole program. A null write was recovered only under QEMU with FreeDOS, by hand; DOSBox Staging aborts on any page fault. | [`docs/S1-DXE3.md`](repo:docs/S1-DXE3.md) |
| s1-dxe3, PR #4 | Does load and unload leak? | Not in 200 rounds, with or without a panic each round. A deliberate 4 KB leak is detected. | `spikes/s1-dxe3/expected/leak*.txt` |
| d0, PR #7 | Can a DOS program be pure Rust? | Yes: `rust-lld` plus a 139-line packer plus the 2 KiB stub, with no gcc, ld, libc or crt0. | [`docs/D0-RUST-DOS.md`](repo:docs/D0-RUST-DOS.md) |
| d0, PR #7 | How do DOS calls work from protected mode? | CWSDPMI does not translate pointers for a direct INT 21h, so every call goes through a 32 KiB transfer buffer and DPMI function 0300h. | [`docs/D0-RUST-DOS.md`](repo:docs/D0-RUST-DOS.md) |
| d0, PR #7 | Does hardware work from Rust? | An IRQ 0 hook counted ticks that matched the BIOS counter, and mode 13h wrote and read back a matching checksum, in headless DOSBox. | `spikes/d0-rust-dos/expected/hw.txt` |
| d0, PR #7 | What are the traps? | DOSBox's `cputype = 486` rejects `CMPXCHG`; a COFF section with a long name (`.text.start`) silently breaks the entry; returning from `_start` aborts DOSBox, so `exit` is required. | [`docs/D0-RUST-DOS.md`](repo:docs/D0-RUST-DOS.md) |
| d0, PR #7 | What does it cost? | `HELLO.EXE` is {{size.HELLO_EXE}} bytes against {{size.DESK_EXE}} for `DESK.EXE`; a cold build takes about 7 s. | [`docs/D0-RUST-DOS.md`](repo:docs/D0-RUST-DOS.md) |
| Native, PR #6 | Does a NASM and Rust floppy boot as a 486? | Yes in QEMU, with E820, A20, a GDT, an IDT, the PIC, the PIT, a heap and the x87 all demonstrated; three loader variants boot. | [`docs/NATIVE-BOOT.md`](repo:docs/NATIVE-BOOT.md) |
| Native, PR #6 | Is `-cpu 486` a faithful 486? | No: it rejects instructions a 486 lacks, but it is not cycle-accurate and models few of the chip's quirks. | [`docs/NATIVE-BOOT.md`](repo:docs/NATIVE-BOOT.md) |
| HAL, PR #8 (open) | Can one driver be written once and tested without an emulator? | Yes, for the serial port, the timer and the interrupt controller, against a mock port bus: {{pr8.hostTests}} host tests. Nothing has run in the kernel or under DOS in this pull request itself. | [`docs/HAL.md`](pr:8) |
| HAL, PR #8 (open) | Can the atomics be portable to a 486 under DOSBox? | Only if compare-and-swap is avoided: DOSBox's 486 mode rejects `CMPXCHG`, and 64-bit atomics need a Pentium. Loads, stores, `xchg` and `lock xadd` suffice for a ring and a tick counter. | [`docs/HAL.md`](pr:8) |
| Input, PR #10 (open) | Does the native kernel take real keyboard and mouse input? | In QEMU, yes: injected keys and mouse motion decode to 26 key events and 8 mouse events matching the expected transcript, with nothing dropped. | `tests/native/expected-input.txt` ([PR #10](pr:10)) |
| Input, PR #10 (open) | Does QEMU behave like a real controller? | Not known. Without short pauses QEMU folded mouse events; USB legacy emulation, touchpads and real timings were not tried. | [`docs/C-INPUT.md`](pr:10) |
| VGA and VBE on DOS, PR #11 (open) | Can Rust program the VGA by registers and use VBE with a linear frame buffer under CWSDPMI? | Yes in DOSBox Staging: modes 13h and 12h by registers and by `INT 10h`, and VBE 640 by 480 by 8 with the frame buffer mapped by DPMI 0800h, each read back with a matching checksum. | [`docs/A-VGA-VBE.md`](pr:11) |
| Native graphics, PR #14 (open) | Can the kernel draw with no BIOS after the loader? | Yes in QEMU: the loader sets VBE before the switch, the kernel draws a window with a cursor, and the checksums equal the DOS run's. | [`docs/D-NATIVE-VGA.md`](pr:14) |
| Native graphics, PR #14 (open) | Can the kernel leave a VBE mode without the BIOS? | Only on some cards: one register write on QEMU's standard card, and a garbled screen on `-vga cirrus`. VBE is a one-way trip without a BIOS path. | [`docs/D-NATIVE-VGA.md`](pr:14) |
| `std`, PR #12 (open) | Does `std` build for an invented OS? | No: 21 errors from four missing pieces. A patch of 55 lines in 12 files plus about 340 new lines makes it work; a hello grows by 9.7 KB (19,456 against 9,728 bytes). | [`docs/E-STD-STUBS.md`](pr:12) |
| js-dos, PR #13 (open) | Does the VBE linear frame buffer work in a browser? | Yes in js-dos 8.5.3 in headless Chromium: 640 by 480 by 8 with the buffer, zero of 307,200 pixels differ; every repository DOS program runs unchanged. | [`docs/S5-JSDOS.md`](pr:13) |
| DOS input, PR #15 (open) | Can the DOS runtime hand the core the same events as the native kernel? | Yes: the same decoder and ring for the keyboard, and the mouse by polling `INT 33h`; a quick click between two polls is lost. | [`docs/B-DOS-INPUT.md`](pr:15) |
| Emulators, PR #16 (open) | Does the native image boot in other emulators and BIOSes? | Yes: Bochs 2.7 as a 486 with three BIOSes, and 86Box 6.0 on a 1993 AMIBIOS board, the latter only after a loader fix. | [`docs/NATIVE-EMULATORS.md`](pr:16) |
| Emulators, PR #16 (open) | What does a BIOS from 1993 not have? | Neither INT 15h E820 nor E801, so the loader gained an AH=88h fallback and a test variant for it; it also leaves A20 closed. | [`docs/NATIVE-EMULATORS.md`](pr:16) |
| Emulators, PR #16 (open) | Is QEMU's `-cpu 486` a faithful 486? | No: it runs `rdtsc`, `rdmsr`, MMX and reads of `CR4`, which Bochs's 486 rejects. | [`docs/NATIVE-EMULATORS.md`](pr:16) |
| BIOS path, PR #17 (open) | Can the protected-mode kernel use the BIOS again? | Yes, in emulators, through a real-mode trampoline: VBE set and left on three QEMU cards, floppy and LBA reads, the memory map and the clock; tens of microseconds a call, emulated. | [`docs/N3-BIOS-PATH.md`](pr:17) |
| BIOS path, PR #17 (open) | Does that settle leaving VBE? | In emulators, yes: `INT 10h` function 0003h restores text on the Cirrus card that spike D could not. A real card was not tried. | [`docs/N3-BIOS-PATH.md`](pr:17) |
| Fonts, PR #1 | Do the faces render everywhere? | Not known: Chromium treats `Desk Fixed DOS`'s `.ttf` as a symbol font, and Windows and macOS were not tried. | [`docs/FONTS.md`](repo:docs/FONTS.md) |

## The shelves

### Catalogued
- Each row is a result that one of the pull requests' test plans or a committed transcript supports, as named in its last column.

### Uncatalogued
- Every row: none was repeated on real hardware, and the rows from open pull requests are not merged, and the generator checks that the cited files exist, not that the prose of this table matches them word for word.

### Wish list
- The next spikes: Bochs and 86Box (spike G), a real BIOS path for the native kernel (n3), an own stub and extender, loadable modules on the native track, TrueType reading, the shared core, and a real-hardware run.
