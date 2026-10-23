# Native boot

The native track's walking skeleton: a floppy image whose boot sector loads a protected-mode Rust kernel, which runs headless in QEMU as a 486 and is checked by `tools/check.sh`. It proves the toolchain and the boot path end to end; it is not a desktop. Nothing here has run on real hardware.

## Contents

- [What the check proves](#what-the-check-proves)
- [Boot flow](#boot-flow)
- [Image layout](#image-layout)
- [The kernel target](#the-kernel-target)
- [The linker script and the flat binary](#the-linker-script-and-the-flat-binary)
- [Memory](#memory)
- [Interrupts and the timer](#interrupts-and-the-timer)
- [Input](#input)
- [The FPU](#the-fpu)
- [Running it and the CPU model](#running-it-and-the-cpu-model)
- [The checks](#the-checks)
- [Debugging](#debugging)
- [What was and was not verified](#what-was-and-was-not-verified)

## What the check proves

`tools/native-test.sh` (run by `tools/check.sh`) builds `dist/desk-native.img`, boots it and compares the serial transcript with `tests/native/expected.txt`. A passing run shows that the boot sector reads the loader and kernel with INT 13h, the loader opens A20, reads a memory map and enters protected mode, and the kernel sets up its own GDT, IDT, PIC and PIT, counts timer ticks, parses the memory map, allocates frames and heap memory (`Vec`, `String`, `format!`) and does floating-point maths on the x87.

## Boot flow

1. **BIOS to stage 1.** The BIOS loads sector 0 (`boot/stage1.asm`) to `0000:7C00` and jumps to it in 16-bit real mode with the boot drive in DL. Stage 1 normalises the segment registers and sets a stack just below itself.
2. **Stage 1 reads the rest.** With INT 13h AH=02 it reads stage 2 (17 sectors) to `0000:7E00`, then the kernel to physical `0x10000`. Reads are CHS (a 1.44 MB floppy is 80 cylinders, 2 heads, 18 sectors per track), go up to the end of a track at a time, never cross a 64 KiB physical boundary (the floppy DMA cannot), retry five times with a controller reset between tries and, on failure, print `Disk read error` and stop. The kernel is staged below 1 MiB because real mode cannot address memory above it.
3. **Stage 2: output.** `boot/stage2.asm` starts COM1 and prints each step to both the screen and the serial port, so a headless run records the loader too.
4. **A20.** The 8086 wrapped addresses at 1 MiB and the PC keeps that until the A20 gate is opened. Stage 2 tests the gate (a byte written at `0000:0500` shows up at `FFFF:0510` when it is closed), then tries the fast gate (port `0x92`), then the keyboard controller (commands `0xD0` and `0xD1` on port `0x64`), re-testing after each, and stops with a message if none worked.
5. **Memory map.** Only the BIOS knows what RAM exists, and only in real mode. Stage 2 walks INT 15h AX=E820 (up to 32 entries of 24 bytes, ignoring empty entries and ACPI 3.0 entries marked "ignore"). If E820 is missing it falls back to INT 15h AX=E801 and rewrites the answer in the same entry format (conventional memory from INT 12h, 1 to 16 MiB, and above 16 MiB), so the kernel parses one format.
6. **Boot information block.** Stage 2 writes a block at physical `0x500`: magic `DESK`, boot drive, test mode, map source (1 E820, 2 E801), entry count, a version (1), the address of the video block and the address of the BIOS font, then the entries from offset 32. The kernel receives its address in EAX. The layouts, and how the version lets the block grow, are in [`D-NATIVE-VGA.md`](D-NATIVE-VGA.md#the-boot-information-block).
7. **Video (real mode, last chance).** The kernel cannot call the BIOS, so stage 2 copies the BIOS 8×16 font to `0x2000` (INT 10h AX=1130h) and, when the image asks for it (`tools/native-image.sh --video vbe`, a byte at stage 2 offset 4), asks VBE 2.0 for a 640×480×8 linear frame buffer mode (4F00h, 4F01h over the mode list, 4F02h with the linear bit), falls back to "nothing set" with a printed reason when VBE is missing, older than 2.0, has no such mode or refuses, and records the answer in a 32-byte video block at `0x820`. Details in [`D-NATIVE-VGA.md`](D-NATIVE-VGA.md#the-loader-vbe-before-protected-mode).
8. **Protected mode.** Stage 2 loads a three-entry GDT (null, flat 4 GiB code at `0x08`, flat 4 GiB data at `0x10`), sets CR0.PE and far-jumps to a 32-bit code segment, which also flushes the prefetch queue. Interrupts stay off throughout.
9. **Kernel copy and jump.** In protected mode the kernel is copied from `0x10000` to `0x100000` (1 MiB), the stack is set and stage 2 jumps to the kernel's first byte.
10. **Kernel start-up.** `_start` (a `global_asm!` stub in `kernel/src/main.rs`) points ESP at a 64 KiB stack in `.bss`, clears `.bss` (the flat image does not contain it) and calls `kmain(boot_info)`. `kmain` then brings up the serial port, the VGA text screen, the GDT, the IDT, the PIC and the PIT, in that order, and the demonstrations follow.

## Image layout

A 1,474,560-byte image (2,880 sectors of 512 bytes), assembled by `tools/native-image.sh`:

| LBA | Contents |
|---|---|
| 0 | Stage 1, the boot sector (`boot/stage1.asm`) |
| 1–17 | Stage 2 (`boot/stage2.asm`), padded to 17 sectors; a build error if it outgrows them |
| 18– | The flat kernel image, starting on the second track so that every read is a whole track |
| rest | Zeros |

Three values are patched in after the build, so the loader never needs rebuilding when the kernel grows: the kernel length in sectors at stage 1 offset 3 (16 bits), the test mode at stage 2 offset 3 (see [The checks](#the-checks)) and the video request at stage 2 offset 4 (0 keeps the BIOS text screen, 1 tries a VBE linear mode). The kernel must fit between `0x10000` and the BIOS data area (the image tool refuses more than 1,000 sectors).

Physical memory used while booting: `0x500` boot information and memory map (to `0x820`), `0x820` the video block (32 bytes), `0x2000` the BIOS font (4 KiB), `0x3000` and `0x3200` the VBE information and mode blocks while stage 2 runs, `0x7C00` stage 1, `0x7E00` stage 2 (to about `0xA000`), `0x10000` the kernel's staging copy, `0x100000` the kernel. Once the kernel is running, `0x10000` to `0x13FFF` is the BIOS trampoline, the bounce buffer for BIOS calls and the real-mode stack (see [`N3-BIOS-PATH.md`](N3-BIOS-PATH.md#the-low-memory-map)).

## The kernel target

`kernel/i486-desk-none.json` is a custom target for the pinned stable toolchain (with `RUSTC_BOOTSTRAP=1`, as for `-Zbuild-std`; cargo also needs `-Zjson-target-spec` for a `.json` target, set in `kernel/.cargo/config.toml`). The fields that matter:

- `llvm-target: i486-unknown-none-elf` and `cpu: i486`: code generation limited to 486 instructions. No CMOV, no CPUID use, no `cmpxchg8b`, so `max-atomic-width` is 32.
- `features: -mmx,-sse,-sse2`: a 486 has none of them. The FPU is the x87, so there is no soft-float: the compiler emits x87 instructions for `f32` and `f64`.
- `panic-strategy: abort`: no unwinding machinery.
- `relocation-model: static` and `position-independent-executables: false`: the kernel is linked at a fixed address and nothing relocates it.
- `disable-redzone: true`: an interrupt would otherwise overwrite data below the stack pointer. (A 32-bit target has no red zone, so this is belt and braces and keeps the spec ready to copy for x86-64.)
- `linker: rust-lld`, `linker-flavor: gnu-lld`: the linker that ships with rustc; no GCC or binutils is needed.
- `os: none`, `executables: true`, `dynamic-linking: false`: a freestanding static executable.

The build is `cargo build --release` in `kernel/` with `build-std = ["core", "alloc", "compiler_builtins"]` and `compiler-builtins-mem` (so `memcpy` and friends exist), all in `kernel/.cargo/config.toml`. `rust-toolchain.toml` already lists `rust-src`.

## The linker script and the flat binary

`kernel/link.ld` places `.text` (with the `_start` stub first), `.rodata`, `.data` and a `NOLOAD` `.bss` from `0x100000` and defines `__bss_start`, `__bss_end`, `__kernel_start` and `__kernel_end`. `tools/elf2bin.py` flattens the ELF's loadable segments into `dist/native/kernel.bin`; it stands in for `objcopy -O binary` so the build needs no binutils. The ELF (`dist/native/kernel.elf`) keeps its symbols for gdb.

## Memory

The kernel reads the entries the loader stored. `kernel/src/memory.rs` keeps a one-bit-per-frame bitmap for the first 256 MiB (RAM above that is ignored for now): usable regions are freed (rounded inward to whole 4 KiB frames), then every other kind of region is reserved (rounded outward), so an overlap resolves to reserved. The first megabyte and the kernel image (`__kernel_start` to `__kernel_end`, including `.bss` and the stack) stay reserved. `alloc_frame` and `alloc_contiguous` are first fit. The reported "usable RAM" is the sum of the usable entries as the BIOS gave them.

The heap (`kernel/src/heap.rs`) takes 4 MiB of contiguous frames from the frame allocator and manages them as a first-fit free list sorted by address, with merging of neighbours on free, behind `#[global_allocator]`. Allocation sizes and alignments are rounded to 8 bytes. It is a plain version of a well-known design, simple rather than fast, and interrupt-safe because the lock is "interrupts off" on a single processor.

## Interrupts and the timer

The kernel loads its own GDT (the loader's lives in memory the kernel may overwrite) and an IDT of 256 gates. Vectors 0 to 31 point to small assembly stubs that push a dummy error code where the CPU pushes none, push the vector, save the registers and call one Rust handler, which prints the vector name, the error code, EIP, CS, EFLAGS, the general registers and CR0, CR2 and CR3 to serial and the screen, then ends the run with a failure status. The PIC is remapped to vectors `0x20` and `0x28` (the BIOS default of `0x08` collides with CPU exceptions) and every line but IRQ0 is masked. The PIT runs channel 0 as a rate generator at 100 Hz (divisor 11,932). The IRQ0 handler increments an atomic counter and sends the end-of-interrupt. The kernel waits for the first tick in a bounded spin (`hlt` would wait forever if the timer were broken, and the run would end only by the watchdog), then waits for 10 more ticks with `hlt` and prints the elapsed count.

## Input

The kernel's serial, PIC and PIT code runs on the shared drivers in `hal/`, and `kernel/src/hal.rs` is the native platform: the port bus, the statics for those drivers, and an interrupt registry for IRQ 0 to 15 (one IDT stub per line, the end-of-interrupt sent by the kernel after the handler, spurious IRQ 7 and 15 handled). After the checks above, `kernel/src/input.rs` sets up the 8042, the keyboard and the PS/2 mouse, registers the IRQ 1 and IRQ 12 handlers (which only fill lock-free rings), unmasks the lines and prints four `input:` lines. The test modes `input`, `input-masked` and `input-idle` then drain the rings in a `hlt` loop and print each decoded event until Escape is released; `tools/native-run.sh --inject` sends the keys and mouse motion through the QEMU monitor. The design, the tables and the limits are in [`C-INPUT.md`](C-INPUT.md).

## Graphics

Spike D (n2) adds the loader's VBE step above and, in the kernel, `kernel/src/video.rs` (the video block, and checks that the linear frame buffer overlaps no RAM, the heap or the kernel) and `kernel/src/gfx.rs` (the graphics tour the `gfx` test mode runs: modes 13h and 12h by registers, the VBE frame buffer, a palette fade, a software cursor driven by the mouse, a window, and the way back to text mode), all through the shared `desk-vga` driver. A normal boot keeps the BIOS text screen; only the `gfx` images ask for VBE. The design, the tables, the screenshots and what QEMU hides are in [`D-NATIVE-VGA.md`](D-NATIVE-VGA.md).

## The BIOS path

Spike n3 gives the protected-mode kernel the BIOS back, one call at a time. `kernel/bios16.asm` (assembled by `tools/native-build.sh` and embedded in the kernel) is a one-page blob that `kernel/src/bios.rs` copies to `0x10000` at boot; a call drops the CPU to real mode, runs one `INT n` with a register block and a buffer in low memory, and returns, with the 8259s put on the BIOS vectors and back, NMI masked, the BIOS's timer ticks added to the kernel's clock and any keyboard byte left in the 8042 handed to its driver. `bios::NativeBios` implements the HAL's `Bios` trait, so the `desk-vga` VBE layer runs in the kernel unchanged, and `INT 13h`, `INT 15h` and `INT 1Ah` work too. The `bios` test mode exercises it. Every boot prints one `bios:` line. The design, the contract and the measurements are in [`N3-BIOS-PATH.md`](N3-BIOS-PATH.md).

## The FPU

The x87 is enabled with CR0.EM clear (run x87 instructions instead of trapping them), CR0.MP and CR0.NE set (errors report as exception 16 rather than the legacy IRQ 13 wiring) and CR0.TS clear, then `fninit`. The kernel checks the control word is `0x037F`, computes `sqrt(2)` with `fsqrt` and the harmonic sum H(10) in ordinary Rust `f64` (which the compiler turns into x87 code), prints both and asserts them. (Some 486SX machines have no FPU; this kernel assumes one.)

## Running it and the CPU model

`tools/native-run.sh` runs `qemu-system-i386 -cpu 486 -m 16 -drive if=floppy,format=raw,file=IMAGE -display none -serial file:LOG -device isa-debug-exit,iobase=0xf4,iosize=0x04 -no-reboot -monitor none` under `timeout` (with `--inject`, `-monitor unix:SOCKET,server,nowait` instead). `-cpu 486` makes QEMU's CPU model reject instructions a 486 lacks (it has no CPUID, TSC, CMOV, MMX or SSE), so a stray newer instruction fails in the run instead of on a real machine. It is an approximation: QEMU's translation is not cycle-accurate and models few of a real 486's quirks. The guest ends the run by writing to the `isa-debug-exit` port, which makes QEMU exit with `(value << 1) | 1`: status 33 for success and 35 for failure. `-no-reboot` turns a triple fault into a QEMU exit instead of a reboot loop. The script maps the statuses to its own exit codes (0 success, 1 guest failure, 2 other exit, 124 timeout, 3 no QEMU or Docker). It uses the host's QEMU, or Docker with an `ubuntu:24.04` image (`tools/docker/Dockerfile.native-qemu`, built once as `desk-native-qemu:1`); set `DESK_NATIVE_BACKEND=docker` to force Docker.

## The checks

`tools/native-test.sh` runs, in order:

- **Loader variants.** Stage 2 rebuilt with `-DA20_TEST=1` (the gate is closed first, so the fast-gate code runs), `-DA20_TEST=2` (the fast gate is also skipped, so the keyboard-controller code runs) and `-DNO_E820` (so the E801 fallback runs). Each must boot and print the matching loader line.
- **Normal boot.** The transcript must match `tests/native/expected.txt` line by line. `tools/native-check.py` allows `{N:lo..hi}` (a number in range, for the memory total and entry counts), `{HEX}`, `{ANY}` and a lone `...` line for any number of lines (the BIOS-dependent memory map).
- **A wrong expectation fails.** `tests/native/wrong-expected.txt` differs in one number and must be rejected.
- **Deliberate faults.** Test modes patched into the image make the kernel divide by zero (`div0`), load a bad segment selector (`gp`), execute `ud2` (`ud`), panic (`panic`) or mask the timer (`notick`). Each must end with exit code 1 and the expected exception dump or panic message.
- **Keyboard and mouse.** In `input` mode keys and mouse motion injected through the QEMU monitor must appear decoded in the transcript; with an event missing from the injected input the comparison must fail; with IRQ 1 masked (`input-masked`) no key arrives and the kernel must report it (exit 1); with nothing injected (`input-idle`) the watchdog must end the run.
- **Graphics.** The `gfx` test mode boots with a VBE linear frame buffer (`-vga std`), with the register modes only (`-vga cirrus`) and with a loader whose VBE mode set is made to fail (`-DVBE_FAIL=1`); screenshots taken through the QEMU monitor must match reference checksums, the cursor must have moved between two of them, and seven wrong inputs must be rejected (`D-NATIVE-VGA.md`, "How it is tested").
- **BIOS calls.** The `bios` test mode calls the BIOS from the kernel on `-vga std`, `-vga cirrus` and `-vga vmware` (VBE set after boot and the way back to text, INT 13h sector reads equal to memory, E820 after boot, the RTC, a key sent during a call, 1,000 calls, the error paths), once more with an IDE disk for the LBA extensions, and the VBE tour on Cirrus leaves VBE through the BIOS; screenshots must match references and five wrong inputs must be rejected (`N3-BIOS-PATH.md`, "How it is tested").
- **A hang.** In `hang` mode the kernel stops with interrupts off; the run must end by the five-second watchdog (exit 124).

## Debugging

- **gdb.** Run QEMU with `-s -S` (add them to `qemu_args` in `tools/native-run.sh`, or run QEMU by hand) and connect: `gdb dist/native/kernel.elf -ex 'target remote :1234' -ex 'break kmain' -ex continue`. For 16-bit code use `set architecture i8086`; the host gdb is enough, and in Docker use `ubuntu:24.04` with `gdb`.
- **Interrupt and reset logging.** `tools/native-run.sh --debug-log FILE` adds `-d int,cpu_reset -D FILE`, which records every interrupt and CPU reset, the quickest way to see a triple fault.
- **Screen.** The VGA text buffer can be dumped from the QEMU monitor with `xp /2000hx 0xb8000`; stage 1's messages go only to the screen.
- **Listings.** `dist/native/stage1.lst` and `stage2.lst` give the address of every instruction.

## What was and was not verified

Verified: the whole chain above, including each negative check and the three loader variants, on the development machine with QEMU 11.1 and NASM 3.02, and again inside the shared CI image `ghcr.io/liminal-hq/ci-dos` (pinned by digest in the workflow) with `qemu-system-x86` and `nasm` added by `apt-get` (Ubuntu 24.04's QEMU). The Docker backend of `tools/native-run.sh` was exercised with an Ubuntu 24.04 image carrying the same two packages; the exact `tools/docker/Dockerfile.native-qemu` image was not built.

The keyboard and mouse checks (spike C) were verified on the host's QEMU only; the CI image's QEMU runs them as part of `tools/native-test.sh`. The same goes for the graphics checks (spike D): the screenshot checksums were recorded on QEMU 11.1.2 and CI's QEMU decides whether they hold there. The BIOS checks (spike n3) were also run on Ubuntu 24.04's QEMU 8.2.2 with the same transcript and screenshots, and the BIOS scenarios without key injection on Bochs 2.7 with two Bochs BIOSes (`N3-BIOS-PATH.md`, "Results").

Not verified:

- Anything on real hardware, a real floppy drive or a real 486. QEMU's `-cpu 486` is an approximation.
- 86Box or Bochs, which model old hardware more closely, were not tried.
- Booting from a hard disk or IDE (the loader assumes the 1.44 MB floppy geometry of 18 sectors and 2 heads).
- BIOSes other than SeaBIOS for the loader: the E820 and E801 paths and the keyboard-controller A20 path were exercised only against QEMU, and the E801 and A20 paths only through the test builds.
- A machine with an 8086-style A20 wrap that needs INT 15h AX=2401 or other chipset-specific methods, or one with no FPU.
- Memory above 256 MiB, and the speed of the allocators.
