---
id: m-pixels
n: M13
part: V
title: The core I: surfaces, damage and presenting
summary: The 8-bit back buffer, rectangles and sprites, dirty rectangles and how damage merges, the compositor, frame digests, and the linear and planar presenters with the capture read-back.
date: 2026-10-23
reviewed: 44f0265
watches: [shell/gfx/, shell/present/, tests/shell/, tests/native/expected-shell-8.txt, tests/native/expected-shell-4.txt, kernel/src/shell.rs]
log: [firstwindow]
anatomy: [desk:gfx, desk:present, dos:gfx, dos:present]
---
## What it is

Everything the shell shows on the screen is first drawn into memory. This chapter is about the two crates that make that possible and the seam between them and the hardware. `desk-gfx` holds the [back buffer](term:back-buffer) and everything that draws into it or keeps track of what changed: the [rectangle](term:rectangle) type, the surface, the sprite, the [damage](term:damage) list, the [compositor](term:compositor), the `Present` trait and the [frame digests](term:frame-digest). `desk-present` holds the [presenters](term:presenter): the small pieces of code that take finished rows of pixels from the compositor and write them into a real frame buffer, linear or planar, plus the one function family that reads a frame buffer back.

On the map of the system ([M1](chapter:m-map)) these are two boxes of the [shared core](term:shared-core) and of the platform-side code that both tracks use: [`gfx`](chapter:anatomydesk#box-desk-gfx) and [`present`](chapter:anatomydesk#box-desk-present) on the native page, [`gfx`](chapter:anatomydos#box-dos-gfx) and [`present`](chapter:anatomydos#box-dos-present) on the DOS page. They are the same crates in both places. The hardware under them is the VGA card and its [DAC](term:dac), taught in [M3](chapter:m-devices) and [M12](chapter:m-graphics); the contract they are written against is the [HAL](term:hal) ([M9](chapter:m-hal)). The next chapter, [M14](chapter:m-text), puts text, colour roles and window chrome on top of the surface this chapter builds.

After it you should be able to say where one pixel of the first window lives in memory and what it costs to move it to the screen; read a line such as `shell: damage moved: 0 pixels repainted, 456 presented in 2 rectangles` and say why each number is what it is; explain why a cursor move repaints nothing; say what the two digests of a frame are and why two different machines can agree on them; and add a presenter for another kind of frame buffer by implementing three required methods (`width`, `height` and `span`) and, if it needs one, `align`.

## How it works

### The pixel model

The core draws into a `Vec<u8>` of `width × height` bytes, whatever the display behind it is. Each byte is a palette index, not a colour: a number from 0 to 255 that the [palette](term:palette) in the DAC turns into red, green and blue at scan-out. The buffer is row-major with a [pitch](term:pitch) equal to the width, so the pixel at `(x, y)` is byte `y × width + x`. The widest surface the compositor takes is `MAX_WIDTH`, 1,280 pixels, because it keeps a row buffer of that size on the stack; the first window is 640 × 480, which is 307,200 bytes (300 KiB), and both the kernel's 4 MiB heap and the DOS program's heap hold that comfortably.

Two depths exist. At depth 8 the byte is written to the screen as it is, to a linear frame buffer ([VBE](term:vbe) mode `0x101` is 640 × 480 × 8). At depth 4 the screen is VGA mode 12h, the [planar mode](term:planar-mode) with 16 colours, and only the low four bits of each byte reach the [bit planes](term:bit-plane). Nothing in the drawing code knows which depth it is drawing for: the theme ([M14](chapter:m-text)) hands it palette indices, and at depth 4 every index it hands out is 0 to 15. The platform loads the palette into the DAC before the first frame.

The decision to keep a full 8-bit buffer in RAM even for a 4-bit screen is the second of the design's decisions, and the reason is on the card: reading video memory is the slow direction on real hardware, and a buffer in RAM means nothing ever has to. Nothing reads the frame buffer back except the test captures (below).

### Rectangles

Everything that tracks change is made of `Rect`: four `i32` fields `x0, y0, x1, y1`, half-open, so a rectangle covers `[x0, x1) × [y0, y1)` and is empty when `x1 <= x0` or `y1 <= y0`. Half-open edges are what make adjacent rectangles share no pixel: a 12-wide rectangle at x = 320 ends at 332, and the next pixel to its right is 332, which belongs to whatever starts there. The type is `Copy`, has no allocation, and every operation saturates instead of overflowing, because a frame's arithmetic must not be able to overflow.

| Operation | What it does |
|---|---|
| `Rect::new(x, y, w, h)` | Builds from a corner and a size; far edges saturate at `i32::MAX` |
| `width`, `height`, `area` | Zero for an empty rectangle; `area` saturates at `u32::MAX` |
| `intersect` | The common part, or the default empty rectangle |
| `union` | The *bounding box* of both; an empty rectangle contributes nothing |
| `overlaps`, `contains` | Whether two share a pixel; whether a point is inside |
| `inflate(by)` | Grows every side by `by` (shrinks if negative); an empty rectangle stays empty |
| `offset(dx, dy)` | Moves it |
| `align_x(a)` | Rounds `x0` down and `x1` up to multiples of `a`; used for the planar presenter |

`union` is the whole story of merging: the code never computes an irregular union of two shapes, only the smallest rectangle that holds both. That keeps every list of damaged areas a list of rectangles, at the price of sometimes covering pixels nobody changed.

### The surface

`Surface` is the back buffer: the width, the height, the pixel vector, and a *clip* rectangle. Every drawing call is limited to the clip, which is itself kept inside the bounds, so the primitives need no bounds checks of their own and cannot write outside the buffer. The primitives are few: `fill` a rectangle, `hline` and `vline`, `outline` (a one-pixel ring just inside a rectangle), `blit` (copy a block of indices from another buffer with its own pitch), and `mono`, which draws a one-bit bitmap in a single colour. `mono` takes the form of a `.DFN` glyph row: the leftmost pixel is the highest bit of a byte, set bits are painted, clear bits are left alone, so text and icons draw over whatever is under them. Reading with `get` returns `None` off the surface and ignores the clip. The tests draw every shape at every offset around a 12 × 9 surface, with and without clips, and compare each pixel against a model, which is the cheapest way to be sure nothing leaks past an edge.

`Sprite` is a two-ink picture written as string art: `X` is the outline, `o` the fill, anything else transparent, with a hot spot. The only sprite is `ARROW`, 12 × 19 with the hot spot at its top-left pixel, whose rows are copied from the VGA driver's cursor (a test compares the two, which is why `desk-gfx` has a dev-dependency on `desk-vga` and the library does not). A sprite is not drawn into a surface; the compositor draws it into rows on their way to the screen (below).

### Damage

A frame that redraws 307,200 pixels because a cursor moved would be hopeless on a 486, so the core tracks *damage*: the rectangles of the screen whose pixels may have changed since the last frame. `Damage` is a fixed array of `MAX_RECTS` = 16 rectangles, a length, and the bounds of the screen. It keeps four invariants, each with a test: every rectangle is inside the bounds and not empty; the rectangles are pairwise disjoint, so a present writes each pixel at most once; there are never more than 16; and their union covers every rectangle added since the last `clear`.

`add(r)` keeps them like this:

1. Clip `r` to the bounds. If nothing is left, stop.
2. Put it in a scratch list after the existing rectangles.
3. Repeat until no pair merges: if two rectangles *overlap or share part of an edge*, replace them with their bounding box.
4. If the list now holds 17, merge the pair whose bounding box adds the least area beyond the two, and go back to step 3.
5. Store the list.

The test for step 3 is `mergeable`: two non-empty rectangles are mergeable when they overlap in one axis and overlap or touch in the other. Touching along an edge merges, and touching only at a corner does not (the test `touching_edges_merge_but_corners_do_not` pins it). Each merge removes one rectangle, which is why the loop ends. Step 4's cost is `extra_area`: the area of the bounding box minus the areas of the two parts. Merging is by bounding box only and never splits a rectangle, so it overdraws a little; the design prefers that to the code and the unpredictable accounting a splitter would need.

`area` is the sum of the areas, which is the number of pixels a present writes. `align_x(a)` widens every rectangle to multiples of `a` columns, clips again, and merges again, because two neighbours widened to the same 8-pixel column can start to overlap. `add_all` unions another list in.

### The `Present` trait

`Present` is where finished pixels go. It is a span writer: a width, a height, an alignment, and three calls.

| Method | Meaning |
|---|---|
| `width`, `height` | The screen size; the compositor refuses a presenter whose size differs from its own |
| `align` | Spans must start and end on multiples of this: 1 for a linear frame buffer, 8 for mode 12h; the default is 1 |
| `begin` | Before the first span of a frame; the default does nothing |
| `span(x, y, pixels)` | Row `y` from column `x`, as palette indices; the caller has kept it on the screen and aligned |
| `end` | After the last span; the default does nothing |

`MemoryPresent` is the implementation for tests and for the host: a frame buffer in RAM that also counts what it is given (`PresentStats`: frames, spans, pixels). It refuses with `InvalidArgument` a span that is off the screen or not aligned, and counts nothing for a refused span. It is the meter of the damage accounting: when a test says a cursor move "wrote 456 pixels", that number came from here.

### The compositor

`Compositor` owns the back buffer and two `Damage` lists. *Scene damage* is what must be repainted into the back buffer and also presented. *Overlay damage* is the cursor's: it is only presented, because the cursor is not stored in the back buffer at all. `CursorLayer` is a position, a visible flag, a sprite and two palette indices for its outline and fill. A frame is, in order:

1. If both lists are empty, return zeros without calling anything: an idle frame costs nothing.
2. Check the presenter. If its size differs from the compositor's, or its alignment does not divide the width, return `InvalidArgument` before painting anything.
3. For each scene rectangle, set the surface's clip to it and call the caller's `paint` closure. The compositor does not know what a window is; the closure repaints the rectangle from whatever the scene is.
4. Union the scene damage and the overlay damage and widen it to the presenter's alignment. Clear both lists.
5. Call `begin`. For each rectangle and each row, copy the back buffer's bytes for that span into a stack buffer, draw the cursor's pixels that fall in the row over it, and call `span`. Call `end`.
6. Return `FrameStats`: pixels repainted (the scene damage's area), pixels presented (the aligned union's area) and the number of rectangles.

If the presenter fails part-way, `end` is still called (the planar presenter's `begin` changed the hardware's state and `end` undoes it) and the damage that was being presented goes back into the scene damage, so the next frame tries again.

A cursor move is the case the structure exists for. `move_cursor` adds the old and the new sprite rectangles to the *overlay* list and touches nothing else, so the next frame repaints nothing and presents two small rectangles, with the cursor drawn over the unchanged back buffer. There is no save-under: the pixels beneath a cursor are in the back buffer already, and the presenter writes them back from there when the cursor leaves. That is why the design could do without ever reading video memory, which is how the [software cursor](term:software-cursor) of [M12](chapter:m-graphics) has to work without it. `digest` computes the digests of the screen as it is now, back buffer plus cursor, a row at a time, with no second buffer; `back()` is the scene without the cursor, and a test checks that the two differ exactly by the cursor.

### Frame digests

A picture is checked by reducing it to two numbers. Both are CRC-32, the IEEE polynomial as zlib uses it (the reflected constant `0xEDB88320`; the check value of the ASCII string `123456789` is `0xCBF43926`, and a test asserts it). The *index digest* is the CRC-32 of the palette indices, row by row. The *colour digest* is the CRC-32 of each pixel's six-bit `Rgb6` as three bytes `r g b`, row by row, an index past the end of the palette counting as black. A [frame digest](term:frame-digest) is the pair, with the size.

Two digests exist because two questions exist. The index digest answers "did the code draw the same bytes?" and can be compared wherever indices are visible: on the host, and on DOS, where the program reads the frame buffer back. The colour digest answers "does the screen show the same colours?", and it is the only one a QEMU screendump can give, because a screendump has colours and no indices. A screendump holds eight bits per channel; shifting each channel right by two recovers the DAC's six bits whatever expansion the emulator used. So the native run, the DOS run and the host agree on a frame without having to number their palettes the same way. The digests are computed in 64-pixel pieces so the stack does not grow with the picture.

The digests are what the goldens hold: `tests/shell/first-window.sums` has one line per frame and depth, `NAME DEPTH WIDTHxHEIGHT INDEX COLOUR`, and `tools/frames.py` reads, checks and diffs them. The [expected transcripts](term:expected-transcript) of the native run repeat each frame's digests and damage counts, so the same numbers are checked in three places.

### The linear presenter

`LinearPresent` writes to an 8-bit linear frame buffer: a VBE mode such as 640 × 480 × 8, or mode 13h ([M12](chapter:m-graphics)). It borrows an `MmioRegion` (the HAL's wrapper for memory-mapped hardware, [M9](chapter:m-hal)) and knows a width, a height and a [pitch](term:pitch) in bytes, which may exceed the width. `new` refuses a pitch smaller than the width and a region smaller than `pitch × height`, so every span that passes the check in `span` lands inside the region. A span is then one `write_bytes` at `y × pitch + x`. The alignment is 1. The HAL's `write_bytes` stores one volatile byte at a time; the design note on speed is that copying words would be faster and would not change a pixel, and it is left for a later HAL change.

### The planar presenter

`PlanarPresent` writes VGA mode 12h: four [bit planes](term:bit-plane) at `0xA0000`, 80 bytes per row each (a plane is 80 × 480 = 38,400 bytes). A pixel's four bits are in the same bit position of the same byte offset in the four planes. So eight pixels (one aligned group) are four bytes, one per plane, and `planarise` makes them: bit `p` of pixel `i` goes to bit `7 − i` of plane `p`'s byte. Eight pixels of colour 10 (binary 1010) give plane 0 `0x00`, plane 1 `0xFF`, plane 2 `0x00` and plane 3 `0xFF`; a group whose first pixel is 15 and whose other seven are 0 gives `0x80` in all four planes. `planarise` is a pure function and has its own tests, including a random row against a per-pixel reference.

That is why the planar presenter declares an alignment of 8 and the compositor widens its damage to match. A byte written to a plane holds eight pixels; if a span began mid-byte, writing it would overwrite the neighbours' bits, and the only fix would be to read the byte first, which is the slow direction. With whole bytes only, nothing is ever read. `span` refuses (`InvalidArgument`) a span that is off the screen or whose start or length is not a multiple of 8.

The hardware side is a separate trait, `PlaneSink`, so the arithmetic can be tested without ports: `begin`, `select(plane)`, `write(offset, bytes)` and `end`. `VgaPlanes` is the real sink. Its `begin` writes the graphics controller's state every frame and trusts nothing from before (write mode 0, data rotate 0 with replace, set/reset disabled so the CPU's byte is what is written, bit mask `FF`), because the VGA driver's `Planar16`, used by the read-back, turns set/reset on while it lives. `select` writes the sequencer's map mask with `1 << plane`, which sends the next write to one plane only. `end` restores the map mask to `0F`, all four planes, which is what the BIOS and the other drivers expect. A span is then, for each of four planes, one map-mask write and one `write_bytes`. If any step fails the presenter calls `end` itself before it returns, so a failed frame does not leave the map mask on one plane. The only crate in `shell/` that calls `desk-vga`'s register-level code is this one; the core crates never do.

### Reading a frame buffer back

`read_back_linear` and `read_back_planar` read the visible `width × height` indices of a frame buffer into a slice, row by row, through the VGA driver's `read_row` (the planar one reads each plane with the read-map-select register). They exist for one customer: the DOS program, which after each frame of the scenario reads its own screen and the DAC back, computes the digests of what the card holds and writes the raw dump. The native kernel does not use them; QEMU's screendump is its independent view. A read-back records what the emulated card holds, which is not the same thing as what an emulator draws, and the log chapter says so ([the first window](chapter:firstwindow)).

## Why it is this way

The decisions are `docs/FIRST-WINDOW.md`, "Decisions", written as part of the design and built as [PR #20](pr:20) (`desk-gfx`) and [PR #23](pr:23) (`desk-present`).

- **An 8-bit back buffer in RAM, always** (decision 2). The alternative, rejected in "Alternatives considered", is to draw straight into the frame buffer through the VGA driver's `Canvas`, as spike D's window did. That needs no buffer, but every overlap redraws over the bus, a cursor needs save-under reads, and mode 12h pays port writes for every primitive. `Canvas` stays the drivers' and the spikes' API, not the compositor's.
- **One drawing path, many presenters.** The second alternative was a `Canvas`-like trait for the core with presenters that implement each primitive on the frame buffer directly. It would be faster for large flat fills, and every primitive would then have two implementations whose pixels must agree, which is exactly the drift this design exists to prevent. Only the last step differs between targets, and it is a span copy.
- **Damage in two layers** (decision 3) and **the cursor composited at present time** (decision 4). Drawing the cursor into the back buffer is simpler, but then a cursor move damages the scene and repaints the windows under it. Keeping it out means a pointer move costs two 12 × 19 rectangles.
- **Bounding-box merging with at most 16 rectangles.** The comment on `Damage` says why: it overdraws a little and never splits. The numbers in the worked example show how little: a drag costs one rectangle that is the union of the old and new extents.
- **A span writer with an alignment** (decision 5). The `Present` trait is a span writer: a presenter says what it can write and the compositor adapts to it, rather than every presenter handling arbitrary rectangles.
- **The presenters are platform-side code shared by both tracks** (decision 6). They are in `shell/present` and not in the core because the rule in `docs/HAL.md` forbids the core to call a driver's register-level API; the core sees only the trait. Both the DOS program and the kernel's `shell` test mode use the same two presenter types, so a bug in the planar byte split is a bug on both tracks, found once.
- **"Identical pixels" means identical six-bit colours** (decision 8). Index equality is checked wherever indices exist, but the DAC's colours are what the eye sees, and a comparison of QEMU's 8-bit screendump would bake in how QEMU expands six bits to eight. Goldens are PNGs as well as sums, because a failing checksum says nothing about what changed and a PNG can be diffed.
- **No float, no unsafe code, no external crates** in `shell/` (decision 11), enforced by lints (`#![forbid(unsafe_code)]` and `#![deny(clippy::float_arithmetic)]` are at the top of both crates).

> **From the stacks — Why CRC.** The Wikipedia article on cyclic redundancy checks says CRCs are commonly used to detect accidental changes to digital data, by attaching a short check value to each block, and lists the CRC-32 polynomial `0x04C11DB7` in its normal form and `0xEDB88320` in its reversed form. It also says plainly that CRCs are not suitable for protecting against intentional alteration. The Desk's digests need only the first property: a wrong pixel is an accident, and no one is trying to forge a frame. The constant in `shell/gfx/src/digest.rs` is the reversed one.
> Source: [Cyclic redundancy check, Wikipedia](https://en.wikipedia.org/wiki/Cyclic_redundancy_check)

## The code

| Where | What it holds |
|---|---|
| [`shell/gfx/src/rect.rs`](repo:shell/gfx/src/rect.rs) | [`Rect`](sym:shell/gfx/src/rect.rs::Rect): half-open, saturating; `intersect`, `union`, `inflate`, `offset`, [`align_x`](sym:shell/gfx/src/rect.rs::align_x) |
| [`shell/gfx/src/surface.rs`](repo:shell/gfx/src/surface.rs) | [`Surface`](sym:shell/gfx/src/surface.rs::Surface): the back buffer, the clip, `fill`, `outline`, `blit` and [`mono`](sym:shell/gfx/src/surface.rs::mono) |
| [`shell/gfx/src/sprite.rs`](repo:shell/gfx/src/sprite.rs) | [`Sprite`](sym:shell/gfx/src/sprite.rs::Sprite) and the 12 × 19 [`ARROW`](sym:shell/gfx/src/sprite.rs::ARROW) |
| [`shell/gfx/src/damage.rs`](repo:shell/gfx/src/damage.rs) | [`Damage`](sym:shell/gfx/src/damage.rs::Damage), [`MAX_RECTS`](sym:shell/gfx/src/damage.rs::MAX_RECTS), and the merge rules in [`mergeable`](sym:shell/gfx/src/damage.rs::mergeable) and [`extra_area`](sym:shell/gfx/src/damage.rs::extra_area) |
| [`shell/gfx/src/present.rs`](repo:shell/gfx/src/present.rs) | The [`Present`](sym:shell/gfx/src/present.rs::Present) trait and [`MemoryPresent`](sym:shell/gfx/src/present.rs::MemoryPresent), the counting frame buffer in RAM |
| [`shell/gfx/src/compose.rs`](repo:shell/gfx/src/compose.rs) | [`Compositor`](sym:shell/gfx/src/compose.rs::Compositor), [`CursorLayer`](sym:shell/gfx/src/compose.rs::CursorLayer), [`FrameStats`](sym:shell/gfx/src/compose.rs::FrameStats) and [`MAX_WIDTH`](sym:shell/gfx/src/compose.rs::MAX_WIDTH) |
| [`shell/gfx/src/digest.rs`](repo:shell/gfx/src/digest.rs) | [`crc32`](sym:shell/gfx/src/digest.rs::crc32), [`Crc32`](sym:shell/gfx/src/digest.rs::Crc32) and [`FrameDigest`](sym:shell/gfx/src/digest.rs::FrameDigest) |
| [`shell/present/src/linear.rs`](repo:shell/present/src/linear.rs) | [`LinearPresent`](sym:shell/present/src/linear.rs::LinearPresent) |
| [`shell/present/src/planar.rs`](repo:shell/present/src/planar.rs) | [`planarise`](sym:shell/present/src/planar.rs::planarise), [`PlaneSink`](sym:shell/present/src/planar.rs::PlaneSink), [`VgaPlanes`](sym:shell/present/src/planar.rs::VgaPlanes), [`PlanarPresent`](sym:shell/present/src/planar.rs::PlanarPresent) |
| [`shell/present/src/readback.rs`](repo:shell/present/src/readback.rs) | [`read_back_linear`](sym:shell/present/src/readback.rs::read_back_linear) and [`read_back_planar`](sym:shell/present/src/readback.rs::read_back_planar) |
| [`tests/shell/first-window.sums`](repo:tests/shell/first-window.sums) | The twelve digest lines every run is held to |
| [`tests/native/expected-shell-8.txt`](repo:tests/native/expected-shell-8.txt) | The native transcript at depth 8, with the `shell: damage` lines |
| [`kernel/src/shell.rs`](repo:kernel/src/shell.rs) | The native wiring: builds the presenter for the depth and runs the scenario |

For depth: `docs/FIRST-WINDOW.md` ([the pixel model](doc:docs/FIRST-WINDOW.md#the-pixel-model), [desk-gfx](doc:docs/FIRST-WINDOW.md#desk-gfx-surfaces-damage-and-presenting), [desk-present](doc:docs/FIRST-WINDOW.md#desk-present-the-linear-and-planar-presenters) and [goldens and frame digests](doc:docs/FIRST-WINDOW.md#goldens-and-frame-digests)); the design notes on speed are in `docs/D-NATIVE-VGA.md`.

## Worked example

**One frame, the moved one.** The scenario `tests/shell/first-window.scn` opens two windows with the pointer at (320, 240), captures `open`, then runs `move 100 60` and captures `moved`. The native transcript at depth 8 says:

```
shell: damage moved: 0 pixels repainted, 456 presented in 2 rectangles
```

and at depth 4 `tests/native/expected-shell-4.txt` says `0 pixels repainted, 608 presented in 2 rectangles`. Here is how both come out of the code.

The pointer moves from (320, 240) to (420, 300). `move_cursor` adds the old and new sprite rectangles to the overlay list. `ARROW.rect_at` gives a 12 × 19 rectangle with the hot spot at its corner, so:

| | x0 | y0 | x1 | y1 | area |
|---|---|---|---|---|---|
| old cursor | 320 | 240 | 332 | 259 | 12 × 19 = 228 |
| new cursor | 420 | 300 | 432 | 319 | 12 × 19 = 228 |

`Damage::add` asks `mergeable` about the pair. In x, 320 < 432 and 420 < 332 is false, so they do not overlap, and they do not touch (332 < 420); the same holds in y. They stay two rectangles. The scene list is empty, so nothing is repainted (`0 pixels repainted`), and the union of the lists is the two overlay rectangles. At depth 8 the alignment is 1, so 228 + 228 = **456 pixels in 2 rectangles**. The scenario even states the expectation: `expect-damage 8 456 2`.

At depth 4 the alignment is 8, so `align_x(8)` widens each rectangle to whole bytes. The old one: x0 = 320 is already a multiple of 8 (320 = 40 × 8), and x1 = 332 rounds up to 336, so it is 16 wide. The new one: x0 = 420 rounds down to 416, x1 = 432 is already a multiple (54 × 8), so it is 16 wide. Each is 16 × 19 = 304 and the two total **608**, the second `expect-damage` line (`expect-damage 4 608 2`). After the widening the compositor merges again, and 336 < 416, so they are still apart.

**The other frames, where merging happens.** The next four lines of the transcript have one rectangle each. The windows' damage is their *extent*, the outer box inflated by the 3-pixel halo ([M14](chapter:m-text)). Notes is opened at (60, 60, 300 × 180), so its extent is (57, 57) to (363, 243). Welcome is (220, 150, 340 × 200), extent (217, 147) to (563, 353).

| Frame | What is added | Merged box | Pixels |
|---|---|---|---|
| `focus` | Notes' extent and Welcome's (it loses focus) | (57, 57)–(563, 353) | 506 × 296 = 149,776 |
| `dragged` | Notes' old extent and its new one, 40 right and 30 down: (97, 87)–(403, 273) | (57, 57)–(403, 273) | 346 × 216 = 74,736 |
| `typed` | Welcome's extent and Notes' (new place) | (97, 87)–(563, 353) | 466 × 266 = 123,956 |
| `closed` | Welcome's extent and Notes' | the same box | 123,956 |

In `focus` the two extents overlap (Welcome starts at x = 217, inside Notes' 57 to 363, and at y = 147, inside 57 to 243), so `mergeable` is true and the two become their bounding box: 563 − 57 = 506 wide, 353 − 57 = 296 high, and 506 × 296 = 149,776, which is the transcript's `149776 pixels repainted`. The cursor's old and new rectangles lie inside that box, so they merge into it and add nothing: `149776 presented in 1 rectangles`. The same sum for `dragged` is 74,736 and for `typed` and `closed` is 123,956, each equal to the transcript. The scenario's keystrokes in `typed` add small rectangles inside the field, which merge into the box as well.

At depth 4 the same boxes are widened to multiples of 8 in x: the `focus` box becomes 56 to 568, which is 512 × 296 = 151,552, and the transcript at depth 4 says `149776 pixels repainted, 151552 presented in 1 rectangles`. The difference between 149,776 and 151,552 is the price of the planar presenter's whole bytes. The rest of the depth-4 column follows the same way: 76,032 for `dragged` (352 × 216) and 125,552 for `typed` and `closed` (472 × 266).

The explainer below steps through the whole scenario, with the damage line and the digests of each frame.

<!-- explainer firstwindow -->

**The first frame.** `Compositor::new` damages the whole bounds, so `open` repaints and presents 640 × 480 = 307,200 pixels in one rectangle, as the first line of damage says.

**A small digest.** The check value used by the tests: `crc32` of the nine bytes `123456789` is `0xCBF43926`. The six frames' digests for depth 8 start with `open 8 640x480 920263aa c310363f` in `tests/shell/first-window.sums`: `920263aa` is the CRC-32 of the 307,200 indices of that frame and `c310363f` that of its 921,600 colour bytes.

The table of digests that all three runs are held to:

<!-- figure fw-digests -->

## State of this layer

Verified here means that the record cites a test that runs in `cargo test` in `shell/` or a native transcript that the native test checks. It means on the host and in QEMU, never on a real machine. The 161 host tests were run for this chapter on 2026-10-23 and passed; the native `shell` mode was also run for this chapter at both depths in QEMU 11.1.2 (std card at depth 8, Cirrus at depth 4), and both transcripts and all twelve screendump digests matched.

```status
id: pixels.rect
label: Rect and Surface: half-open saturating rectangles and clipped drawing
state: built-verified
evidence: test:shell/gfx/src/surface.rs::fill_clips_at_every_edge_and_to_the_clip, test:shell/gfx/src/rect.rs::align_x_widens_to_multiples, sym:shell/gfx/src/surface.rs::Surface
note: Every primitive is checked at every offset around a small surface, with several clips.

id: pixels.sprite
label: Sprite and the 12 x 19 arrow, equal to the VGA driver's arrow
state: built-verified
evidence: test:shell/gfx/src/sprite.rs::arrow_equals_the_vga_drivers_arrow, sym:shell/gfx/src/sprite.rs::ARROW

id: pixels.damage
label: Damage: at most 16 disjoint rectangles, merged by bounding box
state: built-verified
evidence: test:shell/gfx/src/damage.rs::invariants_hold_over_ten_thousand_random_adds, test:shell/gfx/src/damage.rs::touching_edges_merge_but_corners_do_not, test:shell/gfx/src/damage.rs::the_list_fills_to_sixteen_before_merging_the_cheapest_pair, sym:shell/gfx/src/damage.rs::Damage
note: The random test uses a fixed seed and checks all four invariants after every add.

id: pixels.compositor
label: The compositor: scene and overlay damage, the cursor drawn at present time, idle frames free
state: built-verified
evidence: test:shell/gfx/src/compose.rs::a_cursor_move_presents_only_the_two_sprite_rectangles_and_paints_nothing, test:shell/gfx/src/compose.rs::presented_frames_equal_a_full_redraw_after_every_step_of_a_random_sequence, test:shell/gfx/src/compose.rs::a_failing_presenter_leaves_the_damage_for_the_next_frame, sym:shell/gfx/src/compose.rs::Compositor

id: pixels.digest
label: Frame digests: CRC-32 of the indices and of the six-bit colours
state: built-verified
evidence: test:shell/gfx/src/digest.rs::crc32_check_value, test:shell/gfx/src/digest.rs::digest_of_a_known_two_by_two_picture, test:tests/shell/first-window.sums::open 8 640x480 920263aa c310363f, sym:shell/gfx/src/digest.rs::FrameDigest

id: pixels.linear
label: The linear presenter, equal to the in-memory presenter for the same spans
state: built-verified
evidence: test:shell/present/tests/linear.rs::the_same_spans_give_the_same_bytes_as_memory_present, test:shell/present/tests/linear.rs::spans_off_the_screen_are_refused_and_write_nothing, sym:shell/present/src/linear.rs::LinearPresent
note: On the host over a vector, and in QEMU at depth 8 on the std card with the VBE linear frame buffer.

id: pixels.planar
label: The planar presenter: planarise, the map mask per plane and whole bytes only
state: built-verified
evidence: test:shell/present/tests/planar.rs::every_colour_at_every_bit_position, test:shell/present/tests/planar.rs::recombined_planes_equal_memory_present, test:tests/native/expected-shell-4.txt::shell: damage moved: 0 pixels repainted, sym:shell/present/src/planar.rs::PlanarPresent
note: On the host through a four-plane model, and in QEMU at depth 4 on a Cirrus card in mode 12h set by registers. The register writes of VgaPlanes are checked against the mock port bus; no real card has been written to.

id: pixels.readback
label: The linear capture read-back through the VGA driver's read_row
state: built-verified
evidence: test:shell/present/tests/linear.rs::a_known_picture_reads_back, sym:shell/present/src/readback.rs::read_back_linear
note: The linear read-back has a unit test on the host.

id: pixels.readback-planar
label: The planar capture read-back, as the DOS program uses it
state: built-unverified
evidence: sym:shell/present/src/readback.rs::read_back_planar, doc:docs/FIRST-WINDOW.md#result
note: The planar read-back is used by the DOS program in DOSBox, which this chapter did not run, and no test in shell/present/tests runs it, so that path rests on the committed DOS transcripts (docs/FIRST-WINDOW.md, Result), which were not rerun.

id: pixels.damage-numbers
label: The damage counts of the scenario, host and native
state: built-verified
evidence: test:tests/native/expected-shell-8.txt::shell: damage focus: 149776 pixels repainted, test:tests/native/expected-shell-4.txt::shell: damage typed: 123956 pixels repainted, test:tests/shell/first-window.scn::expect-damage 8 456 2, sym:kernel/src/shell.rs::LinearPresent
note: Run for this chapter in QEMU at both depths and in the host test; the numbers are also reproduced by the chapter's own check from the merge rules.

id: pixels.hardware
label: Presenting on a real VGA card or a real monitor
state: planned
evidence: doc:docs/FIRST-WINDOW.md#not-verified
note: Nothing here ran on real hardware; real DAC rounding and bus timing are untested.

id: pixels.wordcopy
label: Word-wide copies in the linear presenter
state: planned
evidence: doc:docs/FIRST-WINDOW.md#where-it-sits
note: Each span byte is one volatile store today; the design leaves faster copies to a later HAL change.
```

The back buffer, the damage lists and the compositor are {{state.pixels.compositor}} on the host, and the two presenters are {{state.pixels.linear}} and {{state.pixels.planar}} in QEMU. What the table cannot say is how a real card behaves when its map mask is written four times per row: the cost figures in the design (a full mode 12h frame is 4 × 38,400 byte writes and 4 × 480 map-mask writes) are counts of operations, not measured times.

## Try it yourself

The first block runs the whole host test suite of the shared core and counts the passing tests. The second runs the native kernel's `shell` test mode at depth 8 in QEMU and checks its transcript, its damage lines and its screendumps against the committed sums. They need a Rust toolchain, NASM and `qemu-system-i386` on the host (or Docker for the last two; `tools/native-run.sh` falls back to it). Output is from a run on 2026-10-23 at `44f0265` with QEMU 11.1.2 and NASM 3.02.

```try
needs: host: cargo
ran: 2026-10-23 on the host, cargo 1.98.0
$ cd shell && cargo test 2>&1 | grep -E "^test result: ok. [1-9]"
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.74s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.72s
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.95s
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cd shell && cargo test 2>&1 | grep -c "^test .* ok$"
161
$ cd shell && cargo test -p desk-gfx 2>&1 | grep -E "^test .*(touching|the_list_fills|cursor_move)"
test compose::tests::a_cursor_move_presents_only_the_two_sprite_rectangles_and_paints_nothing ... ok
test damage::tests::touching_edges_merge_but_corners_do_not ... ok
test damage::tests::the_list_fills_to_sixteen_before_merging_the_cheapest_pair ... ok
```

The eleven lines are the test binaries that have tests: `desk-font` (15), `desk-gfx` (42), `desk-present` (5 linear and 7 planar), `desk-shell` (11, 12, 16 and 1), `desk-ui` (24) and `desk-wm` (17 and 11), which add up to the 161 that the second command counts. The scenario test (the 1) takes about a second because it runs every frame through three presenters. Now the same scenario on the native kernel:

```try
needs: host: bash, nasm, cargo, python3, qemu-system-i386 (or Docker for nasm and qemu)
ran: 2026-10-23 on the host, QEMU 11.1.2, NASM 3.02
$ tools/native-build.sh
dist/native/kernel.bin: 173968 bytes, load address 0x100000
$ tools/native-image.sh --test-mode shell --video vbe --out dist/native-shell-8.img
dist/native-shell-8.img: 1474560 bytes, kernel 173968 bytes in 340 sectors, test mode shell, video vbe
$ python3 -I tools/frames.py inject tests/shell/first-window.scn > dist/shell.inject
$ tools/native-run.sh --timeout 60 --vga std --inject dist/shell.inject --shots dist/shell-native-8 dist/native-shell-8.img > dist/shell-8.txt
$ tr -d '\r' < dist/shell-8.txt > dist/shell-8.plain
$ python3 -I tools/native-check.py tests/native/expected-shell-8.txt dist/shell-8.plain
transcript matches tests/native/expected-shell-8.txt (64 lines)
$ grep "shell: damage" dist/shell-8.plain
shell: damage open: 307200 pixels repainted, 307200 presented in 1 rectangles
shell: damage moved: 0 pixels repainted, 456 presented in 2 rectangles
shell: damage focus: 149776 pixels repainted, 149776 presented in 1 rectangles
shell: damage dragged: 74736 pixels repainted, 74736 presented in 1 rectangles
shell: damage typed: 123956 pixels repainted, 123956 presented in 1 rectangles
shell: damage closed: 123956 pixels repainted, 123956 presented in 1 rectangles
$ python3 -I tools/frames.py check tests/shell/first-window.sums 8 dist/shell-native-8
ok: 6 frames at depth 8 match tests/shell/first-window.sums
```

The depth-4 run is the same with `--test-mode shell --video text`, `--vga cirrus` and `expected-shell-4.txt`; it prints 608, 151552, 76032 and 125552 where the depth-8 run printed 456 and the equal pairs, and `frames.py check` accepts the screendumps against the depth-4 sums.

**Exercises.** (1) Without running anything, predict the `presented` count at depth 4 for a cursor move from (0, 0) to (4, 0): the old rectangle is 12 × 19 at x = 0, the new at x = 4, so they overlap; what is their bounding box, and what does it become after `align_x(8)`? Then check against `a_cursor_move_that_overlaps_writes_the_bounding_box` in `shell/shell/tests/accounting.rs`. (2) In a scratch branch change `mergeable` so that a corner touch merges; predict which `Damage` test fails and why a drag's numbers do not move. (3) Predict the four plane bytes `planarise` gives for the eight indices 1, 2, 4, 8, 15, 0, 0, 0, then add the case to a scratch test and see.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Back buffer
def: The picture the Desk draws into before anything reaches the screen: a block of RAM with one byte (a palette index) per pixel, 640 by 480 for the first window. The screen is updated from it, so video memory is never read.

term: Rectangle
def: A half-open box of pixels, covering from x0 up to but not including x1 and from y0 up to but not including y1. Two rectangles that share only an edge share no pixel.

term: Damage
def: The rectangles of the screen that may have changed since the last frame. The compositor repaints and presents only those, and keeps them at most 16, disjoint, by merging neighbours into their bounding box.

term: Compositor
def: The part of desk-gfx that owns the back buffer and the damage lists, repaints the damaged rectangles, draws the cursor over each row on its way out, and hands the rows to a presenter.

term: Frame digest
def: Two CRC-32 numbers that stand for a whole picture: one of its palette indices and one of the six-bit colours those indices stand for. Runs on different machines are held to the same digests.

term: Presenter
def: Code that takes rows of palette indices from the compositor and writes them to a real or simulated frame buffer: the linear presenter for 8-bit frame buffers and the planar presenter for VGA mode 12h.
```

```figures
id: m13-steps
reuse: first-window-steps
at: explainer firstwindow

id: m13-digests
reuse: first-window-digests
at: figure fw-digests
```

## Revisions

- 2026-10-23: first written, against `44f0265`. The host tests (161) and the native `shell` mode at both depths were run for this chapter. Every damage number in the worked example is derived from the merge rules and checked against `tests/native/expected-shell-8.txt` and `expected-shell-4.txt` by the chapter's test. Disagreements between the documents and the code were small: `docs/FIRST-WINDOW.md` describes the sprite's transparent cells as `.` where the code treats any byte other than `X` and `o` as transparent, and a comment in `shell/wm/src/wm.rs` says a focus change adds "four rings" while the code paints three (`HALO` is 3); neither changes a pixel.
