# progress-report

The generator for the Desk Progress Report, a multi-page archive of the project's history and status. It reads the history and tree of `main` and the pull request and CI data from `gh`, and writes a static site to `dist/progress-report/` (git-ignored). Python standard library only, plus `git` and, optionally, `gh` on the PATH; Chromium is only needed if you want screenshots.

## Contents

- [Quick start](#quick-start)
- [Layout](#layout)
- [How the data is gathered](#how-the-data-is-gathered)
- [Chapters](#chapters)
- [Explainers and figures](#explainers-and-figures)
- [The figure registry and the gallery](#the-figure-registry-and-the-gallery)
- [The manual](#the-manual)
- [Updating the manual](#updating-the-manual)
- [Checks](#checks)
- [Publishing](#publishing)

## Quick start

```bash
python3 tools/progress-report/gather.py --out dist/progress-report-data.json   # needs git; uses gh if signed in; reads the open branches too
python3 tools/progress-report/build.py --data dist/progress-report-data.json --check
python3 -m http.server -d dist/progress-report 8000                            # then open http://127.0.0.1:8000/
```

`build.py` without `--data` gathers first. `gather.py --offline` skips `gh` and reads the committed snapshot; `gather.py --update-snapshot` refreshes `snapshots/gh.json` from `gh` and the branches (it needs a signed-in `gh` and every open pull request's branch fetched); `gather.py --update-captures` re-copies the files under `media/` that other branches hold. `gather.py --dry-run` prints a summary and writes nothing; it works in a shallow clone and reads whatever is checked out, which is what `tools/check.sh` runs.

## Layout

| Path | What it is |
|---|---|
| `gather.py` | Reads `git` and `gh`, counts from the tree, and writes one JSON record (`data.json`) |
| `build.py` | Renders the pages, copies the assets, writes `manifest.json`, and checks the result with `--check` |
| `mdlite.py` | The small markdown reader for the chapter files |
| `explainers.py`, `arch.py` | The step-through explainers and figures of the first chapters, each driven by real data |
| `explainers_hw.py` | The explainers of the hardware chapters, driven by captured files, and the screenshots |
| `explainers_fw.py` | The first window's step-through (read from the scenario's sums and transcripts) and the anatomy diagrams (read from `anatomy-dos.json` and `anatomy-desk.json`) |
| `anatomy-dos.json`, `anatomy-desk.json` | The boxes, arrows, statuses and evidence of the two anatomy diagrams |
| `charts.py` | The font matrix, emoji coverage, progress by track and the stack of open pull requests (drawn only while pull requests are open) |
| `figures.py` | The figure registry: parsing, validation and the gallery's previews |
| `chapters/NN-id.md` | One chapter per stretch of the journey, in reading order |
| `pages/front.md` | The text at the top of the front page |
| `roadmap.json` | The path tree: steps, dependencies, status, evidence |
| `not-verified.json` | The "not verified" panel |
| `snapshots/` | `gh.json` (a snapshot of `gh` data for offline runs) and `measured.json` (numbers that need a build to measure, with the command) |
| `media/` | A captured serial transcript and screenshot of the native boot (how they were made is in `snapshots/measured.json`), and the files copied from other branches, listed in `captures.json` |
| `manual.py`, `manual/` | The Manual: `plan.json` (parts, chapters, both reading orders, the task index), `MNN-id.md` (one file per chapter), `explainers/*.py` (one module per manual explainer), and the code that loads, checks and draws them |
| `manual_maps.py` | The tables of appendix B (memory while booting, ports, IRQ lines and vectors, exception vectors, BIOS calls, exit statuses, test modes), read from the files that hold the numbers, each row with a proof that a test checks |
| `pages/manual-preface.md` | The archivist's preface on `manual.html` |
| `pages/front.md`, `pages/coming.json` | The text at the top of the front page, and the chapters that are expected and not written, which show as "coming" rows |
| `templates/site.css`, `templates/site.js` | Shared style and behaviour |
| `tests/` | `unittest` tests and a fixture |

## How the data is gathered

- **History** comes from `git log --first-parent main` (the merge commit of each pull request, found by its `(#N)` subject), `git log base..head` on each merge commit's parents (the branch's commits, even when the branch is deleted), `git diff --numstat` for sizes, and `git log --merges` for the "Merge main into …" commits whose messages hold the conflict notes.
- **Counts** come from the tree of `main`: files, lines of source per area, test declarations (`test(` and `it(` in `fonts/test`), expected transcripts, the emoji counts in the `.dem` headers, glyph and kerning counts parsed from the font tables, and the executable sizes recorded in `docs/D0-RUST-DOS.md`. The test figures are counts of declarations and files, not a test runner's total, and the pages say so.
- **Open pull requests** come from `gh pr list` (state, draft, the check rollup of the head commit) and their branches from `git`: the commits a branch has that its base and the other open branches it contains do not, the `#[test]` attributes under `hal/`, and the expected transcripts, counted at the tip of each. The state tag of every pull request and roadmap step (merged, in review, in progress or planned) is derived from that, never typed.
- **Pull requests and CI** come from `gh pr list --state all` and `gh run list --branch main`. Without a signed-in `gh` the generator falls back to `snapshots/gh.json` and every page footer says so.
- **Measured numbers** that a tree cannot give (the size of a built kernel) live in `snapshots/measured.json` with the command that produced them.
- **Honesty checks** run on every build: a roadmap step marked done whose pull request is not merged with CI passing drops to "partly done"; the boot explainer refuses a transcript that does not satisfy `tests/native/expected.txt` or whose usable-RAM line is not the sum of its map entries; every placeholder and link in the prose must resolve.

## Chapters

A chapter is `chapters/NN-id.md`: a front matter, then the narrative. Front matter keys: `id` (the page is `id.html`), `n` (the chapter number, empty for reference pages), `title`, `date` (ISO), `summary`, `prs` (a list of pull request numbers, which fill the evidence table), `section` (`journey` or `reference`), and `kicker` for pages without a number. Prose follows the repository's markdown rules (`AGENTS.md`): one line per paragraph, real em dashes, Canadian spelling.

- Inline: `**bold**`, `*em*`, `` `code` ``, Markdown links, and placeholders such as `{{m.prs}}`, `{{pr3.merge}}`, `{{size.HELLO_EXE}}` or `{{kern.desk_sans}}`, which come from the data and fail the build if unknown. Link schemes: `chapter:ID#anchor`, `pr:N`, `commit:HASH`, `repo:PATH`.
- Directives (HTML comments on their own line): `<!-- explainer boot -->` (also `modules`, `pipeline`, `mergeday`, `pit`, `input`, `vga13`, `vbeattrs`, `pipelines`, `firstwindow`), `<!-- figure NAME -->` (`fw-digests`, `anatomy-dos`, `anatomy-desk`, `hero`, `arch`, `tree`, `nexts`, `specimens`, `contact`, `faces`, `design`, `qemu`, `conflicts`, `hal-layers`, `bootinfo`, `std-sizes`, `font-matrix`, `emoji-coverage`, `track-progress`, `pr-stack`, and `shots FILE...` for captured screenshots), `<!-- table FILE -->` (a table captured from another branch's document) and `<!-- facts KEY KEY -->` (headline numbers with "how measured"). A chapter ends with a ```` ```figures ```` block (see below) and then the three shelves.
- Every chapter ends with the three shelves, as `### Catalogued`, `### Uncatalogued` and `### Wish list` followed by bullet lists. *Catalogued* entries say how they were verified, *uncatalogued* entries are built but unverified (nothing has run on real hardware, so an emulator result has an uncatalogued twin), and the *wish list* is planned or an idea. A "From the stacks" sidebar is a block quote that starts `> **From the stacks — Title.**`, with a `> Source: [..](..)` line; it is history for context, set apart, and never evidence.

## Explainers and figures

Each step-through explainer is plain HTML with every step's text in an ordered list, so it reads without script. `templates/site.js` adds Restart, Previous, Play or Pause and Next buttons, the arrow, Home and End keys, and a live region; nothing autoplays, and transitions are off under `prefers-reduced-motion`. In the markup, `data-from="N"` shows an element from step N, `data-to="N"` hides it after step N, and `data-at="N"` marks the step in which something happens.

## The figure registry and the gallery

Every picture, diagram, chart and explainer is declared once, in the chapter that holds it, in a fenced block that starts with `figures`: records separated by a blank line, each line `key: value`. The keys are `id`, `kind` (`screenshot`, `specimen`, `diagram`, `chart`, `explainer` or `history`), `track` (`hosted`, `native`, `shared` or `design`), `title`, `alt`, `caption`, `at` (the directive that draws it, without the comment marks), `thumb` (`img:PATH`, `svg`, `svgs`, `steps` or `specimen`), and optionally `anchor`, `state` and `date`. From those records the generator wraps each figure in an anchor (`fig-ID`), builds `gallery.html` (tiles that link to the chapter and anchor, filters by kind and track that are links and work from the keyboard, a newest-first order and a list view) and picks the front page's strip of recent figures. Thumbnails of screenshots are made with Pillow when it is installed; without it the tiles use the full images.

The build refuses a record with no alt text, an unknown kind or track, a duplicate id, a chapter that does not exist, a directive that the chapter does not contain, or a drawing directive that no record claims; and `--check` fails on a link from the gallery to an anchor or an image that does not exist. Adding a chapter's figures to its `figures` block adds them to the gallery.

## Updating the anatomy pages

`chapters/22-anatomy-rust-dos.md` is finished for now. `chapters/23-anatomy-rust-desk.md` is meant to grow as the app model is built, and its diagram is data: `anatomy-desk.json` holds the layers, the boxes (`layer`, `col` and `span` on a grid of twelve columns), the arrows, and for each box a `status`, one line of `text` and a list of `evidence`.

- `status` is `verified` (built and run in an emulator or tested on the host), `unverified` (built, not verified) or `planned` (drawn dashed, the wish list). A box is never promoted without evidence: `evidence` entries are `pr:N`, `commit:HASH` or `repo:PATH`, a built box must have at least one, and a `repo:` path must exist in the tree.
- When something lands (an app model, a loader, system calls, window ownership), change that box's `status`, `sub`, `text` and `evidence`, add boxes or arrows if the picture needs them, and add a dated entry to "Revisions" at the bottom of the chapter; move the matching line from its "Open questions" and "Wish list" to the shelf it has earned, and update the `updated` date in the JSON. No drawing code changes.
- The tests refuse a box with a status outside the three, a built box without evidence, a `repo:` path that does not exist, overlapping boxes and an arrow to a box that is not there.

The first window's step-through reads `tests/shell/first-window.sums`, the damage lines of `tests/native/expected-shell-*.txt` and the result table of `docs/FIRST-WINDOW.md`, and refuses to build if the table and the sums disagree or the scenario's frames change; its pictures are `media/img-fw-*.png`, copies of the goldens and of the kept native screendumps. The boot explainer's transcript, `media/native-serial.txt`, is the unedited output of `tools/native-run.sh --serial FILE` on the image built from `main` (carriage returns removed), with the build and the image's checksum recorded in `snapshots/measured.json`.

## The manual

A second group of pages beside the log, planned in [`docs/MANUAL-PLAN.md`](../../docs/MANUAL-PLAN.md): a technical manual and textbook of what has been built, by layer from the processor upward. A chapter is `manual/MNN-id.md`, written to the template in the plan (what it is, how it works, why, the code, a worked example, the state of the layer, history sidebars, commands to try, where next). `manual/plan.json` lists every chapter, written or not, the two reading orders ("Learn it", bottom-up, which the pager follows, and "Find it", top-down) and the task index. Chapters in the plan that are not written show as "coming" rows on the contents and on `manual.html`.

Three fenced blocks in a chapter are data, and the generator renders them in place:

- `status`: records separated by a blank line, each with `id`, `label`, `state` (`built-verified`, `built-unverified`, `planned` or `external`), `evidence` and an optional `note`. Evidence items are `pr:N`, `commit:HASH`, `repo:PATH`, `sym:PATH::NAME` (the name appears in the file), `test:PATH::NAME` (a test function or a transcript line containing the name) and `doc:PATH#ANCHOR` (the heading exists). A `built-verified` record needs a `test:` item or a `pr:` item whose pull request is merged with CI passing; without one it is shown as built, not verified, and the table says why. A `planned` record cites where it is planned. `{{state.ID}}` in the prose draws the same tag as the table.
- `try`: `needs:` and `ran:` lines, then commands (`$ cmd`), each followed by the output it printed. Every command must start with a script that exists in the repository or a known tool, and the script an interpreter runs must exist.
- `glossary`: records with `term` and `def`. Each term is defined once and used at least once with a `[word](term:id)` link, and the glossary page is built from them.

Other link schemes in the manual are `sym:PATH::NAME`, `test:PATH::NAME`, `doc:PATH#ANCHOR` and `term:ID`, besides `repo:`, `pr:`, `commit:` and `chapter:`. A link to a chapter that is not written yet points at its row on `manual.html`. Two directives are required in every chapter, `<!-- manual proved -->` (the log chapters and pull requests that proved the layer) and `<!-- manual next -->` (the next chapter on each path and the tasks that lead here). A manual chapter can draw a figure the log already declares by giving it a `figures` record with `reuse: FIGURE-ID` and the same `at`; the gallery keeps the one tile. New manual explainers are modules in `manual/explainers/` with an `ID` and `render(D, ctx)`; they are found by file name, loaded by path and used as `<!-- explainer ID -->`.

Each chapter's `reviewed` is the commit of `main` it was last checked against and `watches` the paths whose changes may make it stale. At build time the generator runs `git log REVIEWED..HEAD -- WATCHES` and shows a notice with the commits and pull requests, and a pill on the contents; in a clone that does not have the commit it says it could not tell. `build.py --manual-report` prints the same as text, with the status records that are shown lower than they claim.

The build refuses a chapter with a missing section, a path or symbol that does not exist, a state that its evidence does not support, a command that does not exist, an undefined or unused glossary term, or a `log` or `anatomy` reference that does not resolve. The log pages are unchanged by the manual except for a computed "In the manual" aside on the chapters a manual chapter names, a "Manual" column (and anchors) on the anatomy pages' text tables, and the "Manual" link in the header.

A chapter that needs checks of its own (a number it quotes against a source, a table against the code) keeps them in `tests/test_chapter_NAME.py`, a self-contained `unittest` module that `test_report.py` picks up, so the one test command runs them all.

## Updating the manual

1. **In a code pull request.** If the checks fail on a manual pointer, fix the pointer (a path or a name), nothing more.
2. **After the merge.** Run `python3 tools/progress-report/build.py --manual-report` to list, for each chapter, the commits that touched its watched paths since `reviewed` and the status records shown lower than they claim.
3. **Update the chapter.** Change the prose the change made wrong; promote or add status records with their evidence (never promote without it); add the new log chapter to `log`; add a Revisions entry; set `reviewed` to the merge commit. If an anatomy box changed, change the box and the chapter in the same branch.
4. **Check and publish.** `python3 -I tools/progress-report/tests/test_report.py`, `python3 tools/progress-report/build.py --check`, then [Publishing](#publishing).

## Checks

`tools/check.sh` runs the cheap parts: the unit tests and a dry run of `gather.py` (no network; `gh` is skipped when it is not signed in).

```bash
python3 -I tools/progress-report/tests/test_report.py     # the unit tests
python3 -I tools/progress-report/gather.py --dry-run      # a dry run of the data gather
python3 -I tools/progress-report/build.py --data DATA --check   # a full build and link check
```

The tests cover the figure registry (a broken record, a missing alt text, an unclaimed drawing, an anchor or image that does not exist), the open-work reader on a throwaway repository with stacked branches, the roadmap's tags, the markdown reader, `gather.py` on a throwaway git repository and on a shallow clone, a build of a small fixture with checks for valid tags, unique ids, internal links and anchors, the checker itself on a deliberately broken page, the manual's checks, each with a twin that breaks the rule and must be refused (a missing section, a pointer to a renamed symbol, a state without run evidence, a command that does not exist, an unused glossary term, a figure reused wrongly), a fixture build of a manual with its contents, pager, backlinks and anatomy column, and the real chapters (front matter, three shelves, sidebar sources, chapter links) and the boot explainer against `tests/native/expected.txt`; the hardware explainers are built from the captured files and must refuse a capture that disagrees with the arithmetic or the decoder they replay.

For a visual check, serve `dist/progress-report/` and look at every page at 400 and 1280 px, in light and dark, with and without `prefers-reduced-motion`; the file protocol does not load the pages' fonts and assets, so use a server.

## Publishing

Publishing is manual, with the Artifact tool, because it needs the owner's sign-in. The page is a multi-file Artifact at a fixed address, `https://claude.ai/artifact/UiyueA93UpACK4UnMMh5as`, titled "Desk Progress Report". The build writes `dist/progress-report/manifest.json` with the exact call; this is the recipe it encodes:

1. Build: `python3 tools/progress-report/build.py --check` (gather runs first).
2. Read the live artifact once with `action: "read"` (the read prints a lot), so a republish is allowed.
3. Publish to the same `url` with `file_path` set to `dist/progress-report/_publish/index.html` (the front page as a fragment: the Artifact skeleton supplies the doctype, head and body), `root` set to `dist/progress-report`, and `files` set to the `publish.files` map in `manifest.json` (published path to source path under `root`, every page, thumbnail and asset except the index, the gallery among them). Pass the `description` from the manifest, omit `icon` and `title` on a republish, and keep the title in the page: "Desk Progress Report".
4. Fetch a few published paths back with `action: "read"` and `path` to confirm they exist.

The paths in `manifest.json` are relative to the repository root; use absolute paths in the call when the tool's working directory is another checkout or a worktree. Every page other than the index is a full HTML document with its own doctype, charset and viewport metas, served as it is. Limits: 16 MB per page and per file, 255 files and 64 MB per publish; the manifest lists the sizes and SHA-256 of every file.
