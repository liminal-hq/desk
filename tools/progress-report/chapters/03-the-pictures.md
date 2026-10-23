---
id: pictures
n: 3
title: The pictures
date: 2026-10-22
summary: Four hundred and eighty pixel emoji, two colour fonts and a file format of our own, and the afternoon spent drawing one distorted face three times.
prs: [2]
---
With letters done, the Desk needed pictures, and PR #2 (stacked on #1, {{pr2.commits}} commits, {{pr2.added}} lines added) drew them. There are **{{m.emojiPixel}} pixel emoji** at real Unicode code points, hand-drawn as text grids in the 16-colour Desk palette, with 12 by 12 versions of {{emoji.pixel12}} of them and a core of {{emoji.pixel8}} at 8 by 8 for `Desk Small`. Beside them sit {{m.emojiVector}} flat vector emoji for use with the outline faces, two colour fonts (`Desk Emoji Pixel` and `Desk Emoji`), the monochrome glyphs added to the faces' OpenType files, and `.DEM`, a layered colour bitmap format for the DOS side with its own exporter and an independent decoder in the tests. The counts above are read from the headers of the generated `.dem` files, not typed in.

<!-- figure contact -->

And how much of the request is drawn, by form:

<!-- figure emoji-coverage -->

The wish behind the set is a list: every emoji in Gboard's Smileys and emotions panel. `docs/EMOJI-ROADMAP.md` tracks all {{emoji.requested}} requested sequences, of which {{emoji.requestedDone}} are drawn and {{emoji.requestedPlanned}} are planned, and a test fails if the roadmap and the manifest disagree. The faces are complete; hands, the seven joined sequences and 240 skin-tone variants are the work that remains.

<!-- facts emojiPixel emojiVector -->

## The face that took three tries

Unicode 17.0 added 🫪, the "distorted face", and the Desk's set wanted it. It was drawn three times in one afternoon, and the three commits are the best account I have of how the owner and the drawing agent worked. The grids below are read straight out of git at those commits and painted with the palette file at the last one.

<!-- figure faces -->

The first drawing, in [`67f7e9a`](commit:67f7e9a) at 16:32, did not work. The owner "could not tell what 🫪 was: the lumpy outline, a square eye and a dotted one, and a toothy squiggle did not read as a face, let alone a distorted one" (the commit message of [`1d094a2`](commit:1d094a2)). The second, at 17:34, was a clean face whose middle band had slipped a pixel sideways, with cyan and rose ghosts to each side, like a glitching screen. That idea did not survive either: at 17:53 the owner asked for the design the platforms share ("on his phone, Google's": a face with squashed, warped features, big mismatched eyes and a tiny mouth), and [`7e4a127`](commit:7e4a127) redrew it as an original pixel drawing of that idea, in both sizes, with two big white eyes of different sizes and a small low mouth. There is no 8 by 8 version: two big eyes with pupils do not fit beside a mouth at that size. The third drawing also reached the vector set, which grew from 155 to 156.

The lesson for the archive is small and useful: a picture that has to be understood at a glance is tested by showing it to a person, and a clever drawing is not the same as a legible one.

## The shelves

### Catalogued
- {{m.emojiPixel}} pixel emoji at 16 px and {{emoji.pixel12}} at 12 px, 40 at 8 px: counted from the `.dem` headers in the tree.
- Checked in PR #2's test plan: 436 tests passing from a clean `bun install --frozen-lockfile`, 58 generated files current under `bun build.ts --check`; fontTools, OTS and FreeType accepted every emoji file with 0 failures; every colour emoji renders pixel-exact at 16 and 32 px; all 155 vector emoji (before the 156th) are reachable through the outline `.otf` files' format 12 `cmap`; the era `.ttf` files carry none.
- The three drawings of 🫪 exist at the three commits named above, and the roadmap total ({{emoji.requestedDone}} done of {{emoji.requested}}) is read from `docs/EMOJI-ROADMAP.md`.

### Uncatalogued
- Windows (DirectWrite, GDI), macOS, Safari and Firefox: none was tried with the colour fonts.
- A `.DEM` file drawn by a DOS program on a DOS screen: only a decoder in the tests has read them.
- The weakest drawings, named by PR #2: at 8 by 8 the ⏰ 🎮 and 🔓; at 12 by 12 the 🆕 😤 🐌 🥳 and 🍳; 🗝 resembles 🔑; in vector, 👍. With no yellow in the palette 🟡 and 🟠 differ only by a band of brown.

### Wish list
- Skin tones by palette substitution (240 variants planned), the seven joined sequences such as 😮‍💨 and ❤️‍🔥, and 54 new base pictures, all planned in `docs/EMOJI-ROADMAP.md`, none built.
- More vector emoji where the geometric kit can draw them well; the vector set leaves out animals, food, hands and vehicles because geometry makes them stiff.

> **From the stacks — Twelve by twelve.** The tiny pictures that became emoji have a pixel-art past. As the story has been told for years, NTT DoCoMo's 1999 i-mode set by Shigetaka Kurita was the first; later research found earlier sets, among them a 1997 J-Phone set whose 90 emoji were 12 by 12 pixels and monochrome. The Desk's 12 px grid is the same size; the repository does not claim that it is an homage, and neither do I.
> Source: [Emoji, Wikipedia](https://en.wikipedia.org/wiki/Emoji)

```figures
id: emoji-contact
kind: specimen
track: design
title: The emoji contact sheets
alt: A contact sheet of the 480 pixel emoji at 16 by 16 pixels, drawn in the sixteen colours of the Desk palette.
caption: All the pixel emoji at 16, 12 and 8 px.
at: figure contact
thumb: img:img/pixel-16.png

id: emoji-coverage
kind: chart
track: design
title: Emoji coverage
alt: Bars of how many emoji are drawn as pixel and vector forms, and a stacked bar of how much of the 244-sequence Gboard request is done and planned.
caption: Emoji by form, and how much of the Gboard request is done.
at: figure emoji-coverage
thumb: svg

id: face-drawings
kind: history
track: design
title: Three drawings of the distorted face
alt: Three 16 by 16 pixel drawings of the distorted face emoji, each from a later commit than the one before.
caption: The same emoji redrawn in three commits.
at: figure faces
thumb: svgs
```
