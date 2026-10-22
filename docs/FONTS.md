# Fonts

Desk draws all of its text with four bitmap faces of its own (the decision is in [`OPEN-QUESTIONS.md`](OPEN-QUESTIONS.md) and [`SPEC.md`](SPEC.md#fonts)). Every glyph is written once as a grid of pixels in `fonts/src/faces/`. A Bun build turns the grids into OpenType (CFF) and TrueType files in two styles, Square and Round, for the browser build, design work, other tools and Windows 3.1-era systems, and into `.DFN` files ([`FORMATS.md`](FORMATS.md#dfn-a-bitmap-font)) for the Desk on DOS. Both come from the same assembled pixels, so they cannot drift apart. Nothing is drawn by hand in a font editor.

The faces are original drawings in the spirit of the type the design boards used (Inter for the UI, Space Grotesk for titles); no other font's bitmaps were traced or transcribed.

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
4. Run `bun build.ts` and `bun test` in `fonts/`, look at the specimen, and commit the source with the regenerated files in `fonts/generated/`.

The specimen page (`bun build.ts --specimen <file>`) renders each face with its built OTF file at 1×, 2× and 3×, charts every glyph from the build's own grids, and has a pixel editor that produces the source line for a changed glyph.

## Building

```bash
cd fonts
bun install                            # opentype.js 2.0.0, pinned exactly
bun build.ts                           # write generated/*.otf, *.ttf and *.dfn
bun build.ts --check                   # rebuild in memory; fail if generated/ is stale
bun build.ts --specimen specimen.html  # also write the specimen page
bun test                               # coverage, cell and advance rules, traced area = ink, round trips
```

The generated files are committed so the Desk build and anyone using the fonts need no Bun. `tools/check.sh` runs the check and the tests when Bun and `fonts/node_modules` are present and says it skipped them otherwise.

The tests also check the TrueType structure (the required tables, tag order, alignment, checksums, `maxp`, `loca`, `glyf` and `hmtx` agreeing, no composites, the cmap formats with every 1252 character mapped, and the name records) and that each `.ttf` glyph has the same advance and area as the `.otf` one. For Round, they check that contours alternate correctly and keep their winding, never leave the inked pixels (sampled along every curve and supersampled inside the cell), lose no pixel centre, and keep between 80 and 100 % of the square ink.

The tests check that each face covers its character sets, that every glyph fills exactly its cell, that fixed faces keep one advance and proportional faces add the gap, that box-drawing arms reach the cell edges, that capitals, digits and lowercase sit on the right rows, that every glyph's traced outline encloses exactly its ink (both from the tracer and read back through the opentype.js parser), that the `.DFN` files decode to the same pixels, that builds are reproducible, and that `fonts/generated/` is current.

## Validation

`fonts/validate.sh` runs third-party checkers. It is optional and not part of `tools/check.sh`, because its first run builds a small Docker image, which takes about a minute. Nothing is installed on the host. It runs fc-scan and `otfinfo -i` on the host if they are present, then:

```bash
docker build -q -t desk-fonts-validate:local - <<'DOCKERFILE'
FROM python:3.12-slim
RUN pip install --no-cache-dir fonttools==4.55.3 opentype-sanitizer==9.2.0 freetype-py==2.5.1
DOCKERFILE
docker run --rm --network none -v "$PWD/fonts/generated:/fonts:ro" -v "$PWD/fonts/validate.py:/validate.py:ro" \
  desk-fonts-validate:local python3 /validate.py /fonts
```

`validate.py` has fontTools decompile and recompile every table of all 20 font files and check the TrueType requirements above (for the DOS variant: the (0, 3), (3, 0) and (1, 0) cmaps and no (3, 1), all 256 bytes reached through each, and the symbol character set declared), runs OTS (`ots-sanitize`, the sanitiser Chrome and Firefox use) on each, and has FreeType render every glyph of every Unicode file at the native size and twice it, in monochrome without and with hinting and in grey, comparing the pixels with the `.DFN` bitmaps. For Desk Fixed DOS it checks that FreeType's default charmap reaches ☺ ♥ ░ ╔ ═ α, then selects the symbol charmap and renders the box drawing, blocks and eight of the pictures at 16 px through `0xF000 + byte`, comparing them with the Unicode glyphs in `fixed.dfn`. Results on 2026-10-22: fontTools read all 20 files; OTS passed all 20 with no warnings; every Square glyph rendered exactly in every mode at 1× and 2× (Desk UI at 12 and 24 ppem), every Round glyph exactly in monochrome, with partly covered pixels at the corners in grey as intended, and the DOS variant matched in all four files. fc-scan lists the ten families with the right styles and spacing, and otfinfo shows the names and "Installable" permission.

## Not yet verified

- No DOS code reads the `.DFN` files yet; only the build's decoder and the FreeType comparison in `validate.py` have read them.
- The TrueType files have not been installed on real Windows 3.1, 95, NT 3.5x or OS/2 Warp. Trying them in an emulator needs licensed installation media for those systems, which this repository does not have, so the era support rests on the structure checks and on FreeType, which is a different rasteriser from Windows'.
- The faces have been looked at only in headless Chromium, at 1×, 2× and 3×. They have not been seen in the Desk, in DOSBox, or on a real VGA monitor, where the 640 × 480 pixel aspect and the CRT will change how the thin strokes read.

## Licence

The font files (`fonts/generated/*.otf`, `*.ttf` and `*.dfn`) are under the SIL Open Font License 1.1, copyright 2026 Liminal HQ, Scott Morris, with no Reserved Font Name; the licence is `fonts/generated/OFL.txt`. The code that builds them is Apache-2.0 OR MIT, like the rest of the repository.
