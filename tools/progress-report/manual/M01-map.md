---
id: m-map
n: M1
part: I
title: The map
summary: The two tracks, the four kinds of code, which directory is which layer, what is built and what is not, and how to read the state tags.
date: 2026-10-23
reviewed: 44f0265
watches: [docs/DIRECTION.md, docs/HAL.md, docs/FIRST-WINDOW.md, tools/progress-report/anatomy-desk.json, tools/progress-report/anatomy-dos.json, hal/, shell/, kernel/, spikes/d0-rust-dos/rt/]
log: [pivot, hal, firstwindow]
anatomy: [desk:appmodel, desk:sdk, desk:winown, desk:syscalls, desk:loader, desk:process, desk:wm, desk:shell, desk:ui, desk:gfx, desk:seam, desk:present, desk:hal, desk:drivers, desk:live, desk:native, desk:dos, desk:boot, desk:kernel, desk:freedos, desk:emu, desk:real]
---
## What it is

Desk is an operating system, written in Rust, for old hardware: 486-class PCs with VGA or VBE graphics, a PS/2 keyboard and mouse, and floppy storage. It is graphical and calm. That is the goal, and this manual is about how much of it exists. The honest answer is that a good deal sits at the bottom, a small and well-tested piece sits in the middle, and nothing sits at the top: there is no application, no system call and no way to load a program yet.

This chapter is the map the rest of the manual is drawn on. It says what the [hosted track](term:hosted-track) and the [native track](term:native-track) are, what the four kinds of code are and where each lives in the repository, which of the 22 boxes of the [anatomy page](chapter:anatomydesk) are built and which are only planned, and how to read the [state tags](term:state-tag) this manual puts on every mechanism. After it you should be able to open any directory in the repository and say which layer of the system it belongs to, and to tell, for any sentence in the manual, whether it describes code that has run or code that has only been written.

The chapters that follow go from the processor upward. [M2](chapter:m-cpu) teaches the processor facts the boot code depends on, and [M3](chapter:m-devices) the devices of a 486 PC. The boot chapters ([M4](chapter:m-loader), [M5](chapter:m-boot)) follow the machine from power-on to the kernel. Then come the kernel, the HAL and its drivers, the shared core, the platforms and the tools. Each chapter ends with a "Where next" box that gives the next chapter in two orders: "Learn it", from the hardware up, and "Find it", from the shell down.

## How it works

### Two tracks, one core

The repository builds a hosted track and a native track, and both run one [shared core](term:shared-core).

The direction was decided on 2026-10-22 and is recorded in `docs/DIRECTION.md`: build the system on two tracks in parallel and share what can be shared.

| | Hosted track | Native track |
|---|---|---|
| What it is | Desk running on top of DOS | A bootable Rust kernel that runs Desk on bare metal |
| Hardware access | DOS, the BIOS and a DPMI host (CWSDPMI) | Our own boot code, kernel and drivers |
| Tested in | DOSBox Staging, headless | QEMU with `-cpu 486`, also booted in Bochs and 86Box |
| What runs today | `SHELL.EXE`, the first window | The kernel's `shell` test mode, the same first window |

The point of the hosted track is fast feedback on everything above the hardware: DOSBox emulates DOS, not a PC, so it can run only this track. The point of the native track is the actual goal. The risk of two tracks is that they drift apart, and `docs/DIRECTION.md` names it: if the hardware abstraction layer is wrong, the tracks diverge, and the shared core is only shared if it is built against both. The first window ([M15](chapter:m-shell), [M16](chapter:m-platforms)) is the first proof: one scenario, one set of crates, the same pixels on both tracks.

Underneath both is one fact about every PC of this kind: the processor starts in a 1980s mode and has to be switched into the one the Rust code needs. The hosted track inherits that switch from DOS and its extender. The native track makes it itself, in the loader, which is why [M2](chapter:m-cpu) comes before any code is read.

### The four kinds of code

`docs/HAL.md` ("The four kinds of code") sorts every piece of the shared design into one of four kinds, each with a rule about what it may call. The rule that holds the tracks together is that a line of device logic exists once, in a driver; a platform contains only what genuinely differs.

| Kind | What it is | May call | Must not call | Lives in |
|---|---|---|---|---|
| HAL | The traits and small shared types: the contract | `core` | Anything platform-specific | `hal/hal/` (`desk-hal`) |
| Driver | Device logic written once: register sequences, timing, decoding | `desk-hal`, `core` | DOS, BIOS, DPMI, kernel internals | `hal/drivers/*` |
| Platform | The HAL implemented for one environment | Its environment, drivers, `desk-hal` | The Desk core | `kernel/` (native), `spikes/d0-rust-dos/rt` (DOS), `hal/platform/mock/` (host tests) |
| Desk core | The OS-independent shell: windows, text, frames | The HAL's upper edge, `alloc` | Ports, DOS, kernel internals, drivers' register-level API | `shell/` |

One part of this table differs from `docs/HAL.md`, which names `core/` as the home of the Desk core. The code puts it in `shell/`; `core/` is the first hosted skeleton's one Rust function, kept until the owner decides to retire the C layer ([`docs/FIRST-WINDOW.md`](doc:docs/FIRST-WINDOW.md#where-it-sits)). The manual follows the code.

### Which directory is which layer

| Directory | Kind or layer | What is in it | Taught in |
|---|---|---|---|
| `boot/` | Boot | The two NASM stages: the boot sector and the loader | [M4](chapter:m-loader) |
| `kernel/` | Platform (native) and kernel | The Rust kernel: GDT, IDT, PIC and PIT set-up, memory, the BIOS trampoline, the native platform, test modes | [M5](chapter:m-boot) to [M8](chapter:m-bioscalls) |
| `hal/hal/` | HAL | `desk-hal`: ports, interrupts, time, rings, memory-mapped regions, video and input types | [M9](chapter:m-hal) |
| `hal/drivers/` | Drivers | `desk-serial`, `desk-pit`, `desk-pic`, `desk-vga`, `desk-kbd`, `desk-i8042`, `desk-int33` | [M10](chapter:m-sysdrivers) to [M12](chapter:m-graphics) |
| `hal/platform/mock/` | Platform (host) | A mock port bus and device models, so drivers are tested without a machine | [M9](chapter:m-hal) |
| `shell/` | Desk core | `desk-gfx`, `desk-font`, `desk-ui`, `desk-wm`, `desk-shell`, and `desk-present`, which the platforms use | [M13](chapter:m-pixels) to [M15](chapter:m-shell) |
| `shell/dos/` | Platform (DOS), program | `SHELL.EXE`, the hosted wiring of the first window | [M16](chapter:m-platforms) |
| `spikes/d0-rust-dos/` | Platform (DOS) | `rt`, the pure-Rust DOS runtime, the target and linker script, and the programs that proved them | [M17](chapter:m-rt) |
| `core/`, `c/`, `launcher/` | Legacy hosted skeleton | The first DOS program: Rust, C for DJGPP, and the real-mode launcher | [M17](chapter:m-rt) |
| `tools/`, `tests/` | Build and evidence | Build scripts, the packer, the emulator runners, expected transcripts and goldens | [M19](chapter:m-toolchain) to [M21](chapter:m-evidence) |
| `fonts/` | Assets | The bitmap and outline faces, the emoji, and the build that writes the formats | [M18](chapter:m-fonts) |
| `docs/` | Design | The working design; targets until a result says otherwise | |
| `design/`, `sdk/` | Reference and plan | Boards from the design brainstorm (reference only), and the sketch of the app interface | [M22](chapter:m-frontier) |

The dependency rules are not only prose. Every crate is a Cargo path dependency, so the direction of each edge can be read from the manifests; the "Try it yourself" section shows one check. For instance, `desk-present` is not a dependency of any core crate except as a test-only one: `desk-shell` lists it under `[dev-dependencies]`, and the kernel and `SHELL.EXE` list it as a real one.

### What is built

The anatomy page of the native track (`tools/progress-report/anatomy-desk.json`) draws the whole system as 22 boxes in eight layers. This chapter does not redraw it; the figures below are the same drawings the log uses. The architecture diagram is the plan, the anatomy pages are the inventory.

<!-- figure arch -->

<!-- figure anatomy-desk -->

The top three layers are planned, and the diagram says so: an app model, the SDK ABI and windows owned by apps; system calls, an executable format and loader, and tasks, scheduling and memory protection. `docs/DIRECTION.md` lists the questions that decide them (cooperative or preemptive, one address space or several) under "Open questions", and [M22](chapter:m-frontier) says what each would need from the layers below. The kernel today has no user mode: the chapter on the processor shows why that is a finding about the code and not a gap in the documents.

Below that the system is built. The Desk core is six crates; the seam between them and the platforms is that the platform owns the loop and calls `Shell::handle` and `Shell::frame`; the HAL and the drivers have host tests against a mock port bus; the two platforms run the same scenario; and the loader and kernel boot in QEMU. What is *not* built is also below that line: live keys and mouse reaching the shell (the drivers exist, but the runs hand the shell scripted events), and any real machine.

The second diagram is the hosted track, from the Rust source of `SHELL.EXE` through the runtime, the custom target and the packer, to DOS and the hardware it touches.

<!-- figure anatomy-dos -->

### How to read the state tags

Every mechanism the manual describes has one of four tags, set by a `status` record in the chapter and drawn in its "State of this layer" table. The prose uses the same record by name, so a sentence cannot claim more than the table does.

| Tag | Means |
|---|---|
| {{state.map.wm}} | Built, and something ran: a test, or a merged pull request whose CI passed. In an emulator or on the host, never on real hardware |
| {{state.map.live}} | Built, but no test or merged and passing pull request shows it running |
| {{state.map.real}} | Not built; described in a design document |

A fourth tag, "Not ours", marks code the project relies on and did not write, such as CWSDPMI and the BIOS.

The manual's rule is stricter than the anatomy diagram's. A box on the anatomy page is "verified" with any evidence, a path in the repository included. A manual record is built-verified only with run evidence, a `test:` item or a pull request that is merged with CI passing. Where the two disagree the manual shows the lower tag and the record's note says why; the status table below has one such case. The word "verified" never means more than an emulator or the host. Nothing in this repository has run on a real 486.

## Why it is this way

- **Two tracks rather than one.** The hosted track gives feedback on the compositor, windows and widgets while the native track grows from a boot sector; neither alone would give both (`docs/DIRECTION.md`, "Two tracks in parallel"). The cost is effort split in two, which the document lists among its risks.
- **A shared core behind a narrow seam.** The first window needed only a display and input, and both platforms already had those. So the seam is the platform owning the loop and calling two functions, not a general `Platform` trait; the trait is a later step once a second consumer needs it (`docs/FIRST-WINDOW.md`, "Where it sits"). Nothing in the code claims a `Platform` trait exists, and neither does this manual.
- **Four kinds, with rules, rather than one HAL.** Free functions over a global port bus are simplest but cannot be tested on the host, and one HAL per track would write every driver twice (`docs/HAL.md`, "The four kinds of code"). The kinds exist so that a driver can be tested against a mock bus and then run unchanged in the kernel and under DOS.
- **Platforms live with their environment.** The DOS platform is a module of `rt` and the native platform a module of the kernel, because each is made of that environment's own parts (DPMI calls and the interrupt stub; the IDT and the frame allocator). Only what builds for every target is in `hal/` (`docs/HAL.md`, "Crate layout").
- **No compare-and-swap anywhere in the shared code.** A 486 has `CMPXCHG`, but DOSBox's `cputype = 486` rejects it, so CI could not run code that used it (`docs/HAL.md`, "Decisions"). The rule is enforced by `hal/clippy.toml` and by `shell/clippy.toml`.
- **The code is the source of truth.** Where a document and the code disagree, the code wins and the document is corrected in the same branch (`AGENTS.md`). The manual applies the rule to itself, which is why this chapter lists its disagreements under "Revisions".

## The code

| Where | What it holds |
|---|---|
| [`tools/progress-report/anatomy-desk.json`](repo:tools/progress-report/anatomy-desk.json) | The native track's 22 boxes, their states and evidence; the status block below follows it box for box |
| [`tools/progress-report/anatomy-dos.json`](repo:tools/progress-report/anatomy-dos.json) | The hosted track's boxes for `SHELL.EXE` |
| [`hal/Cargo.toml`](repo:hal/Cargo.toml) | The HAL workspace: `desk-hal`, seven drivers and the mock |
| [`shell/Cargo.toml`](repo:shell/Cargo.toml) | The shared core's workspace: six crates; `dos/` is excluded and has its own |
| [`kernel/Cargo.toml`](repo:kernel/Cargo.toml) | The kernel, which depends on the HAL, the drivers and the core by path |
| [`shell/dos/Cargo.toml`](repo:shell/dos/Cargo.toml) | `SHELL.EXE`, which depends on `rt`, the HAL, the drivers and the core |
| [`spikes/d0-rust-dos/rt/Cargo.toml`](repo:spikes/d0-rust-dos/rt/Cargo.toml) | The DOS runtime, which depends on `desk-hal`, `desk-pic` and `desk-vga` |
| [`kernel/src/hal.rs`](repo:kernel/src/hal.rs) | The native platform: the port bus, the driver statics and the IRQ registry |
| [`spikes/d0-rust-dos/rt/src/hal.rs`](repo:spikes/d0-rust-dos/rt/src/hal.rs) | The DOS platform's implementations of the HAL's traits |
| [`hal/platform/mock/src/lib.rs`](repo:hal/platform/mock/src/lib.rs) | The host platform for driver tests |

For depth: `docs/DIRECTION.md` ([two tracks](doc:docs/DIRECTION.md#two-tracks-in-parallel) and [milestones](doc:docs/DIRECTION.md#milestones)), `docs/HAL.md` ([decisions](doc:docs/HAL.md#decisions) and [the four kinds of code](doc:docs/HAL.md#the-four-kinds-of-code)) and `docs/FIRST-WINDOW.md` ([where it sits](doc:docs/FIRST-WINDOW.md#where-it-sits) and [crate layout](doc:docs/FIRST-WINDOW.md#crate-layout)).

## Worked example

**Tracing one box to its evidence.** Take the box `emu`, "QEMU (-cpu 486) and DOSBox Staging, headless". The anatomy page draws it as verified, with the evidence `repo:docs/FIRST-WINDOW.md`. That is an honest summary of a fact: everything above it was run in these emulators. But the evidence is a document, and a document is not a run. So the manual's record for this box, `map.emu`, is built, not verified, and its note says so. Compare the box `native`, the kernel's `shell` test mode: its record cites `tests/native/expected-shell-8.txt`, whose lines (`shell: frame open 8 640x480 920263aa c310363f`) are what a run of the kernel prints and the CI compares. That record keeps the tag.

The two cases show how the tags are decided. Neither box is more or less true than the other; one cites something that can fail and one does not.

**A layer's size, counted.** The shared core is six crates in `shell/`, and the HAL workspace holds nine members: `desk-hal`, seven drivers and the mock. The host tests pass in both workspaces, 161 tests in `shell/` and 179 in `hal/` when this chapter was written (the commands are below). The log's anatomy chapter quotes the same 161 for the core.

**The edge that goes one way.** `desk-gfx` lists `desk-vga` only under `[dev-dependencies]`, because one of its tests compares the software cursor sprite with the VGA driver's arrow. In production code the core never depends on a driver's register-level API, as the rules require; the tests are allowed to look across.

## State of this layer

One record for each box of `anatomy-desk.json`, in the diagram's order and with the diagram's state, except where noted. "Verified" here means a test or a merged pull request whose CI passed, on the host or in QEMU or DOSBox, never on real hardware. Many pull requests are shown with their CI as unknown in the offline snapshot this chapter was built from, so most records cite a test instead.

```status
id: map.appmodel
label: An app model: what an app is and how it runs
state: planned
evidence: doc:docs/DIRECTION.md#open-questions, repo:docs/SDK.md

id: map.sdk
label: The SDK ABI (desk_*): integers, pointers and lengths
state: planned
evidence: doc:docs/SDK.md#an-apps-entry-points, repo:docs/SDK.md

id: map.winown
label: Windows owned by apps
state: planned
evidence: doc:docs/FIRST-WINDOW.md#where-it-sits
note: Today the scenario, run by the shell, opens both windows.

id: map.syscalls
label: System calls
state: planned
evidence: doc:docs/DIRECTION.md#open-questions
note: The kernel has no user mode, no task-state segment and no system-call gate; the processor chapter shows a search of the kernel's sources that found none.

id: map.loader
label: An executable format and loader for the native track
state: planned
evidence: doc:docs/DIRECTION.md#what-stays-valid-and-what-changes, repo:docs/S1-DXE3.md
note: The hosted track proved loading a Rust module with DJGPP's DXE3 format; a native loader needs a format that does not depend on DJGPP's runtime.

id: map.process
label: Tasks, scheduling and memory protection
state: planned
evidence: doc:docs/DIRECTION.md#open-questions

id: map.wm
label: desk-wm: windows, z-order, focus, hit testing and damage
state: built-verified
evidence: test:shell/wm/tests/behaviour.rs::hits_name_each_part_of_a_window, pr:19, repo:shell/wm
note: Host tests, and the first window's goldens on both platforms.

id: map.shell
label: desk-shell: events, frames and the scenario runner
state: built-verified
evidence: test:shell/shell/tests/accounting.rs::the_first_frame_writes_exactly_the_whole_screen, pr:19, repo:shell/shell

id: map.ui
label: desk-ui and desk-font: theme roles, chrome, text and the .DFN reader
state: built-verified
evidence: test:shell/ui/tests/ui.rs::the_constants_equal_the_hex_values_in_palette_md, pr:19, repo:shell/ui

id: map.gfx
label: desk-gfx: surfaces, damage, the compositor and digests
state: built-verified
evidence: test:shell/gfx/src/damage.rs::the_list_fills_to_sixteen_before_merging_the_cheapest_pair, pr:19, repo:shell/gfx

id: map.seam
label: The platform owns the loop and calls Shell::handle and Shell::frame
state: built-verified
evidence: test:tests/native/expected-shell-8.txt::shell: ready moved, test:tests/shell/expected-dos-8.txt::readback moved 8 640x480, repo:docs/FIRST-WINDOW.md
note: Both platforms drive the core through the same two calls, and their frames equal the same goldens. There is no Platform trait, by decision.

id: map.present
label: desk-present: the linear and planar presenters
state: built-verified
evidence: test:shell/present/tests/linear.rs::the_same_spans_give_the_same_bytes_as_memory_present, pr:19, repo:shell/present

id: map.hal
label: desk-hal: ports, interrupts, time, rings, regions, video and input types
state: built-verified
evidence: test:hal/hal/src/mmio.rs::accesses_are_bounded_and_aligned, pr:8, repo:docs/HAL.md
note: Host tests against the mock bus, and the kernel and the DOS runtime build against it.

id: map.drivers
label: Shared drivers: the 16550, 8254, 8259, 8042, keyboard decoder, VGA and INT 33h
state: built-verified
evidence: test:hal/drivers/pit/src/lib.rs::the_divisor_is_rounded_to_the_nearest_as_in_the_kernel, test:hal/drivers/pic/src/lib.rs::native_init_matches_the_kernel_with_its_delays, test:tests/native/expected.txt::pit: channel 0 at 100 Hz, repo:hal/drivers
note: Tested on the host against the mock, and the serial, PIC, PIT, 8042 and VGA drivers run in the kernel in QEMU.

id: map.live
label: Live input into the shell
state: built-unverified
evidence: repo:docs/FIRST-WINDOW.md
note: Both tracks have input drivers, but the comparison runs hand the shell scripted events. Only on DOS were keys typed through IRQ 1 into the focused field, by an interactive run in DOSBox; the mouse reached the shell on neither track, and nothing live reached the native shell. No test records it.

id: map.native
label: The native platform: the kernel's shell test mode in QEMU
state: built-verified
evidence: test:tests/native/expected-shell-8.txt::shell: frame open 8 640x480 920263aa c310363f, test:tests/native/expected-shell-4.txt::shell: ready, repo:kernel/src/shell.rs

id: map.dos
label: The hosted platform: SHELL.EXE in DOSBox
state: built-verified
evidence: test:tests/shell/expected-dos-8.txt::readback open 8 640x480 920263aa c310363f equal, test:tests/shell/expected-dos-4.txt::readback, repo:shell/dos

id: map.boot
label: The boot sector and loader
state: built-verified
evidence: pr:6, test:tests/native/expected.txt::boot complete, repo:boot/stage2.asm
note: Booted in QEMU; Bochs and 86Box are run by hand (see the loader chapter).

id: map.kernel
label: The kernel's services: GDT, IDT, frames, heap, timer and BIOS calls
state: built-verified
evidence: pr:6, test:tests/native/expected.txt::boot complete, test:tests/native/expected-bios.txt::bios, repo:kernel/src

id: map.freedos
label: FreeDOS for DOS programs
state: planned
evidence: doc:docs/DIRECTION.md#dos-programs-bundle-freedos

id: map.emu
label: QEMU (-cpu 486) and DOSBox Staging, headless
state: built-unverified
evidence: repo:docs/FIRST-WINDOW.md, repo:tools/native-run.sh, repo:tools/dosrun.sh
note: The anatomy diagram draws this box as verified on the strength of a document. It is the machine the other records ran on, but no test of its own cites it, so the manual's stricter rule shows it as built, not verified. The emulators chapter has the table of what each one showed.

id: map.real
label: A real 486 PC
state: planned
evidence: doc:docs/NATIVE-BOOT.md#what-was-and-was-not-verified
```

The shared core, the HAL, the loader and the kernel are {{state.map.kernel}} in an emulator or on the host, and their tests are the only evidence. The application layer is {{state.map.appmodel}}. A real machine is {{state.map.real}}, and no record in this manual says otherwise.

## Try it yourself

These commands read the map out of the repository. They need only `cargo` and a shell. Output is from a run on 2026-10-23 at `44f0265`; the test counts will change when tests are added.

```try
needs: host: bash, cargo (host tests only; no emulator)
ran: 2026-10-23 on the host
$ grep -h '^name = ' hal/hal/Cargo.toml hal/drivers/*/Cargo.toml hal/platform/*/Cargo.toml shell/*/Cargo.toml kernel/Cargo.toml | sort
name = "desk-font"
name = "desk-gfx"
name = "desk-hal"
name = "desk-hal-mock"
name = "desk-i8042"
name = "desk-int33"
name = "desk-kbd"
name = "desk-kernel"
name = "desk-pic"
name = "desk-pit"
name = "desk-present"
name = "desk-serial"
name = "desk-shell"
name = "desk-shell-dos"
name = "desk-ui"
name = "desk-vga"
name = "desk-wm"
name = "shell"
$ grep -n "desk-present" shell/gfx/Cargo.toml shell/font/Cargo.toml shell/ui/Cargo.toml shell/wm/Cargo.toml shell/shell/Cargo.toml shell/dos/Cargo.toml kernel/Cargo.toml
shell/shell/Cargo.toml:21:desk-present = { path = "../present", features = ["std"] }
shell/dos/Cargo.toml:24:desk-present = { path = "../present" }
kernel/Cargo.toml:21:desk-present = { path = "../shell/present" }
$ cd hal && cargo test 2>&1 | grep -c "^test .* ok$"
179
$ cd shell && cargo test 2>&1 | grep -c "^test .* ok$"
161
```

The first listing is the 17 packages of the three workspaces and the kernel, and the last line, `shell`, is the name of the program that `shell/dos` builds beside its package `desk-shell-dos`; the second shows that no core crate depends on the presenters except `desk-shell`, and that only in its tests (line 21 is under `[dev-dependencies]`). The two counts are the host tests of the two workspaces.

**Exercises.** (1) Without running anything, pick three directories of the repository and say which of the four kinds each is, then check against the table. (2) The `live` record is built-unverified. Say what a test would have to show to promote it, and which of the two tracks could show it first. (3) Find a second box of the anatomy page that cites only a document, say whether the manual's stricter rule would lower it, and check your answer against the status block.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Hosted track
def: Desk running on top of DOS, with the BIOS and a DPMI host underneath it; tested headless in DOSBox. It is where the shared core was built first.

term: Native track
def: The bootable Rust kernel that runs Desk on bare metal; tested in QEMU as a 486, and also booted in Bochs and 86Box.

term: Shared core
def: The OS-independent part of Desk, in shell/: graphics, text, window chrome, the window manager and the shell. It reaches the machine only through the HAL.

term: State tag
def: One of built-verified, built-unverified, planned or external, put on a mechanism by a status record. Built-verified needs a test or a merged pull request with CI passing, and never means more than an emulator or the host.
```

```figures
id: m1-arch
reuse: architecture
at: figure arch

id: m1-anatomy-desk
reuse: anatomy-desk
at: figure anatomy-desk

id: m1-anatomy-dos
reuse: anatomy-dos
at: figure anatomy-dos
```

## Revisions

- 2026-10-23: first written, against `44f0265`. The status block has one record for each of the 22 boxes of `anatomy-desk.json`; `map.emu` is lowered from the box's verified to built-unverified, and `map.live` follows the box. Disagreements found: `docs/HAL.md` and `AGENTS.md` name `core/` as the home of the Desk core where the code uses `shell/`; the Status paragraph at the top of `docs/HAL.md` and the comment in `hal/Cargo.toml` say nothing yet uses the HAL, while the kernel, `rt` and `SHELL.EXE` all depend on it (and the same document's "Crate layout" says so); `docs/HAL.md`'s "Today `hal/` holds" list omits `desk-vga` and `desk-int33`, which are workspace members; and `AGENTS.md` describes `shell/` as holding `desk-gfx` "so far", where it holds six crates.
