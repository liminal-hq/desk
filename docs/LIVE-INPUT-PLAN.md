# Live input: the build plan

The task-by-task plan for building [`LIVE-INPUT.md`](LIVE-INPUT.md): the core's keyboard window management and cursor coalescing, the live loop on both tracks, the tools that turn live scenarios into injected input and check the marks, the INT 33h click counters, the launchers a person runs, and the result. Each task is one pull request in one linear stack, sized for one Sonnet builder working from this document, the design and the files it names. It replaces T12 of [`FIRST-WINDOW-PLAN.md`](FIRST-WINDOW-PLAN.md).

> **Status.** A plan, written on 2026-10-23 against `main` at `44f0265`. No task has started. The API sketches are the design's; a builder may change a signature when the code shows a better one, and says so in the pull request and in [`LIVE-INPUT.md`](LIVE-INPUT.md) in the same branch.

## Contents

- [Order, parallelism and the stack](#order-parallelism-and-the-stack)
- [Rules for every task](#rules-for-every-task)
- [L1. Cursor coalescing, is_dirty and the live module](#l1-cursor-coalescing-is_dirty-and-the-live-module)
- [L2. Keyboard window management in desk-wm](#l2-keyboard-window-management-in-desk-wm)
- [L3. Live scenarios, frames.py live and marks](#l3-live-scenarios-framespy-live-and-marks)
- [L4. The INT 33h press and release counters](#l4-the-int-33h-press-and-release-counters)
- [L5. Native: the shell-live mode and its QEMU checks](#l5-native-the-shell-live-mode-and-its-qemu-checks)
- [L6. Hosted: the live loop, marks and the AUTOTYPE checks](#l6-hosted-the-live-loop-marks-and-the-autotype-checks)
- [L7. The launchers](#l7-the-launchers)
- [L8. The DOS mouse under Xvfb](#l8-the-dos-mouse-under-xvfb)
- [L9. The result](#l9-the-result)
- [Definition of done](#definition-of-done)

## Order, parallelism and the stack

At most two builders work at once. The pull requests form **one linear stack**, in the order L1 to L9, each based on the one before; `gh stack submit` pushes the chain and keeps each PR's base the previous branch. Two tasks of a wave touch disjoint files, so they are built in parallel from the previous wave's tip, and the second of the pair is rebased onto the first before it is submitted (a rebase with no conflicts, because the files do not overlap). Nothing is merged without the owner's go-ahead; when the bottom of the stack merges, the stack is rebased on `main` by the next `gh stack submit`, never squashed.

| Wave | Tasks, in parallel | Depends on | Size (new lines, roughly, tests included) |
|---|---|---|---|
| 1 | L1 cursor coalescing, `is_dirty`, `live` ∥ L2 keyboard window management | — | 350 ∥ 450 |
| 2 | L3 live scenarios, `frames.py live` and `marks` ∥ L4 INT 33h counters | L3: L1, L2; L4: — | 600 and PNGs ∥ 300 |
| 3 | L5 native live mode ∥ L6 hosted live loop | L5: L1, L3; L6: L1, L3, L4 | 400 ∥ 400 |
| 4 | L7 launchers ∥ L8 DOS mouse under Xvfb | L7: L5, L6; L8: L3, L4, L6 | 300 ∥ 250 |
| 5 | L9 the result | everything | 150 and images |

Stack order and branch names:

```text
main
└── feat/live-cursor          L1
    └── feat/live-wm-keys     L2
        └── feat/live-frames  L3
            └── feat/live-int33-clicks   L4
                └── feat/live-native     L5
                    └── feat/live-dos    L6
                        └── feat/live-launchers   L7
                            └── test/live-dos-xvfb  L8
                                └── docs/live-input-result  L9
```

## Rules for every task

- **Read first**: [`AGENTS.md`](../AGENTS.md), [`LIVE-INPUT.md`](LIVE-INPUT.md), [`FIRST-WINDOW.md`](FIRST-WINDOW.md), and the documents the task names. Follow `AGENTS.md`: Canadian English, no hard-wrapped markdown, real em dashes, licence headers on new source files, Conventional Commits ending with the agent co-author trailer, a PR title without a prefix, labels on the PR, merge commits only, and no merging without the owner's go-ahead.
- **Work in a git worktree** under `.claude/worktrees/` on the task's branch, never in the main checkout.
- **Missing tools run in Docker**, never installed on the host: the CI image `ghcr.io/liminal-hq/ci-dos` (its digest is in `.github/workflows/ci.yml`) has Rust, DJGPP, DOSBox Staging and CWSDPMI, with QEMU and NASM added as the workflow adds them; `desk-dos-xvfb:1` and `desk-native-qemu:1` are built by the scripts that use them. Docker leaves root-owned files in a mounted tree: run containers with `--user "$(id -u):$(id -g)"`, and delete anything root-owned through a container. Do not prune Docker images or volumes. The disk is tight: delete `dist/` captures a task no longer needs.
- **The pinned files do not change**: `tests/shell/first-window.scn`, `first-window.sums`, `golden/first-window/`, `expected-dos-8.txt`, `expected-dos-4.txt`, `tests/native/expected-shell-8.txt` and `-4.txt`. A task that needs to change one has found a regression and stops to report it.
- **The rules of `shell/`** ([`FIRST-WINDOW.md`](FIRST-WINDOW.md#rules-carried-over)): `no_std` + `alloc`, `#![forbid(unsafe_code)]`, no float, no compare-and-swap, no external crates, no clocks or randomness in the core.
- **Fix a bug with a check that fails before the fix**: L4's `--quick` run and L1's coalescing test are written first and seen failing.
- **Verify before the PR**: the task's acceptance commands, `python3 -I tools/check-docs.py`, `tools/check-headers.sh`, and `tools/check.sh` (in the CI image where the host lacks a tool) where the task touches what it runs. The PR's test plan says exactly what ran and what did not.
- **Update the documents the task changes** in the same branch ([`LIVE-INPUT.md`](LIVE-INPUT.md#what-changes-in-other-documents)), and correct `LIVE-INPUT.md` where the code chose differently.

## L1. Cursor coalescing, is_dirty and the live module

**Goal.** The compositor damages only where the cursor was last presented and where it is now; the shell can say whether a frame would draw anything; the platforms have one pure module for the live loop's key routing and lines.

**Files.**

- `shell/gfx/src/compose.rs`: `shown: Option<Rect>` in `Compositor`; `move_cursor` records the position only; `frame` adds `shown` and the current sprite rectangle to the overlay damage when they differ (or when either is `None` and the cursor is visible), and sets `shown` after a successful present; `set_cursor_visible(false)` damages `shown`; a failed present leaves `shown` as it was. `pub fn is_dirty(&self) -> bool`.
- `shell/shell/src/shell.rs`: `pub fn is_dirty(&self) -> bool`.
- `shell/shell/src/live.rs` (new) and `lib.rs`: `Route`, `route`, `Tally`, `mark_line`, `summary_line` as in [`LIVE-INPUT.md`](LIVE-INPUT.md#the-live-loop).
- Tests: `shell/gfx` (fifty moves between two frames present at most two sprite rectangles; a move and back presents nothing new beyond the old rectangle; hide and show; a failed present then a good one), `shell/shell/tests/accounting.rs` (the existing counts unchanged), `shell/shell/src/live.rs` unit tests (Escape and F12 presses and releases, every other key `Shell`, the line formats, two-digit marks).

**Acceptance.** `(cd shell && cargo test --locked)`, clippy and the two cross builds as `tools/check.sh` runs them; `tools/check.sh` end to end with the scripted DOS and native shell checks unchanged (their expected transcripts pin the presented counts); `git diff origin/main -- tests/` is empty.

**Labels**: `enhancement`, `shell`, `compositor`, `performance`.

**Risks.** A scripted frame whose cursor moved twice would present less than its pinned count. None of the first window's frames does; if one did, the transcript diff shows it and the task stops.

## L2. Keyboard window management in desk-wm

**Goal.** Alt+Tab (release picks), Shift+Alt+Tab, Alt+F4 and Tab and Shift+Tab between fields, as the key table of [`LIVE-INPUT.md`](LIVE-INPUT.md#the-keyboard-text-focus-cycling-and-closing) says, with the damage it lists.

**Files.**

- `shell/wm/src/wm.rs`: a `cycle: Option<usize>` (how far down the stacking order the target is); `key` handles the table before handing the key to the widget; `pub fn cycling(&self) -> Option<WindowId>` for tests.
- `shell/wm/src/paint.rs`, `shell/ui/src/widget.rs`: the caret is drawn only in the focused window's `focus_widget`.
- `shell/shell/src/scenario.rs`: nothing should be needed (`key LeftAlt down`, `key Tab down` and so on already parse); if a key name is missing from `key_code`, add it.
- `tests/shell/keys.scn` (new, scripted): the first window's set-up, Alt+Tab (Notes on top), Alt+Tab again (Welcome back), Shift+Alt+Tab with three windows open, Alt+F4, and a window with two fields and Tab between them; its `.sums` and `golden/keys/` from `tools/shell-goldens.sh`.
- Tests in `shell/wm/tests/` for each row of the table, including: Alt pressed and released with no Tab changes nothing; Tab with Ctrl is the widget's; a cycle with one window; a click while Alt is held ends the cycle and leaves no stale index; and the damage of each change.

**Acceptance.** As L1, plus `python3 -I tools/frames.py goldens tests/shell/keys.sums tests/shell/golden/keys`; the incremental-equals-full test passes over `keys.scn` (it runs every scenario); the reviewer looks at the new PNGs.

**Depends on**: nothing (parallel with L1). **Labels**: `enhancement`, `shell`, `input`, `ui`.

**Risks.** The caret rule changes how a focused window paints a second field; no first-window frame has one, which the unchanged sums show.

## L3. Live scenarios, frames.py live and marks

**Goal.** The two live scenarios and their goldens, and the tools that translate them for each emulator and check the marks a live run prints.

**Files.**

- `tests/shell/live.scn` and `tests/shell/live-keys.scn` as [`LIVE-INPUT.md`](LIVE-INPUT.md#live-scenarios) lists their frames; `live.sums`, `live-keys.sums`, `golden/live/`, `golden/live-keys/` from `tools/shell-goldens.sh`. Each scenario has a comment above every stage, as `first-window.scn` has.
- `shell/shell/tests/scenarios.rs`: every `live*.scn` has the first window's set-up (its steps before the first frame equal `first-window.scn`'s).
- `tools/frames.py`: `live SCENARIO --qemu | --autotype | --xdotool [--quick]` and `marks SUMS SCENARIO DEPTH TRANSCRIPT [--dumps DIR]`, with the step table, the pauses, the key-name table (`KeyCode` names to QEMU `sendkey`, DOSBox `AUTOTYPE` and X keysym names), the set-up rule and the refusals of the design. The tracked pointer clamps to the scenario's `screen` as `Shell` does.
- `tools/test_frames.py`: each translation, each refusal (with its line number), `--quick`, the set-up rule, and `marks` passing, failing on a missing, extra or wrong mark and renaming dumps.
- `tools/check.sh`: run `frames.py live` over both scenarios in all three forms, so a scenario an emulator cannot play fails CI before any emulator runs.

**Acceptance.** `python3 -I tools/test_frames.py`; `python3 -I tools/frames.py live tests/shell/live.scn --qemu` (and the other forms) print scripts a reviewer can read; `frames.py goldens` for both new sets; `tools/check.sh`.

**Depends on**: L1 (nothing in the scenarios needs it, but the stack is linear) and L2 (Alt+Tab and Alt+F4 in `live.scn`). Parallel with L4. **Labels**: `testing`, `shell`, `input`.

**Risks.** `AUTOTYPE`'s key names are DOSBox Staging's mapper names; check them with `autotype -list` in the CI image's DOSBox and record the table's source in a comment. QEMU's `sendkey` names are QEMU's `QKeyCode` names (`spc`, `ret`, `backspace`, `tab`, `f12`, `esc`, `alt`, `shift`).

## L4. The INT 33h press and release counters

**Goal.** A click shorter than one poll reaches the shell under DOS, as [`LIVE-INPUT.md`](LIVE-INPUT.md#the-pointer-focus-drag-and-close) describes, for the left button.

**Files.**

- `hal/drivers/int33/src/lib.rs` (or the module that holds `Mouse` and `MouseInput`): `Mouse::presses(button) -> (count, x, y)` and `Mouse::releases(button)` (functions 0005h and 0006h through `Bios::int`); `MouseInput` reads both for the left button on every poll and synthesises the missing edge (at most one press and release pair per poll), so `poll_mouse` may return two events from one poll.
- `hal/platform/mock/src/int33.rs`: the model counts presses and releases per button, clears them on read, and can be told "a click happened between polls".
- Tests: a click inside one poll gives a press then a release; a release and press inside one poll while held gives a release then a press; a click with motion puts the motion on the first event; two clicks in a poll give one pair; a driver that answers the counters with zeros changes nothing; the existing 16 tests unchanged. The whole-path test's transcript (`spikes/b-dos-input/expected/keys.txt`) unchanged.
- [`B-DOS-INPUT.md`](B-DOS-INPUT.md): the counters move from the recommendations to what was built; [`HAL.md`](HAL.md): the driver list.

**Acceptance.** `(cd hal && cargo test --locked)`, clippy, the cross builds; `spikes/b-dos-input/check.sh` unchanged in result; with Docker, `spikes/b-dos-input/check-xvfb.sh` still passes (its `xdotool click` row may now show a click, and the check's expectation is updated to say so, which is the failing-first evidence for this task).

**Depends on**: nothing (parallel with L3). **Labels**: `enhancement`, `input`, `dos`.

**Risks.** Drivers differ in what 0005h and 0006h report for a button that does not exist and whether the counts saturate; only DOSBox's built-in driver is tested. Two more DPMI round trips per poll slow the loop slightly; the design accepts it.

## L5. Native: the shell-live mode and its QEMU checks

**Goal.** `shell-live` (code 15): the first window on the PS/2 keyboard and mouse at depth 8 or 4, with PIT pacing, `hlt` when idle, F12 marks and Escape to end, checked in headless QEMU against `live.sums` and `live-keys.sums`.

**Files.**

- `kernel/src/main.rs`: `TestMode::ShellLive = 15`, handed to `shell::live` after `input: ready`.
- `kernel/src/shell.rs`: `pub fn live(video: &Video, ps2: Ps2Input)`, sharing the mode set-up, palette and presenter choice with `run` (factor them out without changing `run`'s transcript); the loop of [`LIVE-INPUT.md`](LIVE-INPUT.md#the-live-loop) with `MIN_FRAME_TICKS = 1`, `input::idle` between frames, `live::route`, the mark lines and the summary; text mode restored as `run` does.
- `tools/native-image.sh`: `shell-live` in the mode list and the usage comment.
- `tests/native/expected-shell-live-8.txt`, `-4.txt`, and `expected-shell-live-keys-8.txt` (mark digests exact, counts as ranges).
- `tools/native-test.sh`: the section of [`LIVE-INPUT.md`](LIVE-INPUT.md#native-through-the-qemu-monitor), and its native negatives from the [table](LIVE-INPUT.md#negative-checks) (each must fail; the script says so when one does not). Kept captures as PNGs in `dist/shell-live-native-DEPTH/`, as the scripted run keeps its own.
- [`FIRST-WINDOW.md`](FIRST-WINDOW.md): the native test modes list.

**Acceptance.** `tools/native-test.sh` (host QEMU or `DESK_NATIVE_BACKEND=docker`) passes with the new section; the scripted `shell` and `shell-corrupt` transcripts are unchanged; each negative fails; three runs in a row give the same mark digests.

**Depends on**: L1, L3. Parallel with L6. **Labels**: `enhancement`, `input`, `shell`, `testing`.

**Risks.** Code 15 colliding with another branch's test mode. Mouse packets still in QEMU when a key is handled: the translator's pauses and the extra mouse drain before a mark are the cure; if a run is flaky, lengthen the pauses in `frames.py`, never loosen the check.

## L6. Hosted: the live loop, marks and the AUTOTYPE checks

**Goal.** `SHELL.EXE`'s interactive mode on `live::route`, with `marks` (read-back dumps at each mark), checked by `AUTOTYPE` against `live-keys.sums` instead of a digest written into the script.

**Files.**

- `shell/dos/src/main.rs`: the loop of [`LIVE-INPUT.md`](LIVE-INPUT.md#the-live-loop) (Escape and F12 kept by `live::route`, mark 00 after the set-up, a present only when `Shell::is_dirty`), `marks` in the argument list and the usage line, `Mkk.RAW` and `Mkk.PAL` with the read-back line at each mark, the summary line. `dump` keeps writing `SCREEN.RAW` at the end.
- `shell/dos/check.sh`: the interactive step replaced by the `live-keys` runs at depth 8 and depth 4 and the keyboard-only run, and the DOS negatives of the [table](LIVE-INPUT.md#negative-checks).
- [`FIRST-WINDOW.md`](FIRST-WINDOW.md): the hosted program's arguments and its check.

**Acceptance.** `shell/dos/check.sh` (in the CI image) passes; the scripted runs' transcripts against `expected-dos-8.txt` and `-4.txt` are unchanged; each negative fails.

**Depends on**: L1, L3, L4. Parallel with L5. **Labels**: `enhancement`, `input`, `shell`, `dos`, `testing`.

**Risks.** The read-back at a mark takes a while (300 KB of video memory and a file write); the keys typed meanwhile wait in the 64-byte ring, so `AUTOTYPE`'s pace must leave the ring room (`-p 0.3` does for the scenarios as written).

## L7. The launchers

**Goal.** A person runs `tools/run-shell.sh native` or `tools/run-shell.sh dos` and plays with the first window in a window on their desktop.

**Files.**

- `tools/run-shell.sh` (new), as [`LIVE-INPUT.md`](LIVE-INPUT.md#the-launchers) describes, with `--depth`, `--display`, `--nomouse` and `--demo`, printing the capture hotkeys before it starts and a one-line reason when something is missing (no desktop session, no host QEMU, no DOSBox in `vendor/`).
- `tools/native-run.sh`: `--window DISPLAY` (and `--timeout 0` honoured), the serial transcript shown as it arrives, the GTK and SDL options of the design, status 2 after a closed window reported as a normal end. Without `--window`, the arguments it passes QEMU are byte for byte what they were.
- `tools/dosrun.sh`: `DOS_WINDOW=1` as the design describes. Without it, the generated `dosbox.conf` and environment are what they were.
- `tools/README.md` and `README.md`: how to try each track.
- [`LIVE-INPUT.md`](LIVE-INPUT.md): every option and hotkey marked "to confirm" checked against QEMU 11.1 and the DOSBox Staging in `vendor/` and corrected; whether QEMU's GTK window scales relative motion when zoomed, measured.

**Acceptance.** `tools/check.sh` unchanged in result (the headless runners are untouched without the new switches); `bash -n` and `shellcheck` (in Docker if not installed) on the three scripts; by hand on a desktop, recorded in the PR's test plan: each track at each depth opens a window, the pointer is captured on a click and released on the hotkey, a window drags and closes, "desk" types into the field, Alt+Tab and Alt+F4 work while grabbed, Escape ends the session and the terminal shows the summary line; `--demo` plays the live scenario. What the builder could not try by hand (no desktop session) is said plainly.

**Depends on**: L5, L6. Parallel with L8. **Labels**: `enhancement`, `input`, `chore`.

**Risks.** The host's QEMU may lack the GTK or SDL module; the launcher tries `gtk`, then `sdl`, and names what is missing without installing anything. Wayland compositors may refuse the keyboard grab.

## L8. The DOS mouse under Xvfb

**Goal.** The mouse path of `SHELL.EXE` checked opt-in against `live.sums` through DOSBox's window under Xvfb, with quick clicks.

**Files.**

- `tools/xvfb-dos.sh` (new): spike B's `xvfb-inner.sh` generalised: the directory, the program and its arguments, the `xdotool` script, the mouse driver switch and the waits as arguments or environment; runs inside `desk-dos-xvfb:1`.
- `spikes/b-dos-input/check-xvfb.sh` and `xvfb-inner.sh`: use `tools/xvfb-dos.sh` (the spike's result unchanged), or keep their own copy if the generalisation would change the spike's behaviour; say which in the PR.
- `shell/dos/check-xvfb.sh` (new), run by `tools/check.sh` when `INPUT_EXTRA=1`: `live.scn` through `frames.py live --xdotool`, then `marks --dumps` and `check` against `live.sums`; the same with `--quick`; a run with the `mousemove_relative` lines removed must fail.

**Acceptance.** With Docker: `INPUT_EXTRA=1 tools/check.sh` passes, five runs in a row give the same marks; without Docker the step says it was skipped and `tools/check.sh` passes.

**Depends on**: L3, L4, L6. Parallel with L7. **Labels**: `testing`, `input`, `dos`.

**Risks.** Alt+Tab and Alt+F4 under Xvfb with no window manager reach DOSBox, but SDL or DOSBox may act on Alt+F4 itself; if so, the `--xdotool` form of `live.scn` uses a scenario variant without Alt+F4 for DOS, recorded in the design.

## L9. The result

**Goal.** A "Result" section in [`LIVE-INPUT.md`](LIVE-INPUT.md), as [`FIRST-WINDOW.md`](FIRST-WINDOW.md#result) has: what was built, the mark digests on each track and depth against the host's, the counts the runs gave, what a person saw in each window (with a screenshot of each track in `docs/images/live-input/`, taken from the captures, not from a desktop), the negatives that failed as they should, and what was not verified, first.

**Files.** `docs/LIVE-INPUT.md`, `docs/images/live-input/`, [`FIRST-WINDOW.md`](FIRST-WINDOW.md) (its "Not verified" mouse item points to the result), [`DIRECTION.md`](DIRECTION.md) (the first window's milestone runs live on both tracks), `docs/README.md` (the row's description gains the result).

**Acceptance.** `python3 -I tools/check-docs.py`; every number in the section comes from a run recorded in the PR's test plan.

**Depends on**: L1 to L8. **Labels**: `documentation`, `input`, `shell`.

## Definition of done

The milestone is done when, on `main`:

- `tools/check.sh` runs the new host tests and goldens, the native live checks at both depths, and the DOS live-keys checks at both depths, and all pass in the CI image; `INPUT_EXTRA=1` adds the DOS mouse under Xvfb, which passes where Docker exists;
- every mark of every live run equals its golden by colour digest, and by index digest where the capture has indices;
- the negative runs fail as they should, and the scripted first-window checks and their pinned files are unchanged;
- `tools/run-shell.sh native` and `tools/run-shell.sh dos` open a window a person can use, as recorded by hand in L7;
- [`LIVE-INPUT.md`](LIVE-INPUT.md) has its result section, and the documents it lists are updated.
