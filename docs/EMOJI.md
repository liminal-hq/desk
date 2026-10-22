# Emoji

Desk has its own emoji in two styles. The **pixel** emoji are 480 original pictures at their real Unicode code points, drawn as pixel art in the 16-colour Desk set ([`PALETTE.md`](PALETTE.md)). They come as monochrome glyphs in the OpenType files of the pixel faces, as a colour font, Desk Emoji Pixel, for modern systems, and as `.DEM` files ([`FORMATS.md`](FORMATS.md#dem-colour-emoji)) for the Desk on DOS, all built from the same letter grids in `fonts/src/emoji/pixel/`. The **vector** emoji are 155 of the same set drawn as flat geometric shapes for use beside Desk Sans, Mono and Serif: a colour font, Desk Emoji, and monochrome glyphs in the outline faces' OpenType files (see [Vector emoji](#vector-emoji)). The font build ([`FONTS.md`](FONTS.md)) writes everything, so the forms cannot drift apart.

## The set

Every emoji is a single code point with the `Emoji` property in Unicode 17.0, named by its CLDR short name (`fonts/src/emoji/manifest.ts`). There are no ZWJ sequences, skin-tone modifiers, keycaps, flags or tag sequences: each would need either OpenType layout (which the era files cannot carry and the Desk's text drawing does not do) or many more drawings. That rule holds for what is built today; the requested sequences, skin tones and ZWJ forms, and the plan for them, are in [`EMOJI-ROADMAP.md`](EMOJI-ROADMAP.md). Lt. Fizz's 24 pictures are all in it (✓ and ✗ as their emoji forms ✔ U+2714 and ❌ U+274C), and so are the shell's own pictures: disks, folders, pages, clocks, locks, sound, devices, warnings, arrows, media buttons, coloured circles and weather.

| Category | Emoji | Examples |
|---|---|---|
| Computer and media | 38 | 💾 💿 🖥 ⌨ 🖱 🖨 🎮 🔊 🔇 🔔 🔒 🔑 ⏰ |
| Office and paper | 32 | 📁 📂 📄 📝 📅 📋 📚 ✉ 📌 ✂ 🗑 |
| Faces | 111 | 🙂 😀 😉 😮 😢 😠 😎 😍 🤔 😴 🥵 🥶 🫪 🤣 🥰 🤩 🫠 🥹 🤫 🫡 🧐 🤬 🥴 🤒 🤑 💀 👽 🤖 |
| Hands and body | 22 | 👍 👎 👌 ✌ 👋 👏 🙏 💪 👀 |
| Hearts and feelings | 35 | ❤ 🧡 💛 💚 💙 💜 🩵 🩷 🩶 🤎 💔 💘 💝 💌 💋 💯 💬 💤 🫯 |
| Animals | 44 | 🐶 🐱 🦊 🐻 🐼 🐸 🐧 🐝 🦋 🐟 😺 😻 😿 🙈 🙉 🙊 |
| Food and drink | 34 | 🍎 🍌 🍕 🍔 🍩 🎂 ☕ 🥤 🍷 |
| Nature and weather | 33 | ☀ 🌙 ⭐ ✨ ☁ 🌈 ⚡ ❄ 🔥 💧 🍁 🌲 |
| Activities and objects | 38 | 🏆 🎁 🎈 ⚽ 🎲 🎸 💡 🔧 ⚙ 💎 |
| Travel and places | 32 | 🚗 🚌 🚲 ✈ 🚀 🛸 ⛵ 🏠 🏁 |
| Symbols and signs | 61 | ✔ ❌ ⚠ ⛔ ❓ ❗ ⬆ ➡ ▶ ⏸ ⏹ 🔴 🟢 🔵 |

Of the 480, 403 are outside the Basic Multilingual Plane, which matters for the TrueType files (see [Which file to install](#which-file-to-install)).

🫪 U+1FAEA distorted face (proposal L2/24-250) is one of the newest emoji, added in Unicode 17.0, so many systems do not have it in their own fonts yet; that is why it matters that the Desk emoji fonts carry it. It is drawn as a clean face whose band through the eyes has slipped a pixel sideways, with cyan and rose ghosts of its outline to each side, like a glitching screen; it has no 8 × 8, since the slip and the ghosts do not survive at that size. 🥵 hot face and 🥶 cold face are in the set too, each with a hand-drawn 12 × 12 and a hand-drawn monochrome glyph, because the monochrome rule fills a rose or blue face solid.

## Sizes and tiers

| Tier | Emoji | 16 × 16 | 12 × 12 | 8 × 8 |
|---|---|---|---|---|
| Core | 40 | drawn | drawn | drawn |
| Essential | 40 | drawn | drawn | — |
| Extended | 400 | drawn | derived (280) or hand-drawn (120) | — |

Every 16 × 16 is drawn by hand. The 80 core and essential emoji (the shell's pictures, Fizz's set and the commonest faces) have hand-drawn 12 × 12 grids, and the 40 core ones hand-drawn 8 × 8 grids for Desk Small. The other 12 × 12 grids are derived from the 16 × 16 by the rule below, and a hand-drawn grid in `FIX12` replaces the derived one where it does not read; every emoji added since the first set (the faces, cat and monkey faces and hearts from the Gboard list) has one, since a derived 12 × 12 loses a face's features. There is no derived 8 × 8: removing half the rows and columns of a drawing does not leave a picture, so the 8 × 8 set is the hand-drawn core only.

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
| `fonts/generated/desk-sans.otf`, `desk-mono.otf`, `desk-serif.otf` and their Bold forms | The outline faces with the 155 vector emoji as monochrome glyphs; Desk Mono's are two cells (1200 units) wide | Text in Desk Sans, Mono and Serif |
| `fonts/generated/emoji/desk-emoji.ttf` and `.otf` | Desk Emoji: the vector emoji in colour (COLR version 0 and CPAL), with the monochrome glyph as the base glyph | Colour emoji beside the outline faces |

In each face, the face's own drawing wins where it already has one: the folder and floppy disk in every face, and the code page 437 pictures (☺ ☻ ♥ ♦ ♣ ♠ ♪ ♫ ☼ and the arrows) in Desk Fixed, keep the glyphs drawn to sit in text.

### Which file to install

- **Windows 3.1, 95, NT 3.5x and OS/2 Warp:** the `.ttf` faces, pixel and outline, which carry no emoji. 403 emoji are beyond the Basic Multilingual Plane and need a format 12 cmap, which those systems do not read, and adding the rest would change files whose behaviour on those systems is described in [`FONTS.md`](FONTS.md); the era TrueType files are byte-for-byte what they were before the emoji. Those systems have no colour fonts either.
- **Windows 10 and 11, Linux, macOS:** install the `.otf` faces for emoji in Desk text (monochrome, in the face's own size and weight), and Desk Emoji Pixel (beside the pixel faces) or Desk Emoji (beside Desk Sans, Mono and Serif) for colour. For colour, put Desk Emoji Pixel before the text face in a font list (`font-family: "Desk Emoji Pixel", "Desk UI"`), or let the system's fallback find it; the text faces' monochrome emoji win wherever the text face comes first.
- **Browsers:** `@font-face` with `desk-emoji-pixel.ttf` (or `.otf`) and list it first for colour emoji; the pixel faces' `.otf` files give monochrome emoji.
- **DOS:** the `.DEM` files; the Desk reads the size that matches the text (16 for Desk Fixed, 12 for Desk UI and Title, 8 for Desk Small).

Desk Emoji Pixel is exact at 16 px and whole multiples (12, 24, 36 pt at 96 dpi), like the pixel faces. Its em is the 16-pixel grid (units per em 1600), the grid's top row starts 14 pixels above the baseline and its bottom row ends 2 below, and the line is 15 pixels up and 4 down (1.2 em) with no line gap. `CPAL` holds one palette, the 16 Desk colours slot for slot, so a layer's palette index is its slot. Each emoji's layers are traced from its grid, back to front: black first, then the other colours from the largest area to the smallest, each layer covering its own pixels and every pixel of the colours in front of it, so no two layer edges have to meet exactly and anti-aliasing shows no seams. The zero-width joiner and the variation selectors U+FE0E and U+FE0F map to an empty glyph, so text that carries them shows no missing-glyph box. The `name` table carries the SIL Open Font License 1.1, as the faces do.

## Vector emoji

The vector emoji are drawn in code (`fonts/src/emoji/vector/art.ts`) with a small kit of filled shapes (`kit.ts`): discs and ellipses, boxes with round corners, polygons, round-ended strokes and arcs, stars, hearts and drops, combined with the outline faces' geometry (`fonts/src/modern/geom.ts`, a union and difference by clipper-lib, then fitted to cubic curves). Each drawing is a list of layers, back to front, each one shape in one colour, in a 1000-unit box whose top is 850 units above the baseline, so an emoji spans from the descender to above the capitals as it does in most emoji fonts. Light comes from the top left as in the pixel set: a face, a heart or a disc has a darker crescent on its lower right, and white objects have a light grey edge so they show on a light page.

The palette is defined once (`VCOLOURS` in `kit.ts`): the brand colours of the 16-colour set (amber, rose, purple, cyan, green, blue, brown), each with a lighter or darker tint, and the greys of the set; 22 colours in all, the CPAL table of Desk Emoji. Like the pixel set, it has no yellow distinct from amber.

**Monochrome glyphs** follow the pixel rule, by layer: a layer in a dark or saturated colour is ink; a layer in a light colour (white, light grey, amber, cyan and their light tints) is paper with an ink edge, so a face becomes its outline, eyes and mouth and a page its outline and lines. A drawing can mark a layer as a clean hole (the keys of a keyboard, the check on ✅, the glyph on a media button) or leave a shading layer out. The edge is 64 units in Desk Emoji's base glyphs and in the Regular outline faces, and 120 in the Bold ones, so the emoji in Desk Sans Bold are heavier, like its letters.

**What is drawn:** 155 of the 480 emoji, chosen for value and for what simple geometry can draw well. All of Lt. Fizz's 24 and every shell picture are in it, with the commonest faces and hearts and most of the symbols:

| Category | Vector | Of |
|---|---|---|
| Symbols and signs | 53 | 61 |
| Faces | 28 | 111 |
| Computer and media | 25 | 38 |
| Nature and weather | 18 | 33 |
| Hearts and feelings | 14 | 35 |
| Office and paper | 8 | 32 |
| Activities and objects | 5 | 38 |
| Food and drink | 2 | 34 |
| Hands and body, Travel and places | 1 each | 22, 32 |
| Animals | 0 | 44 |

**What is left out, and why:** animals, food, hands, vehicles, buildings and most objects. Their identity is in organic silhouettes (a cat's head, a hand's fingers, a car's body) that circles and boxes turn into stiff, generic shapes; drawn this way they would read worse than the pixel versions, which already cover them. Where a vector emoji is missing, a font list that names Desk Emoji Pixel after Desk Emoji fills the gap with the pixel drawing. The faces that need hands or props (🤔 🤗 🤠 🥳) and the busy symbols (☯ ♻ ☮) are also out, and so is 🫪: its colour ghosts and slipped band work in colour, but its monochrome glyph is a plain smiley, so the pixel drawing stands in for it.

**How good it is.** Geometric emoji are clean and consistent and read well from about 20 px up; at text sizes below 16 px the eyes and mouths of the faces close up, and in monochrome the thin details (📅's grid, ⌨'s keys) blur. They are flat and plain next to the hand-drawn emoji of the big platforms: a heart, a star, a check or a face is as good as anyone's, a thumbs up or a camera is recognisable but stiff.

## The specimen site

The Emoji page of the specimen site (`bun build.ts --site <dir>`, see [`FONTS.md`](FONTS.md#the-specimen-site)) shows the whole set from the build's own data: contact sheets by category at 16, 12 and 8 pixels and in vector, in colour or mono, on dark or light, at three zooms; a search over names, code points, categories and tiers; a detail panel with every size of one emoji; a tester in the emoji fonts and the faces that carry them; the emoji files; and the number still planned in [`EMOJI-ROADMAP.md`](EMOJI-ROADMAP.md). Its sources are `fonts/src/site/emoji.html`, `emoji.js` and `emoji.ts`.

## What has been checked

| Check | Result |
|---|---|
| `bun test` (`fonts/test/emoji-pixel.test.ts`, `emoji-files.test.ts`) | The manifest (unique code points, the `Emoji` property, no sequences, Fizz's and the shell's pictures, the tiers), the palette against `PALETTE.md`, grid sizes, margins and tiers, the mono rule, the derivation, the cmaps (format 4 and 12 on the right platforms, every emoji reached, the faces' own glyphs unchanged, no emoji in the `.ttf` files), COLR and CPAL read back from the bytes, the `.DEM` files read by a decoder written in the test, and reproducible builds |
| fontTools 4.55.3 and OTS 9.2.0, in Docker (`fonts/validate.sh`) | Both colour files read, recompile and sanitise cleanly; COLR version 0, CPAL the Desk set, cmap (0, 3) and (3, 1) format 4 and (0, 4) and (3, 10) format 12 |
| FreeType (freetype-py 2.5.1), in Docker | Every colour emoji renders through COLR pixel-exact against `emoji16.dem` at 16 and 32 px, from the `.ttf` and the `.otf`; every monochrome base glyph is exact at 16 px; every emoji in Desk Fixed, UI, Title and Small (Square) renders exactly as its `.DEM` mono bitmap at the native size |
| Chromium (headless, Linux, `@font-face`) | Desk Emoji Pixel and Desk Emoji show in colour from the `.ttf` and the `.otf`, on light and dark; the monochrome emoji show in Desk Fixed, UI and Small and in Desk Sans, Mono and Serif |
| Vector, in Docker and `bun test` | Desk Emoji reads and sanitises cleanly (fontTools, OTS); FreeType renders all 155 through COLR in colour; Desk Sans, Mono and Serif (all six `.otf`) reach every vector emoji through format 12 and sanitise cleanly, and their `.ttf` files carry none; the faces' own glyphs are unchanged |

Not checked: Windows (DirectWrite, GDI), macOS (Core Text), Safari and Firefox have not rendered either colour font; COLR version 0 is supported by all of them according to their documentation, but that is not a test. Nothing reads the `.DEM` files on DOS yet, and no emoji has been seen in the Desk, in DOSBox or on a VGA screen.

## Weak spots

From the proof sheets, the pictures that read least well:

- **At 8 × 8:** ⏰ (the bells and legs are one or two pixels), 🎮 (the d-pad is a small dark plus) and 🔓 (the open shackle reaches the top row). They read, but only just.
- **At 12 × 12:** 🆕 (a three-pixel W is close to an H), 😤 (the steam puffs crowd the face), 🐌, 🥳 and 🍳.
- **At 16 × 16:** 🗝 is close to 🔑 at a glance; 📻 and ☎ have small details that turn to dots; 🍦 reads as a cone with a grey cap. The 16 colours have no yellow, so 🟡 and 🟠, and 💛 and 🧡, differ only by a band of brown.
- **Vector:** 👍 is stiff (a box and a rounded thumb), 💯's digits are drawn shapes rather than type, 🍁 and 🍀 are simplified, and ♠ ♣ ⚫ ⬛ are black and nearly vanish on a dark background, as in the pixel set.
- **In mono:** solid saturated shapes (🧩 🐢 🐙 🌳) become silhouettes without inner detail, as the rule intends; 🎶's two note heads touch; ⌨'s keys become a speckle.
