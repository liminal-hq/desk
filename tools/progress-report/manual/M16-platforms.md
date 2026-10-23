---
id: m-platforms
n: M16
part: VI
title: Two platforms, one core
summary: What a platform is, side by side: the native kernel's shell test mode and the DOS program SHELL.EXE compared line for line, who owns the loop, how a display and input are opened, how damage reaches the screen, how a run ends, and why there is no Platform trait yet.
date: 2026-10-23
reviewed: 44f0265
watches: [kernel/src/shell.rs, kernel/src/hal.rs, shell/dos/src/main.rs, docs/FIRST-WINDOW.md, docs/HAL.md, tools/shell-compare.sh, tools/shell-goldens.sh]
log: [firstwindow, oneevent]
anatomy: [desk:native, desk:dos, desk:seam, dos:main]
---
## What it is

The shared core ([M13](chapter:m-pixels) to [M15](chapter:m-shell)) draws pictures and knows nothing of any machine. Something has to give it a screen to draw on and events to react to, decide when a frame is due, and end the run. That something is a [platform](term:platform): the code of one environment that sets up the machine and calls the core. There are two that run the core today. On the [native track](term:native-track) it is the test mode `shell` in `kernel/src/shell.rs`, in a kernel that boots in QEMU. On the [hosted track](term:hosted-track) it is `shell/dos/src/main.rs`, which becomes `SHELL.EXE` and runs on DOS under CWSDPMI, headless in DOSBox. They are two programs, in two crates, built for two different targets, and they play the same file (`tests/shell/first-window.scn`) through the same crates and must produce pictures with the same colours in every pixel.

This chapter reads the two programs against each other. The boxes are `native` and `dos` on the [Desk anatomy page](chapter:anatomydesk#box-desk-native) and `main` on the [DOS anatomy page](chapter:anatomydos#box-dos-main), with the `seam` between them and the core ([M15](chapter:m-shell)). After it you should be able to say, for any step of a run, which of the two programs does it and with what; find the one place a platform has to change if you add a third; and explain why the repository has a sketch of a `Platform` trait in a design document and no such trait in any source file.

The honest limits come first. Both runs are in emulators. Nothing here ran on a real VGA card or a real 486. The input is scripted: neither run moved a real mouse, and the native run has no free-running loop at all. The two tracks' equality is a claim about six-bit colours in an emulated card's memory and in QEMU's rendering of it. [M21](chapter:m-evidence) says what such a claim is worth.

## How it works

### What a platform has to do

Whatever the environment, the work has the same six parts, and the table in the next section follows them in order.

1. **Be started.** Something gets the program running: the BIOS and the loader for the kernel, DOS and the DOS extender for `SHELL.EXE`.
2. **Open a display.** Choose a video mode, set it, load the palette, and make a `Present` over the frame buffer: `LinearPresent` for a 256-colour linear frame buffer, `PlanarPresent` for the four planes of mode 12h ([M12](chapter:m-graphics)).
3. **Get input.** Turn what the keyboard and mouse deliver into `Event`s ([M11](chapter:m-input)); or, in a scripted run, hand the scenario's own events to the shell.
4. **Run the loop.** Call `Shell::handle` for each event and `Shell::frame` with the presenter when a picture is wanted.
5. **Let the damage reach the screen.** This is not the platform's code, but it is the platform's responsibility to supply a presenter that writes the spans it is given, to the right place, and to say what alignment it needs.
6. **End.** Put the machine back (text mode, the keyboard, the mouse), and report success or failure in the way its environment understands.

The [HAL](term:hal) already covers the parts of this that touch devices: ports ([port bus](term:port-bus)), interrupts, time, the display and input traits ([M9](chapter:m-hal)). A platform, in the HAL's definition, is the HAL implemented for one environment. What is not covered is the *program* that uses it: who starts the core and runs its loop. That is the seam.

### The seam

The [seam](term:seam) is the narrow place where a platform meets the core. For the shell it is exactly this: the platform owns the [run loop](term:run-loop) and calls two methods, `Shell::handle` and `Shell::frame`; it may also use the scenario runner (`run`) or just the step function (`apply`) when a scenario drives it. The core calls nothing back. `docs/FIRST-WINDOW.md` ("Where it sits") says why it is that narrow: the first window needs a display and input, both platforms already have those, and folding the two calls into a general type is a later step.

The anatomy pages draw it as the box `seam` ("The platform owns the loop"), and the sentence is accurate in a sense the code makes concrete. In the kernel the loop is the shared runner's: `play` hands the whole scenario to `run_scenario` and the platform is called back for each frame. In the DOS program the loop is the platform's own: `scenario` is a `for` over the steps, and `interactive` is a `loop` that polls. So the shared runner and a platform-owned loop both exist, and the seam is satisfied by both because the core only ever sees `handle`, `frame` and `apply`.

<!-- figure anatomy-desk -->

<!-- figure anatomy-dos -->

### Side by side, line for line

Each row is one thing a platform does. The kernel column cites a symbol in `kernel/src/shell.rs` (or `kernel/src/hal.rs` where the row is about the machine's port bus and interrupts); the DOS column cites a symbol in `shell/dos/src/main.rs`. Every symbol is a link and is checked to exist.

| What | Native kernel (`shell` test mode) | DOS (`SHELL.EXE`) |
|---|---|---|
| Entry | [`run`](sym:kernel/src/shell.rs::run), called by the kernel's `input_section` when the test-mode byte is 13 or 14, after the keyboard and mouse are set up | [`main`](sym:shell/dos/src/main.rs::main), the `rt::entry!` program; arguments `depth8 depth4 scenario dump fontfile corrupt timeout=N` |
| Scenario and faces | [`SCENARIO`](sym:kernel/src/shell.rs::SCENARIO), [`UI_FONT`](sym:kernel/src/shell.rs::UI_FONT) and `TITLE_FONT`: `include_str!` and `include_bytes!` of the repository's files | [`SCENARIO`](sym:shell/dos/src/main.rs::SCENARIO), [`UI_DFN`](sym:shell/dos/src/main.rs::UI_DFN) and `TITLE_DFN`: the same files, embedded; `fontfile` reads `UI.DFN` and `TITLE.DFN` instead through [`load`](sym:shell/dos/src/main.rs::load) |
| Port bus | [`PORTS`](sym:kernel/src/hal.rs::PORTS), one `static` made by the one `unsafe` step in `hal.rs`, ring 0 | [`RawPorts`](sym:shell/dos/src/main.rs::RawPorts), made inside `interactive` with a `SAFETY` note: a DPMI client may use the controller's ports under CWSDPMI |
| Interrupts | [`IRQS`](sym:kernel/src/hal.rs::IRQS), the table the IDT stubs dispatch through | [`DosIrq`](sym:shell/dos/src/main.rs::DosIrq), which hooks IRQ 1 through DPMI, and [`KBD_SERVICE`](sym:shell/dos/src/main.rs::KBD_SERVICE), the handler's state in the image |
| Opening the display | [`run`](sym:kernel/src/shell.rs::run) takes what the loader left in `video.lfb`: a 640 by 480 linear frame buffer is depth 8, none is mode 12h by [`set_mode_12h`](sym:kernel/src/shell.rs::set_mode_12h) and depth 4, any other size is a `panic!` | [`start_screen`](sym:shell/dos/src/main.rs::start_screen) searches `DosDisplay::modes()` for an indexed, linear 640 by 480 mode (`0x101` in DOSBox), falls back to depth 4 if there is none, and returns `None` (exit 2) if no mode can be set |
| Depth | [`sc_depth`](sym:kernel/src/shell.rs::sc_depth), decided by the loader's choice (`--video vbe` or `text`) | [`want`](sym:shell/dos/src/main.rs::want) from the argument; the fallback is a program decision |
| Palette | [`set_palette`](sym:kernel/src/shell.rs::set_palette) of `desk_vga::dac`, over `PORTS`, with `desk_ui::palette(depth)` | [`set_palette`](sym:shell/dos/src/main.rs::set_palette) of `DosDisplay`, with the same `palette(depth)` |
| Presenter | [`LinearPresent`](sym:kernel/src/shell.rs::LinearPresent) over `lfb_region`, or [`PlanarPresent`](sym:kernel/src/shell.rs::PlanarPresent) over `vga_window`, each a concrete type handed to the generic `play` | [`Screen`](sym:shell/dos/src/main.rs::Screen) holds the depth and frame buffer and makes the presenter for each use; [`Dyn`](sym:shell/dos/src/main.rs::Dyn) puts either behind one `Present` so a single closure serves both depths |
| Who runs the steps | [`play`](sym:kernel/src/shell.rs::play) calls the shared [`run_scenario`](sym:kernel/src/shell.rs::run_scenario) with a closure the runner calls after each frame | [`scenario`](sym:shell/dos/src/main.rs::scenario) is the platform's own `for` over `sc.steps`, calling `apply` for each step and `Shell::frame` at each `Frame` step |
| Input source | [`Ps2Input`](sym:kernel/src/shell.rs::Ps2Input), used only to wait for a key that acknowledges a screendump; the scenario's events are scripted | [`KEYS`](sym:shell/dos/src/main.rs::KEYS), a lock-free ring from the IRQ 1 handler, and `DosInput` for the INT 33h mouse, in the interactive mode only; the scenario is scripted as well |
| A free-running loop | none: [`play`](sym:kernel/src/shell.rs::play) only plays the scenario and returns; the kernel has no interactive shell mode | [`interactive`](sym:shell/dos/src/main.rs::interactive): drain keys and mouse into `handle`, present a frame, repeat until Escape or the timeout |
| Damage to the screen | the runner presents one frame per `frame` step; the closure [`play`](sym:kernel/src/shell.rs::play) hands the runner prints a `shell: damage` line from its `FrameStats` | `Shell::frame` through [`Screen`](sym:shell/dos/src/main.rs::Screen); the stats are not printed, and [`scenario`](sym:shell/dos/src/main.rs::scenario) does not look at them |
| Damage bound | [`run_scenario`](sym:kernel/src/shell.rs::run_scenario) enforces `expect-damage` for the depth in use, so a frame that wrote too much panics the run | [`apply`](sym:shell/dos/src/main.rs::apply) ignores `ExpectDamage` and `Frame` steps; nothing on DOS checks the bound |
| Evidence of the picture | [`rendezvous`](sym:kernel/src/gfx.rs::rendezvous), called from the closure in [`play`](sym:kernel/src/shell.rs::play), prints `shell: ready NAME` and waits for a key while the run script takes a QEMU screendump | [`write_capture`](sym:shell/dos/src/main.rs::write_capture) after reading the frame buffer and DAC back, with the digests printed on a `shell: readback` line |
| Negative check | [`CORRUPT_AFTER`](sym:kernel/src/shell.rs::CORRUPT_AFTER) and [`CORRUPT_AT`](sym:kernel/src/shell.rs::CORRUPT_AT): after `moved`, one pixel of the *screen* (600, 440) is changed through a second presenter, so only the screendump is wrong | the `corrupt` argument of [`scenario`](sym:shell/dos/src/main.rs::scenario): after `moved`, one pixel at the corner is changed in the frame buffer and the read-back must report `DIFFERENT` |
| Ending the run | [`run`](sym:kernel/src/shell.rs::run) prints `shell: done`, leaves VBE and restores text mode (`restore_text`), and returns into the kernel, which prints `boot complete` and exits through the debug-exit port | [`main`](sym:shell/dos/src/main.rs::main) returns the status itself: 0, 1 (a read-back differed or a step failed), 2 (no video mode), 4 (the interactive timeout), 5 (an unknown argument); 3 is the runtime's panic status |
| Putting the machine back | [`restore_text`](sym:kernel/src/gfx.rs::restore_text), called at the end of [`run`](sym:kernel/src/shell.rs::run), reloads the registers, the palette and the 8 by 16 font | `display.restore_text_mode()` at the end of [`main`](sym:shell/dos/src/main.rs::main) (and on the error path of [`scenario`](sym:shell/dos/src/main.rs::scenario)); [`report`](sym:shell/dos/src/main.rs::report), registered first so that it runs last on the runtime's clean-up list, prints the mode DOS sees (`0x03`), after the keyboard flags and mouse have been restored by the functions [`interactive`](sym:shell/dos/src/main.rs::interactive) registers |

The rows are cut to what the code does, and a few things in them deserve a sentence. The kernel panics where the DOS program returns a status, because a panic in the kernel is already a failing run: its panic handler prints the message and writes the failure value to the debug-exit port ([M5](chapter:m-boot)). The DOS program cannot panic into a clean exit, so it counts: an exit status is how a script learns what happened. The two programs also differ in *when* they know the picture is right. The kernel does not read its own display back (it only writes the frame buffer and the VGA window, and reading video memory is the slow direction), so a second party (QEMU) takes the picture. The DOS program can read its card back, so it checks itself and writes the files for a script to compare. [`tools/shell-compare.sh`](repo:tools/shell-compare.sh) puts the two kinds of capture side by side.

### How the damage reaches the screen

On both sides the path is the same from the second step on. A change reaches `Shell` as an event or a call, `Wm` reports the rectangles it disturbed, the shell flushes them into the compositor, and at `Shell::frame` the compositor repaints the scene damage into the back buffer, adds the overlay damage of the cursor, widens it to the presenter's alignment (1 pixel for the linear presenter, 8 for the planar one), and calls the presenter's `begin`, `span` for each row and `end`. Only the presenter differs. `LinearPresent` copies a row to the frame buffer at its pitch, a plain write; `PlanarPresent` splits a row of indices into four bit planes and writes each through the VGA's write-mode registers, using the port bus. Both are in `desk-present` and shared by both tracks; the platform supplies the frame buffer region and, for the planar one, the ports. That is why depth 4 widens damage to 8-pixel columns on both platforms. The native depth 4 transcript shows it (`608 presented in 2 rectangles` for the moved cursor); the DOS program prints no damage line, so there the widening is visible only in the pictures matching.

### How input reaches the shell, and where it does not

The input events are the same two types on both tracks, and the chapter on them ([M11](chapter:m-input)) explains how the native PS/2 decoder and the DOS adapter were made to deliver identical [input events](term:input-event) from different hardware. The figure repeats the two pipelines, row by row:

<!-- explainer pipelines -->

What the table above adds is the limit of the evidence. In the scenario runs, neither platform sends the shell an event from a device: the scenario's `press`, `move` and `type` are values built by the parser. The native kernel uses its PS/2 driver only to wait for a key, so `handle` never sees a real report there. The DOS program's interactive mode does feed `handle` from IRQ 1 and INT 33h, and the project's check types keys into it with DOSBox's `AUTOTYPE`; the mouse is not exercised by any check, and nothing on the native side drives the shell live. These are the first items on the "not verified" list of `docs/FIRST-WINDOW.md`.

## Why it is this way

- **No `Platform` trait, yet.** `docs/HAL.md` ("A platform as one value", marked a sketch) draws `pub trait Platform` with associated types for ports, interrupts, clock, display and input, and an entry `fn run<P: Platform>(p: &mut P) -> !`. No file in the repository declares such a trait: the only `Platform` in the HAL source is a variant of the error type, `HalError::Platform(u16)`, a code a platform may attach to a failure. `docs/FIRST-WINDOW.md` ("Where it sits") records the decision to leave it out: the first window needs only a display and input, and the two platforms already have them as separate types (`DosDisplay` and `DosInput`; the kernel's frame buffer description `Lfb`, the VGA registers and `Ps2Input`). A trait written now would be shaped by one consumer and would have to be bent when a second (timers, storage) arrived. The cost is visible in the table: most of its rows are the same job done in two different shapes. The benefit is that nothing had to be guessed.
- **The platform owns the loop.** In an interrupt-driven kernel the right place to wait is the platform's: natively an `hlt` until an interrupt, under DOS a poll, since the INT 33h mouse must be asked once per iteration (`docs/FIRST-WINDOW.md`, "desk-shell"). If the core owned the loop it would need an idea of waiting, and so of time, which it is deliberately without.
- **One scenario, scripted input.** Both wirings play one file with input that is a list of values, so the platforms cannot drift from the goldens through input timing (`docs/FIRST-WINDOW.md`, decision 9). The price is the one stated above: the device paths are not proved by this.
- **Two kinds of capture.** The native kernel does not read its display back and the DOS program does; each is checked in the way its platform allows, and the doc's Result section says the DOS capture "is not a screen capture" but a read-back of an emulated card, while QEMU's screendump is the emulator's rendered output, an independent path (`docs/FIRST-WINDOW.md`, "Not verified").
- **Where document and code differ.** Three things. (1) `docs/HAL.md` ("The platforms") labels the hosted `rt::hal` and the native `kernel::hal` as *planned*; both files exist and are used: `spikes/d0-rust-dos/rt/src/hal.rs` holds `DosDisplay` and `DosBios`, and `kernel/src/hal.rs` holds `PORTS` and the registry. The table's other rows may still be accurate; those two labels are out of date. (2) `docs/FIRST-WINDOW.md` and `tests/shell/README.md` say the platforms skip `expect-damage`; DOS does, the kernel does not (see [M15](chapter:m-shell)). (3) `docs/FIRST-WINDOW.md` ("Native: the kernel's `shell` test mode") says depth 4 is set up with `desk_vga::set_mode_12h`; the call in `kernel/src/shell.rs` is `regs::set_mode_12h`, in the `regs` module of the same crate.

## The code

| Where | What it holds |
|---|---|
| [`kernel/src/shell.rs`](repo:kernel/src/shell.rs) | the native wiring: [`run`](sym:kernel/src/shell.rs::run), [`play`](sym:kernel/src/shell.rs::play), [`corrupt`](sym:kernel/src/shell.rs::corrupt) and the embedded scenario and faces |
| [`kernel/src/hal.rs`](repo:kernel/src/hal.rs) | [`PORTS`](sym:kernel/src/hal.rs::PORTS), the drivers as statics, [`IrqTable`](sym:kernel/src/hal.rs::IrqTable) and the end-of-interrupt rule |
| [`kernel/src/gfx.rs`](repo:kernel/src/gfx.rs) | the pieces `run` uses: `rendezvous`, `lfb_region`, `vga_window`, `leave_vbe`, `restore_text` |
| [`shell/dos/src/main.rs`](repo:shell/dos/src/main.rs) | the hosted wiring: [`main`](sym:shell/dos/src/main.rs::main), [`start_screen`](sym:shell/dos/src/main.rs::start_screen), [`scenario`](sym:shell/dos/src/main.rs::scenario), [`interactive`](sym:shell/dos/src/main.rs::interactive), [`Screen`](sym:shell/dos/src/main.rs::Screen) |
| [`shell/dos/check.sh`](repo:shell/dos/check.sh), [`shell/dos/build.sh`](repo:shell/dos/build.sh) | the DOS build and its checks in headless DOSBox |
| [`tests/native/expected-shell-8.txt`](repo:tests/native/expected-shell-8.txt), [`expected-shell-4.txt`](repo:tests/native/expected-shell-4.txt) | the native transcripts at each depth |
| [`tests/shell/expected-dos-8.txt`](repo:tests/shell/expected-dos-8.txt), [`expected-dos-4.txt`](repo:tests/shell/expected-dos-4.txt) | the DOS transcripts |
| [`tools/shell-compare.sh`](repo:tools/shell-compare.sh) | compares the two tracks' captures, and runs the comparisons that must fail |
| [`tools/shell-goldens.sh`](repo:tools/shell-goldens.sh) | re-records the sums and the golden pictures from the host test |
| [`tests/shell/first-window.sums`](repo:tests/shell/first-window.sums) | the checksums all runs are held to |

For depth: `docs/FIRST-WINDOW.md` ([the platform wirings](doc:docs/FIRST-WINDOW.md#the-platform-wirings), [where it sits](doc:docs/FIRST-WINDOW.md#where-it-sits)) and `docs/HAL.md` ([the platforms](doc:docs/HAL.md#the-platforms), [a platform as one value](doc:docs/HAL.md#a-platform-as-one-value-sketch)).

## Worked example

**One frame, followed through both programs.** Take the frame `moved` at depth 8, the pointer moved by (100, 60) with nothing else changed. Each platform runs the same steps `move 100 60` and `frame moved`, and what differs is only what each does with them.

*Native.* `play` has passed the scenario to `run_scenario`. For `move 100 60` the runner calls `apply`, which calls `Shell::handle` with a `Pointer` event; the pointer goes from (320, 240) to (420, 300) and the cursor moves, which is overlay damage and nothing else. For `frame moved` the runner calls `Shell::frame` with the `LinearPresent` over the loader's linear frame buffer, and then the closure. The closure prints two lines from the shell's own digest and the frame's statistics, and rendezvous prints a third; these are lines 46 to 48 of the run I made for this chapter:

```text
shell: frame moved 8 640x480 10201b69 03faf8b3
shell: damage moved: 0 pixels repainted, 456 presented in 2 rectangles
shell: ready moved
```

*The meaning of the numbers.* Nothing in the scene changed, so nothing was repainted. The presenter was handed the cursor's old and new sprite boxes: two rectangles of 12 by 19, which is 228 pixels each and 456 in all. `first-window.scn` bounds exactly this: `expect-damage 8 456 2`. Because the kernel plays the scenario with `run`, that bound was checked on the way: a frame of 457 pixels would have stopped the run with `RunError::DamageExceeded`. The same check on a transcript is the one the try block below makes by changing 456 to 457 in a copy of the expected file.

*Then the screen.* `rendezvous` waits for a key; the run script (which `tools/frames.py inject` wrote from the scenario's `frame` lines, so it cannot drift from them) has been waiting for the line `shell: ready moved`. It takes a QEMU screendump `moved.ppm` and sends the space key, and the kernel carries on. After the run, `frames.py check` compares the six-bit colours of the dump with the sums line `moved 8 640x480 10201b69 03faf8b3`.

*DOS.* `scenario` is on the same two steps. `apply` handles `move 100 60` the same way. At `frame moved` the loop calls `screen.present(ports, |p| shell.frame(p))`, which makes a `LinearPresent` over the frame buffer DPMI mapped, and calls `Shell::frame`. The digest it prints is the same two numbers, and with `dump` the program reads the frame buffer and DAC back, computes the digests of what it read and prints them on a second line. From `tests/shell/expected-dos-8.txt`:

```text
shell: frame moved 8 640x480 10201b69 03faf8b3
shell: readback moved 8 640x480 10201b69 03faf8b3 equal
shell: wrote MOVED.RAW (307200 bytes) and MOVED.PAL
```

The two checksums `10201b69` and `03faf8b3` appear in three places that a different program wrote: the DOS line, the native line, and `tests/shell/first-window.sums`. The DOS program prints no damage line, so the 456 is only in the native transcript and in the scenario file. That is the difference between the columns of the table, in miniature: the same call, `Shell::frame`, with a platform around it that decides what to say and what to check.

## State of this layer

```status
id: platforms.native
label: The native wiring: the shell test mode plays the scenario in QEMU at depth 8 over the loader's linear frame buffer
state: built-verified
evidence: test:tests/native/expected-shell-8.txt::shell: frame moved 8 640x480 10201b69 03faf8b3, sym:kernel/src/shell.rs::run, sym:kernel/src/shell.rs::play
note: Run for this chapter on QEMU with the standard VGA card (exit 0; the transcript matched its expected file; the digests matched the sums; the six screendumps matched at depth 8). Depth 4 on the Cirrus card was not rerun.

id: platforms.nativecorrupt
label: The native shell-corrupt mode, whose wrong pixel only the screendump can see
state: built-unverified
evidence: sym:kernel/src/shell.rs::CORRUPT_AT, doc:docs/FIRST-WINDOW.md#native-the-kernels-shell-test-mode-in-qemu
note: The mode exists and tools/native-test.sh runs it; this chapter did not run it.

id: platforms.dos
label: The hosted wiring: SHELL.EXE plays the scenario in headless DOSBox and reads its card back
state: built-unverified
evidence: test:tests/shell/expected-dos-8.txt::shell: readback moved 8 640x480 10201b69 03faf8b3 equal, sym:shell/dos/src/main.rs::scenario
note: Rests on the committed transcript, not rerun for this chapter (it needs the DOS toolchain). The capture is a read-back of an emulated card, not a screenshot.

id: platforms.same
label: The two tracks draw pixel-identical frames (six-bit colour), compared by tools/shell-compare.sh
state: built-unverified
evidence: repo:tools/shell-compare.sh, doc:docs/FIRST-WINDOW.md#what-matched
note: The comparison needs both tracks' captures; the native half was rerun here, the DOS half was not, so the script itself was not run. The result is the document's, measured on 2026-10-23 (#29 and #30).

id: platforms.goldens
label: The goldens are reproducible: tools/shell-goldens.sh re-records them with no change
state: built-verified
evidence: repo:tools/shell-goldens.sh, test:shell/shell/tests/scenarios.rs::every_scenario_matches_its_sums_through_every_presenter
note: Run for this chapter on a clean tree: the host test passed in record mode, git status stayed empty, and frames.py goldens printed ok: 12 golden PNGs match.

id: platforms.dosfallback
label: DOS depth 8 with no VBE falls back to depth 4 and matches the depth 4 goldens
state: built-unverified
evidence: sym:shell/dos/src/main.rs::start_screen, doc:docs/FIRST-WINDOW.md#what-matched
note: Rests on the committed check and the document's result; not rerun.

id: platforms.trait
label: A Platform trait and fn run<P: Platform>
state: planned
evidence: doc:docs/HAL.md#a-platform-as-one-value-sketch, doc:docs/FIRST-WINDOW.md#where-it-sits
note: A sketch in a design document only. No trait of that name exists in any source file.

id: platforms.livenative
label: Live keyboard and mouse driving the shell on the native kernel, and a free-running native frame loop
state: planned
evidence: doc:docs/FIRST-WINDOW.md#both-tracks-at-once

id: platforms.hardware
label: Either wiring on a real VGA card, monitor and 486
state: planned
evidence: doc:docs/FIRST-WINDOW.md#not-verified
```

The native wiring is {{state.platforms.native}} in QEMU and the DOS wiring is {{state.platforms.dos}}; the claim that they draw the same pictures is {{state.platforms.same}} in this chapter's run, and the goldens are {{state.platforms.goldens}}. The trait that would unify the wirings is {{state.platforms.trait}}, and a real machine is {{state.platforms.hardware}}. "Verified" means on the host or in an emulator, never on hardware.

## Try it yourself

The native shell run, by hand, is the same as `tools/native-test.sh` does for depth 8: build the kernel, patch the image to test mode `shell` with the VBE loader, and run it in QEMU with the monitor script that `frames.py` writes from the scenario. Then check the transcript against its expected file and the screendumps against the sums, and prove that the check can say no by changing the 456 to 457. It needs `nasm`, `cargo` and `qemu-system-i386`. This was run on the host with QEMU 11.1.2 and NASM 3.02. The commands were run from the repository root; the lines are in the order they were typed, with the output that matters.

```try
needs: host: bash, nasm, cargo, qemu-system-i386, python3
ran: 2026-10-23 on the host, QEMU 11.1.2, NASM 3.02
$ tools/native-build.sh > /dev/null && tools/native-image.sh --test-mode shell --video vbe --out dist/native-shell-8.img > /dev/null
$ python3 -I tools/frames.py inject tests/shell/first-window.scn > dist/shell.inject
$ tools/native-run.sh --timeout 60 --vga std --inject dist/shell.inject --shots dist/shell-native-8 dist/native-shell-8.img > dist/shell-8.txt 2> dist/shell-8.err; echo "exit $?"
exit 0
$ tr -d '\r' < dist/shell-8.txt > dist/shell-8.clean; python3 -I tools/native-check.py tests/native/expected-shell-8.txt dist/shell-8.clean
transcript matches tests/native/expected-shell-8.txt (64 lines)
$ python3 -I tools/frames.py transcript tests/shell/first-window.sums 8 dist/shell-8.clean
ok: 6 frames in dist/shell-8.clean carry the digests of tests/shell/first-window.sums at depth 8
$ python3 -I tools/frames.py check tests/shell/first-window.sums 8 dist/shell-native-8
ok: 6 frames at depth 8 match tests/shell/first-window.sums
$ sed 's/456 presented in 2/457 presented in 2/' tests/native/expected-shell-8.txt > dist/wrong-shell.txt; python3 -I tools/native-check.py dist/wrong-shell.txt dist/shell-8.clean; echo "exit $?"
transcript does not match dist/wrong-shell.txt
  expected line 42: shell: damage moved: 0 pixels repainted, 457 presented in 2 rectangles
  actual line   47: shell: damage moved: 0 pixels repainted, 456 presented in 2 rectangles
exit 1
```

Re-recording the goldens, which also checks that the host test is deterministic, is one command; on a clean tree it leaves no change:

```try
needs: host: bash, cargo, python3
ran: 2026-10-23 on the host
$ tools/shell-goldens.sh 2>&1 | tail -3
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.94s

ok: 12 golden PNGs match tests/shell/first-window.sums
```

`tools/shell-compare.sh` compares the DOS captures with the native ones. It needs `dist/shell/dumps-*` from `shell/dos/check.sh`, which runs DOSBox in the project's toolchain image, so it was **not run** for this chapter; with `--run` it runs both checks first, which is slow. Read its list of comparisons that must fail: five, from the depths swapped to a frame replaced by another.

**Exercises.** (1) Without running anything, say which rows of the table would have to change to add a third platform, and which would not (hint: start with the rows that name `Shell::frame`). (2) Predict the line `shell: damage moved` at depth 4 (the expected file for it is `tests/native/expected-shell-4.txt`, and `first-window.scn` has a second `expect-damage` line), then check. (3) In a scratch branch, change `CORRUPT_AT` in `kernel/src/shell.rs` to a pixel the cursor covers in the `moved` frame, predict whether `shell-corrupt` is still detected and by which part of `tools/native-test.sh`, and run it.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Seam
def: The narrow place where a platform meets the shared core. For the shell it is two calls, Shell::handle for an input event and Shell::frame to repaint and present, with the platform owning everything around them; the core never calls back.

term: Run loop
def: The loop that drains input into the shell, asks for a frame and waits, which in the Desk belongs to the platform, not the core. The DOS program has its own (poll, handle, present, repeat); the native kernel hands the scenario to the shared runner and is called back after each frame.
```

```figures
id: m16-pipelines
reuse: pipelines
at: explainer pipelines

id: m16-anatomy-desk
reuse: anatomy-desk
at: figure anatomy-desk

id: m16-anatomy-dos
reuse: anatomy-dos
at: figure anatomy-dos
```

## Revisions

- 2026-10-23: first written, against `44f0265`. The native depth 8 shell run and `tools/shell-goldens.sh` were rerun for this chapter; `tools/shell-compare.sh` and the DOS run were not (they need the DOS toolchain). Found while writing: `docs/HAL.md` ("The platforms") still labels `rt::hal` and `kernel::hal` planned; `docs/FIRST-WINDOW.md` and `tests/shell/README.md` say the platforms skip `expect-damage` and the native kernel does not; the document names `desk_vga::set_mode_12h` where the code calls `regs::set_mode_12h`.
