---
id: firstwindow
n: 18
title: The first window
date: 2026-10-23
summary: One scenario, three runs and the same pixels: the shared core draws two windows, text and a cursor on the host, on DOS in DOSBox and on the native kernel in QEMU, and every run is held to the same goldens.
prs: [19, 29, 30, 31]
---
For seventeen chapters the two tracks have each drawn their own pictures: a gradient in mode 13h, a window with a cursor from the native kernel's own code. This chapter is the first in which they draw the *same* picture, from the same code. The work is the first piece of the shared core, the `shell/` workspace, designed in `docs/FIRST-WINDOW.md` and built as pull requests [#19](pr:19) (which carries the whole stack), [#29](pr:29), [#30](pr:30) and [#31](pr:31). The milestone it was aimed at, in the archive's own words, is a window drawn by the shared core and moved on both tracks. It is reached in emulators, with scripted input. The mouse, real hardware and every emulator but two are on the shelves below, where they belong.

**Planned, then built.** The design and the twelve-task plan were written on the morning of 23 October by an Opus agent: three commits, `da39c89`, `d382b21` and `d9d77ef` (the last one fixes the review's findings in the design and plan itself), each carrying the Opus trailer. The tasks were then built by Sonnet agents, one task per pull request, at most two at a time: every commit from `b72eb9e` onward carries the Sonnet trailer. The pull requests were stacked, each on the branch of the one it needed (the plan in #19 lists the order: gfx, font, then present and ui, then wm and the scenario parser, then the runtime, the goldens, and finally the two wirings). Pull requests #20 to #28 were merged into their stack bases and reached `main` through #19, whose merge commit holds all of them; #29, #30 and #31 followed at 13:16 and 13:28 UTC. (There is no #22: the number belongs to nothing in the repository, which I note because an archivist notices a gap in a run of numbers.)

**The shared core.** Six `no_std` crates, no external dependencies, no unsafe code, no floating point, no clocks and no randomness, so that a frame is a pure function of the steps that led to it.

| Crate | What it is | Source |
|---|---|---|
| `desk-gfx` | rectangles, the back buffer, damage, the compositor, the `Present` trait and the frame digests | #20 |
| `desk-font` | a reader for the `.DFN` faces | #21 |
| `desk-present` | the linear and the planar presenters | #23 |
| `desk-ui` | theme roles, text, window chrome | #24 |
| `desk-wm` | windows, focus, dragging and the damage each change makes | #25 |
| `desk-shell` | events, the cursor, frames and the scenario runner | #26, #27 |

`cargo test` in `shell/` ran 161 tests, all passing, when I ran it for this chapter. The design's key choice is a narrow seam: the platform owns the loop and calls `Shell::handle` and `Shell::frame` itself, and the core reaches the screen only through the `Present` trait and the HAL. A general `Platform` trait is deliberately postponed (`docs/FIRST-WINDOW.md`, "Where it sits"). The anatomy pages draw that seam: [Anatomy of a Rust DOS application](chapter:anatomydos) for the hosted program, and [Anatomy of a Rust Desk application](chapter:anatomydesk) for what exists under a Desk application today.

**One scenario, three runs.** The idea that makes the whole chapter checkable is a script, `tests/shell/first-window.scn`: two windows opened, the pointer moved, a click that focuses, a drag, four typed keys and a close, with six `frame` steps that capture the screen. The same script runs three times, and every run must reproduce the same digests in `tests/shell/first-window.sums`: on the host in `cargo test`, through three presenters; on DOS as `SHELL.EXE` in DOSBox Staging; and on the native kernel, in its `shell` test mode, in QEMU. Pictures are compared as digests: a CRC-32 of the palette indices and a CRC-32 of the six-bit colours, so that two tracks that number their palettes differently can still agree on what was seen.

<!-- explainer firstwindow -->

The explainer above is the scenario itself. Each step lists its two digests and the damage the compositor reported, read from the repository's files, and the generator refuses to build if those digests disagree with the result table that `docs/FIRST-WINDOW.md` records.

> **From the stacks — The digest.** A cyclic redundancy check is a short code computed from a block of data so that an accidental change shows up when the code is recomputed; the Wikipedia article credits W. Wesley Peterson in 1961 with first proposing systematic cyclic codes for error detection, and lists the 32-bit polynomial among the standards for Ethernet and PNG. It is built for accidents, "not suitable for protecting against intentional alteration", as the article puts it. That is the right tool here: the Desk's digests catch a wrong pixel, which is an accident, and there is no adversary. The constant in `shell/gfx/src/digest.rs` is `0xEDB88320`, the reflected form of that standard polynomial.
> Source: [Cyclic redundancy check, Wikipedia](https://en.wikipedia.org/wiki/Cyclic_redundancy_check)

**The digest table.** These are the numbers each run has to hit. At depth 8 DOS uses VBE mode `0x101` on DOSBox's S3 card and the native kernel uses QEMU's standard card with the loader's linear frame buffer; at depth 4 both use mode 12h, the native kernel on QEMU's Cirrus card.

<!-- figure fw-digests -->

**What was seen.** The two tracks' `typed` frames (and the `dragged` frames beside them, which were kept but not looked at) are in `docs/images/first-window/`, and the `docs/FIRST-WINDOW.md` Result section records that the four `typed` pictures (DOS and native, depth 8 and depth 4) were looked at one after another and were indistinguishable. In the explainer, step 5 shows the host golden next to the native screendump; the DOS read-back of that frame is byte for byte the same file as the golden (the generator checks that as it builds). Only those four `typed` frames were looked at by eye; the rest are held by their digests. I looked at the six depth-8 goldens myself while writing the captions.

**The negative runs.** A check that cannot fail is a decoration, so every comparison has runs that must fail. From `docs/FIRST-WINDOW.md`: the DOS depth-8 frames against the native depth-4 frames (`closed`: `379c4c02` against `df1b4c3a`); the depth-4 DOS frames against the native depth-8 frames; one palette index changed in the DOS `typed` dump (`c6c912f0` against `2cd55463`); a DOS set with `closed` missing; the native `typed` frame replaced by `focus`. The hosted program has its own: a `corrupt` argument that flips one pixel after the `moved` frame (the read-back prints `DIFFERENT` and the exit status is 1), dumps checked against the other depth's sums, and a transcript with one digest changed. The native kernel's `shell-corrupt` mode writes the wrong pixel after the digest is taken, so only the screendump is wrong, and the screendump check must fail from `moved` on.

**The reviews, the CI break and the fixes.** The history records what review found; the pull request pages hold no review records of their own, so the commit messages are my evidence. Three examples: a modifier key that stayed down when the other Shift was released (`ee86f86`); an interactive timeout that counted BIOS ticks as if the counter wrapped at 2³², when it restarts at midnight, so a run across midnight timed out at once (`e93502d`); and a scenario run whose depth did not match the shell's, which checked the wrong damage budget (`28503c1`). The break that the machines found, not the reviewers, was in #24: the `desk-ui` branch added a table of the first window's palette slots to `docs/PALETTE.md`, and the fonts' palette test, which reads that document row by row, matched the new rows too and failed its slot-for-slot comparison. CI was red on `a4c9d09` (run 39927790199, one failing test) and green on the fix, `5c7f79a`, which makes the test read only the 16-colour section. The lesson the commit records is small and useful: a document that a test parses is an interface, and adding a table to it is a change to the interface.

**The boot transcript is now a real one.** The archive's boot explainer (chapter 7) had been using a transcript that had boot lines added by hand to keep it in step with the kernel. For this chapter I built the floppy image from `main` at {{head}} with `tools/native-build.sh` and `tools/native-image.sh`, and ran it with `tools/native-run.sh --serial` on QEMU 11.1.2 with `-cpu 486`. The run reported success, and its output replaced `media/native-serial.txt` with nothing edited but the carriage returns that the loader's BIOS output adds. Three numbers differ from the old file because the kernel is bigger now: the kernel image ends at `0x13c000` and not `0x11a000`, the frame count after reservations is 3,748, and the heap starts at `0x13c000`. The image is 1,474,560 bytes with a kernel of 174,032 bytes (`snapshots/measured.json` has the command and the image's SHA-256). The Result section of `docs/FIRST-WINDOW.md` gives 173,344 bytes, measured on the comparison branch before it was merged with `main`; both are measurements of a build, of different trees.

**What is not verified.** The Result section puts this first, and so do I:

- **The mouse.** Both runs hand the shell scripted events; nothing moved through INT 33h on DOS or the PS/2 mouse on native. Keys reached the shell through IRQ 1 on DOS in an interactive run; they did not on the native side.
- **Real hardware.** Nothing ran on a real VGA card, monitor or 486; DAC rounding on real parts and timing are untested.
- **The DOS capture is a read-back.** `SHELL.EXE` reads the frame buffer and the DAC back, so it records what the emulated card holds, not what DOSBox draws. A fault that wrote the same wrong thing into memory and the DAC would agree with itself on DOS, and be caught only by the goldens, which come from host code with no VGA in it. QEMU's screendump is the emulator's rendered output, an independent path, and it agrees too.
- **One emulator per track.** DOSBox Staging and QEMU. Bochs and 86Box were not run for this.
- **The other documents of the plan's last task** (`HAL.md`, `DIRECTION.md`, `SPEC.md`, `RUST-TARGETS.md`) were not updated by it.

## The shelves

### Catalogued
- The scenario's six frames at depth 8 and depth 4 have equal six-bit colour digests on the host, on DOS in DOSBox Staging (read back through the frame buffer and the DAC) and on the native kernel in QEMU (screendumps), all equal to `tests/shell/first-window.sums`; `frames.py same` passes, with no frame needing a tolerance (`docs/FIRST-WINDOW.md`, Result; #31 merged, CI `{{pr31.ci}}`).
- The five comparison negatives, the hosted program's `corrupt` and depth-mismatch runs and the native `shell-corrupt` run each fail as they must.
- `cargo test` in `shell/` passed 161 tests when run for this chapter on the host, and the DOS run with no VBE falls back to depth 4 and matches the depth-4 goldens.
- The native transcript used by the boot explainer is an unedited capture of `tools/native-run.sh` on QEMU 11.1.2, as described above.

### Uncatalogued
- The mouse path on both tracks, and keys on the native side.
- Real hardware of every kind, and any emulator beyond DOSBox Staging and QEMU.
- The DOS capture is a read-back of an emulated card, not a picture of what DOSBox draws.
- Only the four `typed` frames were looked at by the project's own review; the other frames rest on digests. The kept `dragged` frames were not looked at by the Result section; I looked at the goldens.
- Whether the sizes (SHELL.EXE 102,400 bytes) hold on another build: I did not rebuild `SHELL.EXE` for this chapter.

### Wish list
- Live input into the shell on both tracks (QEMU `sendkey` and `mouse_move`, DOSBox `AUTOTYPE` and a real INT 33h mouse), the plan's optional last task.
- The launcher bar, menus, widgets beyond a label and a text field, and apps.
- A general `Platform` trait in place of the narrow seam.
- A rendered DOSBox screenshot beside the read-back, and a run in Bochs and 86Box.

```figures
id: first-window-steps
kind: explainer
track: shared
title: The six frames of the first window
alt: A step-through of the six frames of the first-window scenario, each with its depth-8 and depth-4 colour digests, the damage the compositor reported and the real golden picture, with the native screendump beside it for the dragged and typed frames.
caption: The scenario, one frame at a time, driven by the sums file and the transcripts.
at: explainer firstwindow
thumb: steps

id: first-window-digests
kind: diagram
track: shared
title: The digests every run must hit
alt: A table of the six frames of the first-window scenario with their six-bit colour digests at depth 8 and depth 4 and the pixels presented at each depth.
caption: One table that the host, DOS and native runs are all held to.
at: figure fw-digests
thumb: img:img/img-fw-typed-8.png
```
