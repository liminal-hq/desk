---
id: m-emulators
n: M20
part: VII
title: The machines we run on
summary: QEMU as a 486, Bochs, 86Box, headless DOSBox Staging and js-dos; what each one is for, what each rejects or allows, how each run is driven and ended, and the standing fact that no real machine has run anything.
date: 2026-10-23
reviewed: 44f0265
watches: [tools/native-run.sh, tools/native-run-bochs.sh, tools/native-run-86box.sh, tools/native-test.sh, tools/native-test-bochs.sh, tools/native-test-86box.sh, tools/dosrun.sh, tools/86box/, docs/NATIVE-EMULATORS.md, docs/S5-JSDOS.md, docs/NATIVE-BOOT.md]
log: [emulators, browser, boot]
anatomy: [desk:emu, dos:dosbox, desk:real]
---
## What it is

Everything the manual calls verified was verified on an [emulator](term:emulator): a program that behaves like a computer closely enough to run another program written for that computer. There is no real 486 in the loop. This chapter says which emulators there are, what each one is faithful to and what it is not, and how a script drives a run and learns whether it passed. It is the chapter to read before trusting any "built, verified" tag in this manual, because the tag always means "in an emulator or on the host".

On the map ([M1](chapter:m-map)) the emulators are the bottom layer of both anatomy pages: `desk:emu` (QEMU, with DOSBox Staging) on the native page, `dos:dosbox` (DOSBox Staging, headless) on the DOS page, and beside each a box for a real 486 PC, `desk:real`, which is planned and has never been tried. The test modes and the way a run ends were introduced in [M5](chapter:m-boot) for QEMU, and the loader's fallbacks that the emulators exposed are in [M4](chapter:m-loader); this chapter is the rest of the picture. After it you should be able to say which emulator can run which track and why, state what `-cpu 486` does not reject, run a check in the emulator that suits it, and read the status a runner returns.

## How it works

### Two kinds of emulator

The [hosted track](term:hosted-track) is a DOS program, so it needs something that behaves like DOS. The [native track](term:native-track) is a bootable image, so it needs something that behaves like a PC: a BIOS, a floppy controller, a keyboard controller, a serial port. `docs/DIRECTION.md` ("Emulation and test matrix") puts the difference in one column.

| Emulator | Hosted track | Native track | What the document says |
|---|---|---|---|
| DOSBox Staging, headless | Yes | No | Emulates DOS, not PC hardware; the CI target for the hosted track |
| js-dos (browser) | Yes | No | DOSBox in the browser; the programs and the VBE linear frame buffer work in headless Chromium |
| QEMU, `-cpu 486` | Only by booting a DOS image | Yes | The first target for the native track; scriptable and headless |
| 86Box | Optional | Yes | Closer to period hardware, including specific sound and video cards |
| Bochs | Optional | Yes | Slow but careful |
| Real 486 | Later | Later | No real hardware is in the loop yet |

The reason the DOS emulators cannot run the native image is stated in `docs/OPEN-QUESTIONS.md` ("Emulators"): DOSBox and js-dos "emulate DOS, not PC hardware". The native image is a boot sector, a loader and a kernel that take over the machine: they ask the BIOS for a memory map, open the [A20](term:a20) gate, switch the processor to [protected mode](term:protected-mode) and program the interrupt controller and timer directly ([M2](chapter:m-cpu), [M3](chapter:m-devices)). DOSBox supplies DOS services and a few emulated cards, not a machine that boots a floppy into its own kernel. The repository has never tried to boot the native image in DOSBox, so this chapter reports the document's reason and not a test of it.

### QEMU as a 486

`tools/native-run.sh` is the main runner. Its fixed arguments, from the script and `docs/NATIVE-BOOT.md` ("Running it and the CPU model"):

| Argument | What it does |
|---|---|
| `-cpu 486` | Picks QEMU's 486 model, so some newer instructions fault |
| `-m 16` | 16 MiB of RAM |
| `-drive if=floppy,format=raw,file=IMAGE` | The image is the floppy |
| `-display none` | No window; headless |
| `-serial file:FILE` | COM1 is written to a file, which is the transcript |
| `-device isa-debug-exit,iobase=0xf4,iosize=0x04` | The [debug-exit port](term:debug-exit-port) the kernel writes to end the run |
| `-no-reboot` | A triple fault ends QEMU instead of looping |
| `-monitor none`, or `-monitor unix:SOCKET,server,nowait` | The monitor is off, or, with `--inject`, a socket that `tools/native-inject.py` types into |

The whole command is run under `timeout`, which is the [watchdog](term:watchdog). The script uses the host's QEMU when there is one, and otherwise builds an `ubuntu:24.04` image from `tools/docker/Dockerfile.native-qemu` once and runs QEMU inside it, with the repository mounted at the same path. The status mapping is the table in [M5](chapter:m-boot).

The screen of a QEMU run, which is the same boot the serial file records, looks like this.

<!-- figure qemu -->

**What `-cpu 486` does and does not reject.** The documents first said that `-cpu 486` "makes QEMU's CPU model reject some instructions a 486 lacks". Spike G measured it with a one-sector probe (`tests/native/probe.asm`) that executes one instruction and prints `probe: #UD` if the processor raised an invalid-opcode exception and `probe: executed` if not:

| Instruction | QEMU `-cpu 486` | Bochs 486 |
|---|---|---|
| `rdtsc`, `rdmsr`, `emms`, `mov eax, cr4` | executes | `#UD` |
| `cmov`, `cmpxchg8b`, `ud2` | `#UD` | `#UD` |
| `cpuid`, `cmpxchg`, `bswap`, `xadd`, `invlpg`, `wbinvd` | executes | executes |

This table is copied from `docs/NATIVE-EMULATORS.md` ("Bochs", "CPU model effects"). I reran the QEMU column for two of the rows: `rdtsc` prints `probe: executed` and `cmov` prints `probe: #UD`, as the table says (see "Try it yourself"). So QEMU's 486 removes CMOV and CMPXCHG8B but not the time-stamp counter, the model-specific registers, MMX or CR4. A stray Pentium instruction in the kernel would therefore pass the QEMU test. The Bochs test closes the gap, since it requires `#UD` for `rdtsc`, `rdmsr`, `emms` and a read of CR4. A caution from the same document: Bochs's 486 still answers `cpuid` with a 486DX2 signature, while many real 486s have no `cpuid`, so the kernel must keep not using it either.

### Bochs

Bochs is a portable emulator of the x86 PC that models the processor, memory, disks, display and devices in software. Ubuntu's package cannot be a 486: its models start at a generic part with CPUID level 5 and then a Pentium, and it has no `nogui` display library and starts in its debugger. So `tools/docker/Dockerfile.native-bochs` builds Bochs 2.7 from the upstream tarball, checksummed, with `--enable-cpu-level=4 --with-nogui --disable-debugger --disable-plugins`. That gives a 486DX with the 486 instruction set and the BIOS images installed beside it.

A [BIOS image](term:bios-image) is the firmware a PC runs first: it tests the machine, answers the interrupt calls that a boot sector and a loader depend on, and loads sector 0 of the boot disk. Bochs ships three that the runner can choose with `--bios`: `BIOS-bochs-legacy` (the default), `BIOS-bochs-latest` and SeaBIOS 1.13 (the BIOS QEMU uses, here under Bochs). The loader's INT 13h reads work on all of them; the memory map they return differs in shape (5 or 6 entries, `BIOS-bochs-latest` adding an ACPI reclaimable entry), which is why the checker ignores the map lines. The same image booted on all three.

`tools/native-run-bochs.sh` writes a `bochsrc` for each run: `cpu: model=bx_generic, reset_on_triple_fault=0` (a triple fault ends Bochs, as `-no-reboot` does in QEMU), 16 MiB, the image as `floppya` with `boot: floppy`, COM1 as a file, `port_e9_hack` on, `panic: action=fatal` and `display_library: nogui`. Bochs has no exit device, so the runner starts it detached in a container and polls the serial file every 0.2 seconds. `boot complete` is success. A `KERNEL PANIC`, an `EXCEPTION 0x` dump, a `Disk read error` or one of the loader's error lines, followed by one second of silence (five polls with an unchanged size), is failure. Bochs dying by itself is status 2, and the timeout is 124. The guest was not changed for this; the cost is that the status comes from the transcript and not from the guest.

Two Bochs behaviours matter to a loader. The CPU model is stricter, as the table shows. And Bochs honours the serial port's reset word length of 5 bits, so a program that writes to COM1 without setting 8N1 loses the top bits (an `S` arrives as `0x13`); QEMU ignores the word length. Stage 2 programs 8N1 first, so it is unaffected, and the probe does the same.

### 86Box

86Box emulates a specific period machine. `tools/86box/86box.cfg` selects the board `ami471`: a SiS 471 chipset, a 486DX at 33 MHz, 16 MiB, VGA, with the image in a 3.5-inch 1.44 MB floppy drive and an AMIBIOS dated 08/08/93. The program is the 86Box 6.0 Linux AppImage, unpacked, and its ROM set `v6.0`, both downloaded at image-build time and checked by SHA-256; nothing is committed, because the ROMs are third-party BIOS images with their own terms.

This is the emulator that found a bug. The 1993 AMIBIOS has neither INT 15h AX=E820 nor AX=E801 (E801 dates from 1994), and the loader tried only those two, so the first boot stopped at `stage2: no memory map from the BIOS`. The fix is an INT 15h AH=88 fallback, tested by the `e88` loader variant. The same boot showed the A20 gate closed at power-on, so the fast-gate code ran for real for the first time and worked ([M4](chapter:m-loader)).

Driving it is the awkward part, and `tools/86box/run-in-container.sh` does it inside the Docker image: it starts a virtual X server (Xvfb on display `:99`), then 86Box with the floppy image. 86Box has no headless mode, so the X server is its screen. COM1 goes to a "Virtual Console" device that attaches a pseudo-terminal and shows a message box that pauses the machine until dismissed, so the script waits for the new `/dev/pts` entry, sets it raw, starts `cat` on it into `serial.txt`, and presses Return with `xdotool`, again every five seconds until the guest has written something. It then polls the transcript with the same rules as the Bochs runner and takes a final screenshot with `import -window root`. A fresh machine also stops at "CMOS system options not set — Press F1 to RESUME", so a 128-byte CMOS image, `tools/86box/ami471.nvr.hex`, made once by driving the BIOS setup by hand, is written to the machine's NVR file before the run. A boot takes about 15 seconds, so `tools/native-test-86box.sh` runs only a normal boot and a divide error.

### Headless DOSBox Staging

`tools/dosrun.sh DIR COMMAND...` runs a DOS command in DOSBox Staging and hands back its output and exit status. The word [headless](term:headless) here means a program running with no display: `SDL_VIDEODRIVER=dummy` gives SDL a video driver that draws nowhere (and `SDL_AUDIODRIVER=dummy` the same for sound), `DISPLAY` and `WAYLAND_DISPLAY` are removed so a developer's desktop is never used by accident, and the configuration sets `output = texture`, the software renderer. The script generates a `dosbox.conf` and a batch file, mounts the program's directory as `C:` and a scratch directory as `D:`, and runs the command with its output redirected to `D:\STDOUT.TXT`.

DOS has no way to hand an exit status to its host, and DOSBox Staging does not expand `%ERRORLEVEL%`. The batch file therefore recovers it with a ladder of 255 lines `if errorlevel n set RC=n`, each of which holds for every `n` up to the status, so the last one that matches is the status, and it writes that to `D:\EXITCODE.TXT`. The script exits with it. Two statuses are the script's own: 124 when the command outlived `DOS_TIMEOUT` seconds (default 60) and 125 when DOSBox itself failed.

The CPU section is `core = normal` (the slowest core and the most predictable) and `cputype = 486`, which makes DOSBox fault on an instruction a 486 lacks. The video card is `svga_s3`, which gives VBE and a [linear frame buffer](term:lfb). Keys are typed in with `DOS_AUTOTYPE`, the arguments of DOSBox's `autotype` command (`-w 3 -p 0.3 d e s k esc`): they go through the emulated keyboard controller, so a program that reads port 0x60 sees them as a keyboard would send them. `DOS_X11=1` instead keeps the environment's display and uses SDL's x11 driver, for a run under a virtual X server (the `desk-dos-xvfb:1` image, `tools/docker/Dockerfile.dos-xvfb`), which is how a pointer can be driven with `xdotool`.

### js-dos

js-dos 8 is the browser build of the hosted track: a fork of DOSBox compiled to WebAssembly, run in a page. Spike s5 (`docs/S5-JSDOS.md`, PR #13) ran every DOS program the repository builds in js-dos 8.5.3 in headless Chromium 154 and compared the result with native DOSBox Staging 0.83.0: 18 scenarios, three clean runs, transcripts byte for byte the same, mode 13h and VBE 640 × 480 × 8 pictures exact against a computed reference through the [linear frame buffer](term:lfb) and the bank window. It is driven from Node: `spikes/s5-jsdos/drive.mjs` serves the site, starts Chromium with the DevTools protocol on a local port and evaluates calls in the page; `tools/check-jsdos.sh` is the opt-in check. The document's four rules for code that runs there are the lasting result: read the frame buffer address from VBE function 4F01h, do not rely on more than 2 MiB of video memory, do not rely on the exact mode list, and keep a banked path. A result from Staging is not automatically a result from js-dos, because the revision inside js-dos is not recorded.

### How each run ends

| Emulator | How the run ends | Who decides the status |
|---|---|---|
| QEMU | The kernel writes `0x10` or `0x11` to port `0xF4`, and QEMU exits with 33 or 35 | The guest, mapped by `native-run.sh` |
| Bochs | The runner polls the serial file for `boot complete` or a failure line | The transcript |
| 86Box | The container script polls the serial capture the same way | The transcript |
| DOSBox Staging | The program exits, and the `if errorlevel` ladder recovers its status | The program |
| js-dos | The page reads `DONE.TXT`, written by a batch file with the same ladder, from the emulated disk | The program |

## Why it is this way

- **More than one opinion.** QEMU with SeaBIOS was the first and only emulator for the native track until Spike G (PR #16), which asked whether the native boot path is "specific to QEMU and SeaBIOS". A second BIOS and a second CPU emulator find different bugs: the 1993 AMIBIOS found a real loader bug, and Bochs's 486 showed that QEMU's is lenient. The document's own caveat is that QEMU configurations (`-cpu pentium`, `isapc`, `q35`) "share SeaBIOS and QEMU's translator, so they are not independent evidence the way a different BIOS and a different CPU emulator are".
- **Docker, and nothing on the host.** Bochs and 86Box run only in Docker images the runner builds on first use (`desk-native-bochs:1`, a few minutes of compiling; `desk-native-86box:1`, about a minute and a half and 170 MB of downloads), because `AGENTS.md` forbids installing system packages to get a build working. QEMU falls back to Docker the same way.
- **Opt-in, not in CI.** The CI container has no Docker, so the Bochs and 86Box tests run only on a developer machine (`NATIVE_EXTRA=1 tools/check.sh` adds the Bochs test). The document records this as a limit, not a design preference.
- **Statuses that match.** All three native runners return 0, 1, 2, 124 and 3 the same way, so `tools/native-test.sh` runs against any of them through `DESK_NATIVE_RUNNER` and one set of expected transcripts serves all of them. The Bochs and 86Box runners decide from the transcript because they have no exit device (`docs/NATIVE-EMULATORS.md`, "How to run it").
- **A probe for the CPU.** Because QEMU's claim to be a 486 was found to be weaker than documented, the Bochs test begins by proving that this Bochs rejects instructions a 486 lacks, and `docs/NATIVE-EMULATORS.md` says the kernel must be kept to the Bochs 486's instruction set.
- **The 86Box limits are chosen.** A boot takes about 15 seconds, so only the normal boot and one fault run there, with three lines of the expected transcript rewritten for what a 1993 BIOS does (the A20 line and the two memory-map source lines).
- **A decision recorded, not made.** The state of `CR0` at entry depends on the BIOS: Bochs's legacy BIOS leaves the cache-disable bits set (`0x60000033` in the dump), SeaBIOS and the AMIBIOS clear them. The kernel never touches them, so on some BIOSes it runs with the cache off. The spike records enabling the cache as an open question, and the expected dumps accept either value with `{BITS:mask=value}` ([M5](chapter:m-boot)).

## The code

| Where | What it holds |
|---|---|
| [`tools/native-run.sh`](repo:tools/native-run.sh) | QEMU as a 486: arguments, the watchdog, the Docker fallback, the status mapping, `--inject` |
| [`tools/native-inject.py`](repo:tools/native-inject.py) | Types scripted keys and mouse motion into the QEMU monitor socket |
| [`tools/native-run-bochs.sh`](repo:tools/native-run-bochs.sh) | Writes a `bochsrc`, starts Bochs in Docker, polls the serial file |
| [`tools/native-run-86box.sh`](repo:tools/native-run-86box.sh) | Starts the 86Box container and reads back its status file |
| [`tools/86box/run-in-container.sh`](repo:tools/86box/run-in-container.sh) | Xvfb, 86Box, the pseudo-terminal capture and `xdotool` |
| [`tools/86box/86box.cfg`](repo:tools/86box/86box.cfg) | The `ami471` machine |
| [`tools/86box/ami471.nvr.hex`](repo:tools/86box/ami471.nvr.hex) | The 128-byte CMOS image |
| [`tools/native-test.sh`](repo:tools/native-test.sh) | The whole native check, runnable against any runner |
| [`tools/native-test-bochs.sh`](repo:tools/native-test-bochs.sh) | The CPU probes, then the whole check under Bochs |
| [`tools/native-test-86box.sh`](repo:tools/native-test-86box.sh) | A normal boot and a divide error under 86Box, with a rewritten transcript |
| [`tests/native/probe.asm`](repo:tests/native/probe.asm) | The one-sector probe that runs one instruction and reports `#UD` or not |
| [`tools/dosrun.sh`](repo:tools/dosrun.sh) | Headless DOSBox Staging: the dummy video driver, the errorlevel ladder, `DOS_AUTOTYPE` |
| [`tools/docker/`](repo:tools/docker/) | The four Dockerfiles: QEMU, Bochs, 86Box, DOS with Xvfb |
| [`tools/check-jsdos.sh`](repo:tools/check-jsdos.sh) | The opt-in js-dos check, with the driver in [`spikes/s5-jsdos/drive.mjs`](repo:spikes/s5-jsdos/drive.mjs) |

For depth: `docs/NATIVE-EMULATORS.md` ([results](doc:docs/NATIVE-EMULATORS.md#results), [Bochs](doc:docs/NATIVE-EMULATORS.md#bochs), [86Box](doc:docs/NATIVE-EMULATORS.md#86box), [what is not verified](doc:docs/NATIVE-EMULATORS.md#what-is-not-verified)), `docs/NATIVE-BOOT.md` ([running it and the CPU model](doc:docs/NATIVE-BOOT.md#running-it-and-the-cpu-model)) and `docs/S5-JSDOS.md` ([verdict](doc:docs/S5-JSDOS.md#verdict-and-recommendation), [what was and was not verified](doc:docs/S5-JSDOS.md#what-was-and-was-not-verified)).

## The results, as the document records them

The next table is copied from `docs/NATIVE-EMULATORS.md` ("Results"), which is its source and its citation. All runs boot `dist/desk-native.img` to `boot complete` and compare the transcript with `tests/native/expected.txt`, which tolerates the BIOS-dependent lines. Only the QEMU reference row was rerun for this chapter; the others rest on the document and on the spike's pull request (#16).

| Emulator | BIOS | CPU model | Result | Differences from the QEMU transcript |
|---|---|---|---|---|
| QEMU 11.1 `-cpu 486` | SeaBIOS (QEMU's) | QEMU 486 | pass, the reference | E820, 6 entries, 15,871 KiB usable |
| QEMU `-cpu pentium`, `-machine q35`, `-machine isapc` | SeaBIOS | Pentium, 486 | pass | `isapc` has 5 entries and 15,999 KiB; `q35` has 8 entries and 15,867 KiB |
| Bochs 2.7 as a 486 (`--enable-cpu-level=4`) | `BIOS-bochs-legacy` | 486DX (`486ni` only) | pass, the full test (loader variants, faults, hang) | E820, 5 entries, 15,996 KiB; A20 already on |
| Bochs 2.7 as a 486 | `BIOS-bochs-latest` | as above | pass, the full test | E820, 6 entries, the last one "ACPI reclaimable" (kept reserved by the kernel), 15,932 KiB |
| Bochs 2.7 as a 486 | SeaBIOS 1.13 (shipped with Bochs) | as above | pass, the full test | E820, 6 entries, 15,991 KiB |
| 86Box 6.0, `ami471` (SiS 471, 33 MHz 486DX, 16 MiB) | AMIBIOS 08/08/93 | 486DX | pass after a loader fix; `tools/native-test-86box.sh` runs the normal boot and a divide error | A20 closed at boot (the loader opens it through the fast gate); neither E820 nor E801, so the AH=88 fallback runs, 2 entries, 16,000 KiB; the BIOS enables the cache, so the CR0 dump is `0x00000033` as on QEMU |

The document also lists a Bochs 2.7 row from Ubuntu's package with a Pentium model that passes by hand with the legacy BIOS and whose latest BIOS crashes before the boot sector; it is left out above because it is not a 486 and the build in the repository does not use it.

## Worked example

**An exit status, from the guest to the shell.** Take the divide-error mode of [M5](chapter:m-boot). The kernel executes `div` with a zero divisor, prints the register dump, and writes `0x11` to port `0xF4`. QEMU turns that into a process exit of `(0x11 << 1) | 1 = 35`. Run directly, QEMU's own status is exactly that, and `native-run.sh` maps 35 to its own 1:

```text
$ qemu-system-i386 -cpu 486 -m 16 ... -device isa-debug-exit,iobase=0xf4,iosize=0x04 -no-reboot
qemu exit 35
$ tools/native-run.sh dist/n-div0.img
native-run: guest reported failure          (exit 1)
```

Under Bochs there is no 35. The same kernel prints the same dump, ending `halting with a failure status`, and writes to a port nothing listens on. The runner has been polling the serial file: it sees a line starting `EXCEPTION 0x`, then five polls (one second) in which the file does not grow, and returns 1 with `native-run-bochs: guest reported failure`. Under 86Box the container script applies the same rule and writes `1` to `/vm/status`, which the host script reads and returns. And if the kernel had hung instead, the three runners would all end it by their timeout and return 124. That is why `tools/native-test.sh` can run unchanged against any of them: it wants, for the divide error, exactly exit 1 and a transcript that matches `tests/native/expected-div0.txt`.

For the DOS emulators the same job is done another way. `tools/dosrun.sh dist DESK.EXE panic` makes a batch file with 255 lines `if errorlevel n set RC=n`. If the program exits with 7, the lines for 1 to 7 set `RC` and the lines for 8 and above do not, so `RC` ends as 7, and the script exits 7. The js-dos spike checked this in the browser: `HELLO.EXE exit 7` read back as 7 in js-dos and 7 natively.

## State of this layer

```status
id: emu.qemu
label: QEMU as a 486 boots the native image, reports through the debug-exit port and ends hung runs by a watchdog
state: built-verified
evidence: sym:tools/native-run.sh::isa-debug-exit, test:tools/native-test.sh::hang, test:tests/native/expected.txt::boot complete
note: Rerun for this chapter on the host: a normal boot exits 0, a divide error 1, a hang 124.

id: emu.cpu-lenient
label: QEMU's -cpu 486 executes rdtsc and rejects cmov
state: built-verified
evidence: test:tests/native/probe.asm::probe: #UD, repo:docs/NATIVE-EMULATORS.md
note: Rerun for this chapter with the probe on QEMU 11.1.2. The Bochs half of the table is copied from the document.

id: emu.bochs
label: The native image passes in Bochs 2.7 as a 486 with three BIOS images, including the full test
state: built-unverified
evidence: pr:16, test:tools/native-test-bochs.sh::rdtsc, doc:docs/NATIVE-EMULATORS.md#results
note: Run by hand on a developer machine and recorded in the document; this chapter did not rerun it (the Docker image takes minutes to build), and CI does not run it.

id: emu.86box
label: The image boots in 86Box on an AMIBIOS 486 board, and the AH=88 fallback found there is tested
state: built-unverified
evidence: pr:16, test:tools/native-test-86box.sh::div0, doc:docs/NATIVE-EMULATORS.md#86box
note: Run by hand and recorded in the document; not rerun here, and the screenshot the runner takes is not in the pull request. Only a normal boot and a divide error run.

id: emu.dosbox
label: Headless DOSBox Staging runs the DOS programs and returns their exit status
state: built-verified
evidence: sym:tools/dosrun.sh::errorlevel, test:shell/dos/check.sh::DOS_AUTOTYPE
note: The CI target for the hosted track according to docs/DIRECTION.md. Not rerun for this chapter: DOSBox is fetched by tools/setup.sh and was not set up in this worktree.

id: emu.jsdos
label: js-dos 8 runs the repository's DOS programs in a browser, with the linear frame buffer
state: built-unverified
evidence: pr:13, repo:docs/S5-JSDOS.md, repo:tools/check-jsdos.sh
note: Run by the spike in headless Chromium 154 only and recorded in the document, with screenshots and transcripts in spikes/s5-jsdos/evidence/. Opt-in; not in CI; other browsers and sound playback are untried.

id: emu.dosboxboot
label: Booting the native image in DOSBox or js-dos
state: planned
evidence: doc:docs/DIRECTION.md#emulation-and-test-matrix
note: The document says they emulate DOS, not PC hardware; the repository has not tried the DOSBox boot command.

id: emu.v86
label: The native image in the browser through v86
state: planned
evidence: doc:docs/DIRECTION.md#emulation-and-test-matrix

id: emu.hardware
label: Any run on a real 486, a real floppy drive, a real BIOS or a real monitor
state: planned
evidence: doc:docs/NATIVE-EMULATORS.md#what-is-not-verified
note: Nothing in this repository has run on real hardware.
```

QEMU is {{state.emu.qemu}} and DOSBox {{state.emu.dosbox}}, which are the two the continuous checks use. The two other native emulators are {{state.emu.bochs}} and {{state.emu.86box}} and the browser is {{state.emu.jsdos}}: each passed by hand on a developer machine and was written up, and none runs in CI. A real machine is {{state.emu.hardware}}. The document's own words are the right summary: "Nothing here has run on real hardware", and no emulator is cycle-accurate, so no cache or timing behaviour has been checked.

> **From the stacks — What Bochs is.** The Wikipedia article calls Bochs a portable IA-32 and x86-64 IBM PC compatible emulator and debugger, mostly written in C++ and free software under the GNU Lesser General Public License. It says the emulator supports the processor, including protected mode, along with memory, disks, display, Ethernet, the BIOS and common PC peripherals. According to the article, Kevin Lawton wrote it starting in 1994, and Mandrakesoft bought it in March 2000 and released it for Linux under the LGPL. The Desk uses it for a different reason: a build configured as a 486 rejects instructions that QEMU's 486 does not.
> Source: [Bochs, Wikipedia](https://en.wikipedia.org/wiki/Bochs)

## Try it yourself

These commands need QEMU and NASM (the image is built by the native-track scripts, which use Cargo). They build the image, boot it, and run the two CPU probes. Output is from a run on 2026-10-23. I did not run `tools/native-test-bochs.sh`, `tools/native-run-bochs.sh`, `tools/native-run-86box.sh` or `tools/native-test-86box.sh`: they need the Docker images `desk-native-bochs:1` and `desk-native-86box:1`, which are not on this machine, and building them compiles Bochs and downloads about 170 MB. `tools/dosrun.sh` needs DOSBox from `tools/setup.sh`, which was not set up here.

```try
needs: host: bash, nasm, cargo, qemu-system-i386
ran: 2026-10-23 on the host, QEMU 11.1.2, NASM 3.02
$ tools/native-build.sh > /dev/null && tools/native-image.sh > /dev/null && tools/native-run.sh --serial dist/serial.txt > /dev/null; echo "exit $?"
native-run: guest reported success
exit 0
$ tools/native-image.sh --test-mode div0 --out dist/n-div0.img > /dev/null; qemu-system-i386 -cpu 486 -m 16 -drive if=floppy,format=raw,file=dist/n-div0.img -display none -serial file:dist/q.txt -device isa-debug-exit,iobase=0xf4,iosize=0x04 -no-reboot -monitor none; echo "qemu exit $?"
qemu exit 35
$ tools/native-run.sh dist/n-div0.img > /dev/null; echo "native-run exit $?"
native-run: guest reported failure
native-run exit 1
$ tools/native-image.sh --test-mode hang --out dist/n-hang.img > /dev/null; tools/native-run.sh --timeout 5 dist/n-hang.img > /dev/null 2>&1; echo "exit $?"
exit 124
$ nasm -f bin '-DOPC=db 0x0f, 0x31' tests/native/probe.asm -o dist/rdtsc.bin; tools/native-run.sh --timeout 3 dist/rdtsc.bin 2>&1 | tr -d '\r'
qemu-system-i386: terminating on signal 15 from pid 470797 (timeout)
probe: executed
boot complete
native-run: timed out after 3s
$ nasm -f bin '-DOPC=db 0x0f, 0x44, 0xc0' tests/native/probe.asm -o dist/cmov.bin; tools/native-run.sh --timeout 3 dist/cmov.bin 2>&1 | tr -d '\r'
qemu-system-i386: terminating on signal 15 from pid 470928 (timeout)
probe: #UD
boot complete
native-run: timed out after 3s
```

The first command is the run of [M5](chapter:m-boot). The second shows QEMU's raw status, 35, and the third the mapped status, 1. The fourth is the watchdog: the hung kernel never reports, so the run ends at the timeout with 124. The last two are the probe, and the "timed out" lines are expected: the probe prints and halts, and does not write to the exit port, so `native-run.sh` can only end it by the timeout (`rdtsc` executes under QEMU's 486, `cmov` raises `#UD`). The process ids in the first line will differ on your machine. The Bochs commands, if you have Docker and a few minutes, are `tools/native-test-bochs.sh` (about 20 seconds once the image exists) and `tools/native-run-86box.sh --screenshot out.png`.

**Exercises.** (1) Without running it, predict what `probe: ...` prints for `mov eax, cr4` (`db 0x0f, 0x20, 0xe0`) under QEMU's 486 and under Bochs's, then check the QEMU half. (2) Without running it, say what each of the three native runners returns for a kernel that stops with interrupts off and writes nothing, and what the QEMU watchdog's `timeout` makes of the process. (3) Read `tools/dosrun.sh` and say what its exit status is for a program that exits with 3, and why the batch file has 255 `if errorlevel` lines and not a loop.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Emulator
def: A program that behaves like a computer closely enough to run software written for it. Every run in this manual is on an emulator or on the host; the repository has never run on a real machine.

term: BIOS image
def: The firmware a PC runs first, as a file an emulator loads. Bochs ships three, QEMU uses SeaBIOS, and 86Box uses a 1993 AMIBIOS; they answer the interrupt calls a loader depends on, in slightly different ways.

term: Headless
def: Run with no display. DOSBox is headless through SDL's dummy video driver, QEMU through -display none; the result is read from files and exit statuses.
```

```figures
id: m20-qemu
reuse: qemu-boot
at: figure qemu
```

## Revisions

- 2026-10-23: first written, against `44f0265`. The QEMU runs and the two CPU probes in the try block were run on the host; the Bochs, 86Box, DOSBox Staging and js-dos results are copied from `docs/NATIVE-EMULATORS.md` and `docs/S5-JSDOS.md` and were not rerun. The results table is copied from `docs/NATIVE-EMULATORS.md`. Found while writing: a comment in `tools/native-run.sh` says `-cpu 486` keeps the guest to 486 instructions with "no CPUID, no TSC, no MMX or SSE", while `docs/NATIVE-EMULATORS.md` and the probe run for this chapter show that QEMU's 486 executes `rdtsc` (and `cpuid`, `rdmsr`, MMX and CR4 reads, per the document); `tools/native-run-86box.sh` says a boot takes about 20 seconds where `docs/NATIVE-EMULATORS.md` says about 15; and `docs/DIRECTION.md` says of Bochs that "its debugger helps with boot code" while the repository's Bochs is built with `--disable-debugger`.
