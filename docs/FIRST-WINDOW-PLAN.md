# First Window: the build plan

The task-by-task plan for building [`FIRST-WINDOW.md`](FIRST-WINDOW.md): the `shell/` workspace, its host tests and goldens, and the hosted and native wirings that must match the goldens pixel for pixel. Each task is one pull request, sized for one builder (a Sonnet agent or a person) working from this document, the design and the files it names.

> **Status.** A plan, written on 2026-10-23 against `main` at `c4a897c`. No task has started. The API sketches are the design's; a builder may change a signature when the code shows a better one, and says so in the pull request and in [`FIRST-WINDOW.md`](FIRST-WINDOW.md) in the same branch.

## Contents

- [Order and parallelism](#order-and-parallelism)
- [Rules for every task](#rules-for-every-task)
- [T1. The shell workspace and desk-gfx](#t1-the-shell-workspace-and-desk-gfx)
- [T2. desk-font](#t2-desk-font)
- [T3. desk-present](#t3-desk-present)
- [T4. desk-ui](#t4-desk-ui)
- [T5. desk-wm](#t5-desk-wm)
- [T6. The scenario parser and tools/frames.py](#t6-the-scenario-parser-and-toolsframespy)
- [T7. desk-shell: events, cursor and frames](#t7-desk-shell-events-cursor-and-frames)
- [T8. The first-window scenario and its goldens](#t8-the-first-window-scenario-and-its-goldens)
- [T9. Hosted wiring: SHELL.EXE in DOSBox](#t9-hosted-wiring-shellexe-in-dosbox)
- [T10. Native wiring: the kernel's shell mode in QEMU](#t10-native-wiring-the-kernels-shell-mode-in-qemu)
- [T11. Both tracks at once, and the result](#t11-both-tracks-at-once-and-the-result)
- [T12. Live input (optional)](#t12-live-input-optional)
- [Definition of done](#definition-of-done)

## Order and parallelism

At most two tasks run at once. A wave starts when everything it depends on is merged.

| Wave | Tasks, in parallel | Depends on | Size (new lines, roughly, tests included) |
|---|---|---|---|
| 1 | T1 desk-gfx and the workspace ∥ T2 desk-font | — | 900 ∥ 400 |
| 2 | T3 desk-present ∥ T4 desk-ui | T1; T4 also T2 | 500 ∥ 900 |
| 3 | T5 desk-wm ∥ T6 scenario parser and `frames.py` | T4 | 700 ∥ 800 |
| 4 | T7 desk-shell runtime | T5, T6 | 700 |
| 5 | T8 the first-window scenario and goldens | T3, T7 | 400 and PNGs |
| 6 | T9 hosted wiring ∥ T10 native wiring | T8 | 500 ∥ 500 |
| 7 | T11 both tracks and the result; then T12, optional | T9, T10 | 200; 300 |

The dependencies, task by task: T3 needs T1; T4 needs T1 and T2; T5 and T6 need T4; T7 needs T5 and T6; T8 needs T3 and T7 (it runs the scenario through both presenters); T9 and T10 need T8; T11 needs T9 and T10; T12 needs T11.

Two tasks in the same wave touch shared files: T1 and T2 both edit `shell/Cargo.toml`'s `members` (whichever merges second adds its line when it rebases; T2 may create the workspace itself if it is first, with the same `[workspace.package]` table), and T9 and T10 both add to `tools/check.sh` (different sections; the second rebases).

## Rules for every task

- **Read first**: [`AGENTS.md`](../AGENTS.md), [`FIRST-WINDOW.md`](FIRST-WINDOW.md), and the documents the task names. Follow `AGENTS.md`: Canadian English, no hard-wrapped markdown, real em dashes, licence headers on new source files, Conventional Commits ending with the agent co-author trailer, a PR title without a prefix, labels on the PR, merge commits only, and no merging without the owner's go-ahead.
- **Work in a git worktree** on its own branch off `origin/main` (`feat/shell-gfx`, `feat/shell-font`, …), never in the main checkout.
- **Missing tools run in Docker**, never installed on the host: the CI image `ghcr.io/liminal-hq/ci-dos` (its digest is in `.github/workflows/ci.yml`) has Rust, DJGPP, DOSBox Staging and CWSDPMI; QEMU and NASM are added as the workflow adds them. Do not prune Docker images or volumes.
- **The rules of `shell/`** ([`FIRST-WINDOW.md`](FIRST-WINDOW.md#rules-carried-over)): `no_std` + `alloc`, `#![forbid(unsafe_code)]`, `#![deny(clippy::float_arithmetic)]`, no compare-and-swap, no external crates, no clocks or randomness in the core.
- **Verify before the PR**: the task's acceptance commands, `python3 -I tools/check-docs.py`, `tools/check-headers.sh`, and `tools/check.sh` where the task touches what it runs. The PR's test plan says exactly what ran and what did not.
- **Update the documents the task changes** in the same branch ([`FIRST-WINDOW.md`](FIRST-WINDOW.md#what-changes-in-other-documents)), and correct `FIRST-WINDOW.md` where the code chose differently.
- **Labels**: `enhancement` plus the scopes the task names.

## T1. The shell workspace and desk-gfx

**Goal.** The `shell/` workspace exists, builds for the host and both 486 targets in `tools/check.sh`, and has `desk-gfx`: rectangles, the back buffer, sprites, damage, the `Present` trait, `MemoryPresent`, the compositor and the digests.

**Files.**

- `shell/Cargo.toml` (workspace, `members = ["gfx"]`, `exclude = ["dos"]`, `[workspace.package]` as `hal/`'s, `[workspace.dependencies]` for `desk-hal` by path `../hal/hal`), `shell/Cargo.lock`, `shell/clippy.toml` (a copy of `hal/clippy.toml`'s bans), `shell/rustfmt.toml` (as `hal/`'s).
- `shell/gfx/Cargo.toml`, `shell/gfx/src/lib.rs`, `rect.rs`, `surface.rs`, `sprite.rs` (with `ARROW`), `damage.rs`, `present.rs` (`Present`, `MemoryPresent`, `PresentStats`), `compose.rs` (`Compositor`, `CursorLayer`, `FrameStats`), `digest.rs` (`crc32`, `FrameDigest`).
- `hal/drivers/vga/src/cursor.rs`: make the arrow's rows public (`pub const ARROW_ROWS`), so `desk-gfx` can test that its arrow is the same shape (a dev-dependency on `desk-vga` by path).
- `tools/check.sh`: a "Shell: host tests and no_std builds" step after the HAL's, with the five commands in [`FIRST-WINDOW.md`](FIRST-WINDOW.md#host-tests) (the `frames.py goldens` line arrives with T8).
- `AGENTS.md`, `CLAUDE.md`: `shell/` in the repository layout and the Rust conventions. `docs/RUST-TARGETS.md`: a row for the shell crates. `docs/README.md` lists the First Window documents already.

**API.** As [`FIRST-WINDOW.md`](FIRST-WINDOW.md#desk-gfx-surfaces-damage-and-presenting): `Rect`, `Surface`, `Sprite`/`ARROW`, `Damage` with `MAX_RECTS = 16`, `Present`, `MemoryPresent`, `Compositor::{new, damage, move_cursor, set_cursor_inks, frame, back, digest}`, `crc32`, `FrameDigest`.

**Acceptance.**

- `cargo test` in `shell/`: `Rect` arithmetic (half-open, empty, saturating constructors at `i32` limits, `align_x`); every `Surface` primitive clips at all four edges and to the clip rectangle; `mono` draws a known 1-bit pattern; `Damage` invariants over 10,000 fixed-seed random adds (inside bounds, pairwise disjoint, at most 16, covering every add) and the merge cases (overlap, touching, full list); `align_x` keeps the invariants; `Compositor`: the first frame presents `width × height` pixels, an idle frame presents 0 and calls neither `begin` nor `span`, a cursor move presents at most the old and new sprite rectangles and calls `paint` zero times, the cursor is never in `back()`, a cursor half off any edge is clipped, presented frames equal a full redraw after every step of a random sequence of damages and cursor moves; `crc32` of `"123456789"` is `0xCBF43926`; `FrameDigest` of a known 2 × 2 picture; `ARROW` equals `desk_vga::cursor::ARROW_ROWS`.
- The cross builds in `tools/check.sh` pass for both targets.

**Depends on**: nothing. **Parallel with**: T2. **Labels**: `enhancement`, `shell`, `compositor`, `graphics`, `rust`.

**Risks.** `-Zbuild-std=core,alloc` for `shell/` must work exactly as for `hal/` (the same flags; `desk-hal-mock` already proves `alloc` builds). `Damage`'s merge loop must terminate (each merge reduces the count). The stack row buffer is 1280 bytes; a wider surface is refused in `Compositor::new`.

## T2. desk-font

**Goal.** `desk-font` parses the four committed `.DFN` faces and answers glyph and width questions, with no drawing code.

**Files.** `shell/font/Cargo.toml`, `shell/font/src/lib.rs`, `shell/font/tests/faces.rs` (reads `../../fonts/generated/*.dfn` with `include_bytes!`), `shell/font/tests/expected/ui-sample.txt` (a text-art rendering of one line in Desk UI and one in Desk Title, reviewed by eye). `shell/Cargo.toml`: add `font` to `members` (or create the workspace as T1 describes if T2 merges first). `docs/FONTS.md`: the "What was and was not verified" line about DOS code not reading `.DFN` is updated to say `desk-font` parses them on the host (T9 adds DOS).

**API.** As [`FIRST-WINDOW.md`](FIRST-WINDOW.md#desk-font-reading-dfn): `Font::{parse, height, baseline, glyph_count, glyph, glyph_or_fallback, advance}`, `Glyph::bit`, `FontError`.

**Acceptance.**

- The four faces parse with the metrics the files hold today (cells 14, 14, 16, 8; baselines 11, 12, 13, 7; glyph counts 289, 289, 332, 289), every glyph's rows are `height × ceil(advance / 8)` bytes, and the parse consumes the file exactly.
- Desk Fixed's advance is 8 for every glyph and Desk Small's 6, as `FONTS.md` says; `advance("Desk")` is the sum of the four glyphs' widths; a character no face has (U+FFFD) falls back to `?`.
- The text-art sample matches `expected/ui-sample.txt` (written once by the test with `DESK_FONT_RECORD=1`, then committed after a look).
- Malformed inputs: bad magic, a header cut short, a glyph cut short (with the offset), zero height, a baseline past the cell, code points out of order, bytes after the last glyph: each its own `FontError`.
- Builds for both targets (the T1 step covers the workspace once both are merged).

**Depends on**: nothing. **Parallel with**: T1. **Labels**: `enhancement`, `shell`, `graphics`, `rust`.

**Risks.** The fonts are under the SIL Open Font License 1.1; reading them in tests and embedding them in binaries is allowed, and a distribution that copies the files carries `OFL.txt` (T9). A rebuild of the fonts changes the files and so the goldens; the T8 goldens name the font build they used in their commit message.

## T3. desk-present

**Goal.** The two real presenters and the read-back the DOS captures use, tested on the host against a memory frame buffer, a four-plane model and the mock port bus.

**Files.** `shell/present/Cargo.toml` (depends on `desk-gfx`, `desk-hal`, `desk-vga` by path; dev-dependency `desk-hal-mock`), `shell/present/src/lib.rs`, `linear.rs`, `planar.rs` (`PlanarPresent`, `planarise`, `PlaneSink`, `VgaPlanes`), `readback.rs`, `shell/present/tests/planar.rs`. `shell/Cargo.toml`: add `present`.

**API.** As [`FIRST-WINDOW.md`](FIRST-WINDOW.md#desk-present-the-linear-and-planar-presenters). `PlanarPresent::align()` is 8; `begin` writes graphics controller mode 0, data rotate 0, enable set/reset 0 and bit mask `FFh`; each span selects each plane with the sequencer map mask (`desk_vga::regs::{sequencer_write, seq::MAP_MASK}`) and writes its bytes; `end` restores the map mask to `0Fh`. The read-back wraps `desk_vga::Linear8::read_row` and `Planar16::read_row`.

**Acceptance.**

- `planarise`: every one of the 16 colours at every bit position, and a random row checked against a per-pixel reference.
- `LinearPresent` over an `MmioRegion` on a `Vec` (pitch larger than the width included): the same spans give the same bytes as `MemoryPresent`; spans at the last column and row; refused geometry.
- `PlanarPresent` over a test `PlaneSink` that keeps four planes: after any sequence of aligned spans, recombining the planes gives the same indices as `MemoryPresent`; an unaligned span is refused with `InvalidArgument`.
- `PlanarPresent` over `desk-hal-mock`'s `MockPortBus`: `begin` writes exactly the four graphics controller registers, each span writes the map mask four times in plane order, `end` restores it; `begin` after a `Planar16` was created and dropped still leaves set/reset off.
- The read-back of a known picture through a `Vec` region (linear) gives it back; the planar read-back is covered by the DOS run (T9), because the mock region is plain memory.

**Depends on**: T1. **Parallel with**: T4. **Labels**: `enhancement`, `shell`, `graphics`, `compositor`, `rust`.

**Risks.** This is the one `shell/` crate that calls `desk-vga`'s registers; it must stay out of the core crates' dependencies (a review point). The volatile byte loop in `write_bytes` is slow on real hardware; the word copy is a separate `hal/` change.

## T4. desk-ui

**Goal.** Theme roles and the palettes, text drawing, the window chrome with the stepped halo, the control icons, and the label and text-field widgets.

**Files.** `shell/ui/Cargo.toml` (depends on `desk-gfx`, `desk-font`, `desk-hal`, `desk-kbd`), `shell/ui/src/lib.rs`, `theme.rs` (`Depth`, `Role`, `Theme`), `palette.rs` (`DESK16`, `AFTERGLOW_SURFACES`, `palette(depth)`), `text.rs`, `chrome.rs` (`ChromeLayout`, `Control`, `paint_chrome`, the constants), `icons.rs` (three 10 × 10 one-bit icons), `widget.rs` (`Widget`, `Label`, `TextLine`). `shell/Cargo.toml`: add `ui`. `docs/PALETTE.md`: the rounding sentence ([question 1](FIRST-WINDOW.md#questions-for-the-owner)) and a short table of the 256-colour slots the first window uses.

**API.** As [`FIRST-WINDOW.md`](FIRST-WINDOW.md#desk-ui-theme-roles-text-and-window-chrome), with the role and slot table and the chrome table exactly as written there.

**Acceptance.**

- `DESK16` and `AFTERGLOW_SURFACES` equal `Rgb6::from_rgb8` of the hex values in `PALETTE.md` (the test lists the hex strings, so a change to either shows); `palette(Bits8)` has them at 0–15 and 97–104 and the cursor at 240–241, black elsewhere; `palette(Bits4)` only fills 0–15.
- Every role resolves at both depths as the table says; at depth 4 every slot is below 16.
- `ChromeLayout::new` for a few boxes: title bar, three control boxes (the close box's right edge at `x + w - 8`), rule, content, extent; the minimum size.
- `paint_chrome` on a `Surface`, focused: the pixels at one, two, three and four pixels outside each edge are `Accent`, `Halo2`, `Halo3` and untouched; unfocused: `Line` at one pixel and untouched at two and three; the title bar, rule and content colours; the title's first glyph lands at `(x + 8, y + 4)`; a title too long for the bar ends in `…` before the control boxes.
- `draw_text` of "Desk" equals the glyphs drawn one by one; a missing character draws `?`.
- `TextLine::key`: letters, digits and space append (through `desk_kbd::char_for`), Shift gives capitals, Backspace removes one, a key at `max_chars` changes nothing and returns `None`, and the returned rectangle covers the old and new text and caret.

**Depends on**: T1, T2. **Parallel with**: T3. **Labels**: `enhancement`, `shell`, `ui`, `design-system`, `graphics`, `rust`.

**Risks.** The design boards are reference only (`AGENTS.md`): measurements come from [`FIRST-WINDOW.md`](FIRST-WINDOW.md#window-chrome), not from copying prototype code. The icons are drawn here, not taken from the design system's icon set. `PALETTE.md`'s rounding change is a decision recorded as a default; if the owner picks the other rule, only `palette.rs` and the goldens change.

## T5. desk-wm

**Goal.** Windows with z-order, focus, hit testing, title-bar dragging and the close box, and the exact damage each change reports.

**Files.** `shell/wm/Cargo.toml`, `shell/wm/src/lib.rs`, `window.rs`, `wm.rs`, `paint.rs`, `shell/wm/tests/damage.rs`. `shell/Cargo.toml`: add `wm`.

**API.** As [`FIRST-WINDOW.md`](FIRST-WINDOW.md#desk-wm-windows-focus-and-damage): `WindowId`, `WindowSpec`, `Window`, `Hit`, `Wm::{new, open, close, focus, by_name, window_mut, hit, pointer, key, paint}`, `Fonts`.

**Acceptance.**

- Hit testing: topmost window wins; title, each control box, content, the ring area and outside.
- Each row of the damage table in the design as its own test, with the exact rectangles.
- Press on a title then motion then release moves the window by the motion; the clamps hold at all four screen edges; release outside the close box does not close; release inside does, and focus goes to the new top window.
- Keys reach the focused window's field only, and nothing when no window is focused.
- `paint` restricted to a damage rectangle changes no pixel outside it (checked by painting into a surface with a sentinel colour).

**Depends on**: T4. **Parallel with**: T6. **Labels**: `enhancement`, `shell`, `compositor`, `ui`, `rust`.

**Risks.** Focus-without-raise and focus-with-raise damage differ; an off-by-one in `extent` leaves a halo pixel behind, which T7's incremental-versus-full test will catch even if this task's tests miss it.

## T6. The scenario parser and tools/frames.py

**Goal.** The scenario format parsed in `no_std` Rust, and the Python tool that writes, reads, digests, checks and diffs captures.

**Files.** `shell/shell/Cargo.toml` (the `desk-shell` crate starts here, with only its `scenario` module and the `Event` type), `shell/shell/src/lib.rs`, `shell/shell/src/scenario.rs`, `shell/shell/tests/parse.rs`. `tools/frames.py`, `tools/test_frames.py` (standard-library `unittest`, run by `tools/check.sh`). `shell/Cargo.toml`: add `shell`. `tools/check.sh`: `python3 -I tools/test_frames.py`.

**API.** Rust: `scenario::{Scenario, Step, FrameName, ParseError, ParseErrorKind, parse}` and `Event`, as [`FIRST-WINDOW.md`](FIRST-WINDOW.md#the-scenario-format). Python: the subcommands in [`FIRST-WINDOW.md`](FIRST-WINDOW.md#goldens-and-frame-digests) (`png`, `digest`, `check`, `transcript`, `goldens`, `same`, `diff`, `inject`), exit status 0 on success, 1 on a failed check, 2 on a usage error, like `tools/native-shot.py`.

**Acceptance.**

- The parser accepts the example scenario in the design and gives the steps it lists; each malformed line (unknown command, missing argument, bad number, unterminated string, a name of nine characters or with a capital, a frame name used twice, `screen` after a step, a key name that is not a `KeyCode`) is an error naming its line; `type` expands to key events with Shift for capitals; the parser is `no_std` (built by the cross step).
- `frames.py`: a PNG written then read gives the same indices and palette; a PPM of known colours gives the expected colour digest, and the same picture as RAW plus PAL gives the same colour digest (the six-bit comparison holds for both `v << 2 | v >> 4` and QEMU-style expansions, tested with both); `check` passes on matching captures and fails, with a diff PNG and the bounding box, on a one-pixel change; names match without case; `transcript` finds a changed digest; `inject` on the example gives one `wait shell: ready NAME`, `screendump {out}/NAME.ppm`, `sendkey spc` triple per frame after `wait input: ready`, in the format `tools/native-inject.py` reads.
- The CRC-32 in `frames.py` is `zlib.crc32`, and a test checks it against the Rust `crc32` values T1's tests use.

**Depends on**: T4 (for `Depth`), T1 (for `Rect`). **Parallel with**: T5. **Labels**: `enhancement`, `testing`, `shell`, `rust`.

**Risks.** Two digest implementations (Rust and Python) must agree; the shared test vector and T8's end-to-end check keep them honest. Frame names are limited by DOS's 8.3; the parser enforces it so the problem never reaches DOSBox.

## T7. desk-shell: events, cursor and frames

**Goal.** `Shell`: events in, cursor and window manager updated, damage collected, frames presented; the scenario runner; and the damage-accounting tests on `MemoryPresent`.

**Files.** `shell/shell/src/shell.rs`, `shell/shell/src/run.rs` (`scenario::run`), `shell/shell/tests/accounting.rs`, `shell/shell/tests/incremental.rs`. `shell/shell/Cargo.toml`: the dependencies on `desk-gfx`, `desk-font`, `desk-ui`, `desk-wm`, `desk-hal`.

**API.** As [`FIRST-WINDOW.md`](FIRST-WINDOW.md#desk-shell-events-frames-and-scenarios): `Shell::{new, handle, frame, digest, wm}`, `scenario::run`, `RunError`.

**Acceptance.**

- Every bullet of the damage accounting in [`FIRST-WINDOW.md`](FIRST-WINDOW.md#host-tests), at both depths, with `MemoryPresent` (its alignment set to 8 for depth 4).
- Incremental equals full: after each step of a hand-written scenario that opens, focuses, drags, types and closes, the incremental front buffer equals a fresh shell's full redraw of the same state.
- The pointer clamps at the screen edges; `Pointer` and `PointerTo` agree; button edges, not levels, drive presses.
- `run` calls `capture` once per `frame` step, after the frame is presented, and fails an `expect-damage` step that is exceeded.
- Determinism: two runs give the same digests.

**Depends on**: T5, T6. **Parallel with**: nothing (wave 4). **Labels**: `enhancement`, `shell`, `compositor`, `testing`, `rust`.

**Risks.** The full-redraw comparison needs a way to rebuild the same state: run the same steps into a second shell and force a whole-screen damage before its frame. Keep `Shell` free of platform types, or T9 and T10 will each need a shim.

## T8. The first-window scenario and its goldens

**Goal.** `tests/shell/first-window.scn`, its `.sums` and golden PNGs at both depths, the recording script, and the scenario run through the real presenters on the host.

**Files.** `tests/shell/first-window.scn`, `tests/shell/first-window.sums`, `tests/shell/golden/first-window/*.png`, `tests/shell/README.md` (what the files are and how to re-record), `shell/shell/tests/scenarios.rs` (every `.scn` in `tests/shell/` at every depth it lists, through `MemoryPresent`, `LinearPresent` over a `Vec` and `PlanarPresent` over the plane model; `desk-present` as a dev-dependency), `tools/shell-goldens.sh`. `tools/check.sh`: the `frames.py goldens` line.

**Acceptance.**

- `cargo test` passes against the committed `.sums`; with one byte of a `.sums` line changed it fails and writes the dumps and the `frames.py diff` command to run.
- All three presenters give the same digests for every frame at each depth they support (linear at 8, planar at 4, memory at both).
- `tools/shell-goldens.sh` re-records from clean and leaves no diff.
- `python3 -I tools/frames.py goldens tests/shell/first-window.sums tests/shell/golden/first-window` passes.
- The PNGs are looked at: halo rings, hairline, title text, control icons, field and caret, cursor; the PR shows them.
- The `expect-damage` step after the cursor move passes.

**Depends on**: T3, T7. **Parallel with**: nothing (wave 5). **Labels**: `enhancement`, `testing`, `shell`, `graphics`.

**Risks.** Whatever the goldens show becomes the reference for both tracks, so a visual mistake here is copied everywhere: the owner reviews the PNGs before T9 and T10 start. The PNG sizes should be a few kilobytes each; anything much larger means the writer is not compressing.

## T9. Hosted wiring: SHELL.EXE in DOSBox

**Goal.** The shell running on the pure-Rust DOS runtime at both depths, its read-back dumps equal to the goldens, in `tools/check.sh`.

**Files.** `shell/dos/Cargo.toml` (its own `[workspace]`), `shell/dos/Cargo.lock`, `shell/dos/.cargo/config.toml` (as `spikes/a-vga-vbe/.cargo/config.toml`, with paths to `../../spikes/d0-rust-dos/dos-elf.json` and its `link.ld`), `shell/dos/rustfmt.toml`, `shell/dos/src/main.rs`, `shell/dos/build.sh` (into `dist/shell/`, copying `CWSDPMI.EXE`, `UI.DFN`, `TITLE.DFN` and `OFL.txt`), `shell/dos/check.sh`, `tests/shell/expected-dos-8.txt`, `tests/shell/expected-dos-4.txt`. `tools/check.sh`: a "Shell on DOS" step after spike B's. `docs/FONTS.md`: DOS code now reads `.DFN`. `docs/RUST-TARGETS.md`: the hosted wiring in the spikes table.

**What `main` does.** As [`FIRST-WINDOW.md`](FIRST-WINDOW.md#hosted-shellexe-in-dosbox): parse the arguments; `DosDisplay::new()`; set the mode for the depth (falling back from 8 to 4 without VBE, and saying so); load `desk_ui::palette(depth)`; make the presenter (`RawPorts` is the one `unsafe` step, with its `SAFETY:` line, as in spike A); load the fonts (embedded, or from files with `fontfile`); run the scenario with a `capture` that prints the digests and, with `dump`, reads back the frame buffer and DAC and writes `NAME.RAW` and `NAME.PAL`; or, without `scenario`, the interactive loop on `DosInput` until Escape. Exit status 0 on success; a failed capture is a panic, which `rt` turns into a report and the text mode.

**Acceptance.**

- `shell/dos/check.sh` in the CI image: format and lints, build, `SHELL.EXE depth8 scenario dump` and `depth4 scenario dump` with transcripts equal to the expected files, the core's and the read-back's digests equal on every frame, `frames.py check tests/shell/first-window.sums 8 dist/shell/dumps-8` and the same at 4; `fontfile` gives the same transcript; `DOS_MACHINE=vgaonly` with `depth8` reports the fallback and matches the depth 4 sums.
- Negative runs that must fail: `corrupt`; the depth 8 dumps against the depth 4 sums; the expected transcript with one digest changed.
- The interactive mode is tried by hand once (or with spike B's opt-in Xvfb harness) and the PR says how.
- The program size is reported in the PR.

**Depends on**: T8. **Parallel with**: T10. **Labels**: `enhancement`, `dos`, `shell`, `graphics`, `rust`.

**Risks.** The 300 KiB back buffer comes from `rt`'s heap, which grows by at least the request (`rt::heap::grow`), so it should be one DPMI allocation; confirm it in DOSBox. DOSBox's `cputype = 486` rejects `cmpxchg`, which the shell never emits (the clippy ban), but a dependency's code could: check the disassembly for `cmpxchg` once, as spike H did. The scenario's `dump` writes about 600 KiB a frame through INT 21h; if the run nears `DOS_TIMEOUT`, dump only the frames the check needs.

## T10. Native wiring: the kernel's shell mode in QEMU

**Goal.** The shell running in the native kernel at both depths, its QEMU screendumps equal to the goldens, in `tools/native-test.sh`.

**Files.** `kernel/Cargo.toml` and `kernel/Cargo.lock` (path dependencies on the shell crates and `desk-present`), `kernel/src/main.rs` (test modes `Shell = 13` and `ShellCorrupt = 14`), `kernel/src/shell.rs`, `kernel/src/gfx.rs` (make `rendezvous` and `lfb_region` usable from `shell.rs`), `tools/native-image.sh` (`--test-mode shell`, `shell-corrupt`), `tools/native-test.sh` (a "Shell: first window" section), `tests/native/expected-shell-8.txt`, `tests/native/expected-shell-4.txt`. `docs/NATIVE-BOOT.md`: the new test modes. `docs/RUST-TARGETS.md`: the native wiring in the spikes table.

**What `shell.rs` does.** As [`FIRST-WINDOW.md`](FIRST-WINDOW.md#native-the-kernels-shell-test-mode-in-qemu): depth 8 over the loader's linear frame buffer when there is one, otherwise mode 12h by registers at depth 4; the palette through `desk_vga::dac::set_palette`; the embedded fonts and scenario; at each frame the digests, `shell: ready NAME` and the rendezvous; `shell: done`, then text mode as the `gfx` tour restores it. `shell-corrupt` flips one pixel after the `moved` frame.

**Acceptance.**

- `tools/native-test.sh`: the depth 8 run on `-vga std` with `--video vbe` and the depth 4 run on `-vga cirrus` with `--video text`, each with its inject script from `frames.py inject`, transcripts equal (through `tools/native-check.py`) and `frames.py transcript` passing, screendumps passing `frames.py check` against the goldens.
- Negative runs that must fail: `shell-corrupt`'s screendumps; the depth 8 screendumps against the depth 4 sums.
- The existing native checks are unchanged (the `gfx` and `bios` transcripts and screenshots).
- `kernel.bin`'s size before and after is in the PR.
- Optional, reported if tried: the same image under Bochs with `NATIVE_EXTRA=1`.

**Depends on**: T8. **Parallel with**: T9. **Labels**: `enhancement`, `shell`, `graphics`, `rust`.

**Risks.** The kernel builds its path dependencies with its own profile (`opt-level = "s"`, LTO); a size jump is worth a look. The rendezvous waits for a real key while the scenario's input is scripted; the PS/2 queue must not deliver the rendezvous key to the shell. Test modes 13 and 14 must not collide with another open branch's.

## T11. Both tracks at once, and the result

**Goal.** One check that the two tracks' captures are the same, and the documents brought up to date with what was built.

**Files.** `tools/check.sh` (after the native test: `frames.py same` of the DOS dumps and the native screendumps at each depth), `docs/FIRST-WINDOW.md` (a "Result" section: what was built, the measurements, screenshots from both tracks, what was not verified), `docs/images/first-window/` (the PNGs the result shows), `docs/HAL.md` (status and the narrower seam), `docs/DIRECTION.md` (the milestones), `docs/SPEC.md` (the headless frame checks), `docs/RUST-TARGETS.md` (statuses).

**Acceptance.** `tools/check.sh` passes in full in the CI image; `frames.py same` fails when one capture set is swapped for the other depth's; the documents pass `tools/check-docs.py`.

**Depends on**: T9, T10. **Labels**: `documentation`, `design-docs`, `shell`, `testing`.

**Risks.** None new; this is where an unverified claim is most tempting, so the "not verified" list is written first.

## T12. Live input (optional)

**Goal.** The interactive modes driven by real input: QEMU's `sendkey` and `mouse_move` against the kernel's interactive shell, and DOSBox's `AUTOTYPE` (keys) against `SHELL.EXE`, ending at a picture that equals a golden.

**Files.** A second scenario (`tests/shell/live.scn`) whose input `frames.py inject --live` turns into monitor commands; the kernel's interactive shell mode (code 15); `tools/native-test.sh` and `shell/dos/check.sh` sections, opt-in at first.

**Acceptance.** The final frame's screendump equals the live scenario's golden on QEMU; on DOSBox, the typed text frame equals its golden. Mouse motion under DOSBox stays with spike B's Xvfb harness.

**Depends on**: T11. **Labels**: `enhancement`, `input`, `shell`, `testing`.

**Risks.** Input timing: events must not cross a frame boundary differently from the scripted run, so the live scenario only captures after the input has settled (the transcript line that says so, as spike D's `wait gfx: cursor` does).

## Definition of done

The milestone is done when, on `main`:

- `tools/check.sh` runs the shell's host tests, goldens and cross builds, `SHELL.EXE` at both depths in DOSBox, the kernel's shell mode at both depths in QEMU, and the cross-track comparison, and all pass in the CI image;
- each capture equals its golden by colour digest, and by index digest where it has indices;
- the negative runs fail as they should;
- [`FIRST-WINDOW.md`](FIRST-WINDOW.md) has its result section, and the documents it lists are updated.
