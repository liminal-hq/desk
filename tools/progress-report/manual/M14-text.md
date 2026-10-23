---
id: m-text
n: M14
part: V
title: The core II: text, themes and window chrome
summary: Reading .DFN faces, glyph lookup and measuring, colour roles and palettes, drawing text, the window chrome with its stepped halo, the control icons and the first two widgets.
date: 2026-10-23
reviewed: 44f0265
watches: [shell/font/, shell/ui/, fonts/generated/, docs/PALETTE.md, docs/HIG.md, tests/shell/]
log: [firstwindow, faces]
anatomy: [desk:ui, dos:ui]
---
## What it is

[The previous chapter](chapter:m-pixels) built a surface that can be filled, outlined and given one-bit bitmaps, and a compositor that presents only what changed. It knows nothing about letters, colours with meanings, or windows. This chapter is the layer that does. Two small crates make it: `desk-font` reads the Desk's bitmap faces, the `.DFN` files, and measures text with them; `desk-ui` decides which palette slot each kind of thing is drawn in, draws text with a face, lays out and paints a window's chrome, and holds the first two widgets, a label and a one-line text field. Neither calls the hardware. They draw into a `Surface`, and that is all.

On the map ([M1](chapter:m-map)) both crates are one box, [`ui`](chapter:anatomydesk#box-desk-ui) on the native page and [`ui`](chapter:anatomydos#box-dos-ui) on the DOS page, labelled "desk-ui, desk-font". They are part of the [shared core](term:shared-core): the same code draws the same title bars on the [native track](term:native-track) and the [hosted track](term:hosted-track). The faces themselves, how they were drawn and built, are the subject of the log chapter [The faces](chapter:faces) and, later, the fonts chapter of the manual; this chapter is about reading them.

After it you should be able to read the header of a `.DFN` file by eye, say how a character becomes a column of pixels, name the slot and the colour of every part of a window at either depth, work out the rectangle of a control box from the window's outer box, say why an unfocused window's halo is a hairline, and add a widget by following what the label and the text field do.

## How it works

### The `.DFN` file

A [DFN](term:dfn) file is a bitmap face: every character is a small picture of dots, stored as bytes. Its layout is written in `docs/FORMATS.md` ("`.DFN`: a bitmap font"), and `shell/font/src/lib.rs` is the reader. Numbers are little-endian. The file is an 8-byte header and then the glyphs, one after another:

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | The magic, the four bytes `DFN1` |
| 4 | 1 | Height: the number of rows in every glyph, the whole cell |
| 5 | 1 | Baseline: the number of rows above the baseline |
| 6 | 2 | The glyph count |
| 8 | … | For each glyph, in ascending code-point order: the code point (4 bytes), the width (1 byte) and `height` rows of `ceil(width / 8)` bytes |

The width is the *advance*: how far the pen moves after the glyph, including the gap. The leftmost pixel of a row is the highest bit of its first byte, which is exactly the form that `Surface::mono` draws ([M13](chapter:m-pixels)), so the drawing step needs no conversion. A [glyph](term:glyph) here means one character's picture and its advance; the reader hands one out as a `Glyph` holding its code point, advance, height, stride (bytes per row) and a slice of its rows borrowed from the file.

The four committed faces, read from their headers:

| File | Header bytes | Height | Baseline | Glyphs | Size in bytes |
|---|---|---|---|---|---|
| `fonts/generated/ui.dfn` | `44 46 4E 31 0E 0B 21 01` | 14 | 11 | 289 | 5,639 |
| `fonts/generated/title.dfn` | `44 46 4E 31 0E 0C 21 01` | 14 | 12 | 289 | 5,975 |
| `fonts/generated/small.dfn` | `44 46 4E 31 08 07 21 01` | 8 | 7 | 289 | 3,765 |
| `fonts/generated/fixed.dfn` | `44 46 4E 31 10 0D 4C 01` | 16 | 13 | 332 | 6,980 |

The count is two bytes, so `21 01` is `0x0121` = 289 and `4C 01` is `0x014C` = 332. The first window uses two of the four: Desk Title (`title.dfn`) for window titles and Desk UI (`ui.dfn`) for everything inside a window. The kernel embeds both with `include_bytes!` because it has no filesystem yet, and the DOS program embeds them too and can also load them from files.

`Font::parse` takes the whole file's bytes and checks every glyph once, so the lookups that come after it cannot fail on a malformed file. It refuses, with a `FontError` that carries the offset where that helps: a file that does not start with `DFN1` (`BadMagic`), one that ends inside the header or inside a glyph (`Truncated`), a height of zero (`ZeroHeight`), a baseline larger than the height (`BaselineOutOfCell`), a glyph whose code point is not above the previous one (`OutOfOrder`) and bytes left after the last glyph (`TrailingBytes`). The parse builds an index of `(code point, offset)` pairs in a `Vec` and keeps the borrowed bytes; the crate is `no_std` with `alloc`, and has no dependencies at all.

`glyph(c)` is a binary search of the index by code point, which is why the format requires ascending order. `glyph_or_fallback(c)` returns the glyph for `?` when the face lacks `c`, and an empty glyph of advance 0 if it lacks `?` too. `advance(s)` is the sum of the advances of the characters of `s`, fallbacks included: the width of a line of text is a sum, with no kerning. `Glyph::bit(x, y)` reads one pixel and is clear outside the cell. All four files have `?`, `…` (U+2026) and `×` (U+00D7) and none has U+FFFD, so the fallback is always `?`. The crate has no drawing code.

### Drawing text

`draw_text` takes a surface, a face, an `x`, the `top` of the cell, a string and a palette index. For each character it fetches the glyph (or the `?`) and calls `Surface::mono` with the glyph's rows, then moves right by the advance and returns the new `x`. It does not move down for the baseline: the caller names the top of the cell, which is why the layout code below says "cell top at `y + 4`". Set bits take the colour; clear bits leave whatever is underneath, so text does not erase its background.

`fit_with_ellipsis` cuts a line to a width. If the whole text fits in `max` pixels it comes back unchanged with `false`. Otherwise it finds the longest prefix whose advance, plus the advance of `…`, fits, and returns it with `true`; the caller draws the prefix and then `…`. If not even the ellipsis fits, the prefix is empty. For example, with the title face (where `…` is 9 pixels wide) and 60 pixels of room, `Welcome to Desk` is 99 pixels and does not fit; the room left for text is 60 − 9 = 51, the characters `Welcome` take 49, and the next, the space, would make 53, so the result is `Welcome` followed by `…`.

### Roles, slots and the palette

A drawing routine in the core never names a palette number. It names a *role*, and a [theme](term:theme) maps roles to [slots](term:slot) for the depth in use. A slot is an entry in the palette: a number from 0 to 255 that the DAC turns into a colour. The design reason is in `docs/PALETTE.md`: "Apps draw with colour roles, not slot numbers", because the Desk owns the 256-colour [palette](term:palette) and can change what a role looks like without touching any app.

`Role` has 15 values and `Theme::afterglow(depth)` holds the slot of each, one table for depth 8 and one for depth 4. The colours are the Afterglow look's; the hex values are `docs/PALETTE.md`'s.

| Role | Used for | Colour | Slot at depth 8 | Slot at depth 4 |
|---|---|---|---|---|
| `Desktop` | The Void wallpaper | `#050507` | 97 | 0 |
| `Chrome` | Title bars, control boxes | `#0c0c10` | 98 | 1 |
| `Content` | Window content, the inside of a field | `#111116` | 99 | 2 |
| `Raised` | Hovered control boxes (not used yet) | `#1b1e28` | 102 | 3 |
| `Line` | The rule under a title bar, the unfocused hairline | `#262a37` | 103 | 4 (`#333847`) |
| `Border` | Field outlines | `#333847` | 104 | 4 |
| `TextMuted` | Unfocused control icons | `#7c8796` | 5 | 5 |
| `TextSecondary` | Unfocused titles, focused control icons | `#b4bfce` | 6 | 6 |
| `Text` | Focused titles, body text | `#e0e0e0` | 7 | 7 |
| `Accent` | Halo ring 1, the caret | `#ffaa40` | 8 | 8 |
| `Halo2` | Halo ring 2 | `#7d5422` | 9 | 9 |
| `Halo3` | Halo ring 3 | `#372612` | 10 | 10 |
| `Selection` | Not used yet | `#372612` | 10 | 10 |
| `CursorOutline` | The arrow's outline | `#050507` | 240 | 0 |
| `CursorFill` | The arrow's fill | `#e0e0e0` | 241 | 7 |

`palette(depth)` builds the 256 DAC entries the platform loads before the first frame. Slots 0 to 15 are always the 16-colour set (`DESK16`), which is also the only set in mode 12h; at depth 8 the eight Afterglow surfaces (`AFTERGLOW_SURFACES`) go into 97 to 104 and the cursor into 240 and 241; every other entry is black until `PALETTE.md` gives the ramps values. At depth 4 only the first sixteen are filled and the attribute palette is the identity, so slot *n* is DAC entry *n* ([M12](chapter:m-graphics)). The two depths differ where `PALETTE.md` says they must: the 4-bit theme has no `#262a37`, so `Line` falls to slot 4, `#333847`, and that is why the two depths have separate goldens.

The hex values become six-bit DAC colours with `Rgb6::from_rgb8`, which is `(v × 63 + 127) / 255` per channel, the nearest DAC value. `#e0e0e0` is 224 in each channel and becomes (224 × 63 + 127) / 255 = 55. A test (`the_constants_equal_the_hex_values_in_palette_md`) reads the tables of `docs/PALETTE.md` and compares them to the constants, so the document and the code cannot drift apart silently.

### The window chrome

[Window chrome](term:window-chrome) is the Desk-drawn frame around a window: the halo, the title bar, the three control boxes, the rule and the content area. `docs/HIG.md` says the Desk draws the title bar and the border and apps draw everything inside. All the measurements come from the window's *outer box*, which excludes the halo; the halo is drawn outside it. `ChromeLayout::new(outer)` turns an outer box into the rectangles, and widens a box smaller than 120 × 40 to that minimum so a layout is never degenerate.

| Part | Rectangle (outer box at `x, y`, `w` × `h`) | Role |
|---|---|---|
| Title bar | `(x, y, w, 22)` | `Chrome` |
| Title text | cell top at `y + 4`, from `x + 8`, 14 rows high, ending 8 pixels before the first control box | `Text` focused, `TextSecondary` unfocused |
| Control boxes | three 18 × 18 at `y + 2`; the close box's right edge at `x + w − 8`; 2 pixels apart; minimise, maximise, close | `Chrome` |
| Control icons | 10 × 10 one-bit bitmaps centred in each box | `TextSecondary` focused, `TextMuted` unfocused |
| Rule | `(x, y + 22, w, 1)` | `Line` |
| Content | `(x, y + 23, w, h − 23)` | `Content` |
| Halo | outside the outer box, 3 pixels | below |
| `extent` | the outer box inflated by 3 | what the window owns on screen |

The **stepped halo** is the focused window's border. It is three one-pixel rings just outside the outer box: ring 1 at `inflate(1)` in `Accent` (orange `#ffaa40`), ring 2 at `inflate(2)` in `Halo2` (`#7d5422`) and ring 3 at `inflate(3)` in `Halo3` (`#372612`), each darker than the last, so the glow fades into the desktop in steps instead of a gradient. The same three slots serve at both depths. An unfocused window paints only ring 1, and in `Line`: a hairline. It paints nothing at 2 and 3 pixels out, so whatever lies beneath (the desktop, or a lower window) shows through, and tests assert both cases (`the_focused_halo_has_three_rings_and_nothing_at_four`, `the_unfocused_window_has_a_hairline_and_nothing_further_out`). `extent`, the outer box plus 3 on every side, is what the window manager damages for a window ([M15](chapter:m-shell)): since it is the same in both focus states, a focus change never has to know what the other state looked like.

`paint_chrome` paints in this order, all inside the surface's clip: the three rings (or the hairline), the title-bar fill, the rule, the content fill, the title (cut with `…` if it does not fit and the ellipsis fits), and the three boxes, each filled and then given its icon. The painter's order is also why the compositor can repaint any sub-rectangle of a window: every call is clipped.

The three icons are `Icon`s: 10 rows of two bytes in the `.DFN` row form, written as string art with `X` for a set pixel (`Icon::from_art` packs them at compile time), drawn with `Surface::mono`. The minimise icon is a bar along the bottom, the maximise a square with a heavier top edge, and the close icon a cross two pixels thick so it holds its weight against the box. They are drawn but inert: nothing in the first window handles a click on minimise or maximise.

### The first widgets

A widget lives in a window's content area; its position is relative to the content's top-left, and `paint` takes that corner as an `origin`. `Widget` is an enum of two: a `Label` (a position, a string and a colour role) and a `TextLine`, a one-line text field. A label paints its text. A field is `FIELD_HEIGHT` = 20 pixels high at its position and as wide as it was given: a `Border` outline, `Content` inside, the text in `Text` starting 6 pixels in with the cell 3 pixels down, and, while its window is focused, a 1 × 12 caret in `Accent` directly after the text, starting 4 pixels down. The caret does not blink, because blinking needs time and a frame in this core depends on nothing but the steps that led to it. The field paints its text with the clip set to the inside of the outline, so text can never run over the border.

`key` is how a widget takes input. Only key presses count; chords with Control, Alt or the Windows key are ignored; Backspace removes the last character; any other key is turned into a character with `desk_kbd::char_for` ([M11](chapter:m-input)), and only printable ASCII is accepted. A character is also refused when the field is at `max_chars` or when the text and its caret would no longer fit inside the outline. The return value is the useful part: `Some(rect)` when the field changed, where the rectangle covers the old and new text and caret, relative to the content area, so the window manager can damage exactly that and nothing more. A label takes no keys.

## Why it is this way

- **Roles, resolved per depth** (`docs/FIRST-WINDOW.md`, "Decisions", number 7; built in [PR #24](pr:24)). Drawing code names `Chrome` or `Text`; the theme maps that to a slot. The 8-bit theme uses the look's surfaces in 97 to 127 and the cursor in 240 and 241, the 4-bit theme uses the 16-colour set, and the drawing code is identical at both. The alternative, drawing with slot numbers, would make every depth a separate copy of every painting routine.
- **The code wins on rounding.** `Rgb6::from_rgb8` is the nearest DAC colour, not the older "rounded to a multiple of four" rule, because the two differ by one step in some channel for 16 of the 19 colours involved and spikes A and D had already used the first. `docs/PALETTE.md` now says so; `docs/FIRST-WINDOW.md`, "The pixel model", still describes `PALETTE.md` as saying the multiple-of-four rule and records the choice as a question for the owner.
- **A bitmap face is the whole of the font system, for now.** The faces are drawn on grids and built into `.DFN` by `fonts/build.ts` ([The faces](chapter:faces)); the reader is about 170 lines of Rust with no dependencies, and drawing a glyph is a single `mono` call with no scaling. The alternative, reading TrueType files inside the Desk, is spike `s6-ttf` in `docs/OPEN-QUESTIONS.md` and has not started.
- **Fonts are embedded for the first window** (decision 10), because the native kernel has no filesystem yet. The DOS program loads the same files as a second path that must give the same digests.
- **No kerning.** A line's width is the sum of advances, so measuring is trivial and exact and a golden is not at the mercy of a pairing table.
- **Chrome is measured from the outer box, and the halo is outside it.** That makes the layout independent of focus and lets `extent` be a constant inflation. The measurements follow board 3a and the v4 prototype's "Stepped halo" in `design/`, which is reference only.
- **The first window has two widgets.** A label and a one-line field are enough to exercise text drawing and keyboard focus; menus, tabs, a toolbar and the rest of `docs/HIG.md`'s anatomy are not built.

> **From the stacks — One image set per size.** A bitmap font stores each glyph as an array of pixels, and the Wikipedia article says a bitmap typeface "requires a separate font design for each size". Its example is that a font with three sizes and any combination of bold and italic needs 12 complete sets of images. It also says the limited processing power and memory of early computer systems forced the exclusive use of bitmap fonts. The Desk accepts the same trade with four faces of three cell heights (8, 14 and 16 rows) and draws each by hand.
> Source: [Bitmap fonts, Wikipedia](https://en.wikipedia.org/wiki/Bitmap_fonts)

## The code

| Where | What it holds |
|---|---|
| [`shell/font/src/lib.rs`](repo:shell/font/src/lib.rs) | [`Font`](sym:shell/font/src/lib.rs::Font) with [`parse`](sym:shell/font/src/lib.rs::parse), [`glyph`](sym:shell/font/src/lib.rs::glyph), [`glyph_or_fallback`](sym:shell/font/src/lib.rs::glyph_or_fallback) and [`advance`](sym:shell/font/src/lib.rs::advance); [`Glyph`](sym:shell/font/src/lib.rs::Glyph) and [`FontError`](sym:shell/font/src/lib.rs::FontError) |
| [`shell/ui/src/theme.rs`](repo:shell/ui/src/theme.rs) | [`Role`](sym:shell/ui/src/theme.rs::Role), [`Depth`](sym:shell/ui/src/theme.rs::Depth), [`Theme`](sym:shell/ui/src/theme.rs::Theme) and the two slot tables |
| [`shell/ui/src/palette.rs`](repo:shell/ui/src/palette.rs) | [`DESK16`](sym:shell/ui/src/palette.rs::DESK16), [`AFTERGLOW_SURFACES`](sym:shell/ui/src/palette.rs::AFTERGLOW_SURFACES) and [`palette`](sym:shell/ui/src/palette.rs::palette) |
| [`shell/ui/src/text.rs`](repo:shell/ui/src/text.rs) | [`draw_text`](sym:shell/ui/src/text.rs::draw_text) and [`fit_with_ellipsis`](sym:shell/ui/src/text.rs::fit_with_ellipsis) |
| [`shell/ui/src/chrome.rs`](repo:shell/ui/src/chrome.rs) | [`ChromeLayout`](sym:shell/ui/src/chrome.rs::ChromeLayout), [`paint_chrome`](sym:shell/ui/src/chrome.rs::paint_chrome), [`HALO`](sym:shell/ui/src/chrome.rs::HALO), [`TITLE_HEIGHT`](sym:shell/ui/src/chrome.rs::TITLE_HEIGHT) and [`BOX`](sym:shell/ui/src/chrome.rs::BOX) |
| [`shell/ui/src/icons.rs`](repo:shell/ui/src/icons.rs) | [`Icon`](sym:shell/ui/src/icons.rs::Icon) and the three icons [`MINIMISE`](sym:shell/ui/src/icons.rs::MINIMISE), [`MAXIMISE`](sym:shell/ui/src/icons.rs::MAXIMISE) and [`CLOSE`](sym:shell/ui/src/icons.rs::CLOSE) |
| [`shell/ui/src/widget.rs`](repo:shell/ui/src/widget.rs) | [`Widget`](sym:shell/ui/src/widget.rs::Widget), [`Label`](sym:shell/ui/src/widget.rs::Label), [`TextLine`](sym:shell/ui/src/widget.rs::TextLine) and the field's measurements |
| [`fonts/generated/ui.dfn`](repo:fonts/generated/ui.dfn) | The Desk UI face the first window draws its body text in; [`title.dfn`](repo:fonts/generated/title.dfn) is the title face |
| [`docs/PALETTE.md`](repo:docs/PALETTE.md) | The slot table and the 16-colour set that a test compares with the code |

For depth: `docs/FIRST-WINDOW.md` ([desk-font](doc:docs/FIRST-WINDOW.md#desk-font-reading-dfn), [roles, slots and palettes](doc:docs/FIRST-WINDOW.md#roles-slots-and-palettes), [text](doc:docs/FIRST-WINDOW.md#text), [window chrome](doc:docs/FIRST-WINDOW.md#window-chrome) and [widgets](doc:docs/FIRST-WINDOW.md#widgets)), `docs/FORMATS.md` ([the format](doc:docs/FORMATS.md#dfn-a-bitmap-font)), `docs/PALETTE.md` ([the slots the first window uses](doc:docs/PALETTE.md#the-slots-the-first-window-uses)) and `docs/HIG.md` ([anatomy](doc:docs/HIG.md#anatomy)).

## Worked example

The matrix below is the Desk's OpenType files, which the same build makes from the same drawings. The four `.dfn` faces this chapter reads are not in it, and their glyph counts (289 and 332) are not the OpenType files' counts:

<!-- figure font-matrix -->

**One glyph out of the Desk UI face.** The lower-case `d` of Desk UI is at byte offset 1,328 of the file. Its record starts with the code point `64 00 00 00` (`0x64` is 100, the letter `d`), then the width `06`, then 14 rows of one byte each (the stride is `ceil(6 / 8)` = 1):

```
00 00 08 08 08 68 98 88 88 98 68 00 00 00
```

Reading each byte as bits with the leftmost pixel in the highest bit and keeping the first six columns gives the picture (`#` is set):

```
......
......
....#.
....#.
....#.
.##.#.
#..##.
#...#.
#...#.
#..##.
.##.#.
......
......
......
```

The ascender of the `d` starts at row 2 and the bowl at row 5. The face's baseline is 11, which means 11 rows are above it, and the bottom of the bowl is at row 10, the last of those. Rows 11 to 13 are the room for descenders, empty for `d`. The advance is 6, so the pen moves six pixels after drawing it.

**The first window's Notes window.** The scenario opens it with `open notes 60 60 300 180 "Notes"`. `ChromeLayout::new` gives, from the outer box (60, 60) to (360, 240):

| Part | Rectangle | How |
|---|---|---|
| Title bar | (60, 60), 300 × 22 | the outer box's width and `TITLE_HEIGHT` |
| Close box | (334, 62), 18 × 18 | 360 − 8 − 18 = 334; `y + 2` = 62 |
| Maximise box | (314, 62) | 334 − 20 (a box and a gap of 2) |
| Minimise box | (294, 62) | 334 − 40 |
| Title text | (68, 64), 218 × 14 | from `x + 8`, `y + 4`; it ends 8 pixels before 294, at 286: 286 − 68 = 218 |
| Rule | (60, 82), 300 × 1 | `y + 22` |
| Content | (60, 83), 300 × 157 | `y + 23`, height 180 − 23 |
| Extent | (57, 57) to (363, 243) | the outer box inflated by 3 |

The title `Notes` in Desk Title is 35 pixels wide (the sum of its five advances), so it fits in 218 pixels and is not cut. The same arithmetic for `Welcome to Desk`, opened at (220, 150) with width 340, puts its close box at 560 − 8 − 18 = 534 to 552 and y from 152 to 170; the scenario's click at (543, 161) is in the middle of it. The room for that title is 258 pixels and the title is 99, so it is not cut either.

**Typing into the field.** `field welcome 12 36 300` puts the field at (12, 36) in Welcome's content, whose top-left is (220, 173), so the field's box is (232, 209), 300 × 20. The text cell starts at x = 232 + 6 = 238, y = 209 + 3 = 212, and the caret at y = 209 + 4 = 213. Typing `d` appends a glyph of advance 6 (the file says `d` is 6, `e` 6, `s` 5, `k` 5: "desk" is 22 pixels). Before the key the text is empty, so the cell is empty and the caret is the 1 × 12 box at (238, 213). After it the cell is (238, 212) 6 × 14 and the caret moves to x = 244. `key` returns the union of the old caret, the new cell and the new caret, which is x from 238 to 245 and y from 212 to 226 on the screen: 7 × 14 = 98 pixels, inside the field, and that is all the window manager damages for that key. (The rectangle `key` hands back is relative to the content area, so the value it returns is (18, 39) to (25, 53); the window manager adds the content's corner at (220, 173).) In the scenario's `typed` frame the keystroke boxes lie inside the box that the focus change had already damaged ([M13](chapter:m-pixels), worked example), so they add nothing to the transcript's 123,956.

**The ellipsis.** With 60 pixels of room the title `Welcome to Desk` is cut to `Welcome` and `…`: 49 + 9 = 58 pixels, as the arithmetic above gives. A test (`fit_with_ellipsis_keeps_the_longest_prefix_that_fits_with_the_ellipsis`) holds the rule.

## State of this layer

Verified here means that the record cites a test that runs in `cargo test` in `shell/`, which was run for this chapter on 2026-10-23 (161 tests, all passing), or a native transcript that the native test checks and that was also run. It means on the host and in QEMU, never on a real machine. The frames the window draws were looked at by the project's own review only for the four `typed` frames (see [the first window](chapter:firstwindow)); the rest rest on digests.

```status
id: text.dfn
label: The .DFN reader: parse with full validation, glyph lookup by binary search, fallback and measuring
state: built-verified
evidence: test:shell/font/tests/faces.rs::the_four_faces_parse_with_the_metrics_the_files_hold, test:shell/font/tests/faces.rs::a_missing_character_falls_back_to_the_question_mark, test:shell/font/tests/faces.rs::code_points_out_of_order, sym:shell/font/src/lib.rs::Font
note: The four committed faces parse and every glyph is consumed exactly; each of the six errors has a test on a small file.

id: text.glyphs
label: Glyph rows equal the face file's, bit order leftmost in the high bit
state: built-verified
evidence: test:shell/font/tests/faces.rs::glyph_bit_reads_the_leftmost_pixel_from_the_highest_bit_and_is_clear_outside, test:shell/font/tests/faces.rs::the_text_art_sample_matches_the_committed_one, sym:shell/font/src/lib.rs::Glyph

id: text.palette
label: Palettes and roles: the constants equal PALETTE.md, both depths resolve every role
state: built-verified
evidence: test:shell/ui/tests/ui.rs::the_constants_equal_the_hex_values_in_palette_md, test:shell/ui/tests/ui.rs::every_role_resolves_at_both_depths_as_the_table_says, test:shell/ui/tests/ui.rs::depth_4_fills_only_the_first_sixteen, sym:shell/ui/src/theme.rs::Theme
note: Only the 16-colour set, the eight surfaces and the cursor have values. Slots 16 to 96 and 105 to 239 are black; the ramps are not given values in PALETTE.md.

id: text.draw
label: Text drawing and the ellipsis cut
state: built-verified
evidence: test:shell/ui/tests/ui.rs::draw_text_equals_the_glyphs_drawn_one_by_one, test:shell/ui/tests/ui.rs::a_missing_character_draws_a_question_mark, test:shell/ui/tests/ui.rs::fit_with_ellipsis_keeps_the_longest_prefix_that_fits_with_the_ellipsis, sym:shell/ui/src/text.rs::draw_text

id: text.chrome
label: Window chrome: layout from the outer box, the stepped halo, the hairline, title and control boxes
state: built-verified
evidence: test:shell/ui/tests/ui.rs::the_layout_follows_the_chrome_table, test:shell/ui/tests/ui.rs::the_focused_halo_has_three_rings_and_nothing_at_four, test:shell/ui/tests/ui.rs::the_unfocused_window_has_a_hairline_and_nothing_further_out, sym:shell/ui/src/chrome.rs::paint_chrome
note: The pictures were checked by digest against the goldens on the host, DOS and native tracks; only the four typed frames were looked at by eye by the project's own review.

id: text.icons
label: The three control icons, drawn centred in their boxes
state: built-verified
evidence: test:shell/ui/tests/ui.rs::the_control_icons_are_drawn_centred_in_each_box_in_the_icon_colour, sym:shell/ui/src/icons.rs::CLOSE

id: text.widgets
label: The label and the one-line text field with its caret
state: built-verified
evidence: test:shell/ui/tests/ui.rs::the_returned_rectangle_covers_the_old_and_new_text_and_caret_and_stays_in_the_field, test:shell/ui/tests/ui.rs::a_field_paints_its_outline_inside_text_and_a_caret_only_when_focused, test:shell/ui/tests/ui.rs::chords_do_not_backspace_and_a_full_field_refuses_what_would_not_fit, sym:shell/ui/src/widget.rs::TextLine
note: Keys reach the field from scripted events. Nothing here was driven by a person at a real keyboard on the native track.

id: text.frames
label: The drawn windows are the same pixels on the host and on the native kernel (DOS rests on the committed result)
state: built-verified
evidence: test:tests/native/expected-shell-8.txt::shell: frame typed 8 640x480, test:tests/native/expected-shell-4.txt::shell: frame typed 4 640x480, test:tests/shell/first-window.sums::typed 8 640x480 90edf2e0 2cd55463, sym:kernel/src/shell.rs::UI_FONT
note: The native runs were repeated for this chapter in QEMU at both depths and matched. The DOS equality rests on the committed result in docs/FIRST-WINDOW.md, which this chapter did not rerun (DOSBox was not run).

id: text.controls
label: Minimise and maximise doing anything
state: planned
evidence: doc:docs/FIRST-WINDOW.md#what-the-first-window-is
note: The boxes are drawn; clicks on them do nothing.

id: text.widgets-more
label: Menus, toolbars, tabs and the rest of the guidelines' anatomy
state: planned
evidence: doc:docs/HIG.md#anatomy
note: Only a label and a text field exist.

id: text.ramps
label: The hue ramps and a Horizon wallpaper in the palette
state: planned
evidence: doc:docs/PALETTE.md#the-slots-the-first-window-uses
note: PALETTE.md names ramp steps without their values, so the slots stay black.

id: text.ttf
label: Reading TrueType files inside the Desk
state: planned
evidence: doc:docs/OPEN-QUESTIONS.md#reading-truetype-fonts
note: Spike s6-ttf is described and not started.
```

The reader and the drawing are {{state.text.dfn}} and {{state.text.draw}}, and the chrome is {{state.text.chrome}}, on the host and in QEMU. The tag does not say how the faces look on a real monitor, which is a question about the DAC and the display and not about this code, and it does not say the look is finished: the HIG's menu bar, toolbar and tabs are {{state.text.widgets-more}}.

## Try it yourself

These commands look at the `.DFN` headers this chapter reads by eye and run the tests of the two crates. They need `xxd` and a Rust toolchain on the host. Output is from a run on 2026-10-23 at `44f0265`.

```try
needs: host: xxd, cargo
ran: 2026-10-23 on the host, cargo 1.98.0
$ xxd -l 8 fonts/generated/ui.dfn
00000000: 4446 4e31 0e0b 2101                      DFN1..!.
$ xxd -l 8 fonts/generated/fixed.dfn
00000000: 4446 4e31 100d 4c01                      DFN1..L.
$ cd shell && cargo test -p desk-font 2>&1 | grep -E "^test (the_four|bad_magic|zero_h|a_missing)"
test bad_magic ... ok
test a_missing_character_falls_back_to_the_question_mark ... ok
test zero_height ... ok
test the_four_faces_parse_with_the_metrics_the_files_hold ... ok
$ cd shell && cargo test -p desk-ui 2>&1 | grep -E "^test (the_focused|the_unfocused|a_long)"
test a_long_title_ends_in_an_ellipsis_before_the_control_boxes ... ok
test the_focused_halo_has_three_rings_and_nothing_at_four ... ok
test the_unfocused_window_has_a_hairline_and_nothing_further_out ... ok
$ cd shell && cargo test 2>&1 | grep -c "^test .* ok$"
161
```

The first two commands show the header: `DFN1`, height 14 (`0e`), baseline 11 (`0b`) and the count `21 01` (289) for Desk UI, and height 16 (`10`), baseline 13 (`0d`) and `4c 01` (332) for Desk Fixed. The last command is the whole shared core's test count, the same 161 that [M13](chapter:m-pixels) counts.

**Exercises.** (1) Without running anything, read the header `44 46 4E 31 08 07 21 01` and say the face's height, baseline and glyph count, then find which file it is above. (2) The close box of a window whose outer box is (100, 40, 200 × 120): predict its rectangle, then check against `the_layout_follows_the_chrome_table` in `shell/ui/tests/ui.rs`. (3) Predict what `xxd -s 1328 -l 20 fonts/generated/ui.dfn` prints for the letter `d` and compare it with the worked example.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Glyph
def: One character's picture in a bitmap face, with its advance: the number of pixels the pen moves after drawing it. In a DFN file a glyph is a code point, a width and one row of bytes per row of the cell.

term: DFN
def: The Desk's bitmap font file. An 8-byte header (DFN1, height, baseline, glyph count) followed by glyphs in ascending code-point order, each row's leftmost pixel in the highest bit.

term: Theme
def: A table that maps colour roles such as Chrome and Text to palette slots for one screen depth, so drawing code names what a colour is for and never a slot number.

term: Window chrome
def: The frame the Desk draws around a window: the stepped halo, the title bar with its title, the three control boxes and the rule above the content. The app draws everything inside.

term: Slot
def: An entry in the 256-colour palette: a number the frame buffer holds, which the DAC turns into a colour. Roles resolve to slots, one table per depth.
```

```figures
id: m14-fontmatrix
reuse: font-matrix
at: figure font-matrix
```

## Revisions

- 2026-10-23: first written, against `44f0265`. The `.DFN` headers were read from the files, the host tests (161) and the native `shell` mode at both depths in QEMU were run for this chapter, and every number in the worked example is checked against the font files and the layout code by the chapter's test. Disagreements found: `docs/FIRST-WINDOW.md` ("The pixel model") describes `docs/PALETTE.md` as rounding to a multiple of four, while `docs/PALETTE.md` now says the nearest DAC value and the code uses it; `docs/HIG.md` ("Colour roles") describes fields as sunken `#050507`, where the first window's text field is a `Border` outline round a `Content` fill (`#111116`), so the field follows the first window's design, not the guidelines.
