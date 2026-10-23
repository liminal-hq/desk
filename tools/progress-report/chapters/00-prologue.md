---
id: prologue
n: 0
title: Prologue: a brainstorm in a zip
date: 2026-10-22
summary: Before there was a repository there was a design-agent brainstorm called "Afterglow Desk", and its prototypes are the only pictures of the Desk anyone has seen.
prs: []
---
Every archive has a donation that arrived in a box. Ours arrived as a design brainstorm, and the owner's name for it is "a brainstorm in a zip". I have not seen the zip. What I have is what the first commit says about it, and that is the part I can vouch for: the repository's very first commit, [`a4b0d86`](commit:a4b0d86), at 10:37 on 22 October 2026, brought in the brainstorm's spec, SDK, file formats, palette and prototypes as **working documents, not as something to implement verbatim**. Its own message gives the reason: the design came from an agent that "had no code to check against".

The brainstorm called the project "Afterglow Desk": a graphical shell for MS-DOS, with windows, files and small apps on a 486, that could step aside to run any DOS program full-screen and resume afterwards. The name did not survive — it overlaps with the studio's colour themes — but the *look* did, and keeps its name: the void, the orange accent, the stepped halo on the horizon. `docs/NAMING.md` has the whole argument, and "Desk" is only the working name.

Here is what the prototype looked like. These four pictures are screenshots stored in `design/screenshots/`, taken from the Afterglow Desk prototype. Until the native kernel drew its first window (chapter 12) they were the only pictures of the Desk that existed, and they are still the only ones of the finished design: no code in the repository draws a window yet.

<!-- figure design -->

<!-- facts files docs -->

The first commit also set the rules that every later chapter follows, and the next chapter is about those. For now, the honest summary: this is a **design**, kept as reference only (`design/README.md` says so in its first paragraph), with every number in it a target until something measures it.

## The shelves

### Catalogued
- The brainstorm's material is in the repository: 47 files under `design/` in the initial commit (`git ls-tree -r a4b0d86`), and the commit message states that nothing was built or run on DOS.
- `design/README.md` marks it reference only: "not built or tested", no licence headers, never imported by code.
- The project name is recorded as open, with the candidates and constraints in `docs/NAMING.md`.

### Uncatalogued
- How the brainstorm reached the owner, "in a zip": the owner's phrase, not recorded in the repository. I list it so nobody mistakes it for a documented fact.
- Every figure in the prototype and the early specs (modes, timings, memory use) was written without a code base to check against, and none has been measured.

### Wish list
- A product name chosen (`docs/NAMING.md`; nothing depends on it).
- The 1993-era look, which `docs/OPEN-QUESTIONS.md` defers past the first release.

> **From the stacks — Shells on top of DOS.** Microsoft's first Windows, 1.0, went on sale on 20 November 1985 and was built on the MS-DOS kernel, which makes a graphical shell on top of DOS one of the oldest ideas in the PC world. The Desk's hosted track, which you will meet in chapter 4, is in that lineage; its native track is the part that is not.
> Source: [Windows 1.0, Wikipedia](https://en.wikipedia.org/wiki/Windows_1.0)

```figures
id: design-prototypes
kind: screenshot
track: design
title: The Afterglow Desk prototype
alt: Screenshot of the Afterglow Desk design prototype: a Files window over a dark desktop with a launcher column at the left and a taskbar along the bottom.
caption: Four screenshots of the design brainstorm, the only pictures of the Desk before the first real window.
at: figure design
thumb: img:shots/01-v4-800.jpg
state: merged
```
