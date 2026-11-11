---
id: hands
n: 20
title: The first hands on it
date: 2026-11-11
summary: On 11 November the owner ran the first window by hand on a real desktop, native in QEMU and hosted on FreeDOS, and found four things the headless checks could not; one was fixed within hours.
prs: [72]
---
Every run in this archive until now was a machine talking to a machine: `xdotool` pressing keys on a virtual X server, QEMU's monitor injecting mouse counts, a transcript compared with a file. On 11 November the owner sat at a desktop and used the thing. It is still an emulator, and nothing has touched real hardware, but it is the first time a person's hand was in the loop, and a person's hand found problems that three weeks of careful checks had not.

**What was run.** Two runs, reported by the owner and recorded in the roadmap as items R127 to R130. The native track: `tools/run-shell.sh native`, QEMU's GTK window on a GNOME Wayland desktop. The hosted track: `SHELL.EXE` on FreeDOS (the owner's notes say 1.4) in QEMU, with the CuteMouse driver. I was not there, and the repository holds no screenshots of either run; the evidence for each finding is the owner's report, not a committed test. Two of the findings (R128 and R130) also rest on a headless QEMU measurement made the same day by an agent, which is not committed either.

**What worked.** The native window ran, and the mouse moved the arrow once QEMU was running under XWayland (`GDK_BACKEND=x11`). The hosted first window ran on FreeDOS. Those are the first two times either track has been seen on a person's desktop.

**What was found.**

| Finding | What happened | What is and is not established |
|---|---|---|
| R127 | Interactive `SHELL.EXE` printed its `shell: mark 00 …` lines over the window, top left. The shell's own back buffer was clean; moving the pointer repainted the text away. | The cause is known (the DOS console draws text onto the frame buffer while the VBE mode is on) and the fix has merged (below). |
| R128 | After the host mouse left the QEMU window and came back, the pointer stuck at a screen edge. | **Cause unknown.** A headless measurement (over-drive an edge by 3,000 units, then move 40 back) found the cursor leaves the edge on the first return move on both FreeDOS and the native kernel, so the guest does not reproduce it. The suspect, which a headless run cannot test, is QEMU's GTK window under XWayland. Roadmap status: in progress. |
| R129 | Under native Wayland GTK the pointer was never captured; Alt+Tab went to GNOME's switcher, not the guest. Minimise and maximise did nothing. | The XWayland workaround worked for the pointer. Why the keyboard grab was not in effect is not established. Minimise and maximise do nothing by design in the first window (R97). The launcher does not set `GDK_BACKEND` yet. |
| R130 | On FreeDOS with CuteMouse the pointer moved faster than the host's. | The headless measurement put it at 1.35 to 2.5 times the host distance, against roughly one to one on the native kernel. `docs/LIVE-INPUT.md` says DOSBox's built-in driver turns motion into mickeys one for one; CuteMouse does not, so the hosted track's pointer speed depends on the driver. No fix chosen. |

> **From the stacks — Why a Wayland window may not get the pointer.** On X11 a program can grab the pointer for itself. Wayland's core protocol has no such call; it leaves the question to an extension, `pointer-constraints`, whose description says it covers "confining pointer motions to a given region, or locking it to its current position", and a compositor decides whether to grant it. As the story is usually told, programs that still speak X11 through the XWayland compatibility layer can behave differently from native Wayland clients here, which would fit what the owner saw, but nothing in this repository establishes why; R129 says so too. For the Desk it means the launcher will probably have to choose the display backend for the desktop it finds.
> Source: [wayland-protocols, pointer-constraints-unstable-v1](https://wayland.app/protocols/pointer-constraints-unstable-v1)

**The fix, in an afternoon.** R127 was the one finding with a known cause and a small cure. Pull request #72 ({{pr72.merge}}, merged at 18:07 UTC) files every `shell: …` line in a fixed 16 KiB buffer (`desk_shell::transcript`, atomics only, so no `unsafe` and no compare-and-swap) and prints it from the clean-up list once the video mode is back to text. That covers Escape, a `timeout=N` end, scenario runs and a panic alike, and the lines and their order are unchanged, so every existing check reads the same transcript. If the buffer fills, the beginning is kept whole and the rest dropped, ending with a line that says how many were lost. The approach is the one Lieutenant Fizz 2 already used (chapter 23).

<!-- figure transcript-flow -->

The part an archivist likes is the check. `tools/dosrun.sh` gained `DOS_CONSOLE=1`, which leaves the output on the console as a person would have it, and `shell/dos/check.sh` runs the live scenario that way and requires exit status 0 and every mark's dump to carry the golden digests. According to the pull request, before the fix it exited 1, with mark 00 reading back as different. That is a check that failed before the change and passes after it, which is the rule in `AGENTS.md`. The owner reported the stray text gone when run live. I did not re-run it for this chapter; the pull request's own CI run on its merge commit was cancelled by the next merge, and the first completed run that contains it is the one on {{pr71.merge}}, which passed in the new CI image (chapter 22).

**What this did and did not prove.** It proved that the two tracks can be used by a person: the window opens, the pointer moves, the first window behaves. It proved that headless checks have a blind spot exactly where a person is, because the tests redirect output and never saw the transcript on the screen. It did not prove the hosted track on FreeDOS: that run is reported, not recorded, and R127 to R130 have no checks of their own yet except R127's. It said nothing about real hardware, a real mouse or a real VGA card.

## The shelves

### Catalogued
- The transcript fix: the buffer is host-tested, the live scenario run with the console left on passes in `shell/dos/check.sh`, and the check failed before the fix, as #72 records; the run on {{pr71.merge}}, which contains #72, passed.

### Uncatalogued
- The owner's two hands-on runs themselves: reported in the roadmap, with no screenshots, transcripts or committed checks. The owner's report that the stray transcript is gone is likewise a report.
- R128, R129 and R130: observations and an uncommitted headless measurement; the pointer's sticking, the keyboard grab under Wayland and CuteMouse's speed ratio have no explanation and no test.
- The roadmap on `main` still lists R127 as planned, because the pull request that fixed it did not touch the roadmap; the row will move when the open documentation pull request for R47 merges.
- FreeDOS with another mouse driver, DOSBox in a real window, any run on real hardware.

### Wish list
- Setting `GDK_BACKEND=x11` in the launcher on a Wayland session (R129), and an investigation of the edge sticking with `-display sdl` and `grab-on-hover=on` (R128).
- Finding CuteMouse's ratio options, saying so in the docs, and deciding whether `SHELL.EXE` should scale the mickeys itself (R130).
- An absolute pointer, so that a window needs no grab at all (R66).

```figures
id: transcript-flow
kind: diagram
track: hosted
title: Where the shell's transcript goes
alt: Two rows of four boxes. Before the fix a println reaches the DOS console, which draws on the frame buffer while the VBE mode is on, so the text lands over the window. After the fix the line is filed in a 16 KiB buffer and printed from the clean-up list once text mode is back.
caption: The transcript before and after the fix for R127.
at: figure transcript-flow
thumb: svg
date: 2026-11-11
```
