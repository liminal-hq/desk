---
id: path
n:
kicker: The path
title: The path
date: 2026-10-23
summary: Everything still to do, as a tree that grows from the Desk to its first milestone, with the critical path marked.
prs: []
section: reference
---
A shelf of finished things is only half an archive; the other half is the list of what is missing. This is the path, drawn from `tools/progress-report/roadmap.json`. Its target is one milestone, **First Window**: a window drawn by the shared Desk core and moved with a mouse, on DOS and on the bare-metal kernel. Chapter 18 tells how far it got: the shared core's first piece draws two windows, text and a cursor on both tracks, with the same pixels in two emulators, driven by a scripted drag and not a real mouse. So the milestone box is drawn as work in progress, and nothing on the tree is promoted past what was checked.

The architecture comes first, because the tree is easier to read once you know which layers exist.

<!-- figure arch -->

Now the tree. Every box is a step; the colour and shape of its icon say what has been earned. Choose a box, or tab to it, to read what it is, why it matters, what it needs and what it unlocks. On a narrow screen the same steps appear as a list.

<!-- figure tree -->

The same steps, counted by track and by state:

<!-- figure track-progress -->

And the next move of each branch, with what is in its way.

<!-- figure nexts -->

## The shelves

### Catalogued
- The steps marked done have merged pull requests and a passing CI run on each merge commit: the generator would drop a step to "built, in review" while its pull requests are open, and to "partly done" if one cannot be found.
- Every step carries a tag from `gh` (merged, in review, in progress or planned), so a reader can tell a merged step from one that is built and waiting.
- "Catalogued in an emulator" is as far as any native or DOS step goes: the checks run in QEMU or headless DOSBox Staging.

### Uncatalogued
- Every step marked "catalogued in an emulator" is uncatalogued on real hardware. No step has a real machine behind it.
- The steps for spikes G and n3 (Bochs and 86Box, and a real BIOS path for the native kernel) are merged and were run in emulators only; neither ran on a real machine, and whether the merged image runs the trampoline on 86Box was not tried.
- The first-window steps (the shared core, the hosted shell, the core on native and the milestone) are marked in progress, not done: the mouse, the launcher bar, menus and apps are not built or not verified (chapter 18).
- The roadmap's dependency edges and the critical path are my reading of `docs/DIRECTION.md`, `docs/SPEC.md` and `docs/RUST-TARGETS.md`; the generator checks the statuses, not the edges.

### Wish list
- Every dashed or dotted box in the tree: the rest of the shared core (widgets, menus, an app host), live input into the shell, the handoff, the core apps, the native desktop, the filesystem, the module loader, the scheduling model, FreeDOS under the native OS, Fizz Episode 2, the real-hardware runs and the milestone itself.

```figures
id: architecture
kind: diagram
track: shared
title: The architecture of the two tracks
alt: Layered diagram of the hosted and native tracks from apps down to the machine, drawn solid where work exists and dashed where it is only planned.
caption: Layers from docs/DIRECTION.md, with what is built and what is in review.
at: figure arch
thumb: svg
state: none

id: roadmap-tree
kind: diagram
track: shared
title: The path tree
alt: A tree of every roadmap step growing from the Desk to the First Window milestone, each step marked by its state and the critical path in amber.
caption: Every step, its state, and the critical path.
at: figure tree
thumb: svg
state: none

id: track-progress
kind: chart
track: shared
title: Progress by track
alt: Stacked bars of the roadmap steps in each track, split by state from catalogued to open decision.
caption: Steps by track and state.
at: figure track-progress
thumb: svg
state: none
```
