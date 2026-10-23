# Fonts

Desk draws all of its text with four bitmap faces of its own (the decision is in [`OPEN-QUESTIONS.md`](OPEN-QUESTIONS.md) and [`SPEC.md`](SPEC.md#fonts)). Every glyph is written once as a grid of pixels in `fonts/src/faces/`. A Bun build turns the grids into OpenType (CFF) and TrueType files in two styles, Square and Round, for the browser build, design work, other tools and Windows 3.1-era systems, and into `.DFN` files ([`FORMATS.md`](FORMATS.md#dfn-a-bitmap-font)) for the Desk on DOS. Both come from the same assembled pixels, so they cannot drift apart. Nothing is drawn by hand in a font editor.

The faces are original drawings in the spirit of the type the design boards used (Inter for the UI, Space Grotesk for titles); no other font's bitmaps were traced or transcribed.

Beside the pixel faces there is a modern family of outline faces, Desk Sans, Desk Mono and Desk Serif, each Regular, Bold, Italic and Bold Italic, for larger sizes and modern systems (see [Modern outline faces](#modern-outline-faces)). The pixel faces are the Desk's own text at its native sizes on DOS; the outline faces are for Notes and Paint text, documents, the web, Windows and Linux, and a later TrueType reader in the Desk.

The OpenType files of the pixel faces also carry the Desk pixel emoji as monochrome glyphs, and those of Desk Sans, Mono and Serif the vector emoji; the TrueType files carry none. See [`EMOJI.md`](EMOJI.md).

## Faces

| Face | For | Cell | Em | Cap / x-height | Advance | Files |
|---|---|---|---|---|---|---|
| Desk UI | Menus, lists, buttons, body text | proportional × 14 rows | 12 px (11 px type) | 8 / 6 | drawn width + 1; space 3; box drawing 6 | `desk-ui.otf`, `desk-ui.ttf`, `ui.dfn` |
| Desk Title (bold) | Window titles, headings | proportional × 14 rows | 12 px | 9 / 6 | drawn width + 1; space 4; box drawing 8 | `desk-title.otf`, `desk-title.ttf`, `title.dfn` |
| Desk Fixed | Terminal, code, paths | 8 × 16 | 16 px | 10 / 7 | 8 | `desk-fixed.otf`, `desk-fixed.ttf`, `fixed.dfn` |
| Desk Small | Status bars, badges | 6 × 8 | 8 px | 6 / 4 | 6 | `desk-small.otf`, `desk-small.ttf`, `small.dfn` |

Each face also comes in a Round style (`desk-ui-round.otf` and `.ttf` and so on; families "Desk UI Round", "Desk Title Round", "Desk Fixed Round" and "Desk Small Round"), described below.

Each face covers every printable character of Windows code page 1252 (printable ASCII, all of Latin-1 from U+00A0 to U+00FF, and the 0x80 to 0x9F set € ‚ ƒ „ … † ‡ ˆ ‰ Š ‹ Œ Ž ‘ ’ “ ” • – — ˜ ™ š › œ ž Ÿ, which includes Œ œ Ÿ for French), the single, double and mixed box drawing of code page 437 with the rounded corners ╭ ╮ ╯ ╰, the blocks and shades ░ ▒ ▓ █ ▀ ▄ ▌ ▐, and the typography and shell symbols — – ‘ ’ ‚ “ ” „ … • ‹ › € ← ↑ → ↓ ↔ ↕ ► ◄ ▲ ▼ ● ○ ■ □ ✓ ✗, plus a folder (U+1F4C1) and a floppy disk (U+1F4BE). Desk Fixed also covers every printable character of code page 437 (the control-character pictures, Greek and maths), because Terminal shows what DOS programs print; it is the only face that claims code page 437 in its OpenType code page range. `fonts/src/charset.ts` lists the sets and the code page 437 table.

Points worth knowing when using them:

- Desk Fixed has a dotted zero, an `l` with a tail, and an `I` and `1` with serifs, so `0 O Ø` and `1 l I |` stay apart.
- Desk UI has a double-storey `a`, a single-storey `g`, ascenders one row taller than the capitals, an `I` with serifs and tabular digits (all five pixels wide), so columns of sizes line up.
- Desk Title is drawn as one-pixel skeletons and emboldened by the build (`embolden` in `fonts/src/grid.ts`); where that would merge or thin a diagonal, `boldFixes` in `title.ts` holds a hand-drawn bold form. Its fractions ¼ ½ ¾ stay one pixel wide.
- Desk Small fits accents into one row above six-row capitals by dropping a repeated row from an accented capital (`squashCaps`), so É, À and Ç are five rows tall under their mark. Its descenders are one row deep.

## The grid

- A glyph is rows of `#` (ink) and `.` (blank) joined with `/`. In the face files, `R('..##..', '.#..#.')` joins the rows for you.
- Rows start at the cap-height row unless the glyph says otherwise: `x(...)` starts at the x-height, `asc(...)` at the ascender (UI and Title) and `at(row, ...)` at any row of the cell, counting from 0 at the top.
- In a proportional face the advance is the drawn width plus the gap, so leading or trailing `.` columns are kept as side bearings. In a fixed face the rows are placed from column 0 of the cell exactly as written, and a glyph may not be wider than the cell.
- The cell rows, top to bottom:

| Face | Accents | Ascender | Capitals | x-height | Baseline under row | Descenders |
|---|---|---|---|---|---|---|
| UI | 0–2 | 2 | 3–10 | 5–10 | 10 | 11–13 |
| Title | 0–2 | 2 | 3–11 | 6–11 | 11 | 12–13 |
| Fixed | 0–2 | 3 | 3–12 | 6–12 | 12 | 13–15 |
| Small | 0 | 1 | 1–6 | 3–6 | 6 | 7 |

## Metrics in the OpenType files

- One pixel is 100 font units and the em is the face's pixel size, so units per em is 1200, 1200, 1600 and 800 for UI, Title, Fixed and Small. The faces are exact at whole multiples of those sizes.
- Desk UI is 11 px type: its capitals are 8 rows and its line is the 14-row cell. Its em is 12 rows rather than 11, so the files are exact at 12 ppem, which is a whole point size; nothing about the glyphs or the line changes, only the size you ask for (12 px instead of 11). The ascent and descent stay 1100 and 300 units, so the extra em row is neither line gap nor descender: the em is simply taller than the type, as it is in most fonts. The `.DFN` keeps its 14-row cell, with no em of its own.
- Every em is a multiple of 4 px, so at 96 dpi, where a point is ¾ px, each face is exact at a whole point size:

| Face | Native size | 1× | 2× | 3× |
|---|---|---|---|---|
| Desk UI | 12 px | 9 pt | 18 pt | 27 pt |
| Desk Title | 12 px | 9 pt | 18 pt | 27 pt |
| Desk Fixed (and DOS) | 16 px | 12 pt | 24 pt | 36 pt |
| Desk Small | 8 px | 6 pt | 12 pt | 18 pt |

- The ascender is the rows above the baseline and the descender the rows below it, so ascender minus descender is the whole cell. `hhea`, `sTypo*` and `usWin*` all carry these values, the line gap is 0, and `USE_TYPO_METRICS` is set: lines set solid are exactly one cell apart.
- Set `line-height` to the cell height times the scale (14, 14, 16 and 8 px at 1×) wherever box drawing must join.
- The `.otf` files have CFF outlines (opentype.js writes OpenType-CFF); the `.ttf` files have TrueType `glyf` outlines written by `fonts/src/ttf.ts`. Every Square edge is a straight line and every Round curve is a quadratic stored exactly in both, so the two files describe identical shapes; a test compares every glyph's area. The tracer (`fonts/src/outline.ts`) merges neighbouring pixels into one polygon, winds outer contours counter-clockwise and holes clockwise, keeps pixels that touch only at a corner apart, and drops points on straight runs.
- Desk Fixed and Desk Small set `post.isFixedPitch`. The `name` table carries the copyright, the SIL Open Font License 1.1 text and its URL. A regular face's full name is the family alone ("Desk Fixed"); the bold title face's is "Desk Title Bold". The `head` dates are pinned to 2026-01-01 and `lowestRecPPEM` is the face size, so builds are byte-for-byte reproducible.

## TrueType for Windows 3.1, 95, NT 3.5x and OS/2 Warp

The `.ttf` files are written for the oldest TrueType rasterisers by `fonts/src/ttf.ts`, not by a general-purpose library:

- Exactly the tables those systems read: `head`, `hhea`, `maxp` (version 1.0), `OS/2` (version 1), `hmtx`, `loca` (long offsets), `glyf`, `cmap`, `name` and `post` (format 2.0, with glyph names). Tags are in order, every table is 4-byte aligned with a correct checksum, and `head.checkSumAdjustment` is set. No composite glyphs, no kerning and no OpenType layout tables.
- `cmap` has a format 4 subtable for Windows Unicode BMP (3, 1), shared with Unicode (0, 3), and a format 6 subtable for Macintosh Roman (1, 0) for programs of that era that read it. Windows 3.1 reaches glyphs through the ANSI code page (1252), which it maps to Unicode, so every printable 1252 character, the 0x80 to 0x9F set included, has a glyph; a test checks each one through the cmap.
- `name` has Windows (platform 3, encoding 1, language 0x409) and Macintosh (platform 1) records for IDs 0 to 6 (copyright, family, subfamily, unique ID, full name, version, PostScript name), 13 (licence) and 14 (licence URL). Family names are at most 31 characters.
- `OS/2`: `fsType` 0 (installable), weight 400 or 700, PANOSE, Unicode ranges for the blocks used, code pages Latin 1 and Macintosh Roman (Desk Fixed also claims code page 437), and `sTypo*` and `usWin*` equal to the cell.
- `head`: units per em is the pixel size times 100, flag bit 3 (integer ppem) is set, and `lowestRecPPEM` is the pixel size.
- The two pictures outside the Basic Multilingual Plane (folder and floppy disk) need a format 12 cmap, which systems of the era do not read, so they are only in the `.otf` files.
- **No hinting.** At the native pixel size and its whole multiples every outline point lands on a pixel corner, so FreeType renders every glyph exactly, in monochrome and in grey, with and without hinting (see Validation). At sizes in between the glyphs blur or step unevenly; no hinting program can make a pixel font scale to a size its grid does not divide, so the files carry no `fpgm`, `prep` or `cvt`. Use them at native sizes.

**What Windows 3.1 cannot reach.** The Unicode faces carry the code page 437 box drawing, blocks, shades, arrows and symbols at their Unicode code points, but Windows 3.1 programs type and display text in the ANSI code page (1252), which has none of them, so those glyphs are not reachable from ANSI programs there. Desk Fixed DOS is for that.

## Desk Fixed DOS

Desk Fixed DOS (and Desk Fixed DOS Round) is Desk Fixed with its 256 glyphs addressed by code page 437 byte values, as the DOS screen font is: byte 0xC9 is ╔, 0xB0 is ░, 0x01 is ☺. Bytes 0x01 to 0x1F and 0x7F draw the code page 437 pictures, byte 0x00 is blank, and 0xFF is a blank no-break space. Use it to show DOS text, `.ANS` art or a program's screen in a Windows 3.1-era program, or anywhere the OEM layout is wanted: choose the font and type, paste or load the bytes as they are.

- **Encoding.** The TrueType file is a symbol font: a format 4 cmap on platform 3 encoding 0 puts byte `b` at `0xF000 + b`, and a format 6 Macintosh (1, 0) cmap maps the bytes directly. Windows (3.1 onwards) treats a font with a (3, 0) cmap as SYMBOL_CHARSET and passes a program's bytes straight to `0xF000 + b`, using `usFirstCharIndex` (0xF000) to find the range, so there is no ANSI translation in the way. `OS/2` declares the symbol character set (code page bit 31) and code page 437 (bit 63) for later systems that read it.
- **A Unicode table beside the symbol one.** The TrueType file also has a Unicode-platform subtable (platform 0, encoding 3, format 4) mapping the real code points of the code page 437 characters (☺ U+263A, ╔ U+2554, ░ U+2591, α U+03B1 and so on) to the same glyphs, for modern systems. It has no Windows Unicode (3, 1) subtable: a font with both (3, 1) and (3, 0) is a text font to Windows, which would then ignore the symbol mapping. Windows 3.1 and 95-era TrueType code reads only platform 3 (and 1), so the symbol behaviour there should be unchanged; that is not verified.
- **The `.otf` file** maps both the real code points and the private-use copies at U+F000 to U+F0FF (the codes the symbol cmap uses) in its Unicode cmap, because opentype.js writes only Unicode cmaps. Windows 3.1 does not read OpenType-CFF files at all.
- **Limits.** A symbol font has no case mapping, spell checking or font substitution, and text typed into a Unicode-aware program on a later Windows lands at U+F000 to U+F0FF rather than at the real characters. It is not a replacement for a terminal's own code page handling. Round comes free from the same outlines, so it is built too. There is no `.DFN`: the Desk's own fixed font is already keyed by Unicode.

What each consumer does with the files, as of 2026-10-22:

| Consumer | `desk-fixed-dos.ttf` | `desk-fixed-dos.otf` | Checked |
|---|---|---|---|
| FreeType 2 (freetype-py 2.5.1, in Docker) | Default charmap is the Unicode one: ☺ ♥ ░ ╔ ═ α reach their glyphs. Selecting `FT_ENCODING_MS_SYMBOL` reaches every byte at `0xF000 + b`, and the box drawing, blocks and pictures render pixel-exact through it. | Unicode charmap: real code points and U+F000 + byte both reach the glyphs. | Verified |
| fontTools 4.55.3 and OTS 9.2.0 (in Docker) | Read and sanitise cleanly; the three subtables agree for all 256 bytes. | Clean; real and private-use code points agree. | Verified |
| fontconfig (`fc-scan`, host) | Reports the real code points only (it reads the Unicode table and ignores the symbol one). | Reports the real code points and U+F000 to U+F0FF. | Verified |
| Chromium (headless, Linux, `@font-face`) | Uses the symbol table: private-use text works, and text in U+0020 to U+00FF is taken as bytes, so `A` is A but `É` (0xC9) shows ╔. Real code points above U+00FF (☺ ╔ α) fall back to other fonts. | Real code points and private-use text both render with the font. | Verified, by screenshot |
| Windows 3.1, 95, NT GDI; Windows DirectWrite; macOS | Not tested. | Not tested. | Not verified |

Use the `.otf` (or Desk Fixed itself) for Unicode text on modern systems, and the `.ttf` where a program sends bytes to a symbol font.

**Sizes.** Windows sizes text in points; use the point sizes in the table under Metrics (at 96 dpi, VGA "small fonts"). At 120 dpi ("large fonts") a point is 5⁄3 px, so a face is exact only at five times its native size (UI and Title 36 pt, Fixed 48 pt, Small 24 pt); smaller sizes blur.

### Installing

- **Windows 3.1 and 3.11**: Control Panel → Fonts → Add…, choose the folder that holds the `.ttf` files, select the faces, and keep "Copy fonts to Windows directory" checked. TrueType must be enabled (Control Panel → Fonts → TrueType…).
- **Windows 95 and NT**: open Control Panel → Fonts and use File → Install New Font…, or copy the `.ttf` files into the Fonts folder.
- **OS/2 Warp**: System Setup → Font Palette → Edit font → Add, and point it at the folder.
- **Windows 10 and 11**: right-click a `.ttf` or `.otf` file and choose Install (or Install for all users).
- **Linux**: copy the files to `~/.local/share/fonts/` and run `fc-cache -f`; `fc-list | grep Desk` should list ten families.
- **macOS**: open a file and choose Install Font.

Use the `.ttf` files on old systems and either format on new ones, but do not install both formats of the same family on one system.

## Round

The Round style is a pure function of the same grids (`roundOutline` in `fonts/src/round.ts`). Every convex corner of the traced outline becomes a quadratic curve whose control point is the corner and whose ends lie `ROUND_RADIUS` (0.45 px) along each edge; concave corners stay sharp. The curve stays inside the triangle of the corner and its two ends, so rounding only removes ink and never reaches outside the pixels: advances, metrics and the cell are unchanged, and Round is a drop-in for Square. A one-pixel end becomes a round cap with a sliver of flat edge, and a lone pixel (a period, the dot of an i) a dot covering about 87 % of the pixel. The radius is a multiple of 3 font units so the CFF files, which store quadratics as cubics, stay exact.

At the native size Round renders exactly like Square in monochrome (every pixel centre is still inside the outline) and with lightly softened corners in grey; at 2× and 3× the rounding is plain to see. Desk Small reads the same in Round as in Square at 1×, 2× and 3×. Round is outline-only: there is no Round `.DFN`, and the Desk's own bitmaps stay square.

## How glyphs are made

- **Drawn**: the grids in `fonts/src/faces/*.ts`.
- **Composed**: the accented letters of Latin-1 and Ÿ are a base letter plus a mark (`ACCENTS` in `fonts/src/face.ts`; each face draws its marks once). Marks sit one row above the x-height or the capitals, the cedilla hangs from the baseline, ì í î ï use the dotless ı, and Š š Ž ž take a caron. A glyph drawn by hand under the same character wins over composition (Å in every face, ï and Ï in Desk Fixed). The spacing marks ´ ¨ ¸ ¯ ˆ ˜ come from the same marks, and the soft hyphen copies the hyphen.
- **Generated**: box drawing, blocks and shades fill the whole cell (`fonts/src/boxes.ts`), so lines and shades join between neighbours. A single line meets a double line at the near line where the double line runs through and at the far line where it turns. The shades repeat every two columns and four rows; in the 14-row faces they show a seam every line.

## Adding or changing a glyph

1. Draw it in the face's file in `fonts/src/faces/`, keyed by the character. For Desk Title, draw the one-pixel skeleton in `glyphs`, or, when emboldening goes wrong, the final bold form in `boldFixes`.
2. For an accented letter, add a row to `ACCENTS` in `fonts/src/face.ts` instead of drawing it, unless the composed form looks wrong.
3. For a character every face must have, add it to `fonts/src/charset.ts`.
4. Run `bun build.ts` and `bun test` in `fonts/`, look at the specimen site, and commit the source with the regenerated files in `fonts/generated/`.

### The specimen site

`bun build.ts --site <dir>` (in `fonts/`) writes the specimen site, Desk Type Specimens, to a folder: five pages with a shared header. **Overview** (`index.html`) says what each family is for, which file to use on which system, what has and has not been checked, and how the fonts are built. **Pixel faces** (`pixel.html`) renders each face with its built OTF file at 1×, 2× and 3×, charts every glyph from the build's own grids, shows Desk Fixed DOS, and has a pixel editor that produces the source line for a changed glyph. **Outline faces** (`outline.html`) has Desk Sans, Mono and Serif with their italics, the features and a type tester with italic, feature and emoji switches. **Emoji** (`emoji.html`) is the emoji reference described in [`EMOJI.md`](EMOJI.md#the-specimen-site). **Downloads** (`downloads.html`) links every built file by family and format with its size.

The fonts are served as real files under `fonts/` beside the pages (the same paths as in `fonts/generated/`) and loaded by `@font-face` with relative URLs, so the pages carry no base64. Artifacts serve only web file types, so the `.dfn` and `.dem` files are not published on their own: `downloads.json` lists every entry of `desk-fonts.zip` with either its published path or its bytes in base64, and the pages build the zip in the browser, byte for byte the zip the build's `zip()` writes, and offer it through the viewer's `downloads` capability. The Emoji page carries the `.dem` files in its data for its own emoji zip. Shared styles and scripts are separate files: `site.css` (the base reset and the header), `specimen.css`, `fonts.css`, `site.js` (saving files through the viewer's `downloads` capability where it is available), `specimen.js`, `emoji.js` and `emoji-data.js`. The sources are in `fonts/src/site/` and `fonts/src/specimen.ts`.

The folder also gets `manifest.json`, which is the mapping for publishing it as a multi-file artifact: `file_path` is `index.html` (the main page, written without a document skeleton because the viewer adds one), `files` maps every other published path to its file in the folder with its media type (`font/otf`, `font/ttf`, `image/png` and so on), and `capabilities` is `{"downloads": {}}`. To publish, run `bun build.ts --site <dir>` and give the Artifact tool `file_path: <dir>/index.html`, `root: <dir>` and the manifest's `files`; a later publish to the same artifact replaces the files it names. The generated folder is not committed.

## Modern outline faces

| Family | Styles | For | Files |
|---|---|---|---|
| Desk Sans | Regular, Bold, Italic, Bold Italic | UI and body text in the spirit of Inter; Bold leans a little towards Space Grotesk for titles | `desk-sans.otf`, `desk-sans-bold.otf`, `desk-sans-italic.otf`, `desk-sans-bold-italic.otf`, and a `.ttf` of each |
| Desk Mono | Regular, Bold, Italic, Bold Italic | Code and terminals in the spirit of JetBrains Mono: one advance, a dotted zero, a tailed l, a slab i, a serifed I and 1, code ligatures | `desk-mono.otf`, `desk-mono-bold.otf`, `desk-mono-italic.otf`, `desk-mono-bold-italic.otf`, and a `.ttf` of each |
| Desk Serif | Regular, Bold, Italic, Bold Italic | Long reading (Notes, documents) in the spirit of Charter and Source Serif: sturdy, moderate contrast, bracketed slab serifs, open apertures, a true italic | `desk-serif.otf`, `desk-serif-bold.otf`, `desk-serif-italic.otf`, `desk-serif-bold-italic.otf`, and a `.ttf` of each |

They are original drawings: every glyph is built by the code in `fonts/src/modern/` from skeletons and parameters, and no outline of Inter, Space Grotesk, JetBrains Mono, Charter, Source Serif, Literata or any other font was traced, converted or copied. There is no 8 px outline face: unhinted outlines cannot draw a legible 8 px letter, and Desk Small stays the face for that size.

**Italics.** Each family has an Italic and a Bold Italic with the upright's weights, spaced upright and then slanted about the middle of the x-height so the ink stays centred on its advance: Desk Sans at 9.5°, a little narrower and rounder; Desk Mono at 9° with a cursive f that drops into the descender, every advance still 600; Desk Serif at 12°, 10 % narrower, with a true italic lowercase (`fonts/src/modern/italics.ts`): single-storey a and g, a cursive f through the descender, exit strokes in place of foot serifs, pen entries in place of head serifs and ball terminals. Arrows and geometric shapes stay upright. The italic and bold bits of `OS/2.fsSelection` and `head.macStyle`, `post.italicAngle` and the `hhea` caret slope are set, and the four styles share one family name, so a program's Italic and Bold buttons find them.

### How they are built

- **One drawing per character, three families.** `fonts/src/modern/letters.ts` and `symbols.ts` draw each character once against a kit (`kit.ts`) of parts: stems and bars, bowls as superellipses (squarer in Sans and Mono, rounder in Serif) whose sides take the stem weight and whose top and bottom take the thinner horizontal weight, arcs of bowls with their ends cut along the stroke, diagonals with level ends, round and broad-nib pen strokes for the odd shapes (& { } ~ ¤), dots, slab serifs with bracketed fillets, and bowl terminals with a short vertical spur in Serif. The style (`STYLES` in `kit.ts`) sets the weights, widths, superellipse exponent, side bearings and serif sizes; the same skeleton gives Regular and Bold by weight, Mono by width, and Serif by contrast and serifs. A few characters change their skeleton by family: Mono's i, j, l, I, 1, m, M and W, and Serif's head and foot serifs.
- **Clean outlines.** The parts of a glyph are flattened into polygons, unioned and cut with clipper-lib (a polygon clipper, a build-time development dependency under the Boost Software Licence, not part of the fonts), and fitted back to cubic curves (`geom.ts`): straight parts stay lines, corners stay corners, curves are fitted within half a unit and split at their extrema, points are rounded to whole units, and specks under 150 square units are dropped. A contour whose fit would cross itself is refitted with tighter corners and tolerance. Tests check that every contour is closed and wound the right way, that no contour crosses itself or another, that curves have points at their extrema, and that nothing reaches past the line.
- **Composed accents.** The accented letters, and Š š Ž ž, are a base and a mark (`marks.ts`), placed over the base's ink (and squeezed where a mark over a capital would rise past the ascender); there are no composite glyphs.
- **Two formats, one shape.** The `.otf` files keep the cubics (CFF). The `.ttf` files convert each cubic to as few quadratics as stay within a quarter of a unit, then round, wound clockwise for TrueType. A test compares every glyph's area in the two files, and `validate.sh` compares their FreeType renderings at 48 px.
- **Box drawing in Mono** comes from the same generator as the pixel faces (`boxes.ts`), on a coarse grid whose cells are about a stem wide, so lines join across the 1200-unit line and weigh what the letters do.

### Metrics

| | Sans | Mono | Serif |
|---|---|---|---|
| Units per em | 1000 | 1000 | 1000 |
| Cap height | 700 | 700 | 690 |
| x-height | 528 | 536 | 498 |
| Ascender (b d h k l) / descender | 742 / −208 | 742 / −208 | 735 / −222 |
| Stem / horizontal, Regular | 86 / 76 | 82 / 74 | 98 / 50 |
| Stem / horizontal, Bold | 158 / 132 | 140 / 120 | 156 / 68 |
| Advance | proportional; figures tabular | 600 for every glyph | proportional; figures tabular |

Every face uses the same line: ascender 950, descender −250 and no line gap, so a line is 1.2 em, in `hhea`, `sTypo*` and `usWin*`. Round letters overshoot the baseline, x-height and cap height by 11 units. Figures are tabular by default in every face (one advance, ink centred) so columns of numbers line up. Proportional and (in Desk Serif) old-style figures are optional features; see [Kerning, ligatures and features](#kerning-ligatures-and-features).

**Spacing.** Side bearings come from the kind of each side in the skeleton: a straight side (n, H) gets the face's full bearing, a round side (o, C) about three quarters of it, a diagonal (v, A) a quarter, and an open side (the arm of c, r, T) a half. A test checks that the straight and round sides of each class agree, and that round sides sit closer than straight ones. Mono centres each glyph in its 600-unit cell.

**The TrueType files** follow the same Windows 3.1 rules as the pixel faces (the tables, cmaps (3, 1) and (1, 0) with every code page 1252 character, Windows and Macintosh name records, `OS/2` version 1, `post` 2.0) with four differences: `head` does not set integer ppem, `lowestRecPPEM` is 8, a `gasp` table asks for smoothing at every size (Windows 3.1 ignores it), and they carry layout tables: the old `kern` table, which Windows 3.1 reads, and `GSUB` and `GPOS`, which it ignores. Desk Mono also claims code page 437 and `post.isFixedPitch`.

### Kerning, ligatures and features

Every feature is optional: the cmap maps each character to the same glyph as before, the ligatures and alternates have no code points, and a program that reads no layout tables sees the plain glyphs with their own advances. Browsers turn on `kern`, `liga` and `calt` by default; the rest are asked for with `font-feature-settings` or the `font-variant-*` properties. The tables are written by `fonts/src/modern/layout.ts` from glyph names; the glyphs and the lists are in `features.ts`, `kerning.ts` and `code.ts`.

| Feature | Faces | What it does | Glyph names |
|---|---|---|---|
| `kern` | Sans, Serif (all styles) | Pair kerning, in GPOS for modern shapers and in a format 0 `kern` table in the `.ttf` files for Windows 3.1 and other old systems | — |
| `liga` | Sans, Serif (all styles) | ff, fi, fl, ffi, ffl: the hood of the last f reaches over a dotless i and stands for its dot, or arches into the l; the first f of ff is shortened to clear the second and the crossbars run together; in Desk Serif the crossbar meets the i's head serif, and in the italic it runs on into the i | `f_f`, `f_i`, `f_l`, `f_f_i`, `f_f_l` |
| `pnum`, `tnum` | Sans, Serif | Proportional figures; `tnum` returns to the tabular default | `one.pnum` … |
| `onum`, `lnum` | Serif | Old-style figures (0 1 2 at the x-height, 6 8 rising, 3 4 5 7 9 hanging), tabular, or proportional with `pnum`; `lnum` returns to lining | `one.tosf`, `one.osf` … |
| `frac` | Sans, Serif | 1/2 1/4 3/4 to ½ ¼ ¾, and 1/3 2/3 1/8 3/8 5/8 7/8 to fractions with no code point of their own | `onethird` … `seveneighths` |
| `smcp`, `c2sc` | Serif | Small capitals for A to Z and the accented capitals of Latin-1, a little taller than the x-height with the text's weights | `a.sc`, `uni00E9.sc` … |
| `dlig` | Serif | The historical ct and st, joined by a hairline arch | `c_t`, `s_t` |
| `ss01` | Sans | Single-storey a (and à á â ã ä å), a tailed l, a slashed zero | `a.ss01`, `l.ss01`, `zero.ss01` |
| `calt` | Mono | Code ligatures: `->` `=>` `<-` `<=` `>=` `!=` `==` `===` `!==` `::` `..` `...` `//` `/*` `*/` `&&` `\|\|` `\|>` `<\|` `>>` `<<` `<>` `++` `--` `~=` `=~` `<=>` | `hyphen_greater.liga` …, `lig.spacer` |

**Kerning.** About 435 pairs a face, from a small table of classes (T, V, W, Y, A, L, P, F, round capitals, r, f, v, w, y, round lowercase, quotes and the period and comma on the left; round and straight lowercase, s, v w y, A, round capitals, T, V, W, Y, the period, comma and ellipsis and the closing quotes on the right). Accented letters are on the right side only. Each class pair's value is measured from the outlines of its key glyphs: the open space between them, counted to a depth of 400 units over the height they share, against the space between two straight stems, of which 45 % is taken out, at most 160 units. Each member pair then takes that value unless it would bring the two glyphs closer than about half a straight side bearing anywhere, with 20 units of vertical reach, in which case it takes less (so T before ö keeps clear of the dieresis), or a positive value where a pair already collides (f before a quote in the Serif Italic). Values are rounded to 5 units and pairs under 10 are dropped. Typical values in Desk Sans Regular: To −105, AV −80, Ty −110, Yo −90, r. −120, LT −120, P. −125, f before o −100. In Desk Serif Bold Italic, T and y meet at the drooping end of the T's bar and stay almost unkerned. The `kern` table and GPOS hold the same pairs; Desk Mono has neither.

**Code ligatures.** Each ligature keeps the cells of the characters it stands for. As in other monospaced code faces, a chaining contextual substitution turns every character but the last into an empty 600-unit spacer and the last into the drawing, which reaches back over the spacers, so every glyph in Desk Mono still has the 600-unit advance and the caret steps through the characters one at a time. A sequence next to another operator character (`<!--`, `-->`, `====`, `/**`, `<<=`) is left as plain characters. To switch them off, use `font-variant-ligatures: no-contextual` or `font-feature-settings: "calt" 0` in CSS, or the editor's ligature setting.

**Old systems.** Windows 3.1, 95 and NT 3.5x, OS/2 and older Macintosh programs read no `GSUB` or `GPOS`; they get the plain glyphs, and kerning only where the program reads the `kern` table. The `.ttf` files keep the structure the Windows 3.1 rules ask for: the structure tests and OTS pass with the new tables, and fontTools reads them. That has not been tried on those systems.

### Sizes and rendering

There is no hinting. In FreeType's grey rendering (as on Linux and in browsers) Desk Sans and Desk Mono read cleanly from about 14 px up and are good from 16 px; at 11 and 12 px they are legible but soft, with stems that fall between pixels and uneven weight from letter to letter. Desk Serif needs a little more: its thin horizontals and serifs blur at 11 and 12 px and it is comfortable for reading from about 16 px, which is the size it is meant for. In monochrome (no smoothing, as on Windows 3.1 without font smoothing) all three are lumpy below about 20 px: stems alternate between one and two pixels and thin strokes drop out, so on such systems use the pixel faces at their native sizes, or the outline faces large. `fonts/validate.sh` with `PROOF_DIR` set writes FreeType proof sheets at 11, 12, 14, 16, 24 and 48 px, grey and monochrome.

### Weakest glyphs

From the proof review, the glyphs most in need of another pass are: & and @ (drawn with a round pen rather than built from bowls, so their weight is even where the letters have contrast), ß and ð (assembled from parts, with joins that read heavier than the letters), ‰ ¶ § (busy at text sizes), the fractions and superscripts (scaled figures, so their strokes are thinner than the text), Serif's c, s and r terminals (the spur is subtle and reads as a plain cut below 24 px), Serif Bold's a and e (small counters), and Mono's m, M and W (crowded in the 600-unit cell, as in every monospace). Among the feature glyphs: the denominators of the fractions (8 and 3 clot at text sizes), the shortened descender hook of the second f in Desk Serif Italic's ff, ffi and ffl (it reads as a stub), the code ligatures `~=` and `=~` (a thin wave beside a bar), and Desk Serif's arch between a stem and its shoulder in n, m, h and r, which leaves a small step inside the counter that shows above about 100 px.


```bash
cd fonts
bun install                            # opentype.js 2.0.0 and clipper-lib 6.4.2, pinned exactly
bun build.ts                           # write generated/*.otf, *.ttf and *.dfn
bun build.ts --check                   # rebuild in memory; fail if generated/ is stale
bun build.ts --site site               # also write the specimen site to site/ (not committed)
bun test                               # coverage, cell and advance rules, traced area = ink, clean outlines, round trips
```

The generated files are committed so the Desk build and anyone using the fonts need no Bun. `tools/check.sh` runs the check and the tests when Bun and `fonts/node_modules` are present and says it skipped them otherwise.

The tests also check the TrueType structure (the required tables, tag order, alignment, checksums, `maxp`, `loca`, `glyf` and `hmtx` agreeing, no composites, the cmap formats with every 1252 character mapped, and the name records) and that each `.ttf` glyph has the same advance and area as the `.otf` one. For Round, they check that contours alternate correctly and keep their winding, never leave the inked pixels (sampled along every curve and supersampled inside the cell), lose no pixel centre, and keep between 80 and 100 % of the square ink.

For the layout tables, `test/layout.test.ts` reads the built files back with a small shaper of its own (`test/shaper.ts`, single, ligature and chaining substitutions, pair kerning and the `kern` table), so the checks run without Docker: the `kern` table's header and order, the same pairs in GPOS and `kern`, values between −160 and +120, the pairs that must tighten and the ones that must not move, every ligature and feature substitution in both formats, the code ligatures with every advance at 600 and plain when switched off or next to another operator, and that no unencoded glyph is reached through the cmap. `test/site.test.ts` checks that the site declares each outline family's four styles apart, serves every built file unchanged or carries it in `downloads.json`, publishes only types an artifact serves, links only to files it publishes, and that its manifest names every published path with its media type.

For the outline faces, the tests check coverage, clean contours (closed, wound correctly, no crossings, no slivers, points at extrema), that every glyph stays within the line, tabular figures and the Mono advance, the spacing classes, that each glyph's advance and area agree between the `.otf` and `.ttf` files, and that the build is reproducible.

The tests check that each face covers its character sets, that every glyph fills exactly its cell, that fixed faces keep one advance and proportional faces add the gap, that box-drawing arms reach the cell edges, that capitals, digits and lowercase sit on the right rows, that every glyph's traced outline encloses exactly its ink (both from the tracer and read back through the opentype.js parser), that the `.DFN` files decode to the same pixels, that builds are reproducible, and that `fonts/generated/` is current.

## Validation

`fonts/validate.sh` runs third-party checkers. It is optional and not part of `tools/check.sh`, because its first run builds a small Docker image, which takes about a minute. Nothing is installed on the host. It runs fc-scan and `otfinfo -i` on the host if they are present, then:

```bash
# The image is tagged desk-fonts-validate:<hash of its Dockerfile>, so checkouts with other tool lists keep their own.
docker build -q -t desk-fonts-validate:<hash> - <<'DOCKERFILE'
FROM python:3.12-slim
RUN pip install --no-cache-dir fonttools==4.55.3 opentype-sanitizer==9.2.0 freetype-py==2.5.1 uharfbuzz==0.53.3
DOCKERFILE
docker run --rm --network none -v "$PWD/fonts/generated:/fonts:ro" -v "$PWD/fonts/validate.py:/validate.py:ro" \
  desk-fonts-validate:<hash> python3 /validate.py /fonts
# PROOF_DIR=<dir> fonts/validate.sh also mounts <dir> at /proof and writes the outline faces' proof sheets there
```

`validate.py` has fontTools decompile and recompile every table of all 20 font files and check the TrueType requirements above (for the DOS variant: the (0, 3), (3, 0) and (1, 0) cmaps and no (3, 1), all 256 bytes reached through each, and the symbol character set declared), runs OTS (`ots-sanitize`, the sanitiser Chrome and Firefox use) on each, and has FreeType render every glyph of every Unicode file at the native size and twice it, in monochrome without and with hinting and in grey, comparing the pixels with the `.DFN` bitmaps. For Desk Fixed DOS it checks that FreeType's default charmap reaches ☺ ♥ ░ ╔ ═ α, then selects the symbol charmap and renders the box drawing, blocks and eight of the pictures at 16 px through `0xF000 + byte`, comparing them with the Unicode glyphs in `fixed.dfn`. Results on 2026-10-22: fontTools read all 20 files; OTS passed all 20 with no warnings; every Square glyph rendered exactly in every mode at 1× and 2× (Desk UI at 12 and 24 ppem), every Round glyph exactly in monochrome, with partly covered pixels at the corners in grey as intended, and the DOS variant matched in all four files. fc-scan lists the ten families with the right styles and spacing, and otfinfo shows the names and "Installable" permission.

For the outline faces, fontTools and OTS check all 24 files as above, and FreeType renders every code page 1252 character of each face from the `.otf` and the `.ttf` at 48 px and compares the grey images; they differ by at most about 3 % of a glyph's ink, at the grey edges of the smallest marks (° ¨ ©), which the two rasteriser paths in FreeType anti-alias slightly differently.

HarfBuzz (uharfbuzz 0.53.3) then shapes the outline faces' layout in both formats: in Desk Sans and Desk Serif, that To, Ta, AV, VA, Ty, Yo, r., LT, P., T. and f before o tighten by between 1 and 200 units and HH does not move, that fi, fl, ff, ffi and ffl become their ligatures under `liga` and stay apart without it, and that `frac`, `pnum`, `onum`, `smcp`, `c2sc`, `dlig` and `ss01` give the expected glyphs; in the `.ttf` files it also removes GPOS with fontTools and checks that the `kern` table alone gives the same To. In Desk Mono it checks every code ligature (spacers, then the drawing, every advance 600), that each stays plain with `calt` off, and that `<!--`, `-->`, `====`, `///` and `/**` stay plain. Results on 2026-10-22: all 44 font files read by fontTools and passed by OTS with no warnings (the outline `.ttf` files with their `kern`, `GPOS` and `GSUB` tables), every layout check passed in all 24 outline files, and the outline OTF and TTF renderings differ by at most 3.3 % of a glyph's ink.

## Not yet verified

- No DOS code reads the `.DFN` files yet. `desk-font` (`shell/font/`) parses the four on the host and its tests check their metrics and a text-art rendering; the DOS wiring that loads them is still to come (`docs/FIRST-WINDOW-PLAN.md`, T9). The build's decoder and the FreeType comparison in `validate.py` read them too.
- The TrueType files have not been installed on real Windows 3.1, 95, NT 3.5x or OS/2 Warp. Trying them in an emulator needs licensed installation media for those systems, which this repository does not have, so the era support rests on the structure checks and on FreeType, which is a different rasteriser from Windows'.
- The outline faces have been looked at in headless Chromium and in FreeType proof sheets on Linux only; Windows (GDI, DirectWrite and ClearType), macOS and Windows 3.1 have not rendered them.
- The kerning and features have been checked with HarfBuzz and in headless Chromium only. Whether Windows 3.1 and 95-era programs apply the `kern` table, and how DirectWrite, Core Text, Word, LibreOffice and code editors treat the features and the code ligatures' caret, has not been tried.
- The faces have been looked at only in headless Chromium, at 1×, 2× and 3×. They have not been seen in the Desk, in DOSBox, or on a real VGA monitor, where the 640 × 480 pixel aspect and the CRT will change how the thin strokes read.

## Licence

The font files (`fonts/generated/*.otf`, `*.ttf` and `*.dfn`), the outline faces included, are under the SIL Open Font License 1.1, copyright 2026 Liminal HQ, Scott Morris, with no Reserved Font Name; the licence is `fonts/generated/OFL.txt`. The code that builds them is Apache-2.0 OR MIT, like the rest of the repository.
