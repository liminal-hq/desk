---
id: path
n:
kicker: The path
title: The path
date: 2026-10-22
summary: Everything still to do, as a tree that grows from the Desk to its first milestone, with the critical path marked.
prs: []
section: reference
---
A shelf of finished things is only half an archive; the other half is the list of what is missing. This is the path, drawn from `tools/progress-report/roadmap.json`. Its target is one milestone, **First Window**: a window drawn by the shared Desk core and moved with a mouse, on DOS and on the bare-metal kernel. Nothing like it exists yet, and the tree says so with a box that is still dashed.

The architecture comes first, because the tree is easier to read once you know which layers exist.

<!-- figure arch -->

Now the tree. Every box is a step; the colour and shape of its icon say what has been earned. Choose a box, or tab to it, to read what it is, why it matters, what it needs and what it unlocks. On a narrow screen the same steps appear as a list.

<!-- figure tree -->

And the next move of each branch, with what is in its way.

<!-- figure nexts -->

## The shelves

### Catalogued
- The steps marked done have a merged pull request and a passing CI run on its merge commit: the generator drops any step to "partly done" if that is not true.
- "Catalogued in an emulator" is as far as any native or DOS step goes: the checks run in QEMU or headless DOSBox Staging.

### Uncatalogued
- Every step marked "catalogued in an emulator" is uncatalogued on real hardware. No step has a real machine behind it.
- The roadmap's dependency edges and the critical path are my reading of `docs/DIRECTION.md`, `docs/SPEC.md` and `docs/RUST-TARGETS.md`; the generator checks the statuses, not the edges.

### Wish list
- Every dashed or dotted box in the tree: the HAL, the shared drivers, the shell, the handoff, the core apps, the native desktop and the milestone itself.
