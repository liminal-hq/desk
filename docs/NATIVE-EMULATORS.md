# Native boot in other emulators

Spike G: is the native boot path (the 512-byte NASM boot sector, the loader and the Rust kernel of [`NATIVE-BOOT.md`](NATIVE-BOOT.md)) specific to QEMU and SeaBIOS, or does it boot on emulators with other BIOSes and CPU models? Those are the cheapest stand-ins for different real hardware. Result: it boots unchanged under Bochs (as a 486, with three different BIOS images) and under 86Box (a 1993 AMIBIOS 486 board), after one boot bug that 86Box found and this spike fixed. Nothing here has run on real hardware.

## Contents

- [Results](#results)
- [How to run it](#how-to-run-it)
- [Bochs](#bochs)
- [86Box](#86box)
- [Other QEMU configurations](#other-qemu-configurations)
- [What the emulators exposed](#what-the-emulators-exposed)
- [What failed on the way](#what-failed-on-the-way)
- [What is not verified](#what-is-not-verified)

## Results

All runs boot `dist/desk-native.img` to `boot complete`. The transcript is compared with `tests/native/expected.txt`, which tolerates the BIOS-dependent lines (the memory map, the memory totals).

| Emulator | BIOS | CPU model | Result | Differences from the QEMU transcript |
|---|---|---|---|---|
| QEMU 11.1 `-cpu 486` | SeaBIOS (QEMU's) | QEMU 486 | pass, the reference | E820, 6 entries, 15,871 KiB usable |
| QEMU `-cpu pentium`, `-machine q35`, `-machine isapc` | SeaBIOS | Pentium, 486 | pass | `isapc` has 5 entries and 15,999 KiB; `q35` has 8 entries and 15,867 KiB |
| Bochs 2.7 as a 486 (`--enable-cpu-level=4`) | `BIOS-bochs-legacy` | 486DX (`486ni` only) | pass, the full test (loader variants, faults, hang) | E820, 5 entries, 15,996 KiB; A20 already on |
| Bochs 2.7 as a 486 | `BIOS-bochs-latest` | as above | pass, the full test | E820, 6 entries, the last one "ACPI reclaimable" (kept reserved by the kernel), 15,932 KiB |
| Bochs 2.7 as a 486 | SeaBIOS 1.13 (shipped with Bochs) | as above | pass, the full test | E820, 6 entries, 15,991 KiB |
| Bochs 2.7 from Ubuntu's package, Pentium model | `BIOS-bochs-legacy` | `pentium` | pass (by hand) | as the legacy row; with `BIOS-bochs-latest` the BIOS itself crashes before the boot sector (see [What failed](#what-failed-on-the-way)) |
| 86Box 6.0, `ami471` (SiS 471, 33 MHz 486DX, 16 MiB) | AMIBIOS 08/08/93 | 486DX | pass after a loader fix; `tools/native-test-86box.sh` runs the normal boot and a divide error | A20 closed at boot (the loader opens it through the fast gate); neither E820 nor E801, so the AH=88 fallback runs, 2 entries, 16,000 KiB; the BIOS enables the cache, so the CR0 dump is `0x00000033` as on QEMU |

The expected transcript needed no change for Bochs. For 86Box, `tools/native-test-86box.sh` rewrites three lines of it (the A20 line and the two memory map source lines) and says so in its header.

## How to run it

All of it runs in Docker; nothing is installed on the host, and none of it is in the CI image.

```bash
tools/native-build.sh && tools/native-image.sh      # dist/desk-native.img
tools/native-run-bochs.sh [--bios legacy|latest|seabios] [IMAGE]    # the serial transcript on stdout
tools/native-test-bochs.sh                          # the whole native test under Bochs, about 20 seconds
DESK_BOCHS_BIOS=seabios tools/native-test-bochs.sh  # the same with another BIOS
NATIVE_EXTRA=1 tools/check.sh                       # the default checks, then the Bochs test
tools/native-run-86box.sh --screenshot out.png      # one boot in 86Box, about 15 seconds
tools/native-test-86box.sh                          # normal boot and a divide error in 86Box, about 30 seconds
```

The first Bochs run builds `desk-native-bochs:1` (compiling Bochs, a few minutes); the first 86Box run builds `desk-native-86box:1` (about a minute and a half, 170 MB of downloads). The runners return the same statuses as `tools/native-run.sh` (0 success, 1 guest failure, 2 the emulator ended without a result, 124 timeout, 3 no Docker), so `tools/native-test.sh` runs against any of them through `DESK_NATIVE_RUNNER`.

CI would need Docker (the CI container has none) and either a prebuilt Bochs image or a few minutes of compile time per run, so the Bochs test stays opt-in. Measured here: `tools/native-test-bochs.sh` takes about 20 seconds once the image exists.

## Bochs

**Which build.** Ubuntu's `bochs` package cannot be a 486: its CPU models start at a generic part with CPUID level 5 (with MWAIT and 3DNow enabled by default, which panic at level 5) and then a Pentium; it also has no `nogui` display library and starts in its debugger. `tools/docker/Dockerfile.native-bochs` builds Bochs 2.7 from the upstream tarball (checksummed) with `--enable-cpu-level=4 --with-nogui --disable-debugger --disable-plugins`, which gives a 486DX (`486ni` instruction set, no Pentium instructions) with the BIOS and VGA BIOS images installed beside it.

**Configuration.** The runner writes a `bochsrc` per run: `cpu: model=bx_generic, reset_on_triple_fault=0` (a triple fault ends Bochs, as `-no-reboot` does in QEMU), 16 MiB, the image as `floppya` with `boot: floppy`, COM1 as a file, `port_e9_hack` on, `panic: action=fatal`, `display_library: nogui`.

**Ending the run.** Bochs has no `isa-debug-exit`. The runner starts Bochs detached and polls the serial file every 0.2 seconds: `boot complete` is success; a `KERNEL PANIC`, an `EXCEPTION 0x` dump or a loader error line followed by one second of silence is failure; Bochs dying by itself is status 2; the timeout is 124. The guest was not changed for this, so the exit-device write the guest does stays harmless on real hardware. The cost: the status comes from the transcript, not from the guest.

**CPU model effects.** The Bochs 486 raises `#UD` for instructions a 486 lacks, and QEMU's `-cpu 486` does not for several of them. A one-sector probe (`tests/native/probe.asm`) executes one instruction and reports whether it faulted:

| Instruction | QEMU `-cpu 486` | Bochs 486 |
|---|---|---|
| `rdtsc`, `rdmsr`, `emms`, `mov eax, cr4` | executes | `#UD` |
| `cmov`, `cmpxchg8b`, `ud2` | `#UD` | `#UD` |
| `cpuid`, `cmpxchg`, `bswap`, `xadd`, `invlpg`, `wbinvd` | executes | executes |

So QEMU's `-cpu 486` is weaker than [`NATIVE-BOOT.md`](NATIVE-BOOT.md) said: it removes CMOV and CMPXCHG8B but not TSC, MSR, MMX or CR4, and `tools/native-test-bochs.sh` requires `#UD` for the first four. (Bochs's 486 still answers CPUID, with a 486DX2 signature `0x433`; many real 486s have no CPUID, so the kernel must keep not using it.) The kernel boots and passes everything on the Bochs 486, so it uses none of them.

Another difference that matters to a loader: Bochs honours the UART's reset word length of 5 bits. A program that writes to COM1 without setting 8N1 loses the top bits (`S` arrives as `0x13`); QEMU ignores the word length. Stage 2 programs 8N1 first, so it is unaffected, and `probe.asm` does the same.

**BIOS differences.**

- **INT 13h.** The same CHS reads (2 heads, 18 sectors per track) work on all three Bochs BIOSes, on SeaBIOS under QEMU and on the AMIBIOS in 86Box.
- **E820.** All three Bochs BIOSes provide it. The map shapes differ (5 or 6 entries; `BIOS-bochs-latest` adds an ACPI reclaimable entry at the top of RAM, SeaBIOS a reserved one), which is why the checker ignores the map lines.
- **A20.** Every BIOS here (and SeaBIOS) leaves the gate already open, so on a normal boot the loader's A20 code does not run on Bochs or QEMU. The `a20-fast` and `a20-keyboard` loader variants close the gate first and pass under Bochs, so the fast gate (port `0x92`) and the keyboard controller path work against Bochs's A20 emulation too. A closed gate at boot is real on 86Box.
- **E801.** No Bochs BIOS lacks E820, so the E801 path again runs only as a test build, and passes.

**CR0 at entry.** A 486 resets CR0 to `0x60000010`: CD and NW set, the cache off. Bochs's legacy BIOS leaves it that way, so the exception dump shows `0x60000033`; SeaBIOS under QEMU and the AMIBIOS in 86Box clear the bits (`0x00000033`). The kernel never touches CD and NW, so on a machine whose BIOS leaves them set it runs with the cache off. That works but is slow; enabling the cache on a 486 means `wbinvd` and clearing CD and NW, and which regions may be cached depends on the chipset. This is recorded as an open question, not changed blind. The expected dumps accept either (`{BITS:mask=value}` in `tools/native-check.py`).

## 86Box

**What runs.** 86Box 6.0 (the Linux AppImage, unpacked) in a Docker image with Xvfb and the 86Box ROM set `v6.0`, both downloaded at image build time and checked by SHA-256; nothing is committed or redistributed. The ROMs are third-party BIOS images with their own terms (see `86Box/roms`), the reason they are fetched rather than stored. The machine (`tools/86box/86box.cfg`) is `ami471`: SiS 471 chipset, a 486DX at 33 MHz, 16 MiB, VGA, the floppy image in a 3.5-inch 1.44 MB drive, AMIBIOS dated 08/08/93.

**Serial capture.** 86Box 6.0 passes COM1 to a "Virtual Console" device. Mode 1 attaches a pseudo-terminal and shows a message box (`COM1: Attached to /dev/pts/N`) that pauses the machine until dismissed; the container script waits for the new `/dev/pts` entry, sets it raw, starts `cat` on it and presses Return with `xdotool`. A second screenshot (`import -window root`) is taken at the end and kept as `dist/native-86box.png` by `tools/native-test-86box.sh`, so the run has the VGA screen as evidence as well as the serial transcript. Mode 2 (a TCP connection) produced no data in the attempts made.

**BIOS setup data.** A fresh machine stops at "CMOS system options not set — Press F1 to RESUME". The 128-byte CMOS image `tools/86box/ami471.nvr.hex` is the state after loading the BIOS defaults in the AMIBIOS setup (Auto configuration with BIOS defaults, then Write to CMOS and exit), driven by hand with `xdotool` once in a container. The defaults boot from A: then C:. The CMOS says drive A is a 1.2 MB 5.25-inch drive although the emulated drive is 1.44 MB 3.5-inch; the floppy reads work regardless. To make it again, run the image with `/cfg/86box.cfg` and no NVR file, press F1, Delete into setup, choose the auto configuration item and write to CMOS, then take `nvr/ami471.nvr`.

**What 86Box showed.** The first boot stopped at `stage2: no memory map from the BIOS`: the 1993 AMIBIOS has neither INT 15h AX=E820 nor AX=E801 (E801 dates from 1994), and the loader tried only those two. This is the bug of this spike, fixed by an INT 15h AH=88 fallback ([the boot fix](#what-the-emulators-exposed)). The same boot also showed the A20 gate closed, so the fast-gate code ran for real for the first time, and it works. With the fix the kernel runs to `boot complete`, counting the 15,360 KiB of extended memory the BIOS reports.

**Limits.** A boot takes about 15 seconds, so only the normal boot and one fault are in `tools/native-test-86box.sh`, not the whole matrix. The 86Box run was not made to use a hard disk, other chipsets or other machines, and 86Box's timing (a 33 MHz 486 at full emulated speed) was not measured against the PIT beyond the kernel's own tick check, which passes.

## Other QEMU configurations

Run by hand with `qemu-system-i386 -m 16 ... -device isa-debug-exit,iobase=0xf4,iosize=0x04`: `-cpu 486` (the baseline), `-cpu pentium`, `-cpu 486 -machine isapc` (ISA only, no PCI chipset; SeaBIOS reports a different map, 5 entries and 15,999 KiB) and `-cpu 486 -machine q35` (ICH9, 8 entries and 15,867 KiB). All pass the unchanged expected transcript. They share SeaBIOS and QEMU's translator, so they are not independent evidence the way a different BIOS and a different CPU emulator are.

## What the emulators exposed

- **A real boot bug (86Box).** The loader needed E820 or E801 and nothing else. Fixed in `boot/stage2.asm` with `map_88` (INT 15h AH=88: KiB above 1 MiB, rewritten as the same E820-style entries, map source 3) and tested by the `e88` loader variant of `tools/native-test.sh` (`-DNO_E820 -DNO_E801`), which runs under QEMU and Bochs. AH=88 reports at most 15 MiB (63 MiB on some BIOSes), so RAM above that is invisible when it is the only call available.
- **QEMU's `-cpu 486` is lenient** (above): TSC, MSR, MMX and CR4 instructions are not rejected. The Bochs test keeps the claim honest.
- **The A20 gate is closed at boot on an old AMIBIOS**, which no QEMU or Bochs BIOS shows; the fast-gate path is exercised there.
- **CR0 at entry depends on the BIOS** (cache-disable bits set by Bochs's legacy BIOS, clear on the others): the checks now accept both, and the kernel does not manage the cache.
- **The serial port's word length is real in Bochs**: any program that uses COM1 must set 8N1.

## What failed on the way

- **Ubuntu's Bochs package.** `cpu: model=...` offers nothing below a Pentium-class generic model; `cpuid: level=4` is rejected (range 5 to 6) and level 5 panics on the default MWAIT and 3DNow settings; the package has no `nogui` library (`rfb` panics with no client, SDL needs a dummy video driver) and starts in the debugger prompt; the sound plugin aborts without an audio device unless `sound: waveoutdrv=dummy` is set; with `BIOS-bochs-latest` and the generic CPU its 32-bit BIOS stage triple-faults before the boot sector ("Starting rombios32", then `EIP` in the middle of an ASCII string). `BIOS-bochs-legacy` boots. The source build with CPU level 4 has none of these problems, including `BIOS-bochs-latest`.
- **86Box serial passthrough.** `serial1_passthrough_enabled` from older releases is rewritten to `serial1_device = stdio` with a `[Virtual Console (COM) #1]` section; mode 2 gave no data; mode 1 works but needs the message box dismissed.
- **86Box first-run CMOS.** The BIOS stops at its setup prompt until a CMOS image exists. The first scripted setup attempts landed on the wrong menu items (the setup menu keeps its cursor position between visits), so the sequence was redone one screenshot at a time.
- **`probe.asm` output.** The first version wrote to COM1 without waiting for the transmitter and lost bytes under QEMU; it now polls the line status register.

## What is not verified

- Anything on real hardware, a real floppy drive or a real 486. A 1993 AMIBIOS in 86Box is the closest stand-in tried, and it still emulates the BIOS ROM, the chipset and the floppy controller with the author's approximations.
- Any cache or timing behaviour: no emulator here is cycle-accurate, and the kernel runs with whatever CR0 cache state the BIOS leaves.
- Booting from a hard disk, a CD-ROM or USB, or any machine other than `ami471`.
- The 86Box fault, hang and loader-variant matrix: only the normal boot and a divide error run there.
- Memory above 15 MiB on a BIOS that has only AH=88.
- The result in CI: the CI image has no Docker, so the Bochs and 86Box tests run only on a developer machine.
