---
id: faces
n: 2
title: The faces
date: 2026-10-22
summary: Four pixel faces drawn on grids, and three outline families drawn in code: the Desk's own type, built reproducibly by one script.
prs: [1]
---
Before a system can say anything it needs letters, and the Desk drew its own. PR #1 ({{pr1.commits}} commits, {{pr1.added}} lines added) brought in two families. The **pixel faces** are drawn once as grids of dots and built into three file types: `Desk Fixed` (8 by 16), `Desk UI` (11 px proportional), `Desk Title` (12 px bold) and `Desk Small` (6 by 8), each in Square and Round styles, as `.otf`, `.ttf` and a `.dfn` bitmap that a DOS program can read directly. The **outline faces**, `Desk Sans`, `Desk Mono` and `Desk Serif` in four styles each, are drawn in code with a stroke kit, nothing traced from anything else, for larger sizes and for modern systems.

You are reading this page in them. The headings are `Desk Title`, the prose is `Desk Sans`, the numbers are `Desk Mono`, all loaded from the font files this site publishes under `fonts/`, which the generator copies from `fonts/generated/` in the repository. Here they are at work on the Desk's dark ground and its light one.

<!-- figure specimens -->

The era matters to the way the files are built. The `.ttf` files follow what Windows 3.1 expected: the required tables, a `cmap` for the 3,1 and 1,0 encodings over CP1252, name records, OS/2 version 1, `post` 2.0, and every face lands on a whole point size at 96 dpi. `Desk Fixed DOS` carries the 256 characters of code page 437 at their DOS byte values as a symbol-encoded font for ANSI programs, with a Unicode table beside it for modern systems. Kerning is counted in the file itself: the generator reads {{kern.desk_sans}} pairs from the `kern` table of `desk-sans.ttf` and {{kern.desk_serif}} from `desk-serif.ttf` (PR #1 says "about 430"), and the `.otf` files carry the same pairs as GPOS for modern shaping. Ligatures (`fi`, `fl`, `ff`, `ffi`, `ffl`) and the code ligatures of `Desk Mono` (`->`, `=>`, `!=`, `<=`, `===`) live in the OpenType tables, which era systems ignore.

<!-- facts fontFiles bunTests -->

Everything is built by one script, `fonts/build.ts`, and `bun build.ts --check` fails if a generated file differs from what the sources would produce. That is how a directory of {{m.fontFiles}} generated font files, plus the `.dfn` bitmaps, stays honest.

## The shelves

### Catalogued
- The build is reproducible: PR #1's test plan records `bun build.ts --check` and `bun test` passing, 360 tests at the time (the generator's own count of test declarations is a different number and is labelled as such above), and CI is green on the merge commit `{{pr1.merge}}`.
- In Docker, fontTools and OTS accepted every font file with no warnings, FreeType rendered every Square glyph pixel-exact against the `.dfn` bitmaps, and HarfBuzz shaped every kerning pair, ligature and feature as expected in all 24 outline files (PR #1 test plan).
- The `kern` table alone matches GPOS; the outline `.otf` and `.ttf` files agree within 3.2 percent of glyph ink at 48 px (same source).
- Proof sheets were reviewed in headless Chromium and FreeType at 11 to 48 px.

### Uncatalogued
- Windows 3.1, 95, NT and OS/2, GDI and DirectWrite, and macOS: none was run.
- The faces on a DOS screen or in the Desk: no DOS program has drawn them yet.
- `Desk Small` and the outline faces below about 14 px need the closest look, and `Desk Mono Bold`'s `a` still has a very small counter (PR #1).
- Chromium treats the `Desk Fixed DOS` `.ttf` as a symbol font; the `.otf` works for Unicode text.

### Wish list
- Reading TrueType files inside the Desk, so that any font can be used (spike `s6-ttf`, written up in `docs/OPEN-QUESTIONS.md`, not started).
- Checking whether the four bitmap faces are the ones that ship; `docs/OPEN-QUESTIONS.md` says the 6 by 8 face is the one to check first.

> **From the stacks — TrueType.** As the story is usually told, Apple developed TrueType in the late 1980s as a rival to Adobe's Type 1 fonts and released it with Mac System 7 in May 1991. Microsoft added it to Windows 3.1, the Windows of the Desk's own era, which is why the `.ttf` files here follow its rules. OpenType with CFF outlines, which is what the `.otf` files are, only arrived in the late 1990s, so it is no part of that story.
> Source: [TrueType, Wikipedia](https://en.wikipedia.org/wiki/TrueType)
