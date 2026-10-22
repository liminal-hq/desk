# D0: Rust-only DOS programs

Spike `d0-rust-dos`: can a DOS program be pure Rust — no `gcc`, no C layer, no DJGPP C library — linked and packed by tooling we control, and run headless under CWSDPMI? This is the proving step for "Rust for DOS" as shared machinery (the Desk, and later DOS games such as Lieutenant Fizz Episode 2). The code is in [`../spikes/d0-rust-dos/`](../spikes/d0-rust-dos/) and the packer in [`../tools/elf2go32.py`](../tools/elf2go32.py). Everything below was run under headless DOSBox Staging 0.83.0 with CWSDPMI r7 through [`../tools/dosrun.sh`](../tools/dosrun.sh), on this machine and again from a clean clone in the `ci-dos` image.

## Contents

- [Verdict and recommendation](#verdict-and-recommendation)
- [Method and the commands that work](#method-and-the-commands-that-work)
- [What failed on the way](#what-failed-on-the-way)
- [1. No C, no gcc, no libc](#1-no-c-no-gcc-no-libc)
- [2. The runtime contract](#2-the-runtime-contract)
- [3. The runtime crate](#3-the-runtime-crate)
- [4. Hardware proof](#4-hardware-proof)
- [5. Costs and facts](#5-costs-and-facts)
- [6. Negative and failure cases](#6-negative-and-failure-cases)
- [Independence from DJGPP](#independence-from-djgpp)
- [What was and was not verified](#what-was-and-was-not-verified)

## Verdict and recommendation

**Verdict: yes.** A Rust `no_std` program with its own `_start`, a hand-written DPMI client and heap, and no C anywhere builds with `-Zbuild-std`, links with `rust-lld`, is packed into a DJGPP stub image by a 139-line Python tool and runs under CWSDPMI: output, arguments, the environment, `Vec`/`String`/`format!`, x87 float formatting, file I/O larger than any transfer buffer, a protected-mode IRQ 0 hook, VGA mode 13h, panics with status 3 and clean-up of hooked vectors. Both stages work: stage 2 (our own packer, `rust-lld` as the only linker) with the least, and stage 1 (DJGPP `ld` with `-nostdlib`, no `gcc`, no crt0, no libc), which still needs [`../tools/coff2go32.py`](../tools/coff2go32.py). The limits are about the host, not Rust: CWSDPMI does not translate pointers for a direct protected-mode INT 21h, and DOSBox's `cputype = 486` rejects `CMPXCHG`; both are handled in `rt` and documented below.

**Recommendation.** Make this the shape of Rust for DOS and retire the C layer when the Desk's hardware code has moved over:

- **Target:** a freestanding ELF target ([`dos-elf.json`](../spikes/d0-rust-dos/dos-elf.json), `i486-unknown-none-elf`, x87 hard float, SIMD off) linked by `rust-lld` with [`link.ld`](../spikes/d0-rust-dos/link.ld). It is simpler than the skeleton's MinGW-flavoured COFF target in every way that mattered: no section folding, no REL32 patching, no weak-external objects to drop, no `_alloca`, and `compiler_builtins` links as is.
- **Runtime:** promote [`rt`](../spikes/d0-rust-dos/rt/) to a top-level crate (`rt/`) that every DOS program and the Desk depend on. Keep one module per concern; add an exception handler (DPMI 0203h) that prints a register dump to standard output, so a crash is visible to the harness instead of a bare exit status 1.
- **Packer:** keep `tools/elf2go32.py` (it is small and has no dependencies beyond Python, which the repository already needs); port it to a Rust host tool only if Python leaves the build.
- **What stays on DJGPP:** the 2 KiB stub (written out by `stubify -g`, or vendored; or `CWSDSTUB.EXE` for single-file programs) and CWSDPMI. What leaves: `gcc`, `ld`, `as`, `libc`, `libgcc`, `crt0`, the `c/` layer and `coff2go32.py`, once nothing needs C. DJGPP's C library remains an option, via the skeleton's path, for linking third-party C code.
- **Next spikes:** (1) a Rust driver crate set — keyboard (IRQ 1), mouse (INT 33h with a real-mode callback), VGA/VBE linear frame buffer (DPMI 0800h), PIT, OPL and Sound Blaster DMA; (2) an own-stub and own-extender research spike (the only remaining dependencies that are not ours); (3) loadable modules, now that we own the linker (this reframes `s1-dxe3`); (4) a run on real hardware and under EMM386, QEMM and a Windows 9x DOS box.

The retirement of the C layer and `coff2go32.py` is a later decision; this branch deletes nothing of the skeleton.

## Method and the commands that work

```bash
tools/setup.sh                       # once
spikes/d0-rust-dos/build.sh          # stage 2: rust-lld + tools/elf2go32.py -> dist/d0/*.EXE
spikes/d0-rust-dos/build-stage1.sh   # stage 1: DJGPP ld -nostdlib         -> dist/d0/HELLO1.EXE
tools/dosrun.sh dist/d0 HELLO.EXE    # run it headless
spikes/d0-rust-dos/check.sh          # everything, with transcripts (also the last step of tools/check.sh)
```

Stage 2 is, step by step (the script is the reference):

```bash
# in spikes/d0-rust-dos: -Zbuild-std for dos-elf.json, rust-lld links with link.ld (config in .cargo/config.toml)
RUSTC_BOOTSTRAP=1 cargo build --release --locked
vendor/djgpp/djgpp/i586-pc-msdosdjgpp/bin/stubify -g dist/d0/obj/stub.exe     # writes the 2 KiB stub, nothing else
tools/elf2go32.py dist/d0/obj/stub.exe spikes/d0-rust-dos/target/dos-elf/release/hello dist/d0/HELLO.EXE
```

Stage 1 builds the same `hello` as a static library for [`../core/dos.json`](../core/dos.json) (the skeleton's target, unchanged), converts it and links:

```bash
RUSTC_BOOTSTRAP=1 cargo build --release --locked -p stage1 --target ../../core/dos.json
tools/coff2go32.py spikes/d0-rust-dos/target/dos/release/libstage1.a dist/d0/s1/stage1_go32.a i586-pc-msdosdjgpp-ar
i586-pc-msdosdjgpp-ld -nostdlib -static -e _start --defsym ___image_start=0x1000 --defsym ___image_end=_end \
  -o dist/d0/s1/hello1.exe dist/d0/s1/stage1_go32.a -L vendor/djgpp/djgpp/lib/gcc/i586-pc-msdosdjgpp/12.2.0 -lgcc
```

The program transcript (`expected/hello.txt`) is the same byte for byte from stage 1, stage 2 and the self-contained build:

```
D0-HELLO ok
args: 0 []
comspec: true
path: true
console: AH=09h ok
alloc: squares=10 sum=385 big=385001155
alloc: 1 MB vec of 1000000 bytes, byte 999999 = 63
alloc: churn kept 142500 bytes
float: sqrt(385)=19.621417 sin(1)=0.841471 cos(1)=0.540302 atan2(1,1)*4=3.141593
float: 0.3333333333333333 6.02214076e23 0.3
float: control word 0x037f
dpmi: version 0.90
dpmi: 1 MiB block write/read/free ok
dpmi: real-mode vector 0201h/0200h round trip true
dpmi: protected-mode vector 0205h/0204h round trip true
dpmi: lock/unlock true
dpmi: simulated real-mode INT 10h reports mode 0x03, as the direct call does: true
dpmi: direct INT 21h AH=40h with a pointer: DOS reports 4 bytes, file holds the buffer: false
file: wrote 300000 bytes, length 300000
file: one read call: 300000 bytes, match true
file: 4 KiB reads: 300000 bytes, match true
file: missing file gives Some(DosError(2))
```

## What failed on the way

In the order met, with the cause of each.

- **Output of zeros, then of CWSDPMI's own strings.** The first `hello` ran, panicked and printed 460 NUL bytes. A probe that wrote 64 bytes from `DS:0x1000` got CWSDPMI's interrupt gates; one that wrote from `DS:0x200` got CWSDPMI's own error strings, and a DJGPP-built control program doing the same direct `int 0x21` printed zeros. The host reflects a protected-mode INT 21h to real mode with the registers untouched, so `DX` is an offset in whatever real-mode `DS` happens to be, which is the host's. **CWSDPMI r7 does not translate buffer pointers for a direct protected-mode INT 21h.** Every pointer-taking DOS call in `rt` now goes through a 32 KiB conventional-memory transfer buffer and the DPMI real-mode call (0300h), as DJGPP's C library does. The `hello` transcript keeps the evidence: `direct INT 21h AH=40h with a pointer: ... file holds the buffer: false`.
- **A crash with exit status 1 and no message.** A CPU exception inside the program ends it with status 1 and nothing on standard output (CWSDPMI presumably reports it on the console, which the harness does not capture; we could not see it). The first time this showed was `CMPXCHG`. **DOSBox `cputype = 486` raises an exception on `CMPXCHG`** (found by running the same image under `cputype = auto` and `pentium`, where it works; `XADD` and `BSWAP` are fine under `486`). A real 486 from the B step on has it. Rust's `compare_exchange` emits it for the i486 target, so `rt` uses plain loads and stores (the program is single-threaded). The skeleton's check, which deliberately uses `cputype = 486`, therefore also guards against accidental atomics.
- **`I _start` and no entry.** In stage 1, `ld` reported no error and DOSBox aborted. The cause was a section named `.text.start` (11 characters): PE-flavoured COFF puts long section names in a string table, which coff-go32 does not read, so the symbol came out as an indirect symbol at address 0. Entry code now goes in plain `.text`. Also, DJGPP's `ld` takes the entry symbol exactly as the object spells it: `-e _start`, not `-e start` (which is how DJGPP's own `crt0.o` names its entry).
- **The environment was empty.** The word at `PSP:2Ch` is a protected-mode selector, not a real-mode segment: the host rewrites it on entry to protected mode. Read as a segment it pointed at a DOS device header. `rt::env` asks the host for the selector's base and limit.
- **Stage 1 without `coff2go32.py`.** Removing the `compiler_builtins` object (whose weak externals `ld` mis-reads, with a flood of "local symbol has no section" warnings) and linking the PE-flavoured objects as they come produces an `.exe` with no linker error, and DOSBox aborts with a page fault on it. The converter's work (REL32 addends, folding `.text$name` and the rest) is still needed in stage 1, as [`TOOLCHAIN.md`](TOOLCHAIN.md) found.
- **A program that returns from `_start`.** See [6](#6-negative-and-failure-cases).

## 1. No C, no gcc, no libc

**Stage 2 works and needs the least.** The tools actually run are `cargo` and `rustc` with `rust-lld` (shipped with the toolchain; `-Zbuild-std=core,alloc,compiler_builtins` with the `compiler-builtins-mem` feature, via `RUSTC_BOOTSTRAP=1` as the skeleton does), `python3` for the packer, and the 2 KiB stub as data. `gcc` is gone, `ld` is gone, `as` is gone, `libc`, `libgcc` and `crt0` are gone, and so is `coff2go32.py`. `stubify` is used only as `stubify -g`, which writes DJGPP's stub file and does nothing else; `CWSDSTUB.EXE` from the CWSDPMI package is a drop-in alternative and also embeds the host (see [5](#5-costs-and-facts)). The entry is `_start` in `global_asm!`; `#[unsafe(naked)]` with `naked_asm!` also works on the pinned Rust 1.98.0 (checked with a throwaway program that exits with status 5, which it did), but the entry wants to be a plain label with no prologue and a lot of instructions that must not be reordered, so `global_asm!` is clearer.

**Stage 1 works too, with more.** DJGPP's `ld` with `-nostdlib -static` and its default linker script writes a complete `.EXE` (its `coff-go32-exe` output already contains the stub, so `stubify` is not needed either). Needed besides `ld`: `tools/coff2go32.py` (necessary, shown above), the `memcpy` family from `rt/src/mem.rs` (`compiler_builtins` is dropped by the converter), and `libgcc.a` for the single helper `__udivdi3`. `gcc` is not run and no C library is linked.

What the stub needs of the image, found by disassembling it (it is in `stubify` and can be written out with `-g`) and confirmed by running: the 168 bytes after the COFF start hold exactly a file header, a 28-byte a.out header and three section headers (`.text`, `.data`, `.bss`); only the magic `0x014C` is checked; the entry point is a plain offset; each section is copied from `scnptr` (keep it a multiple of 512) to `vaddr` in 512-byte reads; `.bss` is zeroed by the stub from its `vaddr`; the block is the end of `.bss` rounded up to 64 KiB, at least 128 KiB. There are no symbols, relocations or line numbers in the images [`elf2go32.py`](../tools/elf2go32.py) writes.

## 2. The runtime contract

What the stub (`STUB.EXE`, then CWSDPMI) hands over at the first instruction, as measured by [`ctx`](../spikes/d0-rust-dos/progs/src/ctx.rs) (the transcript is `expected/ctx.txt`) and read in the stub's code:

| Item | At entry |
|---|---|
| `CS` | A 32-bit code selector. Base = the start of the image's memory block (linear `0x400000` in the tests), limit = block size − 1 (`0x4ffff` for a 320 KiB block). The entry is `CS:entry` where `entry` is the COFF header's entry offset |
| `DS`, `ES` | One 32-bit data selector for the same block (base equal to `CS`), limit = block size − 1 at first. Offsets are relative to the block, so an image linked at `0x1000` has its first byte at block offset `0x1000` |
| `FS` | A selector for the stub's own data segment (base = real-mode segment `ds_segment` × 16, limit `0xffff`). `FS:0` is DJGPP's `_GO32_StubInfo` (`stubinfo.h`, 0x54 bytes): magic `go32stub, v 2.05`, memory handle, block size, `psp_selector`, `env_size`, `dpmi_server`. `_start` copies it out |
| `SS:ESP` | The stub's own stack: `SS` is the same selector as `FS`, `ESP` about `0x760`. Not usable by a program, so `_start` switches to a stack of its own in `.bss` |
| `GS` | 0 |
| `EFLAGS` | `0x3212` in the tests: IF set, direction flag clear. Do not rely on `DF`; `_start` runs `CLD` |
| general registers | Leftovers of the stub's work. Not a contract |
| x87 | Control word `0x037F`, status 0 under DOSBox. DOS does not promise it, so `_start` runs `FNINIT` |
| memory | `.text` and `.data` loaded, `.bss` zeroed by the stub, the rest of the block as the host gave it |
| PSP | `StubInfo.psp_selector` is a selector with base = the PSP's linear address. `PSP:80h` holds the command tail (length byte, then up to 126 characters). `PSP:2Ch` is a selector (not a segment) for the environment: strings `NAME=value\0`, an empty string, a word `1`, then the full path of the program |
| exit | `INT 21h AH=4Ch`, status in `AL`. From protected mode it ends the process and the host releases its memory and descriptors |

What `_start` has to do, and does ([`rt/src/start.rs`](../spikes/d0-rust-dos/rt/src/start.rs)): snapshot the registers for inspection, clear the direction flag, copy the stub information, load `SS` from `DS` and set `ESP` to the top of a 256 KiB `.bss` stack, `FNINIT`, then call Rust. `rt_entry` brings up the DPMI client (it lifts the data segment limit to 4 GiB, see below) and calls the program's `main`, then `exit`. A stack larger than the default costs nothing in the file; there is no guard page below it, so an overflow corrupts `.bss`, not a clean fault.

## 3. The runtime crate

[`rt`](../spikes/d0-rust-dos/rt/) is about 1,600 lines in 13 files with the reasons commented in each. Programs declare `rt::entry!(main)` with `fn main() -> i32`.

| Module | What it gives | Notes |
|---|---|---|
| `start` | `_start`, `EntryRegs`, `StubInfo`, the stack | |
| `dpmi` | `int31`, `version` (0400h), `alloc_block`/`free_block` (0501h/0502h), `lock`/`unlock` (0600h/0601h), `simulate_int` (0300h) with `RealRegs`, `get_pm_vector`/`set_pm_vector` (0204h/0205h), `get_rm_vector`/`set_rm_vector` (0200h/0201h), descriptors (0000h–0002h, 0006h–0009h), `alloc_dos_memory` (0100h/0101h) | ESI, EDI and EBX cannot be inline-assembly operands on x86-32, so every call goes through a register block the asm loads and stores |
| `dos` | `write`, `read`, `File` (open, create, read, write, seek, length, `read_to_vec`), `remove`, `print_dollar` (AH=09h), `exit`, `exit_now` | Through the transfer buffer; reads and writes loop over its 32 KiB. `write_direct_unsafe` is the negative demonstration |
| `heap` | `#[global_allocator]` | First fit over 0501h blocks (256 KiB minimum), address-sorted free list that merges neighbours, 8-byte header. A 1 MB `Vec` grown by pushes and a churn of 100 vectors work; a request of 900 MB fails with the standard "memory allocation of N bytes failed" panic |
| `env` | `args`, `command_tail`, `vars`, `var`, `program_path` | |
| `fpu` | `sqrt`, `sin`, `cos`, `atan2`, `control_word`, `set_double_precision` | `core` has `f64` arithmetic and formatting but no `sqrt`; `{:.6}` and `{:e}` run through `core::fmt` and match the skeleton's output |
| `io` | `print!`, `println!` | Standard output, handle 1 |
| `panic` | the panic handler | Runs the clean-up list, prints `RT PANIC: <location and message>` to standard output (the harness captures only that), exits with status 3. A panic inside the handler exits at once |
| `cleanup` | `register`, `forget`, `run_all` | LIFO, run by `exit` and by the panic handler |
| `timer`, `vga` | the hardware proofs below | |
| `mem` | `memcpy`, `memmove`, `memset`, `memcmp` | Only for the COFF target (stage 1); the ELF build takes them from `compiler_builtins` |

**Addressing.** The stub's data selector covers only the image's block. Memory from DPMI 0501h is somewhere else in the linear address space, so `dpmi::init` sets the limit of `DS` to 4 GiB (0008h) and reloads `DS`, `ES` and `SS`; a pointer to any linear address is then `linear − base`, wrapping, and the segment arithmetic (`base + offset` modulo 2^32) undoes it. The first megabyte (the PSP, the environment, the VGA frame buffer, the BIOS data area) is mapped at linear 0 by CWSDPMI, so it needs no selector either. This is the same near-pointer arrangement DJGPP programs use.

**DOS calls.** See the first item of [what failed](#what-failed-on-the-way). The transfer-buffer path passed a 300,000-byte file written and read back in one call, in 4 KiB pieces, and with `read_to_vec`; the 32 KiB buffer is why the calls loop internally.

## 4. Hardware proof

[`hw`](../spikes/d0-rust-dos/progs/src/hw.rs), its transcript in `expected/hw.txt`:

```
hw: dpmi 0.90 pic bases 0x08 0x70
hw: vector 8 moved to our handler: true
hw: ticks counted in about a second: in range (expected 18 to 20): true
hw: BIOS tick counter advanced by the same amount (within 1): true
hw: vector 8 restored: true
hw: no ticks counted after unhook: true
hw: mode before 0x03, in mode 0x13, after 0x03
hw: frame buffer checksum 0x49fd8cc5, pattern in memory 0x49fd8cc5, match true
```

- **IRQ 0.** The handler is eight instructions of assembly in [`timer.rs`](../spikes/d0-rust-dos/rt/src/timer.rs): save `EAX` and `DS`, load the flat data selector (read through `CS`, which has the same base as `DS`, because `DS` cannot be trusted in a handler), increment a counter, restore, and `JMP FWORD` to the previous handler so the BIOS still acknowledges the interrupt and keeps time. The whole image is locked (0600h) first, the handler and its statics included. In about one second of emulated time it counts 18 to 20 ticks, matching the BIOS counter at `0040:006C`; after `unhook` the count stops and the vector is the original. The hook registers `unhook` on the clean-up list, so `exit` and a panic both restore it (checked: `PANICKY.EXE` hooks, panics, and reports the vector restored before the panic report; `HW.EXE leave-hooked` exits with the timer hooked and mode 13h set, and a last clean-up function reports both back).
- **VGA mode 13h.** `INT 10h AX=0013h` through the host, the frame buffer written at `A0000h` through the flat data segment (no selector, no 0800h mapping needed below 1 MiB), a pattern of bars and a diagonal drawn, read back and checksummed (Adler-32): equal to the checksum of the pattern computed in memory. Then mode 3 again.
- **What cannot be checked headless:** that the picture looks right (no screenshot is taken: DOSBox's capture needs a hotkey), real-time accuracy of the tick rate against a real PIT, and anything audible. A matching read-back proves the writes reach VGA memory and that the mode switch happened; it does not prove the display.

## 5. Costs and facts

| | Skeleton (C and Rust) | This spike |
|---|---|---|
| Executable | `DESK.EXE` 236,072 bytes (115,712 stripped) | `HELLO.EXE` 49,664 bytes with the stub (no symbols), doing more: files, DPMI, fmt, float; `HW.EXE` 16,384; `PANICKY.EXE` 11,264; `RETSTART.EXE` 3,072 |
| Stage 1 | | `HELLO1.EXE` 115,112 bytes (52,224 stripped) |
| Self-contained | | `solo/HELLO.EXE` 70,144 bytes, the host (22,528-byte `CWSDSTUB.EXE`) included; `CWSDPMI.EXE` beside the plain builds is 21,325 |
| Build, cold | about 10 s | about 7 s for `-Zbuild-std` and every program, plus 7 s for stage 1; 0.5 s warm |
| Whole spike check | | about 9 s warm; the full `tools/check.sh` about 10 s warm; about 32 s from a clean clone in the `ci-dos` image, cargo downloads included |
| Startup | `dosrun` round trip 0.40 s | 0.36 s (`HELLO.EXE exit 0`); dominated by DOSBox itself, so the 0.04 s difference is probably the larger file and the C runtime's initialisation, not something a program start can be measured by here |
| Steps and tools | `cargo`, `coff2go32.py`, `gcc` (driver, `as`, `ld`, `libc`, `libgcc`, `crt0`) | `cargo` with `rust-lld`, `elf2go32.py` and the stub bytes |
| Source | about 75 lines of C, 91 of Python (converter) | 1,600 lines of `rt` (a lot of it comments), 139 lines of Python (packer) |

`HELLO.EXE` has 46 KiB of code, 0 bytes of initialised data and 256 KiB of `.bss` (the stack). Stage 1 (stripped) is about 2.5 KiB larger than stage 2; the reason was not investigated.

**Single-file programs.** `elf2go32.py` accepts `CWSDSTUB.EXE` as the stub: the result is one `.EXE` that carries its own CWSDPMI (`solo/HELLO.EXE`) and runs in a directory with no `CWSDPMI.EXE`, byte-for-byte the same transcript. This needs no `stubify` at all.

## 6. Negative and failure cases

- **A wrong expected transcript fails the check:** `check.sh` takes the `hello` transcript, changes `squares=10` to `squares=11`, and requires the comparison to reject it. A mutation test on the clean-up code (registering `unhook` removed) turns the `PANICKY.EXE` comparison red.
- **A panic** prints `RT PANIC: panicked at progs/src/panicky.rs:L:C:` and the message, exits with status 3, and has already restored the IRQ 0 vector (the transcript line before the report). Allocation failure is a panic as well (`HELLO.EXE oom`: status 3).
- **No `CWSDPMI.EXE` beside the program:** the stub prints `Load error: no DPMI - Get csdpmi*b.zip` and exits with status 110 (`0x6e` in the stub's code). This is the stub's own message, no code of ours.
- **Returning from `_start` without calling exit** (`RETSTART.EXE`, whose entry is a single `ret`): the `ret` pops whatever the stub left on its own stack and jumps there. In headless DOSBox it aborted the emulator with a page fault (`dosrun` status 125). On real hardware the result is a protection fault or worse; there is nothing sensible to return to, which is why `rt_entry` never returns and `_start` ends in `ud2`.
- **A hooked vector with no clean-up** (`HW.EXE abandon`, which ends with `exit_now` while IRQ 0 is hooked): it ended with status 0 in two runs, but the test is weak, because with no other client the host unloads and the real-mode vectors go with it. Whether CWSDPMI restores protected-mode vectors when a client ends while the host stays resident is not shown. The clean-up list exists so nothing depends on it.
- **A crash by CPU exception** (the early `CMPXCHG` case): exit status 1 and no output the harness can see. Hence the recommendation to print a register dump from an exception handler.

## Independence from DJGPP

What DJGPP still supplies to the stage 2 build, and what it would take to be fully independent:

| Dependency | What it is | To be independent |
|---|---|---|
| The stub (`STUB.EXE`, 2 KiB) | Copyright DJ Delorie, usable for any purpose with the notice unmodified. Written out by `stubify -g`. The packer leaves it byte for byte as it is (apart from the optional `--dpmi` name) | Vendor the 2 KiB (needs a `NOTICE` entry), or **write our own stub**: a real-mode program that finds a DPMI host (INT 2Fh 1687h), switches mode, allocates a block (0501h), loads a flat image and jumps. It is small, and the format is ours to define, which would also remove the three-section-header and 64 KiB-rounding rules. A research spike |
| `CWSDSTUB.EXE` | Stub plus CWSDPMI, by CWSDPMI's author (redistributable under the terms in `cwsdpmi.doc`) | Same: ours, or keep |
| CWSDPMI | The DPMI host. Redistributable, GPL with source or without under the terms in its document; listed in `NOTICE` | **Buy or write** a host (research: HDPMI, Windows, an own extender), or keep it; it is a separate program. All of `rt`'s calls are DPMI 0.9 |
| `libgcc`, `libc`, `crt0`, `gcc`, `ld`, `as` | Not used in stage 2 | Not needed |
| Stage 1 only | `ld`, `libgcc.a` (`__udivdi3`), `coff2go32.py` | Replaced by stage 2 |

Beyond DJGPP, the headless checks still need DOSBox Staging and a Rust toolchain with `rust-src`, `rust-lld` and `RUSTC_BOOTSTRAP=1` for `-Zbuild-std` (an unstable cargo option, as in the skeleton).

## What was and was not verified

Verified (headless DOSBox Staging 0.83.0, `cputype = 486`, `core = normal`, CWSDPMI r7, Rust 1.98.0; `tools/check.sh` passes here and from a clean clone in the `ci-dos` image):

- Stage 1 and stage 2 build and print identical transcripts; the self-contained variant does too.
- Entry state, `StubInfo`, selectors, the environment, command tail and exit status.
- `rt`'s DPMI calls: 0000h–0002h as used by the stub, 0006h–0009h, 0100h/0101h, 0200h/0201h, 0204h/0205h, 0300h, 0400h, 0501h/0502h, 0600h/0601h.
- A 300,000-byte file in one call and in pieces; `Vec`/`String`/`format!`; a 1 MB `Vec`; allocation failure; x87 sqrt, sin, cos, atan2 and float formatting; the IRQ 0 hook; mode 13h written and read back.
- The negative cases above.

Not verified:

- Anything on real hardware, a real 486, an older DOS, or a DPMI host other than CWSDPMI r7 (hosts other than CWSDPMI may translate INT 21h pointers; `rt` does not depend on that either way).
- Any other memory manager (EMM386, QEMM, Windows 9x, DOSBox's own DOS memory with other `memsize` settings).
- That the picture on screen is right, and sound.
- The x87 state at program start on real DOS (`FNINIT` makes it moot); the real-time accuracy of the tick count.
- Command lines longer than 126 characters (no "proxy" convention), long file names, directories.
- The stack: a fixed 256 KiB with no guard page; a deep recursion is not caught.
- That CWSDPMI restores protected-mode vectors when a client ends while the host stays resident.
- Behaviour with the paging file active (DOSBox has 16 MB and nothing is swapped), where locking the interrupt path would matter.
- The licence position of embedding `CWSDSTUB.EXE`-based single-file programs for distribution; `NOTICE` was not changed.
