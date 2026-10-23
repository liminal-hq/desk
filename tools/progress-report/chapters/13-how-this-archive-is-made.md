---
id: archive
n:
kicker: How this archive is made
title: How this archive is made
date: 2026-10-22
summary: The generator, where its data comes from, the rules for the three shelves, and how a new chapter is added.
prs: []
section: reference
---
This site is not hand-built. It is generated from the repository by Python in `tools/progress-report/`, using only the standard library, `git` and, when it is signed in, `gh`. I will say how, because an archive you cannot audit is a rumour.

**The data.** `gather.py` reads the history of `main` with `git log` (never from branches that have since been deleted), reads counts from the tree of `main` (`git ls-tree`, file contents with `git show`), and reads pull request and CI data from `gh pr list` and `gh run list`. If `gh` is missing or signed out it falls back to a committed snapshot, `snapshots/gh.json`, and the footer of every page says which it used. A few numbers cannot be recomputed from a tree, such as the size of a built kernel; those live in `snapshots/measured.json` with the command that produced them. Every headline number on the front page carries a "how measured" note.

**The counts of tests are counts of declarations.** The generator counts `test(` and `it(` calls in `fonts/test/` and the expected transcripts under `tests/native/` and `spikes/`. That is not an official total and not a runner's tally, and the page says so wherever it uses the number.

**The shelves.** Every chapter ends in three shelves. An item is *catalogued* only when the entry says how it was verified: a merged pull request, a passing CI run, a transcript compared byte for byte, a measured number. It is *uncatalogued* when it is built and nobody has verified it (nothing has run on real hardware, so every emulator result carries an uncatalogued twin). It is on the *wish list* when it is planned or an idea. Nothing is promoted to a higher shelf without evidence, and "there is no operating system yet" stays true until it is not.

**The sidebars.** "From the stacks" boxes are history and trivia that explain where a piece of the work came from. Each claim in one is hedged ("as the story is usually told") or sourced, and none of them counts as evidence about the Desk: they are set apart on purpose, with their own dashed border and a note saying so.

**The machines.** The explainers are driven by real data: the boot stepper by the serial transcript of a QEMU run and `docs/NATIVE-BOOT.md`, the module animation by the transcripts in `spikes/s1-dxe3/expected/`, the pipeline by sizes in `docs/D0-RUST-DOS.md`, and the merge-day replay by `git log`. They work without script, honour `prefers-reduced-motion`, and every step's text is in the list beside the picture.

**Adding a chapter.** Write `tools/progress-report/chapters/NN-name.md` with a small front matter (`id`, `n`, `title`, `date`, `summary`, `prs`), the narrative, the three shelves and a sidebar, run `python3 tools/progress-report/build.py --check`, and publish. The page's links and anchors are checked by the build, and a unit test runs the generator on a small fixture.

**Publishing is manual.** The site is a multi-page Artifact at a fixed address. The generator writes `dist/progress-report/manifest.json` with the exact file mapping and options for the publish call, and the README of the tool repeats the recipe.

## The shelves

### Catalogued
- The generator, its test and its dry run are part of `tools/check.sh`, so a pull request that breaks them fails CI.
- The page's internal links, anchors, ids and tag balance are checked by `build.py --check` on every build.

### Uncatalogued
- The published site's own clicks: publishing needs the owner's sign-in, so I tested the files served locally with a browser driver and could not click the live page.
- Browsers other than the one used for the local check, and screen readers: the explainers use native buttons and live regions, but nobody has listened to them.

### Wish list
- More chapters as work is done: the next ones are expected to be the shared drivers, the hardware abstraction layer and the first window.
- A published copy of the generator's data file next to the pages, so a reader can audit the numbers without the repository.
