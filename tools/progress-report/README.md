# progress-report

The generator for the Desk Progress Report, a multi-page archive of the project's history and status. It reads the history and tree of `main` and the pull request and CI data from `gh`, and writes a static site to `dist/progress-report/` (git-ignored). Python standard library only, plus `git` and, optionally, `gh` on the PATH; Chromium is only needed if you want screenshots.

## Contents

- [Quick start](#quick-start)
- [Layout](#layout)
- [How the data is gathered](#how-the-data-is-gathered)
- [Chapters](#chapters)
- [Explainers and figures](#explainers-and-figures)
- [The figure registry and the gallery](#the-figure-registry-and-the-gallery)
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
| `charts.py` | The font matrix, emoji coverage, progress by track and the stack of open pull requests |
| `figures.py` | The figure registry: parsing, validation and the gallery's previews |
| `chapters/NN-id.md` | One chapter per stretch of the journey, in reading order |
| `pages/front.md` | The text at the top of the front page |
| `roadmap.json` | The path tree: steps, dependencies, status, evidence |
| `not-verified.json` | The "not verified" panel |
| `snapshots/` | `gh.json` (a snapshot of `gh` data for offline runs) and `measured.json` (numbers that need a build to measure, with the command) |
| `media/` | A captured serial transcript and screenshot of the native boot (how they were made is in `snapshots/measured.json`), and the files copied from other branches, listed in `captures.json` |
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
- Directives (HTML comments on their own line): `<!-- explainer boot -->` (also `modules`, `pipeline`, `mergeday`, `pit`, `input`, `vga13`, `vbeattrs`, `pipelines`), `<!-- figure NAME -->` (`hero`, `arch`, `tree`, `nexts`, `specimens`, `contact`, `faces`, `design`, `qemu`, `conflicts`, `hal-layers`, `bootinfo`, `std-sizes`, `font-matrix`, `emoji-coverage`, `track-progress`, `pr-stack`, and `shots FILE...` for captured screenshots), `<!-- table FILE -->` (a table captured from another branch's document) and `<!-- facts KEY KEY -->` (headline numbers with "how measured"). A chapter ends with a ```` ```figures ```` block (see below) and then the three shelves.
- Every chapter ends with the three shelves, as `### Catalogued`, `### Uncatalogued` and `### Wish list` followed by bullet lists. *Catalogued* entries say how they were verified, *uncatalogued* entries are built but unverified (nothing has run on real hardware, so an emulator result has an uncatalogued twin), and the *wish list* is planned or an idea. A "From the stacks" sidebar is a block quote that starts `> **From the stacks — Title.**`, with a `> Source: [..](..)` line; it is history for context, set apart, and never evidence.

## Explainers and figures

Each step-through explainer is plain HTML with every step's text in an ordered list, so it reads without script. `templates/site.js` adds Restart, Previous, Play or Pause and Next buttons, the arrow, Home and End keys, and a live region; nothing autoplays, and transitions are off under `prefers-reduced-motion`. In the markup, `data-from="N"` shows an element from step N, `data-to="N"` hides it after step N, and `data-at="N"` marks the step in which something happens.

## The figure registry and the gallery

Every picture, diagram, chart and explainer is declared once, in the chapter that holds it, in a fenced block that starts with `figures`: records separated by a blank line, each line `key: value`. The keys are `id`, `kind` (`screenshot`, `specimen`, `diagram`, `chart`, `explainer` or `history`), `track` (`hosted`, `native`, `shared` or `design`), `title`, `alt`, `caption`, `at` (the directive that draws it, without the comment marks), `thumb` (`img:PATH`, `svg`, `svgs`, `steps` or `specimen`), and optionally `anchor`, `state` and `date`. From those records the generator wraps each figure in an anchor (`fig-ID`), builds `gallery.html` (tiles that link to the chapter and anchor, filters by kind and track that are links and work from the keyboard, a newest-first order and a list view) and picks the front page's strip of recent figures. Thumbnails of screenshots are made with Pillow when it is installed; without it the tiles use the full images.

The build refuses a record with no alt text, an unknown kind or track, a duplicate id, a chapter that does not exist, a directive that the chapter does not contain, or a drawing directive that no record claims; and `--check` fails on a link from the gallery to an anchor or an image that does not exist. Adding a chapter's figures to its `figures` block adds them to the gallery.

## Checks

`tools/check.sh` runs the cheap parts: the unit tests and a dry run of `gather.py` (no network; `gh` is skipped when it is not signed in).

```bash
python3 -I tools/progress-report/tests/test_report.py     # the unit tests
python3 -I tools/progress-report/gather.py --dry-run      # a dry run of the data gather
python3 -I tools/progress-report/build.py --data DATA --check   # a full build and link check
```

The tests cover the figure registry (a broken record, a missing alt text, an unclaimed drawing, an anchor or image that does not exist), the open-work reader on a throwaway repository with stacked branches, the roadmap's tags, the markdown reader, `gather.py` on a throwaway git repository and on a shallow clone, a build of a small fixture with checks for valid tags, unique ids, internal links and anchors, the checker itself on a deliberately broken page, and the real chapters (front matter, three shelves, sidebar sources, chapter links) and the boot explainer against `tests/native/expected.txt`; the hardware explainers are built from the captured files and must refuse a capture that disagrees with the arithmetic or the decoder they replay.

For a visual check, serve `dist/progress-report/` and look at every page at 400 and 1280 px, in light and dark, with and without `prefers-reduced-motion`; the file protocol does not load the pages' fonts and assets, so use a server.

## Publishing

Publishing is manual, with the Artifact tool, because it needs the owner's sign-in. The page is a multi-file Artifact at a fixed address, `https://claude.ai/artifact/UiyueA93UpACK4UnMMh5as`, titled "Desk Progress Report". The build writes `dist/progress-report/manifest.json` with the exact call; this is the recipe it encodes:

1. Build: `python3 tools/progress-report/build.py --check` (gather runs first).
2. Read the live artifact once with `action: "read"` (the read prints a lot), so a republish is allowed.
3. Publish to the same `url` with `file_path` set to `dist/progress-report/_publish/index.html` (the front page as a fragment: the Artifact skeleton supplies the doctype, head and body), `root` set to `dist/progress-report`, and `files` set to the `publish.files` map in `manifest.json` (published path to source path under `root`, every page, thumbnail and asset except the index, the gallery among them). Pass the `description` from the manifest, omit `icon` and `title` on a republish, and keep the title in the page: "Desk Progress Report".
4. Fetch a few published paths back with `action: "read"` and `path` to confirm they exist.

The paths in `manifest.json` are relative to the repository root; use absolute paths in the call when the tool's working directory is another checkout or a worktree. Every page other than the index is a full HTML document with its own doctype, charset and viewport metas, served as it is. Limits: 16 MB per page and per file, 255 files and 64 MB per publish; the manifest lists the sizes and SHA-256 of every file.
