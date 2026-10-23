---
id: m-fonts
n: M18
part: VII
title: Fonts and formats
summary: The four bitmap faces and their grid, how one build writes .DFN, TrueType and OpenType files from the same pixels, the Windows 3.1-era constraints, the outline faces, the emoji set with its tiers and the .DEM file, and the file formats that are only designed.
date: 2026-10-23
reviewed: 44f0265
watches: [fonts/build.ts, fonts/src/, fonts/generated/, docs/FONTS.md, docs/EMOJI.md, docs/FORMATS.md]
log: [faces, pictures]
anatomy: [dos:assets]
---
## What it is

This chapter is about the files that hold the Desk's letters and pictures, and the program that writes them. Every character the Desk draws starts as a grid of `#` and `.` in a TypeScript file under `fonts/src/faces/`. One build, `fonts/build.ts`, turns those grids into the bitmap files the Desk reads (`.DFN` for text, `.DEM` for emoji) and into the TrueType and OpenType files that browsers, design tools and Windows 3.1-era systems read. Because every file comes from the same assembled pixels, they cannot drift apart, and a check (`bun build.ts --check`) fails if a committed file differs from what the sources would produce.

On the map of the system ([M1](chapter:m-map)) the files land in one box, the embedded assets of the DOS program (`dos:assets`: `UI.DFN`, `TITLE.DFN` and the scenario). The native kernel embeds the same two faces with `include_bytes!`. The other end of the story, how a [`.DFN`](term:dfn) file is parsed and how its [glyphs](term:glyph) are drawn, is [M14](chapter:m-text), and this chapter does not repeat it: it adds the writer's side. The log tells the history in [The faces](chapter:faces) and [The pictures](chapter:pictures).

After this chapter you should be able to say what the four bitmap faces are and which grid each uses, read the header of any `.DFN` or `.DEM` file by eye and say what size the rest of the file must be, explain why the TrueType files carry no hinting and no emoji, add a glyph, and tell which of the file formats named in `docs/FORMATS.md` exist as code and which are only words.

## How it works

### The four bitmap faces

A [bitmap face](term:bitmap-face) is a font whose every character is drawn by hand as a grid of pixels, with no outline behind it. The Desk has four, and `docs/FONTS.md` ("Faces") gives their jobs. The numbers below are read from the `.DFN` files themselves, so they are exact.

| Face | For | Cell (rows) | Rows above the baseline | Advance | Glyphs in the `.DFN` | File size in bytes |
|---|---|---|---|---|---|---|
| Desk UI | Menus, lists, buttons, body text | 14 | 11 | proportional, 2 to 11 | 289 | 5,639 |
| Desk Title (bold) | Window titles, headings | 14 | 12 | proportional, 3 to 12 | 289 | 5,975 |
| Desk Fixed | Terminal, code, paths | 16 | 13 | 8 | 332 | 6,980 |
| Desk Small | Status bars, badges | 8 | 7 | 6 | 289 | 3,765 |

Desk UI is called "11 px type" because its capitals are 8 rows tall, but its em is 12 rows and its line is the 14-row cell. Desk Fixed has 43 more glyphs than the others because it also covers every printable character of code page 437 (the control-character pictures, Greek and mathematics), as the Terminal must show what DOS programs print. All four cover Windows code page 1252, the box drawing of code page 437 including the rounded corners, the blocks and shades, a set of typographic and shell symbols, and a folder (U+1F4C1) and a floppy disk (U+1F4BE). The last two are the highest code points in each file, which is why the last glyph of every `.DFN` is `0x1F4C1`.

The same drawings also become the OpenType and TrueType files, in two styles. The matrix below counts the glyphs of each of those files, which is not the `.DFN` count.

<!-- figure font-matrix -->

### The grid

A glyph is rows of `#` (ink) and `.` (blank) joined with `/`. Rows start at the capital-height row unless the glyph says otherwise: `x(...)` starts at the x-height, `asc(...)` at the ascender and `at(row, ...)` at any row of the cell. In a proportional face the advance is the drawn width plus a gap of blank columns, so a glyph's side bearings are part of its drawing; in a fixed face the rows are placed exactly as written from column 0 of the cell, and a glyph may not be wider than the cell. The rows of the cell, top to bottom:

| Face | Accents | Ascender | Capitals | x-height | Baseline under row | Descenders |
|---|---|---|---|---|---|---|
| UI | 0 to 2 | 2 | 3 to 10 | 5 to 10 | 10 | 11 to 13 |
| Title | 0 to 2 | 2 | 3 to 11 | 6 to 11 | 11 | 12 to 13 |
| Fixed | 0 to 2 | 3 | 3 to 12 | 6 to 12 | 12 | 13 to 15 |
| Small | 0 | 1 | 1 to 6 | 3 to 6 | 6 | 7 |

The "baseline under row" column and the header's baseline byte say the same thing in two ways: Desk UI's baseline is under row 10, so 11 rows (0 to 10) lie above it, and the byte in `ui.dfn` is 11. Not every glyph is drawn. `fonts/src/face.ts` (`assemble`) makes three kinds: **drawn** glyphs from the grids, **composed** glyphs (the accented letters of Latin-1, which are a base letter plus a mark from the `ACCENTS` table, with a hand-drawn glyph winning over composition where one exists) and **generated** glyphs (box drawing, blocks and shades, which `fonts/src/boxes.ts` fills across the whole cell so that lines and shades join between neighbours). Desk Title is drawn as one-pixel skeletons and emboldened by the build (`embolden` in `fonts/src/grid.ts`), with a hand-drawn bold form (`boldFixes`) where emboldening would merge or thin a diagonal.

### One source, several files

`buildAll` in `fonts/build.ts` returns every generated file as a name and its bytes; writing them to `fonts/generated/` and checking that they are current are the same call with a different last step. It is the concatenation of three builders, and the first is the one that matters most here.

1. **`buildPixel`** loops over the four faces. For each it calls `assemble(face)` once, to get the placed glyphs, and then writes from that single result: an OpenType (CFF) file and a TrueType file in two styles, Square and Round, and the `.DFN`. The face called Desk Fixed also gets a DOS variant (`dosGlyphs`), in both styles and both formats, described below. The Round style is outline-only: the DOS bitmaps stay square and there is no Round `.DFN`.
2. **`buildModern`** writes the outline faces (Desk Sans, Mono and Serif) as OpenType and TrueType files.
3. **`buildEmoji`** writes the two colour fonts, the `.DEM` files and the contact sheets of the pixel emoji.

The result is 58 files: 44 font files at the top of `fonts/generated/` (16 for the four pixel faces in two styles and two formats, 4 for Desk Fixed DOS, and 24 for the outline faces), the four `.dfn` files, and ten files in `generated/emoji/` (four fonts, three `.dem` files and three contact-sheet images). With `OFL.txt`, the licence, that is the 59 files committed in that directory. They are committed so that the Desk's own build, and anyone who only wants the fonts, needs no Bun. `tools/check.sh` runs the check and the tests when Bun and `fonts/node_modules` are present, and says it skipped them otherwise. The `head` dates are pinned to 2026-01-01, so a rebuild is byte for byte the same.

### The `.DFN` file, from the writer's side

[M14](chapter:m-text) reads the header of `ui.dfn` as `44 46 4E 31 0E 0B 21 01`: the magic `DFN1`, height 14, baseline 11 and the glyph count `0x0121` = 289. The reader checks every glyph once on loading. The writer is `encodeDfn` in `fonts/src/dfn.ts`, and it is short enough to state in full. It computes the size first:

```text
size = 8 + sum over glyphs of ( 5 + height × ceil(advance / 8) )
```

It allocates that many zero bytes, writes the header (the height is the face's `height`, the baseline byte is its `ascent`), and then for each glyph in code-point order writes the code point (4 bytes), the advance (1 byte) and one row per line of the cell, setting bit `0x80 >> (x & 7)` of byte `x >> 3` for each inked column `x`. Two consequences are worth knowing. First, a glyph's stored width is its advance, not its ink: the gap after the glyph is stored as blank columns, which is why the reader can move the pen by the stored width. Second, the width is one byte, so an advance over 255 is a build error (`too wide for .DFN`). No face comes near it: the widest advance in any of the four files is 12.

Because the size is a formula, a file can be checked without reading its glyphs' pictures: walk the records using each width and the walk must end exactly at the end of the file. For all four files it does, and the chapter's tests do it.

<!-- explainer m-dfn -->

### TrueType in the Windows 3.1 era

[TrueType](term:truetype) is the outline font format that Apple developed and Microsoft added to Windows 3.1. The `.ttf` files here are written for the oldest rasterisers that read it, by `fonts/src/ttf.ts` and not by a general-purpose library, because a library writes whatever tables a modern system wants and the old systems are strict. `docs/FONTS.md` ("TrueType for Windows 3.1, 95, NT 3.5x and OS/2 Warp") sets the constraints:

| Constraint | What the file does |
|---|---|
| Tables | Exactly `head`, `hhea`, `maxp` (1.0), `OS/2` (version 1), `hmtx`, `loca` (long), `glyf`, `cmap`, `name` and `post` (2.0, with names); tags in order, each table 4-byte aligned with a correct checksum, `head.checkSumAdjustment` set |
| Glyphs | No composite glyphs, no kerning and no OpenType layout tables in the pixel faces |
| `cmap` | Format 4 for Windows Unicode BMP (3, 1), shared with Unicode (0, 3), and format 6 for Macintosh Roman (1, 0); every printable code page 1252 character, including the 0x80 to 0x9F set, has a glyph |
| `name` | Windows and Macintosh records for IDs 0 to 6, 13 and 14; family names at most 31 characters |
| Size | Units per em is the pixel size times 100, and `head` sets the integer-ppem flag, so the files are exact at whole multiples of the native size |
| Beyond the BMP | The folder and floppy disk need a format 12 `cmap`, which the era does not read, so they are only in the `.otf` files |
| Hinting | None: no `fpgm`, `prep` or `cvt` |

The last row follows from the grid. At the native size and its whole multiples every outline point lands on a pixel corner, so a rasteriser draws each glyph exactly without instructions; at sizes in between no hinting program can make a pixel font fit a size its grid does not divide, so the files say to use them at native sizes. Every em is a multiple of 4 px, so at 96 dpi each face is exact at a whole point size: Desk UI and Desk Title at 9 pt (12 px), Desk Fixed at 12 pt (16 px) and Desk Small at 6 pt (8 px).

The `.otf` files hold the same shapes with CFF outlines. A test compares the area of every glyph in the two formats. The pixel faces' `.otf` files also carry the pixel emoji as monochrome glyphs; the `.ttf` files carry none.

**Desk Fixed DOS.** Windows 3.1 programs type text in the ANSI code page, which has none of the box drawing, so the Unicode faces' glyphs for it are unreachable there. The DOS variant is Desk Fixed with its 256 glyphs addressed by code page 437 byte values (byte `0xC9` is ╔, `0xB0` is ░, `0x01` is ☺). The TrueType file is a symbol font: a format 4 `cmap` on platform 3, encoding 0 puts byte `b` at `0xF000 + b`, and Windows passes a program's bytes straight to that range. A Unicode-platform table (0, 3) sits beside it for modern systems, and deliberately no (3, 1) table, because a font with both is a text font to Windows and the symbol mapping would be ignored. The document says that the symbol behaviour on Windows 3.1 and 95 "is not verified".

### The outline faces

Desk Sans, Desk Mono and Desk Serif are an [outline face](term:outline-face) family: glyphs described by curves, not by pixels, so they scale to any size. Each has Regular, Bold, Italic and Bold Italic, for 12 files of each format. They are original drawings built in code from skeletons and parameters (`fonts/src/modern/`), with no outline of another font traced or copied, and there is no 8 px outline face because unhinted outlines cannot draw a legible 8 px letter. Their metrics share one line (ascender 950, descender −250, no line gap, so a line is 1.2 em), their figures are tabular by default, and Desk Mono has one advance, 600 units, for every glyph.

The pieces to know for the manual are these. The `.otf` files keep the cubic curves; the `.ttf` files convert each cubic to as few quadratics as stay within a quarter of a unit. The `.ttf` files follow the same Windows 3.1 rules as the pixel faces with four differences, among them a `gasp` table and layout tables: the old `kern` table, which Windows 3.1 reads, and `GSUB` and `GPOS`, which it ignores. The OpenType features (kerning of about 435 pairs a face, ligatures, small capitals, old-style figures, the code ligatures of Desk Mono) are in `docs/FONTS.md` ("Kerning, ligatures and features") and are not repeated here, because nothing in the Desk reads them: the outline faces are for the web, documents and a later TrueType reader, and `docs/FONTS.md` ("Not yet verified") says the Desk does not use them.

### The emoji

The pixel emoji are 480 original pictures at their real Unicode code points, each a single code point with the `Emoji` property in Unicode 17.0 (no joined sequences, skin tones, keycaps or flags, because each would need layout the era files cannot carry). Every picture is a grid of letters in `fonts/src/emoji/pixel/<category>.ts`, one letter per pixel, where `.` is transparent and every other letter is one of the sixteen slots of the Desk [palette](term:palette) (`k` is slot 0, the near-black outline, and `b` is slot 15, blue). The set has no yellow and no skin tones: amber is the yellow. A colour font, Desk Emoji Pixel, and a set of vector emoji (156 of the 480, flat geometric shapes for use beside the outline faces) come from the same manifest, so the forms cannot drift apart.

How much of the request is drawn, by form, is in the chart below (the log's account is [The pictures](chapter:pictures)).

<!-- figure emoji-coverage -->

The pictures come in three sizes, and the [emoji tier](term:emoji-tier) of a picture says which sizes it has:

| Tier | Emoji | 16 × 16 | 12 × 12 | 8 × 8 |
|---|---|---|---|---|
| Core | 40 | drawn | drawn | drawn |
| Essential | 40 | drawn | drawn | none |
| Extended | 400 | drawn | derived or hand-fixed | none |

The counts are those of the manifest (`fonts/src/emoji/manifest.ts`) and agree with the headers of the three `.dem` files: 480 emoji in the 16 and 12 files and 40 in the 8 file. The tier sets which of the Desk's faces a picture serves: the core 40 (the shell's pictures, Lt. Fizz's set and the commonest faces) reach Desk Small. The 12 × 12 grids of the extended tier are derived from the 16 × 16 by a rule (`derive12` in `fonts/src/emoji/derive.ts`): four rows are removed, then four columns, one at a time, nothing is resampled, so every remaining pixel is one the artist drew; the row removed is the one that differs from its more similar neighbour in the fewest pixels, so runs of repeated rows shrink first and one-pixel outlines survive. A hand-drawn grid (`FIX12`) replaces the derived one where it does not read. There is no derived 8 × 8, because halving the rows and columns of a drawing does not leave a picture.

Each picture also has a **monochrome form**, for text drawn in one colour (`fonts/src/emoji/mono.ts`). A coloured pixel is ink unless it is a light fill away from the silhouette's edge, so a face becomes its outline, eyes and mouth, and a red heart stays solid.

### The `.DEM` file

[DEM](term:dem) is the Desk's colour emoji file: one file per size, `emoji16.dem`, `emoji12.dem` and `emoji8.dem`, written by `encodeDem` in `fonts/src/emoji/dem.ts`. It is laid out like a `.DFN` with a directory in front, because there are many more records and they vary in length:

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | `DEM1` |
| 4 | 1 | size: the width and height of every emoji in pixels (16, 12 or 8) |
| 5 | 1 | reserved, 0 |
| 6 | 2 | emoji count `n` |
| 8 | 8 × `n` | directory in ascending code-point order: the code point (4 bytes) and the offset of its record (4 bytes) |
| … | | the records, in directory order |

A record is the layer count (1 byte), each layer as its palette slot (1 byte) and `size` rows of `ceil(size / 8)` bytes, and then the monochrome glyph as `size` rows of the same form. Each layer holds exactly the pixels of one colour, so layers never overlap and a pixel set in no layer is transparent. A reader binary-searches the directory by code point, and a code point outside the file means "no emoji at this size": fall back to a smaller or larger file, or to the text face's own glyph. Nothing in the Desk reads a `.DEM` yet; the only reader is the decoder written in the tests.

### The formats that are only designed

`docs/FORMATS.md` is the file-format reference, and it describes much more than the repository builds. Of its sections, `.DFN` and `.DEM` are the two with a writer and a reader. The others describe files that the Desk's apps will read and write once the apps exist; a search of the code for their markers finds nothing that reads or writes any of them. They are named here, and not taught, so that nobody mistakes the document for the code.

| Format | What the document says it is | State |
|---|---|---|
| `APP.INF` | One app: name, id, module, icon, SDK version, group | {{state.fonts.appinf}} |
| `.DAP` | An app archive: `DAP1`, entries with a CRC-32 each | {{state.fonts.dap}} |
| `CATALOG.INF` | A disk of apps, listed without opening every package | {{state.fonts.catalog}} |
| `.DIF` | A DOS program's information, like a `.PIF` | {{state.fonts.dif}} |
| `.ICO` | Icons at 32 × 32 and 16 × 16, header `DIC1` | {{state.fonts.ico}} |
| `DESK.INI` | Settings and layout | {{state.fonts.deskini}} |
| `DESK.RUN` | The handoff from the core to `DESK.COM` before a DOS program runs | {{state.fonts.deskrun}} |
| `.M3U`, `.BMP`, `.WAV` and `.VOC` | A playlist, Paint's pictures, sounds, and two apps' small records | {{state.fonts.apps}} |

## Why it is this way

- **Draw once, write many.** The decision is in `docs/FONTS.md`'s opening: every glyph is written once as a grid, and the build writes both the OpenType and TrueType files and the `.DFN` from the same assembled pixels, so they cannot drift apart, and nothing is drawn by hand in a font editor. The check is `bun test`, which decodes the `.DFN` files back to the same pixels, and a FreeType comparison in `fonts/validate.py` that renders every Square glyph against the `.DFN` bitmaps.
- **Original drawings.** The faces are "original drawings in the spirit of" the type the design boards used (Inter for the UI, Space Grotesk for titles); no other font's bitmaps or outlines were traced, so the files can be under the SIL Open Font License 1.1, with the code under Apache-2.0 OR MIT (`docs/FONTS.md`, "Licence").
- **A simple bitmap format on purpose.** `.DFN` is "as Fizz's `ui_data.rs` font" (`docs/FORMATS.md`): one bit per pixel, leftmost pixel in the highest bit, which is exactly the form the compositor's one-bit blit takes, so drawing a glyph needs no conversion ([M13](chapter:m-pixels), [M14](chapter:m-text)). Reading TrueType inside the Desk is spike `s6-ttf` in `docs/OPEN-QUESTIONS.md` and has not started.
- **No hinting, native sizes.** A pixel font cannot be hinted into sizes its grid does not divide, so the files carry no hinting program and `docs/FONTS.md` tells the reader to use them at native sizes. This was decided in the first fonts pull request (PR #1, [The faces](chapter:faces)).
- **No emoji in the era TrueType files.** 403 of the 480 emoji are beyond the Basic Multilingual Plane and need a format 12 `cmap`, which the old systems do not read, and adding them "would change files whose behaviour on those systems is described in `FONTS.md`". The era `.ttf` files are byte for byte what they were before the emoji arrived (`docs/EMOJI.md`, "Which file to install"; PR #2, [The pictures](chapter:pictures)).
- **Layers, not a palette image.** A `.DEM` record stores one 1-bit layer per colour and a monochrome glyph, in the Desk's sixteen colours, so the picture can be drawn with the same one-bit blit as text, once per colour, and the single-colour fallback is already in the file.
- **Name the unbuilt formats.** The plan's rule (Questions for the owner, 7) is that this chapter names the formats of `docs/FORMATS.md` that are not built and tags them planned, without teaching them.

## The code

| Where | What it holds |
|---|---|
| [`fonts/build.ts`](repo:fonts/build.ts) | [`buildAll`](sym:fonts/build.ts::buildAll), [`buildPixel`](sym:fonts/build.ts::buildPixel), [`buildModern`](sym:fonts/build.ts::buildModern) and the `--check` mode |
| [`fonts/src/face.ts`](repo:fonts/src/face.ts) | [`FaceDef`](sym:fonts/src/face.ts::FaceDef) (the cell, the baseline, the advance rules) and [`assemble`](sym:fonts/src/face.ts::assemble) (drawn, composed and generated glyphs) |
| [`fonts/src/faces/`](repo:fonts/src/faces/) | The four grids: `ui.ts`, `title.ts`, `fixed.ts`, `small.ts` |
| [`fonts/src/dfn.ts`](repo:fonts/src/dfn.ts) | [`encodeDfn`](sym:fonts/src/dfn.ts::encodeDfn) and [`decodeDfn`](sym:fonts/src/dfn.ts::decodeDfn) |
| [`fonts/src/ttf.ts`](repo:fonts/src/ttf.ts) | [`buildTtf`](sym:fonts/src/ttf.ts::buildTtf) and the table writer for the Windows 3.1 constraints |
| [`fonts/src/otf.ts`](repo:fonts/src/otf.ts) | [`buildOtf`](sym:fonts/src/otf.ts::buildOtf), the CFF files |
| [`fonts/src/dos.ts`](repo:fonts/src/dos.ts) | [`dosGlyphs`](sym:fonts/src/dos.ts::dosGlyphs) and the `0xF000` symbol base of Desk Fixed DOS |
| [`fonts/src/round.ts`](repo:fonts/src/round.ts) | [`roundOutline`](sym:fonts/src/round.ts::roundOutline): the Round style as a function of the same grids |
| [`fonts/src/modern/`](repo:fonts/src/modern/) | The outline faces: geometry, kit, letters, kerning, features |
| [`fonts/src/emoji/manifest.ts`](repo:fonts/src/emoji/manifest.ts) | The 480 emoji with name, category and [tier](sym:fonts/src/emoji/manifest.ts::Tier) |
| [`fonts/src/emoji/dem.ts`](repo:fonts/src/emoji/dem.ts) | [`encodeDem`](sym:fonts/src/emoji/dem.ts::encodeDem) and [`demLayers`](sym:fonts/src/emoji/dem.ts::demLayers) |
| [`fonts/src/emoji/derive.ts`](repo:fonts/src/emoji/derive.ts) | [`derive12`](sym:fonts/src/emoji/derive.ts::derive12), the 12 × 12 rule |
| [`fonts/generated/`](repo:fonts/generated/) | The committed output, with `OFL.txt`; `emoji/` holds the colour fonts and `.dem` files |
| [`fonts/validate.sh`](repo:fonts/validate.sh) | Optional third-party checks (fontTools, OTS, FreeType, HarfBuzz) in Docker |

For depth: `docs/FONTS.md` ([the grid](doc:docs/FONTS.md#the-grid), [TrueType for Windows 3.1](doc:docs/FONTS.md#truetype-for-windows-31-95-nt-35x-and-os2-warp), [adding or changing a glyph](doc:docs/FONTS.md#adding-or-changing-a-glyph), [not yet verified](doc:docs/FONTS.md#not-yet-verified)), `docs/EMOJI.md` ([sizes and tiers](doc:docs/EMOJI.md#sizes-and-tiers), [the files](doc:docs/EMOJI.md#the-files)) and `docs/FORMATS.md` ([`.DFN`](doc:docs/FORMATS.md#dfn-a-bitmap-font), [`.DEM`](doc:docs/FORMATS.md#dem-colour-emoji)).

## Worked example

**One emoji, byte by byte.** The floppy disk 💾 (U+1F4BE) is a core emoji, so it is in all three `.dem` files. The 8 × 8 file's header is the eight bytes `44 45 4d 31 08 00 28 00`: `DEM1`, size 8, reserved 0, and the count `0x0028` = 40. The floppy is the 24th directory entry (index 23), at bytes `8 + 8 × 23` = 192 to 199, which read `be f4 01 00 06 04 00 00`: the code point `0x0001F4BE` and the offset `0x00000406` = 1,030. The next entry's offset gives the record's length, 54 bytes. The formula from `docs/FORMATS.md` says what it should be. For a 16-pixel record it is `1 + layers × 33 + 32`, because a layer is a slot byte and 16 rows of 2 bytes (33 bytes) and the monochrome glyph is 32 bytes. For size 8 a row is 1 byte, so a layer is 1 + 8 = 9 bytes and the monochrome glyph is 8:

```text
1 + 5 × 9 + 8 = 54      (8 × 8 record: 5 layers)
```

The first byte of the record is 5, five layers, and 54 is exactly the length. The five layers, in file order, use palette slots 0, 15, 7, 6 and 4: slot 0 is the near-black of the outline and slot 15 is blue, and the first layer is always the outline (the documents say it is drawn first, then the other colours from the largest area to the smallest). Each layer is nine bytes, the slot and then eight rows. The first three, decoded from the bytes, are:

```text
slot 0      slot 15     slot 7
#######.    ........    ........
#......#    .#...##.    ........
#......#    .#...##.    ........
#......#    .######.    ........
#......#    .#....#.    ..####..
#......#    .#....#.    ..#..#..
#......#    .#....#.    ..####..
########    ........    ........
```

No pixel is set in two of the layers, and a pixel in none is transparent. The record ends with the monochrome glyph, eight more bytes whose first row is `#######.` and whose fourth is `########`: the outline stays ink, and so does most of the body, because only light fills away from the edge are left out. The same picture in the 16-pixel file is directory entry 240, at offset 43,844, 198 bytes long, with 5 layers: `1 + 5 × 33 + 32` = 198. The sizes agree for every record in all three files, and the chapter's test walks the directories to check that each record is the length the formula gives and that the last one ends at the end of the file.

This checks the writer against the document: the formula in `docs/FORMATS.md` is for the 16-pixel size, and it generalises to `1 + layers × (1 + size × ceil(size / 8)) + size × ceil(size / 8)`, which is what `encodeDem` produces. Across the three files a record has between 1 and 9 layers.

## State of this layer

```status
id: fonts.build
label: One build writes the .DFN, OpenType, TrueType and .DEM files, and --check finds a stale file
state: built-verified
evidence: sym:fonts/build.ts::buildAll, test:fonts/test/build.test.ts::decodeDfn, doc:docs/FONTS.md#how-glyphs-are-made
note: Run for this chapter on the host (bun build.ts --check reported 58 files up to date; bun test passed 436 tests). Both are host runs, not CI runs that this report can see.

id: fonts.dfn
label: The .DFN writer, with a decoder in the build and a reader in the Desk
state: built-verified
evidence: sym:fonts/src/dfn.ts::encodeDfn, test:fonts/test/build.test.ts::decodeDfn, test:tools/progress-report/tests/test_chapter_fonts_emulators.py::test_dfn_files_have_the_size_the_formula_gives, sym:shell/font/src/lib.rs::parse
note: The reader and its tests are the subject of M14.

id: fonts.ttf
label: TrueType files built to the Windows 3.1 rules (tables, cmaps, names, no hinting)
state: built-unverified
evidence: sym:fonts/src/ttf.ts::buildTtf, doc:docs/FONTS.md#truetype-for-windows-31-95-nt-35x-and-os2-warp
note: Checked by structure tests, fontTools, OTS and FreeType (a different rasteriser from Windows' own) according to docs/FONTS.md, which this report did not rerun. Never installed on Windows 3.1, 95, NT 3.5x or OS/2 Warp.

id: fonts.dos
label: Desk Fixed DOS, the symbol-encoded code page 437 font
state: built-unverified
evidence: sym:fonts/src/dos.ts::dosGlyphs, doc:docs/FONTS.md#desk-fixed-dos
note: Read by FreeType and Chromium according to the document, which this report did not rerun. The symbol behaviour on Windows 3.1 and 95 is not verified.

id: fonts.outline
label: The outline faces Desk Sans, Mono and Serif in four styles
state: built-unverified
evidence: sym:fonts/build.ts::buildModern, doc:docs/FONTS.md#modern-outline-faces
note: Built and checked on the host by tests and validators. Nothing in the Desk reads them, and Windows, macOS and Windows 3.1 have not rendered them.

id: fonts.emoji
label: 480 pixel emoji in three tiers, a colour font and monochrome glyphs
state: built-verified
evidence: sym:fonts/src/emoji/manifest.ts::Tier, test:fonts/test/emoji-files.test.ts::decodes to every drawing of, test:tools/progress-report/tests/test_chapter_fonts_emulators.py::test_dem_headers_match_the_tiers
note: The counts are those of the committed files. Seen only in headless Chromium and by FreeType; not in the Desk, DOSBox or on a VGA screen.

id: fonts.dem
label: The .DEM writer and its test decoder
state: built-verified
evidence: sym:fonts/src/emoji/dem.ts::encodeDem, test:fonts/test/emoji-files.test.ts::decodes to every drawing of, test:tools/progress-report/tests/test_chapter_fonts_emulators.py::test_dem_records_have_the_length_the_formula_gives
note: Nothing in the Desk reads a .DEM file yet; the only readers are the decoders in the tests.

id: fonts.appinf
label: APP.INF, the app description
state: planned
evidence: doc:docs/FORMATS.md#appinf-one-app

id: fonts.dap
label: .DAP, the app archive
state: planned
evidence: doc:docs/FORMATS.md#dap-an-app-archive

id: fonts.catalog
label: CATALOG.INF, the disk catalogue
state: planned
evidence: doc:docs/FORMATS.md#cataloginf-a-disk-of-apps

id: fonts.dif
label: .DIF, a DOS program's information
state: planned
evidence: doc:docs/FORMATS.md#dif-a-dos-programs-information

id: fonts.ico
label: .ICO, icons with a DIC1 header
state: planned
evidence: doc:docs/FORMATS.md#ico-an-icon

id: fonts.deskini
label: DESK.INI, settings and layout
state: planned
evidence: doc:docs/FORMATS.md#deskini-settings-and-layout

id: fonts.deskrun
label: DESK.RUN, the handoff to DESK.COM
state: planned
evidence: doc:docs/FORMATS.md#deskrun-the-handoff

id: fonts.apps
label: The playlist, picture, sound and app-record formats
state: planned
evidence: doc:docs/FORMATS.md#m3u-a-playlist
note: .M3U, .BMP, .WAV and .VOC, the Mines best times and the Clock reminders. They belong to apps that do not exist.

id: fonts.hardware
label: The faces and emoji on a real monitor, and the TrueType files on a real Windows 3.1
state: planned
evidence: doc:docs/FONTS.md#not-yet-verified
```

The writers are {{state.fonts.build}}: they ran on the host, and the committed files are those the sources produce. The TrueType files are {{state.fonts.ttf}} for the era they are written for, because the reference rasteriser used (FreeType) is not the one on those systems, and the `.DEM` format is {{state.fonts.dem}} against its own test decoder only. Every file format of the second half of `docs/FORMATS.md` is {{state.fonts.appinf}}, not built. Nothing here has been seen on a real display.

> **From the stacks — Where TrueType came from.** According to Wikipedia, TrueType was developed and released with Mac System 7 in May 1991, and Apple licensed it to Microsoft for free, which added it to the Windows 3.1 operating environment. The page says that its original strength was giving font developers a high degree of control over how their fonts are displayed, and that parts of its hinting virtual machine were patented by Apple, with the patents now expired. That hinting is the part the Desk's pixel-font files leave out.
> Source: [TrueType, Wikipedia](https://en.wikipedia.org/wiki/TrueType)

## Try it yourself

These commands read the committed files and check that they are current. The first needs Bun and the two pinned packages from `bun install` (nothing is installed on the host: they go into `fonts/node_modules`, which is not committed). The Python one-liners need nothing but Python. Output is from a run on 2026-10-23 at `44f0265`.

```try
needs: host: bun, python3 (the first command needs fonts/node_modules from bun install --frozen-lockfile)
ran: 2026-10-23 on the host, Bun 1.3.10
$ cd fonts && bun build.ts --check
fonts: 58 generated files are up to date
$ python3 -I -c "import struct; d=open('fonts/generated/ui.dfn','rb').read(); print(d[:4], d[4], d[5], struct.unpack('<H', d[6:8])[0], len(d))"
b'DFN1' 14 11 289 5639
$ python3 -I -c "import struct; d=open('fonts/generated/emoji/emoji8.dem','rb').read(); print(d[:4], d[4], struct.unpack('<H', d[6:8])[0], struct.unpack('<II', d[8+8*23:16+8*23]), len(d))"
b'DEM1' 8 40 (128190, 1030) 1858
$ python3 -I -c "import struct; d=open('fonts/generated/emoji/emoji16.dem','rb').read(); print(hex(struct.unpack('<II', d[8+8*240:16+8*240])[0]), d[43844], 1+d[43844]*33+32)"
0x1f4be 5 198
```

The first command is the build's own check. The second prints the header of Desk UI that [M14](chapter:m-text) reads: magic, height 14, baseline 11, 289 glyphs, 5,639 bytes. The third prints the `.DEM` header of the 8 × 8 file (40 emoji) and the floppy's directory entry: code point 128,190 (`0x1F4BE`) at offset 1,030. The last finds the same emoji in the 16-pixel file and applies the formula: 5 layers, 198 bytes. I also ran `bun test` in `fonts/` for this chapter: 436 tests passed in about 36 seconds. I did not run `fonts/validate.sh`, which needs a Docker image built from `python:3.12-slim` with pinned Python packages.

**Exercises.** (1) Without running anything, work out how large a `.DFN` would be for a face 8 rows high with 100 glyphs, each 6 pixels wide, and then say what its header bytes would be after `DFN1` (baseline 7). (2) Add a glyph in a scratch branch by following `docs/FONTS.md` ("Adding or changing a glyph"): draw it in `fonts/src/faces/small.ts`, run `bun build.ts`, and say which files changed in `fonts/generated/` and why `small.dfn` is bigger by exactly 5 + 8 = 13 bytes (then check). (3) Find the emoji whose record in `emoji16.dem` has nine layers and say what the layer count tells you about its picture.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Bitmap face
def: A font whose every character is a hand-drawn grid of pixels, with no outline behind it. The Desk has four, and its text on DOS and in the kernel is drawn from them.

term: Outline face
def: A font whose characters are curves that scale to any size. Desk Sans, Mono and Serif are the Desk's; nothing in the Desk reads them yet.

term: TrueType
def: The outline font format that Apple developed and Microsoft added to Windows 3.1. The Desk writes .ttf files to the rules of that era, with no hinting and no emoji.

term: Emoji tier
def: Which sizes a pixel emoji is drawn at: the core 40 at 16, 12 and 8 pixels, the essential 40 at 16 and 12, and the extended 400 at 16 with a derived or hand-fixed 12.

term: DEM
def: The Desk's colour emoji file, one per size. A directory of code points and offsets, then for each emoji its colour layers as one-bit bitmaps and a monochrome glyph.
```

```figures
id: m18-fontmatrix
reuse: font-matrix
at: figure font-matrix

id: m18-coverage
reuse: emoji-coverage
at: figure emoji-coverage

id: m18-dfn
reuse: m14-dfn
at: explainer m-dfn
```

## Revisions

- 2026-10-23: first written, against `44f0265`. Every number from the `.dfn` and `.dem` files was read from the files, and the chapter's tests check them. `bun build.ts --check` and `bun test` (436 tests) were run on the host; `fonts/validate.sh` was not. The `m-dfn` explainer (a glyph read out of `ui.dfn` field by field) is drawn here by reuse from M14, beside the log's `font-matrix` and `emoji-coverage` figures. Found while writing: the header comment of `fonts/build.ts` and the doc comment of `buildModern` say the outline faces are "Regular and Bold", where `docs/FONTS.md` and the 24 generated files have four styles each.
