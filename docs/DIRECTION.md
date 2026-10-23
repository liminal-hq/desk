# Direction: an operating system in Rust for old hardware

> **Status:** a decision and a plan, recorded on 2026-10-22. Nothing here has been built, booted or measured, and no native code exists. Every number and claim is a target until a spike confirms it, and when code and this document disagree, the code wins and this document is corrected in the same branch.

## The vision

Desk is an operating system, written in Rust, for old hardware: 486-class PCs with VGA or VBE graphics, a Sound Blaster and OPL sound, a PS/2 keyboard and mouse, and floppy and IDE storage. It is graphical and calm. The fonts, the emoji, the palette, the app guidelines and the file formats designed so far are for this machine and carry over unchanged.

Until this decision the documents described Desk as a graphical shell for MS-DOS. That shell is still being built, but as one of two tracks rather than as the whole project.

## Two tracks in parallel

| | Hosted track | Native track |
|---|---|---|
| What it is | Desk running on top of DOS | A bootable Rust kernel that runs Desk on bare metal |
| Toolchain | Rust (`no_std`), C with DJGPP, CWSDPMI as the DPMI host | Rust (`no_std`) and a little assembly; no DJGPP, no CWSDPMI |
| Hardware access | DOS, the BIOS and DJGPP's runtime | Our own boot code, kernel and drivers |
| Why | Fast iteration on the compositor, widgets, apps, SDK and module loading, with DOSBox in CI | The actual goal: an operating system |
| Tested in | DOSBox Staging headless, then js-dos in the browser | QEMU, 86Box and Bochs; v86 in the browser |
| Spec | `SPEC.md`, the detailed spec until a native spec exists | This document, until a native spec exists |

The hosted track is what the repository has been building toward so far: the walking skeleton (pull request #3) and the DXE3 spike. It gives quick feedback on everything above the hardware. The native track starts small and grows; the hosted track keeps the shared core honest and gives the design somewhere to run in the meantime.

### How they share a core

The OS-independent Desk core (compositor, window manager, widgets, palette, fonts, event loop, app host) calls the machine only through a small hardware abstraction layer (HAL). Each track supplies its own HAL. The core must not call DOS or DJGPP APIs directly.

| Layer | Hosted track | Native track |
|---|---|---|
| Apps (modules) and the SDK | Shared | Shared |
| Desk core: compositor, windows, widgets, fonts, palette | Shared | Shared |
| HAL: display, input, timer, sound, storage, memory, module loading | Implemented over DOS, the BIOS, VBE, INT 33h and DJGPP | Implemented over our own drivers |
| Drivers (VGA and VBE, keyboard, PIT, OPL, Sound Blaster), written once against a small port-I/O and IRQ abstraction | Shared (target) | Shared (target) |
| Kernel and runtime | DOS with CWSDPMI, and a Rust runtime crate instead of the C layer and DJGPP's libc (target) | Rust kernel: memory, interrupts, scheduling, filesystem |
| Boot | `DESK.COM` from the DOS prompt | Own boot sector or loader |

The HAL surface is not designed yet. It should be as small as the core's real needs allow, and it should be written against the first two consumers (the hosted skeleton and the native walking skeleton) rather than in the abstract. A native bench build of the core (the `i586-unknown-linux-gnu` bench in `SPEC.md`) is a third consumer that costs little and is worth keeping.

## What each track delivers and how it is tested

**Hosted track.** A shell that runs under DOS: compositor, windows, launcher bar, cursor and keyboard focus, then apps loaded as modules, then the handoff to DOS programs, then the core apps. Tested headless in DOSBox Staging with frame checksums, and later in the browser through js-dos. DOSBox and js-dos emulate DOS, not PC hardware, so they can run only this track.

**Native track.** A kernel that boots, draws, takes input, reads a disk, loads a module and runs the shared core. Tested in QEMU with `-cpu 486`, in 86Box and in Bochs, which emulate PC hardware. For the browser, an x86 emulator such as v86 can boot the same disk image. Emulators are not real hardware and each has its own inaccuracies; see the risks below.

## Rust for DOS: shared machinery

The Desk is the product layer: a shell with apps above whatever sits below it. The owner wants a path for more native machinery below the shell, so that a future DOS game such as Lieutenant Fizz Episode 2 could be a plain native Rust DOS program instead of the pipeline Episode 1 needed: a gcc-built C layer, `coff2go32.py` patching rustc's COFF objects, and DJGPP's libc. None of this exists yet beyond the hosted skeleton and the spike on other branches; all of it is a target, and the extender in item 3 is research.

1. **A Rust compile target for 32-bit DOS programs.** Today that is `core/dos.json` on the hosted-skeleton branch (pull request #3). A published prebuilt sysroot or an upstream Tier 3 target are ideas only.
2. **A Rust runtime crate** that replaces the C layer and DJGPP's libc: startup, a DPMI client (INT 31h) for memory and real-mode calls, INT 21h file and console I/O, and drivers for VGA and VBE, the keyboard, the PIT timer, OPL and the Sound Blaster.
3. **A small packer** that wraps a linked Rust program for CWSDPMI. Much later, and only if CWSDPMI becomes a constraint, an extender of our own could replace it; that would need VCPI, XMS and DPMI-compatibility work, and is research, not planned.
4. **Drivers written once** against a small port-I/O and IRQ abstraction, so Lieutenant Fizz Episode 2 on DOS, the Desk hosted on DOS and the Desk native kernel can share them. This sits beside the HAL: the HAL is what the core needs, the abstraction is what the drivers need.
5. **Its own repository.** Once stable, this machinery may belong in its own repository (for example `liminal-hq/dos-rs`). It incubates in this repository until then.
6. **The d0 Rust-DOS spike** (hosted track, planned): a Rust-only DOS program with its own `_start`, DPMI wrappers and INT 21h output, linked by `lld`, packed with CWSDPMI's stub and run headless in DOSBox. If it works, the hosted skeleton's C layer and `coff2go32.py` can be retired.

[`RUST-TARGETS.md`](RUST-TARGETS.md) tracks how Rust is built for each application space.

The s1-dxe3 result is in (pull request #4, on the spike branch, `docs/S1-DXE3.md`): yes, with limits. It is a hosted-track result. A native module loader would need the same properties: symbol import, per-module state, panic containment and clean unload.

## DOS programs: bundle FreeDOS

The native OS will run DOS programs by bundling FreeDOS, either booting into it or running it in a V86 task (the design is later), rather than reimplementing DOS. This is a later milestone.

What it implies:

- Desk does not need to implement the DOS API, the BIOS interfaces DOS programs expect, or the quirks of real-mode games. FreeDOS already does.
- The old idea of handing off to a DOS program and coming back (`HANDOFF.md`) is reinterpreted. In the hosted track it stays exactly as designed (`DESK.COM`, `DESK.RUN`, `/RESUME`). In the native track, running a DOS program means leaving the native OS for FreeDOS or running FreeDOS beside it, and how the Desk state is saved and restored is an open question.
- FreeDOS is a bundle of programs under different licences, mostly the GPL. What ships, in what form, and what that means for the project's Apache-2.0 OR MIT licence must be checked before a release carries it; `NOTICE` will list it.
- A V86 task needs a 386 or later and a monitor that fakes the screen, keyboard and sound hardware the program expects. `SPEC.md` calls this a much larger project when discussing a DOS box in a window. Booting into FreeDOS is the cheaper first form.

## Emulation and test matrix

| Emulator | Hosted track | Native track | Notes |
|---|---|---|---|
| DOSBox Staging, headless | Yes | No | Emulates DOS, not PC hardware; the CI target for the hosted track |
| js-dos (browser) | Yes | No | DOSBox in the browser; the programs and the VBE linear frame buffer work in headless Chromium (`S5-JSDOS.md`) |
| QEMU, `-cpu 486` | Only by booting a DOS image | Yes | The first target for the native track; scriptable and headless |
| 86Box | Optional | Yes | Closer to period hardware, including specific sound and video cards |
| Bochs | Optional | Yes | Slow but careful; its debugger helps with boot code |
| v86 (browser) | No | Yes, later | An x86 emulator in the browser that boots a disk image |
| Real 486 | Later | Later | No real hardware is in the loop yet |

## What stays valid and what changes

**Stays valid:**

- The fonts (`.DFN`), the emoji, the palette (`PALETTE.md`), the design language (`HIG.md`) and the demo format (`.DEM`).
- The look, the apps, the interaction design and the keyboard map in `SPEC.md`.
- The SDK ABI shape (`SDK.md`): integers, pointers and lengths across the boundary, `desk_*` functions, apps that only talk to the Desk through the SDK.
- The file formats in `FORMATS.md` that describe apps, packages, icons and settings.

**Changes:**

- CWSDPMI and DJGPP stop being the foundation. They remain the foundation of the hosted track only.
- `DESK.COM`, `DESK.RUN` and the handoff are hosted-track features.
- 8.3 file names are a DOS limit. The native filesystem may or may not keep them; FAT12 and FAT16 do.
- The module format may become our own. DXE3 is what the hosted track uses and what the DXE3 spike tests; the native loader needs a format it can load without DJGPP's runtime, and ideally one both tracks share.
- Memory and crash behaviour in `SPEC.md` (CWSDPMI swapping, an app that panics being unloaded) describe the hosted track. The native kernel decides its own.

`SPEC.md` remains the detailed spec of the hosted track and of the OS-independent parts until a native spec is written.

## Open questions

These are new with the decision. `OPEN-QUESTIONS.md` lists them under "Still open".

- **Scheduling.** Cooperative in a single address space, as the hosted shell is, or preemptive processes? Cooperative is simpler and matches the hosted core; preemption protects against a hung app.
- **Memory protection.** Flat and shared, or per-app address spaces on the 486's MMU? This decides how apps are isolated and what a crashed app can damage.
- **App isolation.** What an app may touch, and whether a module is trusted code or a sandboxed process.
- **Filesystem.** FAT12 and FAT16 for floppies and for sharing with FreeDOS, and perhaps a filesystem of our own for the system volume.
- **Boot media.** A bootable floppy, an IDE disk image, or both, and whether the loader is ours or an existing one.
- **Early disk access.** BIOS calls (INT 13h) in the loader versus own floppy and IDE drivers in the kernel.
- **Real-mode switching.** Whether the protected-mode kernel ever goes back to real mode or V86 to call the BIOS (for VBE, for disks), and what that costs in complexity.
- **Graphics driver strategy.** VBE set up through the BIOS in real mode before switching to protected mode, V86 calls into the video BIOS, or native drivers for specific cards. Each trades simplicity against flexibility, such as changing mode later.
- **Sound hardware detection.** Finding a Sound Blaster, OPL chip or MPU-401 without `BLASTER` and without DOS, and handling ISA DMA and IRQs.
- **Input.** Our own PS/2 keyboard and mouse drivers, and whether USB is ever in scope (not for the 486 target).
- **Launching DOS programs and FreeDOS.** Booting into it, running it in a V86 task, or both; how state is saved; how the screen and sound are shared.
- **Licensing of FreeDOS components.** What is bundled and how, relative to our own licence.
- **The HAL.** Its surface, and whether the module loader belongs behind it.

## Risks

- **Hardware diversity.** Video cards, sound cards and chipsets of the period differ in ways the BIOS used to hide. A native OS carries that cost itself. The first targets are the ones the emulators model well.
- **No real hardware in the loop.** Nothing here has run on a real 486. Emulators will pass things real machines reject, and the reverse.
- **Emulator accuracy.** QEMU, 86Box and Bochs disagree with each other and with real parts in timing, DMA, VGA register behaviour and sound. A result in one is not a claim about hardware.
- **Scope.** An operating system is much more than a shell. The two-track plan limits the risk by keeping the hosted track useful on its own, but effort is split.
- **Divergence.** If the HAL is wrong, the two tracks drift apart. The shared core is only shared if it is built against both.

## Milestones

Small, testable and in order. Each track's list starts from what exists.

**Hosted track**

1. The walking skeleton (open as PR #3 at the time of writing): a build that runs headless in DOSBox and produces a frame.
2. The DXE3 spike (s1-dxe3; the result is in, pull request #4: yes, with limits): a Rust module loaded into the core, calling back into it.
3. **d0 Rust-DOS** (done, pull request #7: yes): a Rust-only DOS program with its own runtime, linked by `rust-lld` and packed by a small script, runs headless in DOSBox under CWSDPMI with no gcc, libc or crt0; retiring the C layer and `coff2go32.py` is the next decision.
4. VBE and mode 12h from Rust, and the mouse (the other spikes in `spikes/`), moving into the shared Rust drivers if d0 works.
5. Shell: compositor, windows, launcher bar, cursor, keyboard focus.
6. The handoff to DOS programs and back.
7. Core apps, then optional apps, then release (see `SPEC.md#milestones`).

**Native track**

1. **n0 boot:** a native walking skeleton that boots a Rust kernel in QEMU (`-cpu 486`) and prints to the screen or serial port.
2. Protected mode, interrupts, a timer and a memory map.
3. VGA or VBE graphics and a frame drawn by the shared compositor through a native HAL.
4. PS/2 keyboard and mouse.
5. Floppy or IDE reads and a filesystem the kernel can read.
6. A module loader, and an app loaded from disk.
7. The shared Desk core running on the native HAL with the core apps.
8. Sound (OPL and Sound Blaster).
9. The same image booting in 86Box and Bochs, and in the browser through v86.
10. FreeDOS bundled, and a DOS program launched from the Desk.
11. A real 486, when one is available.

Across both tracks, the shared drivers and the runtime crate (see "Rust for DOS: shared machinery") may be moved to their own repository once stable.

The order of the later native steps is expected to change with what the first ones teach.
