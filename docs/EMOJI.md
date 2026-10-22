# Emoji

Desk has its own emoji: 396 original pictures at their real Unicode code points, drawn as pixel art in the 16-colour Desk set ([`PALETTE.md`](PALETTE.md)). They come as monochrome glyphs in the OpenType files of the pixel faces, as a colour font, Desk Emoji Pixel, for modern systems, and as `.DEM` files ([`FORMATS.md`](FORMATS.md#dem-colour-emoji)) for the Desk on DOS. Everything is built from the same letter grids in `fonts/src/emoji/pixel/` by the font build ([`FONTS.md`](FONTS.md)), so the forms cannot drift apart.

## The set

Every emoji is a single code point with the `Emoji` property in Unicode 17.0, named by its CLDR short name (`fonts/src/emoji/manifest.ts`). There are no ZWJ sequences, skin-tone modifiers, keycaps, flags or tag sequences: each would need either OpenType layout (which the era files cannot carry and the Desk's text drawing does not do) or many more drawings. Lt. Fizz's 24 pictures are all in it (✓ and ✗ as their emoji forms ✔ U+2714 and ❌ U+274C), and so are the shell's own pictures: disks, folders, pages, clocks, locks, sound, devices, warnings, arrows, media buttons, coloured circles and weather.

| Category | Emoji | Examples |
|---|---|---|
| Computer and media | 38 | 💾 💿 🖥 ⌨ 🖱 🖨 🎮 🔊 🔇 🔔 🔒 🔑 ⏰ |
| Office and paper | 32 | 📁 📂 📄 📝 📅 📋 📚 ✉ 📌 ✂ 🗑 |
| Faces | 54 | 🙂 😀 😉 😮 😢 😠 😎 😍 🤔 😴 💀 👽 🤖 |
| Hands and body | 22 | 👍 👎 👌 ✌ 👋 👏 🙏 💪 👀 |
| Hearts and feelings | 20 | ❤ 🧡 💛 💚 💙 💜 💔 💯 💬 💤 |
| Animals | 32 | 🐶 🐱 🦊 🐻 🐼 🐸 🐧 🐝 🦋 🐟 |
| Food and drink | 34 | 🍎 🍌 🍕 🍔 🍩 🎂 ☕ 🥤 🍷 |
| Nature and weather | 33 | ☀ 🌙 ⭐ ✨ ☁ 🌈 ⚡ ❄ 🔥 💧 🍁 🌲 |
| Activities and objects | 38 | 🏆 🎁 🎈 ⚽ 🎲 🎸 💡 🔧 ⚙ 💎 |
| Travel and places | 32 | 🚗 🚌 🚲 ✈ 🚀 🛸 ⛵ 🏠 🏁 |
| Symbols and signs | 61 | ✔ ❌ ⚠ ⛔ ❓ ❗ ⬆ ➡ ▶ ⏸ ⏹ 🔴 🟢 🔵 |

Of the 396, 322 are outside the Basic Multilingual Plane, which matters for the TrueType files (see [Which file to install](#which-file-to-install)).

## Sizes and tiers

| Tier | Emoji | 16 × 16 | 12 × 12 | 8 × 8 |
|---|---|---|---|---|
| Core | 40 | drawn | drawn | drawn |
| Essential | 40 | drawn | drawn | — |
| Extended | 316 | drawn | derived (280) or a hand fix of the derived grid (36) | — |

Every 16 × 16 is drawn by hand. The 80 core and essential emoji (the shell's pictures, Fizz's set and the commonest faces) have hand-drawn 12 × 12 grids, and the 40 core ones hand-drawn 8 × 8 grids for Desk Small. The other 12 × 12 grids are derived from the 16 × 16 by the rule below, and a hand fix replaces the derived grid where it does not read. There is no derived 8 × 8: removing half the rows and columns of a drawing does not leave a picture, so the 8 × 8 set is the hand-drawn core only.

**The 12 × 12 derivation** (`fonts/src/emoji/derive.ts`). Four rows are removed, then four columns, one at a time; nothing is resampled, so every remaining pixel is one the artist drew, in its own colour. First the blank margin goes, from whichever side has more of it so the drawing stays centred. When the margin runs out, the row (or column) removed is the one that differs from its more similar neighbour in the fewest pixels, so runs of repeated rows (the middle of a disc, the side of a box) shrink first and one-pixel outlines and features survive; ties go to the row nearest the middle, then the upper one. Text (🆕 🆒 🆓 🆘), thin diagonals and dense faces are what the rule breaks; those have hand fixes in the category file's `FIX12`.

## Drawing the pixel emoji

Every pixel emoji is a grid of letters in `fonts/src/emoji/pixel/<category>.ts`, one letter per pixel, so a change to a drawing is a readable diff. The drawings are original: nothing is traced, copied or converted from Noto, Twemoji, OpenMoji, Apple, Microsoft, JoyPixels or any other emoji set.

### The letters

`.` is transparent. Every other letter is a slot of the 16-colour Desk set ([`PALETTE.md`](PALETTE.md)); `fonts/src/emoji/palette.ts` defines the map once.

| Letter | Slot | Colour | Used for |
|---|---|---|---|
| `k` | 0 | `#050507` | the outline, eyes, mouths, text on light things |
| `x` | 1 | `#0c0c10` | near-black surfaces |
| `z` | 2 | `#111116` | screens, dark glass |
| `q` | 3 | `#1b1e28` | dark plastic, night sky |
| `l` | 4 | `#333847` | dark grey: shade of mid grey, rubber, seams in metal |
| `m` | 5 | `#7c8796` | mid grey: metal, stone; shade of light grey |
| `s` | 6 | `#b4bfce` | light grey: steel, shade of white |
| `w` | 7 | `#e0e0e0` | white, paper, highlights |
| `o` | 8 | `#ffaa40` | amber: faces, hands, gold, and the yellow of the set |
| `n` | 9 | `#7d5422` | brown: shade of amber, wood |
| `u` | 10 | `#372612` | dark brown: shade of brown, chocolate, coffee |
| `r` | 11 | `#f43f5e` | rose red: hearts, alerts, apples |
| `p` | 12 | `#a78bfa` | purple |
| `c` | 13 | `#22d3ee` | cyan: water, ice, glass |
| `g` | 14 | `#2ec66a` | green: plants, go |
| `b` | 15 | `#60a5fa` | blue: sky, water, cool things |

The set has no yellow, no orange distinct from amber and no skin tones. Amber is the yellow, the skin of faces and hands, and gold; orange things are amber shaded with brown. Where Unicode has both an orange and a yellow emoji (🟠 🟡, 🧡 💛), the yellow one is amber with a white highlight and no brown, and the orange one is amber with a brown lower half; they stay close, which is a limit of the 16 colours.

### Rules

- **Size and placement at 16 × 16.** Leave a clear one-pixel margin: the body sits in rows and columns 1 to 14, so two emoji side by side in Desk Fixed do not touch. A very wide or tall object may use row or column 0 or 15 where it must. Centre symmetric shapes on the grid's middle line (between columns 7 and 8); a shape that needs a one-pixel apex or stem (a warning triangle, an arrow) is an odd width centred on column 7.
- **12 × 12 and 8 × 8.** The body may use the whole grid: the faces add the gap after the glyph. Odd-width shapes centre on column 5 (12) or 3 (8).
- **Outline.** A one-pixel `k` outline around every silhouette at 16 and 12, with single-pixel steps on curves (no doubled stair steps) and `k` lines between parts that must read apart (a lid and a cup, a shutter and a disk). At 8 × 8 the outline is kept on round and light shapes (faces, the floppy disk) and left off where it would fill the shape (stars, the warning sign, arrows); such shapes are solid colour with a darker detail.
- **Light from the top left.** On rounded or glossy bodies, a highlight of one to three pixels near the upper left inside the outline (`w`, or `s` on white things). Shade along the lower right inside the outline with the colour's partner: amber `o` with brown `n`, white `w` with light grey `s`, light grey `s` with mid grey `m`, mid grey `m` with dark grey `l`, brown `n` with dark brown `u`. Rose, purple, cyan, green and blue have no darker step: they take a highlight and no shade.
- **Detail.** Lines and features one pixel wide, in `k` on light colours and `w` on dark ones. Exaggerate the one feature that makes the thing what it is (the floppy's shutter, the bell's clapper, the key's teeth). Four colours plus the outline is plenty; more turns to noise at 16 pixels.
- **No dithering.** Flat colours only, as everywhere in the Desk.
- **Faces** are a 14-pixel amber disc with a brown crescent on the lower right and a white highlight on the upper left (see 🙂 in `faces.ts`); eyes are 1 × 2 `k` columns at columns 5 and 10, rows 5 and 6, unless the expression changes them.

### The monochrome form

Each drawing also gives a monochrome glyph for the Desk faces, the colour font's fallback and the `.DEM` file (`fonts/src/emoji/mono.ts`). A coloured pixel is ink unless it is a *fill* away from the silhouette's edge. Fills are the light slots (white, light grey, amber, cyan: luminance 0.45 or more) and a shade pixel (brown, light grey, mid grey) next to the light slot it shades. So a face becomes its outline, eyes and mouth; a page becomes its outline and lines of text; a red heart, a green leaf or a blue disc stays solid. Where the rule fails for one emoji, the category file's `MONO_RULES` moves slots between ink and fill, and `MONO_HAND` replaces the glyph with a hand-drawn `#` and `.` grid.

### Proofing

`bun src/emoji/proof.ts <out.png> [16|12|8] [category ...]` (in `fonts/`) writes a contact sheet of what is drawn so far: each emoji on the Desk's dark content colour, on a light page, and in mono, enlarged to 48 pixels, over its code point. A `d` after the code point marks a derived 12 × 12, `f` a hand-fixed one and `m` a hand-drawn mono glyph. The build writes the same sheets for the whole set, one per size, to `fonts/generated/emoji/pixel-16.png`, `pixel-12.png` and `pixel-8.png`.

## The files

| File | What | For |
|---|---|---|
| `fonts/generated/desk-fixed.otf`, `desk-ui.otf`, `desk-title.otf`, `desk-small.otf` and their Round forms | The faces with every emoji as a monochrome glyph at its code point: Desk Fixed the 16 × 16, two cells wide; Desk UI and Desk Title the 12 × 12; Desk Small the 40 core 8 × 8 | Text in the pixel faces on modern systems and in browsers |
| `fonts/generated/emoji/desk-emoji-pixel.ttf` and `.otf` | Desk Emoji Pixel: the 16 × 16 drawings in colour (COLR version 0 and CPAL), with the monochrome glyph as the base glyph | Colour emoji beside any font on modern systems |
| `fonts/generated/emoji/emoji16.dem`, `emoji12.dem`, `emoji8.dem` | Every drawing of that size, colour layers and mono ([`FORMATS.md`](FORMATS.md#dem-colour-emoji)) | The Desk on DOS |
| `fonts/generated/emoji/pixel-16.png`, `pixel-12.png`, `pixel-8.png` | Contact sheets: each emoji on dark, on light and in mono | Review |

In each face, the face's own drawing wins where it already has one: the folder and floppy disk in every face, and the code page 437 pictures (☺ ☻ ♥ ♦ ♣ ♠ ♪ ♫ ☼ and the arrows) in Desk Fixed, keep the glyphs drawn to sit in text.

### Which file to install

- **Windows 3.1, 95, NT 3.5x and OS/2 Warp:** the `.ttf` faces, which carry no emoji. 322 emoji are beyond the Basic Multilingual Plane and need a format 12 cmap, which those systems do not read, and adding the rest would change files whose behaviour on those systems is described in [`FONTS.md`](FONTS.md); the era TrueType files are byte-for-byte what they were before the emoji. Those systems have no colour fonts either.
- **Windows 10 and 11, Linux, macOS:** install the `.otf` faces for emoji in Desk text (monochrome, in the face's own size and weight), and Desk Emoji Pixel for colour. For colour, put Desk Emoji Pixel before the text face in a font list (`font-family: "Desk Emoji Pixel", "Desk UI"`), or let the system's fallback find it; the text faces' monochrome emoji win wherever the text face comes first.
- **Browsers:** `@font-face` with `desk-emoji-pixel.ttf` (or `.otf`) and list it first for colour emoji; the pixel faces' `.otf` files give monochrome emoji.
- **DOS:** the `.DEM` files; the Desk reads the size that matches the text (16 for Desk Fixed, 12 for Desk UI and Title, 8 for Desk Small).

Desk Emoji Pixel is exact at 16 px and whole multiples (12, 24, 36 pt at 96 dpi), like the pixel faces. Its em is the 16-pixel grid (units per em 1600), the grid's top row starts 14 pixels above the baseline and its bottom row ends 2 below, and the line is 15 pixels up and 4 down (1.2 em) with no line gap. `CPAL` holds one palette, the 16 Desk colours slot for slot, so a layer's palette index is its slot. Each emoji's layers are traced from its grid, back to front: black first, then the other colours from the largest area to the smallest, each layer covering its own pixels and every pixel of the colours in front of it, so no two layer edges have to meet exactly and anti-aliasing shows no seams. The zero-width joiner and the variation selectors U+FE0E and U+FE0F map to an empty glyph, so text that carries them shows no missing-glyph box. The `name` table carries the SIL Open Font License 1.1, as the faces do.

## What has been checked

| Check | Result |
|---|---|
| `bun test` (`fonts/test/emoji-pixel.test.ts`, `emoji-files.test.ts`) | The manifest (unique code points, the `Emoji` property, no sequences, Fizz's and the shell's pictures, the tiers), the palette against `PALETTE.md`, grid sizes, margins and tiers, the mono rule, the derivation, the cmaps (format 4 and 12 on the right platforms, every emoji reached, the faces' own glyphs unchanged, no emoji in the `.ttf` files), COLR and CPAL read back from the bytes, the `.DEM` files read by a decoder written in the test, and reproducible builds |
| fontTools 4.55.3 and OTS 9.2.0, in Docker (`fonts/validate.sh`) | Both colour files read, recompile and sanitise cleanly; COLR version 0, CPAL the Desk set, cmap (0, 3) and (3, 1) format 4 and (0, 4) and (3, 10) format 12 |
| FreeType (freetype-py 2.5.1), in Docker | Every colour emoji renders through COLR pixel-exact against `emoji16.dem` at 16 and 32 px, from the `.ttf` and the `.otf`; every monochrome base glyph is exact at 16 px; every emoji in Desk Fixed, UI, Title and Small (Square) renders exactly as its `.DEM` mono bitmap at the native size |
| Chromium (headless, Linux, `@font-face`) | Desk Emoji Pixel shows in colour from the `.ttf` and the `.otf`, on light and dark; the monochrome emoji show in Desk Fixed, UI and Small |

Not checked: Windows (DirectWrite, GDI), macOS (Core Text), Safari and Firefox have not rendered the colour font; COLR version 0 is supported by all of them according to their documentation, but that is not a test. Nothing reads the `.DEM` files on DOS yet, and no emoji has been seen in the Desk, in DOSBox or on a VGA screen.

## Weak spots

From the proof sheets, the pictures that read least well:

- **At 8 × 8:** ⏰ (the bells and legs are one or two pixels), 🎮 (the d-pad is a small dark plus) and 🔓 (the open shackle reaches the top row). They read, but only just.
- **At 12 × 12:** 🆕 (a three-pixel W is close to an H), 😤 (the steam puffs crowd the face), 🐌, 🥳 and 🍳.
- **At 16 × 16:** 🗝 is close to 🔑 at a glance; 📻 and ☎ have small details that turn to dots; 🍦 reads as a cone with a grey cap. The 16 colours have no yellow, so 🟡 and 🟠, and 💛 and 🧡, differ only by a band of brown.
- **In mono:** solid saturated shapes (🧩 🐢 🐙 🌳) become silhouettes without inner detail, as the rule intends; 🎶's two note heads touch; ⌨'s keys become a speckle.
