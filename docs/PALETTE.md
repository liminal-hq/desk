# Palette

The Desk owns the 256-colour palette. Apps draw with colour roles (`SDK.md`), not slot numbers.

| Slots | Holds | Notes |
|---|---|---|
| 0–15 | The 16-colour fallback set | Also the only colours in mode 12h. Chosen so every role has a 16-colour stand-in at ≥ 4.5:1 for text |
| 16–96 | Afterglow hue ramps, nine steps each: orange, red, yellow, green, cyan, blue, purple, neutral, grey | Step 200 is the brand colour, as in `afterglow-themes` |
| 97–127 | Look-specific surfaces and bevels | Afterglow: `#050507`, `#0c0c10`, `#111116`, `#14161d`, `#16161c`, `#1b1e28`, `#262a37`, `#333847`. 1993: `#f4f4f8`, `#e0e0e0`, `#b4bfce`, `#8d97a8`, `#5a6273`, `#233a57` |
| 128–223 | Icons and shared images | Fixed, so `.ICO` files need no palette |
| 224–239 | Lent to the focused app | `desk_palette_lend`; returned on blur |
| 240–255 | Cursor, focus ring, reserved | |

## The 16-colour set

In mode 12h the Desk programs all 16 colour registers itself, so the fallback keeps the 1d look rather than falling back to the default VGA colours. Eight greys for surfaces, lines and text; eight hues for the accent, the halo and icons (board 4a).

| Slot | Colour | Role |
|---|---|---|
| 0 | `#050507` | void, text on accent |
| 1 | `#0c0c10` | chrome: title bars, menu bars, sidebars, status bars |
| 2 | `#111116` | content |
| 3 | `#1b1e28` | raised: launcher bar, buttons, icon tiles, toolbar |
| 4 | `#333847` | lines, bevel light, borders |
| 5 | `#7c8796` | muted text |
| 6 | `#b4bfce` | secondary text |
| 7 | `#e0e0e0` | text and headings |
| 8 | `#ffaa40` | accent, focus, halo step 1, horizon top |
| 9 | `#7d5422` | halo step 2, horizon middle |
| 10 | `#372612` | selection, halo step 3, horizon bottom |
| 11 | `#f43f5e` | rose: Lt. Fizz, errors |
| 12 | `#a78bfa` | purple: interactive, Programs |
| 13 | `#22d3ee` | cyan: playing, Audio Player |
| 14 | `#2ec66a` | green: success, Terminal |
| 15 | `#60a5fa` | blue: links, Notes |

What changes from 256 colours: icon tiles all use slot 3; headings use slot 7, not white; `#262a37` lines become slot 4; the horizon has three bands (slots 10, 9, 8) instead of seven; Paint's 16 colours are these 16. The VGA DAC holds 6 bits per channel, so each colour is converted to the nearest DAC value (`(v × 63 + 127) / 255` per channel, `Rgb6::from_rgb8`), not rounded to a multiple of 4: `#e0e0e0` is 55, where a multiple of 4 would give 56.

## The slots the first window uses

The first window (`FIRST-WINDOW.md`) draws with these 256-colour slots; every other slot above 15 is black until the ramps have values. The 16-colour set is also in 0–15 at 4 bits, where the surfaces and the cursor use slots 0–10 instead.

| Slots | Colour | Used for |
|---|---|---|
| 5–7 | `#7c8796`, `#b4bfce`, `#e0e0e0` | muted, secondary and primary text |
| 8–10 | `#ffaa40`, `#7d5422`, `#372612` | the halo's three rings; slot 8 is also the caret |
| 97 | `#050507` | the Void wallpaper |
| 98 | `#0c0c10` | title bars and control boxes |
| 99 | `#111116` | window content and the inside of a field |
| 100–101 | `#14161d`, `#16161c` | loaded with the other Afterglow surfaces; the first window does not draw with them |
| 102 | `#1b1e28` | raised surfaces (hovered control boxes, later) |
| 103 | `#262a37` | the rule under a title bar and the unfocused hairline |
| 104 | `#333847` | field outlines |
| 240–241 | `#050507`, `#e0e0e0` | the cursor's outline and fill |

## Gradients

Gradients are stepped bands of ramp slots, never dithered: the 1a horizon uses seven steps from `purple-800` through `red-700` to `orange-300`. In 16 colours a gradient keeps three steps, using slots 8, 9 and 10.

## Fades

A fade multiplies the 768 DAC values towards black over 16 frames. Dims behind a dialog halve the DAC values of slots 16–127 and leave the dialog's own surfaces (drawn from a reserved copy) alone.
