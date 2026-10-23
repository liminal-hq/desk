---
id: findings
n:
kicker: Findings
title: Findings
date: 2026-10-22
summary: What the spikes and skeletons actually found, in one table, each with where it was written down.
prs: [3, 4, 6, 7]
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
| Fonts, PR #1 | Do the faces render everywhere? | Not known: Chromium treats `Desk Fixed DOS`'s `.ttf` as a symbol font, and Windows and macOS were not tried. | [`docs/FONTS.md`](repo:docs/FONTS.md) |

## The shelves

### Catalogued
- Each row is a result that one of the pull requests' test plans or a committed transcript supports, as named in its last column.

### Uncatalogued
- Every row: none was repeated on real hardware, and the generator checks that the cited files exist, not that the prose of this table matches them word for word.

### Wish list
- The next spikes the documents propose: Rust hardware drivers, an own stub and extender, loadable modules on the native track, VBE and the mouse, TrueType reading, and a real-hardware run.
