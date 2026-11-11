---
id: liveinput
n: 19
title: Live input
date: 2026-10-23
summary: The first window learns to answer a real mouse and keyboard on both tracks: ten stacked pull requests built in an afternoon, held in review for nineteen days, and merged together on 11 November.
prs: [54]
---
Chapter 18 ended with a window that moved only when a script told it to. This chapter is about the stack that made it answer a person. It is also, for an archivist, a lesson in provenance, because the dates on this work do not say what a reader would first assume.

**The stack, and the wait.** Ten pull requests (#33, #34, #37, #39, #43, #44, #49, #50, #53 and #54) were opened on the afternoon of 23 October, each on the branch of the one before: a design and plan (#33), the cursor's coalesced damage (#34), Alt+Tab and its kin in the window manager (#37), the live scenarios and the tools that turn them into input (#39), the INT 33h click counters (#43), the native kernel's live mode (#44), the hosted loop (#49), a launcher (#50), an Xvfb check (#53) and the result write-up (#54). They sat open until 06:26 UTC on 11 November, and then landed together. `main`'s first-parent history shows them as one merge commit, {{pr54.merge}}, titled for #54, the tip of the stack: `git diff --shortstat` from the first-window tree (`44f0265`) to that commit reports 102 files changed, 3,920 insertions and 173 deletions, across 35 commits. The bars in the timeline below are the real opening and merging times from `gh`; the long ones on the left of the gap are this stack and the Manual's.

<!-- figure pr-timeline -->

> **From the stacks — Why a relative mouse needs a grab.** A PS/2 mouse reports how far it moved, never where it is: the guest can only add up the deltas. An emulator window therefore has to capture the desktop's pointer, hide it and send every motion to the guest, or the two pointers drift apart. QEMU's own documentation describes the alternative, a USB tablet, as a "pointer device that uses absolute coordinates (like a touchscreen)" that lets it "report the mouse position without having to grab the mouse"; the Desk deliberately has no driver for one yet (R66, an idea). For the Desk it means a click in the window grabs and Ctrl+Alt+G lets go, as `docs/LIVE-INPUT.md` records.
> Source: [QEMU system emulation invocation, "-usbdevice tablet"](https://www.qemu.org/docs/master/system/invocation.html)

**What was built.** From the Result section of `docs/LIVE-INPUT.md`, which is the only place the figures are recorded:

- **The core.** The compositor remembers where the cursor was last presented, so a burst of mouse reports between two frames costs two sprite rectangles, not a trail (the test failed before the change: 2,440 pixels presented against 456). A small pure module, `desk_shell::live`, decides what each key means to the platform's loop: Escape ends a session, F12 asks for a *mark*, everything else is the shell's. Alt+Tab, Shift+Alt+Tab, Alt+F4, Tab and Shift+Tab live in `desk-wm`, with the caret drawn only in the focused field.
- **The two tracks.** The kernel gains a `shell-live` mode on the PS/2 drivers, paced by the 100 Hz timer and halting when idle; `SHELL.EXE` gains a live loop with marks and reads the INT 33h press and release counters so that a click shorter than one poll still arrives.
- **The tools.** `tools/frames.py live` turns a scenario into injected input for QEMU's monitor, DOSBox's `AUTOTYPE` or `xdotool` under Xvfb, and `tools/run-shell.sh native|dos` opens a window for a person.

<!-- figure live-routes -->

**The mark.** The idea I like best is the synchronisation point. A live check cannot sleep and hope: a slow machine would fail at random. So F12 travels the same keyboard path as the input before it, and when the guest has handled everything ahead of it, it prints a *mark*, the digests of the frame it has just drawn. A check takes its capture at the mark, never by the clock. Mark 00 is the scenario's first frame; the rest are the table below, read from `tests/shell/live.sums`.

<!-- figure live-marks -->

**What matched.** Every mark of every live run equals its golden. Native: `live.scn` at depth 8 (QEMU's standard card) and depth 4 (its Cirrus card), eight of eight marks and eight of eight screendumps each. DOS: `live-keys.scn` by `AUTOTYPE` through IRQ 1 at both depths, and `live.scn` by `xdotool` through DOSBox's window and the INT 33h driver, eight of eight, including a run with 5 ms clicks. `frames.py same` reports the native depth-8 screendumps and the DOS dumps of `live.scn` equal: the arrow, the drag, Alt+Tab, the typed word and the close box come out as the same picture through two quite different input paths. The scripted checks of chapter 18 did not change by a byte. The native counts of keys, reports and frames vary with how the injector splits a move, so the expected transcripts match them as ranges, never exactly.

<!-- figure repo-images img-li-native-8-typed.png img-li-dos-8-dragged.png img-li-native-4-dragged.png img-li-dos-4-again.png -->

**The negative runs.** Native: the script with its `mouse_move` lines removed, one `sendkey` removed from "live", the `alt-tab` line removed, the depth-8 dumps against the depth-4 sums, a transcript with one mark line removed. DOS: `AUTOTYPE` without one key of "desk", a dump with one pixel changed, the Xvfb run with its pointer motion removed. Each failed as it should.

**A premise that was wrong.** Spike B (chapter 15) had concluded that `xdotool click` was lost because the press and release fell inside one poll, and the click counters were built to fix that. The result write-up says the premise was wrong: a press and release with no gap never reach DOSBox's driver at all (X delivers both with one timestamp), so no counter holds them. A click 1 to 5 ms long does arrive and a program polling every few milliseconds sees it by state. The counters matter when a poll is slower than the click, so the check now runs a slow-polling program against six 5 ms clicks: it saw none of them without the counters and all six with them. I find this the most honest sentence in the document, and the evidence was changed rather than the story defended.

**Found in review.** Held Alt+F4 closes one window, not one per typematic repeat; Alt+Tab released during a drag ends the drag; DOSBox's built-in driver holds back a one-pixel move until the next move, so the live scenario's last pointer step is not a one-pixel one; and Alt+F4 under Xvfb reaches `SHELL.EXE` but DOSBox does not act on it, so there is no DOS variant of the scenario that uses it.

**The CI record.** The CI run on {{pr54.merge}} shows *{{pr54.ci}}* in the archive's data: the merge landed within minutes of #36 and #64, and the workflow cancels a run when a newer push arrives on the same ref, exactly the case chapter 18 described. The next completed run on `main` that holds this work is the one on `b8fb9ac`, which passed.

## The shelves

### Catalogued
- Every mark of the live runs listed above equals its host golden, on the native kernel in QEMU 11.1.2 (a 486) and on `SHELL.EXE` in DOSBox Staging 0.83, at depths 8 and 4; three native runs in a row gave one set of mark lines and five DOS Xvfb runs in a row gave the same marks (`docs/LIVE-INPUT.md`, Result, written 2026-10-23; merged in {{pr54.merge}}).
- The negative runs listed above fail as they must, and the INT 33h counters have a check that fails without them.
- The scripted first-window checks are byte for byte what they were, and still pass.
- `SHELL.EXE` was 108,544 bytes and the native `kernel.bin` 179,984 bytes in that run.

### Uncatalogued
- A person at a real desktop. The windows were driven by `xdotool` on a virtual X server without a window manager; the first hands-on runs, three weeks later, are in the next chapter and found problems that this setup could not.
- Real hardware of every kind: a real 8042, a real mouse driver, a real VGA card, a 486. Pacing was designed, not timed.
- Bochs and 86Box did not play the live scenarios, and DOSBox's built-in INT 33h driver is the only one this chapter's runs used (FreeDOS with CuteMouse, in the next chapter, is another).
- The right and middle buttons and the wheel reach the shell and do nothing.
- The Xvfb steps (`INPUT_EXTRA=1`) and the Bochs steps (`NATIVE_EXTRA=1`) in the CI image: the image has no Docker, so they were not part of the later re-check and were last run on the author's host on 23 October. The rest was re-checked from a clean clone in the image on 11 November (#73).

### Wish list
- An absolute pointer in QEMU (`vmmouse`, a USB tablet) so no grab is needed (R66, an idea).
- The Alt+Tab overlay and a focus ring (R67), and a `Platform` trait in place of the narrow seam.

```figures
id: pr-timeline
kind: diagram
track: shared
title: Pull requests from opening to merge
alt: A bar for each pull request from number 19 to 72, from the day it was opened to the day it was merged. The first-window stack opens and merges on 23 October; the live-input and Manual stacks open on 23 October and merge on 11 November; the later pull requests open and merge on 11 November.
caption: Opened and merged times of every pull request from #19 on, in UTC, from gh.
at: figure pr-timeline
thumb: svg
date: 2026-11-11

id: live-routes
kind: diagram
track: shared
title: Two routes into one frame
alt: Two rows of six boxes. The hosted row runs from the INT 33h mouse driver and the keyboard through the SHELL.EXE loop; the native row runs from the PS/2 mouse and keyboard through the kernel's shell-live loop; the last three boxes, the live module, the shell and window manager, and the presenter, are the same on both rows.
caption: The hosted and native routes from a pointer or key to the same frame; each box names a file or crate that exists.
at: figure live-routes
thumb: svg
date: 2026-10-23

id: live-marks
kind: diagram
track: shared
title: The marks of the live scenario
alt: A table of the eight frames of the live scenario with the colour digest of each at depth 8 and depth 4.
caption: The goldens every live run was held to.
at: figure live-marks
thumb: img:img/img-li-native-8-typed.png
date: 2026-10-23

id: live-shots
kind: screenshot
track: shared
title: The first window under live input
alt: Four screendumps of the first window under live input, at depth 8 and depth 4, on the native kernel and on DOS, showing a typed word and a dragged window.
caption: Captures from the live runs' own dumps, kept in docs/images/live-input.
at: figure repo-images img-li-native-8-typed.png img-li-dos-8-dragged.png img-li-native-4-dragged.png img-li-dos-4-again.png
thumb: img:img/img-li-dos-8-dragged.png
date: 2026-10-23
```
