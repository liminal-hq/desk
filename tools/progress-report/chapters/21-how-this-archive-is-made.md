---
id: archive
n:
kicker: How this archive is made
title: How this archive is made
date: 2026-10-23
summary: The generator, where its data comes from, the figure registry behind the gallery, the rules for the three shelves, the commands, and the commits of the branches this edition read.
prs: []
section: reference
---
This site is not hand-built. It is generated from the repository by Python in `tools/progress-report/`, using only the standard library, `git` and, when it is signed in, `gh`; Pillow, if it is installed, makes the gallery's thumbnails. I will say how, because an archive you cannot audit is a rumour. This edition was generated on {{generated}} from `main` at `{{head}}`.

**The data.** `gather.py` reads the history of `main` with `git log` (never from branches that have since been deleted), reads counts from the tree of `main` (`git ls-tree`, file contents with `git show`), and reads pull request and CI data from `gh pr list` and `gh run list`. For the open pull requests it also reads their branches: the commits a branch has that its base and the other open branches it contains do not, the test and transcript counts of each tree, and the state tag of each pull request (merged, in review, in progress), which comes from `gh` and from nothing I typed. If `gh` is missing or signed out it falls back to a committed snapshot, `snapshots/gh.json`, which also keeps what the branches held when it was taken, and the footer of every page says which it used. A few numbers cannot be recomputed from a tree, such as the size of a built kernel; those live in `snapshots/measured.json` with the command that produced them. Every headline number on the front page carries a "how measured" note, and the numbers about open work are kept apart from the numbers about `main`.

**Captures.** The explainers of the hardware chapters replay files that live on other branches. `media/captures.json` lists each one (the branch, the path, and, for a table or a transcript inside a document, where to find it), `gather.py --update-captures` copies them, and every build compares each copy with its branch and says when one differs or could not be checked. A figure that is derived, not recorded, says so in its caption, and the build refuses to render it unless the derivation reproduces the recorded lines. The branches the first edition read, with the commit each now has on `main` (the merge commit, now that every one of them is merged):

| Pull request | Branch | Merge commit | State |
|---|---|---|---|
| #8 | `feat/hal-design` | `{{pr8.head}}` | {{pr8.state}} |
| #10 | `spike/c-kernel-input` | `{{pr10.head}}` | {{pr10.state}} |
| #11 | `spike/a-vga-vbe-rust` | `{{pr11.head}}` | {{pr11.state}} |
| #12 | `spike/e-std-stubs` | `{{pr12.head}}` | {{pr12.state}} |
| #13 | `spike/s5-jsdos` | `{{pr13.head}}` | {{pr13.state}} |
| #14 | `spike/d-native-vga` | `{{pr14.head}}` | {{pr14.state}} |
| #15 | `spike/b-dos-input` | `{{pr15.head}}` | {{pr15.state}} |
| #16 | `spike/g-boot-bochs-86box` | `{{pr16.head}}` | {{pr16.state}} |
| #17 | `spike/n3-native-bios` | `{{pr17.head}}` | {{pr17.state}} |

The generator itself was merged as #9 (merge commit `{{pr9.merge}}`). The pull request that carried the first edition of this report (#18) is merged too and has no row of its own.

**The counts of tests are counts of declarations.** The generator counts `test(` and `it(` calls in `fonts/test/`, `#[test]` attributes in `hal/` on the open branches, and the expected transcripts under `tests/native/` and `spikes/`. That is not an official total and not a runner's tally, and the page says so wherever it uses the number. Where a pull request's own document states a test count, the generator's count agrees with it, which is how I checked the counter.

**The shelves.** Every chapter ends in three shelves. An item is *catalogued* only when the entry says how it was verified: a merged pull request, a passing CI run, a transcript compared byte for byte, a measured number. It is *uncatalogued* when it is built and nobody has verified it (nothing has run on real hardware, so every emulator result carries an uncatalogued twin, and anything in an open pull request is also unmerged). It is on the *wish list* when it is planned or an idea. Nothing is promoted to a higher shelf without evidence, and "there is no operating system yet" stays true until it is not.

**The sidebars.** "From the stacks" boxes are history and trivia that explain where a piece of the work came from. Each claim in one is hedged ("as the story is usually told") or sourced, and none of them counts as evidence about the Desk: they are set apart on purpose, with their own dashed border and a note saying so.

**The manual.** A second group of pages, reached from the header, explains each layer on its own terms in a plain textbook voice, with where the code is and what has been verified. It is checked by the same build: every path and symbol a chapter points at must exist, and a mechanism is tagged built and verified only when its evidence includes a test or a merged pull request with CI passing. The log's chapters link to it, and it links back, with notices when the code a chapter watches has moved since it was reviewed. The details are in the generator's README.

**The machines.** The explainers are driven by real data: the boot stepper by the serial transcript of a QEMU run and `docs/NATIVE-BOOT.md`, the module animation by the transcripts in `spikes/s1-dxe3/expected/`, the pipeline by sizes in `docs/D0-RUST-DOS.md`, the merge-day replay by `git log`, and the newer ones by captures: the PIT test by the driver's own test code, the input replay by the recorded transcript and injection script, the mode 13h walkthrough by the register tables in `regs.rs`, and the two input pipelines by two recorded runs. They work without script, honour `prefers-reduced-motion`, and every step's text is in the list beside the picture.

**The figure registry and the gallery.** Each chapter declares its figures in a `figures` block at the end of the file: an id, a kind (screenshot, specimen, diagram, chart, explainer or history), a track, a title, alt text, a caption, the directive that draws it and the kind of preview. From those records the generator wraps each figure in an anchor in its chapter, builds the gallery page with a tile for each, and picks the strip of recent figures on the front page. The build refuses a record with no alt text, a directive the chapter does not contain, or an anchor or image that does not exist, and it refuses a chapter that draws something no record claims, so the links cannot rot silently. The gallery's tiles are real links; its filters are links that also work from the keyboard and set `aria-pressed`; its list view is a table of the same records.

**Commands.**

```
python3 tools/progress-report/gather.py --out dist/progress-report-data.json     # read git and gh
python3 tools/progress-report/gather.py --update-snapshot                        # refresh the committed gh snapshot
python3 tools/progress-report/gather.py --update-captures                        # re-copy the captured files from their branches
python3 tools/progress-report/build.py --data dist/progress-report-data.json --check
python3 -I tools/progress-report/tests/test_report.py
```

**Adding a chapter.** Write `tools/progress-report/chapters/NN-name.md` with a small front matter (`id`, `n`, `title`, `date`, `summary`, `prs`), the narrative, a `figures` block, the three shelves and a sidebar, run `python3 tools/progress-report/build.py --check`, and publish. The chapter appears in the contents, the pager and the gallery without any other file changing. A chapter that is expected and not yet written can be named in `pages/coming.json` and shows as a "coming" row until its chapter exists; that file is empty now that spikes G and n3 have chapters.

**Publishing is manual.** The site is a multi-page Artifact at a fixed address. The generator writes `dist/progress-report/manifest.json` with the exact file mapping and options for the publish call, and the README of the tool repeats the recipe.

## The shelves

### Catalogued
- The generator, its tests and its dry run are part of `tools/check.sh`, so a pull request that breaks them fails CI.
- The page's internal links, anchors, ids and tag balance are checked by `build.py --check` on every build, and so is every link from the gallery to a figure.
- The registry's rules (alt text, existing chapters, anchors and images) are covered by unit tests that fail when a record is broken.
- The captured files are compared with their branches on every build where the branches are available.

### Uncatalogued
- The published site's own clicks: publishing needs the owner's sign-in, so I tested the files served locally with a browser driver and could not click the live page.
- Browsers other than the one used for the local check, and screen readers: the explainers and the gallery's filters use native buttons, links and live regions, but nobody has listened to them.
- The thumbnails, when Pillow is missing: the tiles then point at the full images, which has been reasoned about and not looked at.
- The derived figures, in the cases where the recorded run did not log what they show (the scancode bytes and the ring indices).

### Wish list
- A published copy of the generator's data file next to the pages, so a reader can audit the numbers without the repository.
- A lightbox for the gallery, if linking to the figure in place turns out to be too far.
