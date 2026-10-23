---
id: m-shell
n: M15
part: V
title: The core III: windows, events and scenarios
summary: The window manager (windows, z-order, focus, hit testing, dragging and the damage each change makes), the shell that turns input events into frames, the scenario format that scripts them, and the narrow seam the two platforms call.
date: 2026-10-23
reviewed: 44f0265
watches: [shell/wm/, shell/shell/, tests/shell/, docs/FIRST-WINDOW.md]
log: [firstwindow]
anatomy: [desk:wm, desk:shell, desk:seam, dos:wm, dos:shell]
---
## What it is

The first two chapters of the shared core ([M13](chapter:m-pixels) for surfaces and damage, [M14](chapter:m-text) for text and window chrome) built the means to draw: a back buffer of palette indices, a list of rectangles that need repainting, a way to hand them to a screen, and the pieces of a window. This chapter is the part that decides what to draw. It covers two crates of the [shared core](term:shared-core), and a file format that exercises them. `desk-wm` is a [window manager](term:window-manager): it keeps the list of open windows, decides which is in front and which has the keyboard, works out what part of a window a point on the screen belongs to, moves a window when its title bar is dragged, and says what rectangle of the screen each of those changes disturbed. `desk-shell` sits on top of it. It takes [input events](term:input-event) in, keeps the pointer and its cursor, passes the events to the window manager, collects the damage and, on request, repaints and presents one frame. The third piece is the [scenario](term:scenario): a short text file of steps that opens windows, moves a pointer, types keys and says where to take a picture, so that the same story can be told to the shell on the host, on DOS and on the native kernel and the pictures compared.

On the map of the system ([M1](chapter:m-map)) these are the boxes `wm` and `shell` of the Desk track, and the same two boxes on the [DOS anatomy page](chapter:anatomydos#box-dos-wm), where they are drawn inside `SHELL.EXE`. The box between the shell and the platforms is `seam` on the [Desk anatomy page](chapter:anatomydesk#box-desk-seam): not a crate but a contract, the two calls (`Shell::handle` and `Shell::frame`) that a platform makes and the rule that the platform, not the core, owns the loop. [M16](chapter:m-platforms) puts the two wirings side by side. Nothing in this chapter touches a port, an interrupt or a video register: both crates are `no_std`, contain no `unsafe` and no floating point, and know the machine only through three small types from the [HAL](term:hal) (`KeyEvent`, `MouseEvent` and `HalError`) and the `Present` trait of `desk-gfx`.

After the chapter you should be able to say, for any change to a window, which rectangles it damages and why that is the minimum the code can say; follow one scenario step from its line in the file through `Shell::handle` and `Wm::pointer` to a number in a native transcript; write a scenario of your own and know which errors the parser will give you; and read `Shell`'s methods well enough to add a way to change the screen without forgetting to report the damage.

## How it works

### Windows, z-order and focus

A `Window` (`shell/wm/src/window.rs`) is a name, a numeric `WindowId`, an outer box (a `Rect` of `x0, y0, x1, y1`), a title, a list of widgets and the index of the widget that takes keys. The `Wm` holds them in one `Vec<Window>`, ordered from the back of the screen to the front. That order is the [z-order](term:z-order): the last window in the vector is drawn last, so it is on top. The same vector answers the question of [focus](term:focus). The window with the keyboard is the one that is last, and `Wm::focused` simply returns the id of `windows.last()`. There is no separate "focused" field to fall out of step with the order. The rule that makes this work is stated in the file header: focus always raises, and closing the top window focuses the next.

Opening a window (`Wm::open`) hands out an id, widens the box to the minimum the chrome needs (`MIN_WIDTH` 120 by `MIN_HEIGHT` 40, constants of `desk-ui`), pushes the window on top and reports its damage. Ids count up from 0, and skip any still in use when the counter wraps at 65,536. `Wm::focus` finds a window and moves it to the end, unless it is already there, in which case it does nothing and reports nothing. `Wm::close` removes the window; if it was on top, the window that becomes the top is damaged too, because it must now draw itself as the focused one.

What a window *owns* on screen is more than its box. The focused window wears a three-pixel halo of rings around the outer box (`HALO` is 3 in `desk-ui`), and an unfocused window wears a one-pixel hairline. `Window::extent` is the outer box inflated by the halo, and it is the unit of damage: whenever a window changes in a way that can touch its halo or its title bar, the manager adds the whole extent. The comment at the head of `wm.rs` gives the reason: a focus change repaints the title bar and the four rings, and the damage set would merge those five rectangles into the whole extent anyway, so the extent is what is added.

### Hit testing

`Wm::hit(x, y)` answers "what is under this point?" with a window id and a `Hit`: `Title`, `Control(Minimise | Maximise | Close)`, `Content` or `Ring`. It walks the windows from the top down and returns the first that claims the point, so the topmost window wins (a [hit test](term:hit-test) is exactly this, a search from the front). Inside one window (`Window::hit`) the order of the questions matters: if the point is inside the outer box, the three 18 by 18 control boxes are tried first, then the rest of the 22-pixel title bar, and what remains, the rule under the title bar and the content area, is `Content`. Outside the outer box the point is a `Ring` only if it lies within the reach of the window's current halo: three pixels for the focused window and one for any other, since an unfocused window paints nothing further out, and the rest of that ground belongs to whatever lies beneath. The function takes a `focused` flag for this reason, and `Wm::hit` passes `i == top`.

### The pointer: presses, drags and the close box

`Wm::pointer(x, y, buttons, damage)` is the only entry for the mouse. It is told where the pointer now is and the state of the buttons, and it works out what happened by comparing with the last buttons it saw. Only the left button matters, and only its *edges* do: a press is a button that was up and is now down, a release the reverse. The level does nothing, which means a stream of reports with the button held changes nothing by itself. A test (`button_edges_not_levels_drive_presses`) holds this down.

The order of events inside one call is fixed. First, if a drag is in progress, the window follows the motion (`drag_to`). Then, on a press, `press` runs a hit test, focuses and raises the window under the pointer, and decides what else the press means. On the title bar it starts a drag, remembering the *grab*: the offset of the pointer inside the outer box (so the window does not jump to put its corner under the pointer). On the close box it *arms* the window by recording its id, and nothing closes yet. On a release, the drag ends, and if a window was armed and the pointer is still over that same window's close box, `close` runs. A release anywhere else disarms and closes nothing, which is the behaviour of every graphical close button and is tested (`a_release_outside_the_close_box_does_not_close`). The minimise and maximise boxes are drawn but inert (`minimise_and_maximise_are_inert`).

Dragging puts the grabbed point under the pointer and then clamps the window so its title bar stays reachable: its top edge between 0 and the screen height less 22, and at least `REACH` (24) pixels of the title bar on screen to the left and right. A window pushed against an edge and then pulled back follows the pointer at once, because the position is recomputed from the pointer each time and not accumulated (`a_clamped_window_follows_the_pointer_back`). If the clamped position equals the current one, `drag_to` returns without adding damage.

### Keys

`Wm::key(e, fonts, damage)` sends a key to the focused window's focused widget, and to nothing else. A window has at most one widget that takes keys, set by whoever adds it; if there is none, the key is dropped. The widget (a text line in this scenario) handles the key and returns the rectangle it changed, in its own coordinates; the manager offsets it by the content area's origin, cuts it to the content area, and adds it. Typing a letter into the field therefore damages the field's changed part, not the window.

### The damage each change makes

The whole design of the manager is that it reports damage precisely, so that the compositor repaints no more than it must. The table is the contract, and the eleven tests of `shell/wm/tests/damage.rs` are its rows.

| Change | Damage added | Where decided |
|---|---|---|
| Open the first window | its `extent` | `Wm::open` |
| Open another | its `extent`, and the old top window's `extent` (it loses its halo) | `Wm::open` |
| Close a window that is not on top | its `extent` | `Wm::close` |
| Close the window on top | its `extent`, and the new top's `extent` (none if it was the last) | `Wm::close` |
| Focus the window already on top | nothing | `Wm::focus` |
| Focus a window that raises | its `extent` and the previous top's `extent` | `Wm::focus` |
| Drag by (dx, dy) | the old `extent` and the new one, which the damage set merges when they overlap | `Wm::drag_to` |
| A drag that does not change the position | nothing | `Wm::drag_to` |
| A key into a text field | the changed rectangle of the field, on screen | `Wm::key` |
| Pointer motion with no drag | nothing: the cursor is overlay damage, added by the shell | `Wm::pointer` |

The merge rule is in `desk-gfx` ([M13](chapter:m-pixels)): the damage set holds at most 16 disjoint rectangles, and two that overlap become their bounding box. That is why the numbers in the worked example below are areas of bounding boxes.

### Painting

`Wm::paint(surface, clip, theme, fonts)` is the painter's algorithm in the smallest form: inside the clip, fill the desktop colour, then paint each window whose extent meets the clip, from the back of the vector to the front. Each window draws its chrome (`paint_chrome` of `desk-ui`, see [M14](chapter:m-text)) and then its widgets with the clip narrowed to the content area, so a widget cannot draw over the title bar. The function saves the surface's clip, restores it at the end, and changes no pixel outside the clip. There is no occlusion culling: `docs/FIRST-WINDOW.md` says windows are few and that culling waits for a measurement that says it is needed.

### The shell: events in, frames out

`Shell` (`shell/shell/src/shell.rs`) owns everything a frame is a function of: a `Compositor` (the back buffer, the damage and the cursor), the `Wm`, the `Theme`, the two fonts, the pointer position and the button state. It holds no type of a platform. `Shell::new` takes a screen size, a colour depth (8 or 4 bits) and the fonts; it refuses a size the compositor cannot take (zero, or wider than `MAX_WIDTH`) with `HalError::InvalidArgument`, puts the pointer at the centre and opens nothing.

`Shell::handle(Event)` is the one door for input. `Event` has three variants. `Pointer(MouseEvent)` is a mouse report: motion (`dx`, `dy`) added to the pointer and the state of the buttons now. `PointerTo(x, y)` sets the pointer to a position and leaves the buttons alone; scenarios use it for clarity. `Key(KeyEvent)` is a key going down or up. A pointer event stores the buttons and calls `point`, which clamps the position to the screen (so the pointer is always on it, from 0 to width minus 1), moves the cursor, creates an empty damage set, passes position and buttons to `Wm::pointer`, and flushes whatever damage the manager reported into the compositor. A key event does the same through `Wm::key`.

The cursor is not a window and not part of the scene. The compositor draws it at present time over the back buffer, and moving it adds *overlay* damage: it is presented but never repainted. That is why pointer motion costs two small rectangles and no repainting.

`Shell` has its own methods to change the screen without input: `open`, `close`, `focus` (by name, returning false for an unknown name), `label` and `field` (which add a widget to the named window, and make the field the window's focused widget), and `damage_all`. Each reports its damage to the compositor, and that is the point of having them. `Shell::wm()` hands out the manager for looking, and its doc comment warns that a change made through it directly is not damaged. A caller that bypasses the shell would draw a stale picture, with no error: the incremental-versus-full test (`incremental_equals_full_after_every_step`) exists to catch such a mistake by repainting the whole screen after every step and comparing it with the incrementally damaged one.

`Shell::frame(&mut out)` repaints what changed and presents it, cursor included, and returns `FrameStats`: `repainted` pixels (the area of the scene damage), `presented` pixels (the area handed to the presenter, after widening to its alignment and adding the overlay) and `rects`, the number of rectangles. A frame with nothing changed writes nothing (`a_frame_with_no_events_writes_nothing`). `Shell::digest` returns the two checksums of what is on the screen. Nothing in the shell sleeps, waits or reads a clock: the core is a function of the events given to it, which is what lets a scenario produce the same bytes everywhere.

### The scenario format

A [scenario](term:scenario) is a UTF-8 text file with one step per line. A `#` starts a comment; blank lines are ignored; strings are in double quotes with `\"` and `\\` as the only escapes. The first line must be `desk-scenario 1`. The parser (`desk_shell::scenario::parse`) turns the text into a `Scenario` (a screen size, a list of depths and a list of `Step`s) or returns a `ParseError` with a line number counted from 1.

| Line | Step | Notes |
|---|---|---|
| `screen W H` | (header) | once, before any step |
| `depths 8 4` | (header) | which depths to run at; both if absent |
| `open NAME X Y W H "TITLE"` | `Step::Open` | focused and on top |
| `label NAME X Y "TEXT"` | `Step::Label` | position from the content area's top-left |
| `field NAME X Y W` | `Step::Field` | takes the window's keys |
| `close NAME`, `focus NAME` | `Step::Close`, `Step::Focus` | |
| `pointer X Y` | `Step::Event(PointerTo)` | |
| `move DX DY` | `Step::Event(Pointer)` | buttons as they were; offsets must fit `i16` |
| `press left`, `release left` | `Step::Event(Pointer)` | also `right`, `middle`; the buttons field is the state afterwards |
| `key NAME down`, `key NAME up` | `Step::Event(Key)` | `NAME` is a `KeyCode` variant, case matters |
| `type "TEXT"` | `Step::Type` | expanded by `type_events` |
| `frame NAME` | `Step::Frame` | present, then capture |
| `expect-damage DEPTH PIXELS [RECTS]` | `Step::ExpectDamage` | bound on the last frame, at that depth only |

The parser does a great deal of checking, so that a bad scenario is an error naming its line and never a wrong picture. It refuses a first line other than `desk-scenario 1`, a `screen` or `depths` line twice or after a step, a name that is not one to eight characters of `a-z`, `0-9` and `-` (so a frame's capture can be an 8.3 file on DOS), a frame name used twice, and one that is a prefix of another (the native run waits for the line `shell: ready NAME` by substring, so a name that began another would match early). It refuses a window used before it is opened or after it is closed, a key name that is not a `KeyCode`, and a `type` string with a character that no key types. `ParseErrorKind` has one variant per mistake, and `tests/parse.rs` checks each of them. The parser keeps the button and modifier state as it goes, so `press` and `release` produce pointer events whose `buttons` are the state afterwards, as the HAL reports them.

`type "desk"` shows what a step becomes. `type_events` turns each character into a key-down and a key-up, found by asking `desk_kbd::char_for` which key produces that character, preferring the unshifted one. A capital or a shifted symbol is wrapped in Left Shift down and up, so `type "aB"` is six events (a down and up for `a`, then shift down, `B` down and up, shift up). "desk" is eight events. Each event's modifiers are the state *after* it, as the HAL reports them.

### The runner

`run` (`shell/shell/src/run.rs`) plays a scenario at one depth. It checks that the shell's depth is the one asked for (`RunError::DepthMismatch` otherwise), and then steps through the list: a `Frame` step presents a frame and calls the caller's `capture(name, shell, stats)` closure; an `ExpectDamage` step for this depth fails the run if the last frame wrote more pixels or more rectangles than allowed (a step for the other depth is skipped, and one before any frame is `NoFrame`); anything else goes to `apply`. `apply` is public on its own: it does the effect of a step without presenting, which is what a platform that wants to keep its own loop uses. A frame is presented only at `Frame` steps, not after every step, so the damage of every step since the previous frame is still pending when it comes. That is why an `expect-damage` line placed after a `frame` covers everything since the frame before it.

### The seam

What the platform does with all this is small, and it is the seam of the title. The platform owns the loop. It makes a presenter over its screen, drains its input source into `Shell::handle` as `Event`s, calls `Shell::frame` with the presenter, and decides when to stop. The core never calls the platform: it has no callback, no trait object for the machine, and no way to ask for time. The platform can choose to hand the shell a scenario instead of live input, and then either call `run` and let it drive the frames, or call `apply` and `frame` itself. [M16](chapter:m-platforms) shows that the two platforms make different choices here, and why a general `Platform` trait is not in the code yet.

## Why it is this way

- **Focus is position.** The last window is the focused one, so there is exactly one state to keep right and "focus always raises" cannot be violated (`Wm` header comment). The price is that focus without raising is impossible, a thing some window systems allow. `docs/FIRST-WINDOW.md` ("desk-wm") states the rule, and out-of-scope lists Alt+Tab and resizing.
- **Report the damage, do not discover it.** Each manager method adds the rectangles it knows it disturbed, and the compositor never compares frames. The alternative, repainting and diffing, would cost a full-screen compare per frame; the decision is recorded in `docs/FIRST-WINDOW.md` ("Decisions", the dirty-rectangle entry), and the cost is the need for a test per row of the table above.
- **The extent is added whole.** A focus change disturbs a title bar and four rings, which merge into the extent anyway; adding the extent directly avoids five calls for the same result (`wm.rs` header).
- **Edges, not levels.** Reports from a real mouse repeat the button state every time. Acting on a change makes the same code right for a scenario (which sends `press` once) and a device (which sends the state every time).
- **The platform owns the loop.** `docs/FIRST-WINDOW.md` ("Where it sits") gives the reason: the first window needs only a display and input, and both platforms already have those; folding the calls into one `Platform` value is a later step, once a second consumer such as timers or storage needs it. Nothing prevents it.
- **One scenario file, embedded everywhere, scripted input.** Decision 9 of `docs/FIRST-WINDOW.md`: the host test, the DOS program and the kernel all `include_str!` the same file, so the platforms cannot drift from the goldens through input timing. The cost is that the scripted events are not evidence about a real mouse; live input is a separate, later check.
- **The parser is strict.** Frame names that fit 8.3 and never prefix one another look pedantic until one remembers that the native run waits for a line by substring and the DOS capture is a file name.
- **Where document and code differ.** `docs/FIRST-WINDOW.md` and `tests/shell/README.md` say that `expect-damage` is checked "on the host only" and that the platforms skip it. The DOS program does skip it, because it steps the scenario with `apply`. The native kernel does not: it calls `run`, which enforces it, so the 456-pixel and two-rectangle bound on the `moved` frame is checked in the kernel at depth 8 too (the transcript's `456 presented in 2 rectangles` equals the bound exactly). The code is what this chapter describes.

## The code

| Where | What it holds |
|---|---|
| [`shell/wm/src/wm.rs`](repo:shell/wm/src/wm.rs) | [`Wm`](sym:shell/wm/src/wm.rs::Wm): [`open`](sym:shell/wm/src/wm.rs::open), [`close`](sym:shell/wm/src/wm.rs::close), [`focus`](sym:shell/wm/src/wm.rs::focus), [`hit`](sym:shell/wm/src/wm.rs::hit), [`pointer`](sym:shell/wm/src/wm.rs::pointer), [`drag_to`](sym:shell/wm/src/wm.rs::drag_to), [`key`](sym:shell/wm/src/wm.rs::key), [`paint`](sym:shell/wm/src/wm.rs::paint) and the constant [`REACH`](sym:shell/wm/src/wm.rs::REACH) |
| [`shell/wm/src/window.rs`](repo:shell/wm/src/window.rs) | [`Window`](sym:shell/wm/src/window.rs::Window), [`WindowId`](sym:shell/wm/src/window.rs::WindowId), [`WindowSpec`](sym:shell/wm/src/window.rs::WindowSpec), [`Hit`](sym:shell/wm/src/window.rs::Hit) and the extent |
| [`shell/wm/src/paint.rs`](repo:shell/wm/src/paint.rs) | the painter's algorithm inside a clip |
| [`shell/wm/src/lib.rs`](repo:shell/wm/src/lib.rs) | the exports and [`Fonts`](sym:shell/wm/src/lib.rs::Fonts) |
| [`shell/shell/src/shell.rs`](repo:shell/shell/src/shell.rs) | [`Shell`](sym:shell/shell/src/shell.rs::Shell): [`handle`](sym:shell/shell/src/shell.rs::handle), [`frame`](sym:shell/shell/src/shell.rs::frame), [`digest`](sym:shell/shell/src/shell.rs::digest), the by-name changes and [`damage_all`](sym:shell/shell/src/shell.rs::damage_all) |
| [`shell/shell/src/lib.rs`](repo:shell/shell/src/lib.rs) | [`Event`](sym:shell/shell/src/lib.rs::Event) |
| [`shell/shell/src/scenario.rs`](repo:shell/shell/src/scenario.rs) | [`Scenario`](sym:shell/shell/src/scenario.rs::Scenario), [`Step`](sym:shell/shell/src/scenario.rs::Step), [`FrameName`](sym:shell/shell/src/scenario.rs::FrameName), [`ParseErrorKind`](sym:shell/shell/src/scenario.rs::ParseErrorKind), [`parse`](sym:shell/shell/src/scenario.rs::parse), [`type_events`](sym:shell/shell/src/scenario.rs::type_events) |
| [`shell/shell/src/run.rs`](repo:shell/shell/src/run.rs) | [`run`](sym:shell/shell/src/run.rs::run), [`apply`](sym:shell/shell/src/run.rs::apply), [`RunError`](sym:shell/shell/src/run.rs::RunError) |
| [`shell/wm/tests/damage.rs`](repo:shell/wm/tests/damage.rs), [`behaviour.rs`](repo:shell/wm/tests/behaviour.rs) | the damage table and the hit, press, drag and close rules, as tests |
| [`shell/shell/tests/`](repo:shell/shell/tests/) | `accounting.rs` (pixels written per change), `incremental.rs` (incremental equals full, the runner), `parse.rs` (every parse error), `scenarios.rs` (every `.scn` through every presenter against its sums) |
| [`tests/shell/first-window.scn`](repo:tests/shell/first-window.scn) | the scenario the worked example follows |
| [`tests/shell/first-window.sums`](repo:tests/shell/first-window.sums) | the two checksums of each frame at each depth |
| [`tests/shell/README.md`](repo:tests/shell/README.md) | what the files are, what the host test checks, how to re-record |

For depth: `docs/FIRST-WINDOW.md` ([desk-wm](doc:docs/FIRST-WINDOW.md#desk-wm-windows-focus-and-damage), [desk-shell](doc:docs/FIRST-WINDOW.md#desk-shell-events-frames-and-scenarios), [the scenario format](doc:docs/FIRST-WINDOW.md#the-scenario-format), [where it sits](doc:docs/FIRST-WINDOW.md#where-it-sits)).

## Worked example

**One click, one drag, and the numbers they leave in the native transcript.** The scenario `tests/shell/first-window.scn` opens `notes` at `60 60 300 180` and then `welcome` at `220 150 340 200`. Welcome is last, so it is on top and focused. The extents (the outer box inflated by 3) are Notes `(57,57)` to `(363,243)`, which is 306 by 186, and Welcome `(217,147)` to `(563,353)`, which is 346 by 206.

*The click on Notes' title bar.* The steps are `pointer 120 70`, `press left`, `release left`, `frame focus`.

1. `pointer 120 70` is `Event::PointerTo(120, 70)`. The shell clamps it (it is on screen), moves the cursor, and calls `Wm::pointer(120, 70, 0, ...)`. No button changed, no drag is running, so the manager adds nothing. The cursor's damage goes to the overlay.
2. `press left` is `Event::Pointer` with `dx` and `dy` zero and the left bit set. The shell stores the buttons and calls `point` at the same position. `Wm::pointer` sees an up-to-down edge and runs `press`. `hit(120, 70)` walks from the top: Welcome's outer box starts at x 220, and the point is not within a halo of it, so it is not a hit. Notes is next (tested unfocused: hairline only), and the point is inside its outer box and inside its title bar (y 60 to 82), clear of the control boxes, which are at the right: `Hit::Title`.
3. `press` calls `focus(notes)`. Notes is not last, so the manager adds the old top's extent (Welcome's), moves Notes to the end and adds its extent. The two overlap, so the set merges them into their bounding box: x from 57 to 563 is 506 wide, y from 57 to 353 is 296 high. Because the hit was `Title`, `press` also records the drag with the grab `(120 - 60, 70 - 60)`, which is `(60, 10)`.
4. `release left` clears the drag. Nothing else changes.
5. `frame focus` repaints the merged rectangle. The native transcript says `shell: damage focus: 149776 pixels repainted, 149776 presented in 1 rectangles`. And 506 × 296 is 149,776. The cursor at (120, 70) lies inside the rectangle, so the overlay adds no area.

*The drag.* `press left` hits Notes' title again (it is now on top, so `focus` does nothing; the grab is again `(60, 10)`). `move 40 30` is a `Pointer` event with `dx` 40, `dy` 30 and the left button down. The shell moves the pointer to (160, 100) and calls `Wm::pointer`; the drag runs first: the new corner is `(160 - 60, 100 - 10)`, which is `(100, 90)`, well inside the clamps. `drag_to` adds the old extent `(57,57,306,186)` and the new one `(97,87,306,186)`. They overlap: the bounding box is 346 wide (57 to 403) and 216 high (57 to 273). 346 × 216 is 74,736, and the transcript says `shell: damage dragged: 74736 pixels repainted, 74736 presented in 1 rectangles`. The picture's two checksums, `2abd99f3` and `a8143a01`, are in `first-window.sums` and in the transcript's `shell: frame dragged 8 640x480 2abd99f3 a8143a01`.

*The three frames that are not what you would guess.* The `open` frame is the whole screen, 640 × 480 = 307,200 pixels in one rectangle, since a new compositor starts with everything damaged. The `moved` frame has `0 pixels repainted, 456 presented in 2 rectangles`: nothing in the scene changed, and the cursor's old and new sprites, each 12 × 19 = 228 pixels, are the two rectangles. The `closed` frame is `123956` pixels: Welcome's extent (217..563 by 147..353) and Notes' new extent (97..403 by 87..273) have the bounding box 466 × 266 = 123,956, the same number as the `typed` frame, whose raise of Welcome damaged the same pair.

The figure below steps through all six frames of the scenario with their checksums and pictures.

<!-- explainer firstwindow -->

The frame lines of `tests/native/expected-shell-8.txt` are a transcript of a run, and the figure is built from the same files; it is not a second source of truth about the machine. Depth 4 differs: each cursor rectangle is widened to 8-pixel columns, so `moved` presents 608 pixels, which `first-window.scn` bounds with its second `expect-damage` line.

## State of this layer

```status
id: shell.wm
label: Windows, z-order, focus, hit testing, dragging, the close box and the damage each change reports
state: built-verified
evidence: test:shell/wm/tests/damage.rs::focus_that_raises_reports_the_raised_extent_and_the_previous_focus, test:shell/wm/tests/behaviour.rs::press_motion_release_on_a_title_moves_the_window_by_the_motion, sym:shell/wm/src/wm.rs::Wm
note: Host tests, run for this chapter (cargo test in shell/: 161 passed, 0 failed). 28 of those are the wm's own. Minimise and maximise are inert by design.

id: shell.shell
label: Shell::handle, Shell::frame, the cursor and the damage accounting
state: built-verified
evidence: test:shell/shell/tests/accounting.rs::a_cursor_move_writes_the_two_sprite_rectangles_and_repaints_nothing, test:shell/shell/tests/incremental.rs::incremental_equals_full_after_every_step, sym:shell/shell/src/shell.rs::Shell

id: shell.scenario
label: The scenario format, its parser and the runner
state: built-verified
evidence: test:shell/shell/tests/parse.rs::each_malformed_line_is_an_error_naming_its_line, test:shell/shell/tests/incremental.rs::run_checks_expect_damage_and_fails_one_that_is_exceeded, test:shell/shell/tests/scenarios.rs::every_scenario_matches_its_sums_through_every_presenter

id: shell.native
label: The scenario on the native kernel, depth 8, with the transcript and the screendumps held to the same sums
state: built-verified
evidence: test:tests/native/expected-shell-8.txt::shell: frame closed 8 640x480 53532002 379c4c02, test:tests/shell/first-window.sums::closed 8 640x480 53532002 379c4c02, sym:kernel/src/shell.rs::run
note: Run for this chapter on QEMU with the standard VGA card: the transcript matched the expected file (64 lines), the digests matched the sums and the six screendumps matched at depth 8. Depth 4 (Cirrus, mode 12h) was not rerun; it rests on the committed expected-shell-4.txt.

id: shell.dos
label: The scenario in SHELL.EXE under DOSBox
state: built-unverified
evidence: test:tests/shell/expected-dos-8.txt::shell: readback closed 8 640x480 53532002 379c4c02 equal, sym:shell/dos/src/main.rs::scenario
note: Rests on a committed transcript, which this chapter did not rerun (it needs the DOS toolchain image). Its capture is a read-back of an emulated card.

id: shell.live
label: Live keys and mouse driving the shell on DOS (the interactive mode)
state: built-unverified
evidence: sym:shell/dos/src/main.rs::interactive, doc:docs/FIRST-WINDOW.md#the-platform-wirings
note: Keys were typed by AUTOTYPE in the project's own check; the mouse is not exercised by any check. Not run for this chapter.

id: shell.livenative
label: Live input and a free-running frame loop on the native kernel
state: planned
evidence: doc:docs/FIRST-WINDOW.md#both-tracks-at-once

id: shell.hardware
label: The shell on a real VGA card, monitor and 486
state: planned
evidence: doc:docs/FIRST-WINDOW.md#not-verified
```

The window manager and the shell are {{state.shell.wm}} and {{state.shell.shell}} on the host, and the runner is {{state.shell.scenario}}. The native run is {{state.shell.native}} in QEMU; the DOS run is {{state.shell.dos}}; live input on DOS is {{state.shell.live}} and on the native kernel {{state.shell.livenative}}; a real machine is {{state.shell.hardware}}. "Verified" here means on the host or in an emulator.

> **From the stacks — Stacking.** The Wikipedia article on stacking window managers says the order in which windows are stacked is called their z-order, and that the Xerox Alto, which it describes as containing the first working commercial GUI, used a stacking window manager. It also says the Xerox Star used tiling for most main application windows and overlapping only for dialogue windows. The page names no inventor.
> Source: [Stacking window manager, Wikipedia](https://en.wikipedia.org/wiki/Stacking_window_manager)

## Try it yourself

The host tests need only `cargo`. The suite is the whole `shell/` workspace, so the totals include the chapters before this one.

```try
needs: host: cargo
ran: 2026-10-23 on the host
$ cd shell && cargo test --locked 2>&1 | grep -E "^test result: ok\. [1-9]"
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.74s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.71s
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.94s
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The eleven lines add up to 161 passed and none failed. The window manager's own are the last two (17 behaviour tests and 11 damage tests); the shell's are the 11 accounting, 12 incremental, 16 parse and 1 scenario test. The scenario test is the one that matters most: it plays every `.scn` through three presenters and holds the result to the sums. To re-record the goldens after a deliberate change, `tools/shell-goldens.sh` runs it in record mode; run on a clean tree it left `git status` empty and ended with `ok: 12 golden PNGs match tests/shell/first-window.sums`.

**Exercises.** (1) Without running anything, work out the damage when `Notes` is dragged by (-100, 0) from its starting place (60, 60): where does the clamp stop it, and is the answer one rectangle or two? Then add a `move -100 0` to a scratch copy of the scenario's drag and read the `dragged` line of the host failure. (2) Add `expect-damage 8 100 1` after `frame focus` in a scratch copy and predict which `RunError` the host test prints and what numbers it contains (the transcript line above has the pixel count). (3) Write a scenario that closes `welcome` by `close welcome` and not by a click; predict whether the `closed` damage changes (compare `Shell::close` with the manager's release path) and the parser errors you would get by closing it twice.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Window manager
def: The part of a graphical system that keeps the open windows, decides which is in front and which has the keyboard, finds what a pointer position means and moves windows. In the Desk it is the crate desk-wm, and it reports the screen area each change disturbed.

term: Z-order
def: The order in which overlapping windows are stacked, back to front. In desk-wm it is the order of one vector, so the last window is the top one and the focused one.

term: Focus
def: Which window receives the keyboard. In the Desk the focused window is always the top window: focus raises, and closing the top window focuses the next.

term: Hit test
def: The search for what is under a point, made from the front: the topmost window that claims the point wins, and within it a control box, the title bar, the content or the halo.

term: Scenario
def: A text file of steps (open a window, move the pointer, type, capture a frame, bound the damage) that the host, the DOS program and the native kernel all play through the same parser and runner, so that their pictures can be compared.
```

```figures
id: m15-first-window
reuse: first-window-steps
at: explainer firstwindow
```

## Revisions

- 2026-10-23: first written, against `44f0265`. The worked example reproduces the `focus`, `dragged`, `moved`, `open` and `closed` damage lines of the native transcript from the window extents (506 × 296, 346 × 216, 2 × 228 and 466 × 266). `cargo test` in `shell/` and the native depth 8 shell run were rerun for this chapter. Found while writing: `docs/FIRST-WINDOW.md` and `tests/shell/README.md` say the platforms skip `expect-damage`; the native kernel enforces it because it plays the scenario with `run`, and only the DOS program skips it (see "Why it is this way").
