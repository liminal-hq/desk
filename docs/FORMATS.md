# File formats

All text formats are INI-style: `[section]`, `key=value`, `;` comments, CRLF line endings, UTF-8. All names are 8.3.

## APP.INF: one app

```ini
[app]
name=Audio Player
id=MIDI            ; 1–8 characters, the folder name under C:\DESK\APPS
version=0.1.0
module=APP.DXE
icon=APP.ICO
sdk=1              ; the Desk SDK version it needs
core=no            ; yes only for Files, Programs and Settings
group=Entertainment ; where it goes in the Desk menu: Accessories, Entertainment, System, or top for the top level
size=84            ; KB once installed

[types]
MID=open
RMI=open

[widgets]
; none

[about]
text=Play MIDI files on the AdLib or an MPU-401.
licence=Apache-2.0 OR MIT
```

## .DAP: an app archive

One file per app, so an app can travel as a single download or a single file on a disk.

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | `DAP1` |
| 4 | 2 | number of entries |
| 6 | 2 | flags (bit 0: entries are compressed) |
| 8 | 32 × n | entries: 8.3 name (13 bytes, NUL-padded), 3 reserved, offset (4), stored length (4), length (4), CRC-32 (4) |
| … | | data |

`APP.INF` must be the first entry. Compression, if used, is LZ4 block format. Programs checks every CRC before it copies anything.

## CATALOG.INF: a disk of apps

At the root of a disk, so Programs can list what is on it without opening every package.

```ini
[disk]
label=DESK APPS
title=Desk apps, disk 1

[MIDI]
file=MIDI.DAP
name=Audio Player
size=84
text=Play MIDI files on the AdLib or an MPU-401.

[FIZZ]
folder=FIZZ        ; a plain folder rather than a .DAP
name=Lt. Fizz Ep. 1
size=935
kind=dos           ; a DOS program, set up with its .DIF
```

## .DIF: a DOS program's information

Like Windows' `.PIF`: beside the program, same base name (`FIZZ.DIF` beside `FIZZ.EXE`). If there is none, Files and Programs work out a name and a generic icon from the file itself.

```ini
[program]
name=Lt. Fizz Episode 1
run=FIZZ.EXE
args=
folder=.           ; the program's own folder
icon=FIZZ.ICO

[needs]
mode=protected     ; real or protected; only informational
memory=8192        ; KB
ems=no             ; yes: DESK.COM loads JEMMEX for this program
sound=adlib        ; adlib, speaker, mpu401, none
pause=no           ; yes: "Press a key to return to the Desk"
menu=yes           ; list it under DOS programs in the Desk menu (any program with a .DIF is listed unless this is no)
```

## .ICO: an icon

32 × 32 and 16 × 16 bitmaps, 8-bit, palette indices from the Desk's shared ranges (no palette of its own). Index 0 is transparent. Header `DIC1`, then width, height, pixels, for each size.

## DESK.INI: settings and layout

```ini
[display]
mode=640x480x8     ; 640x480x8, 800x600x8, 640x480x4
look=afterglow     ; afterglow, 1993
motion=on

[desktop]
wallpaper=horizon  ; horizon, ember, dusk, plum, void, or a path to a .BMP / .PCX (any picture from Files or Paint)
motion=on          ; off: no fades, Jax stays still
wallfit=centre     ; centre, tile, stretch
border=halo        ; hairline, rule, halo, bevel, chunky, double
icon.FILES=12,14
icon.PROGS=82,14
bar=bottom

[sound]
effects=opl2       ; opl2, speaker, off
midi=mpu401        ; mpu401, opl2

[app.MIDI]
last=C:\MUSIC\COCOA.MID
```

## DESK.RUN: the handoff

Written by the core just before it exits to run a DOS program, read by `DESK.COM`, deleted by the core on resume. Plain text so `DESK.COM` stays tiny. In order: the program, its arguments, its folder, a pause flag, then the layout.

```
C:\FIZZ\FIZZ.EXE
nopreview
C:\FIZZ
0
[layout]
FILES 160 30 340 232 C:\FIZZ
MIDI 380 222 230 120
focus=MIDI
```

`HANDOFF.md` covers what each side does with it.

## .M3U: a playlist

What Audio Player saves. Plain text, one full 8.3 path per line, CRLF, no extended `#EXTM3U` headers. Lines that don't point at a `.MID` or `.RMI` are skipped when it loads.

```
C:\MUSIC\COCOA.MID
C:\MUSIC\CAVES.MID
A:\MUSIC\BOSS.MID
```

## .BMP: pictures from Paint

Windows 3.x BMP, 4 bits a pixel, with the Desk's 16-colour set (`PALETTE.md`) as its colour table, so any other paint program can open it. Paint's own pictures are 200 × 120 by default; the wallpapers in `C:\DESK\WALLPAPR` are 640 × 480 × 8 and open in Paint as a new picture.

## .WAV and .VOC: sounds

The Audio Player plays RIFF WAVE (PCM, 8- or 16-bit, mono or stereo, up to 44.1 kHz) and Creative Voice files through the Sound Blaster's DMA. Without a Sound Blaster it says so instead of trying the PC speaker.

## Mines best times

```ini
[app.MINES]
beginner=42
intermediate=191
```

Seconds, kept only when beaten.

## Reminders

Clock keeps reminders in its section of `DESK.INI`, one key per reminder:

```ini
[app.CLOCK]
r1=2026-10-22 17:30 0 Back up the notes to A:
r2=2026-10-23 09:00 1 Stretch
```

Date, 24-hour time, done (0 or 1), then the text. A reminder shows one toast at its time, if the Desk is running, and is never repeated.

## .DFN: a bitmap font

Fixed or proportional, 1 bit per pixel, leftmost pixel in the highest bit, as Fizz's `ui_data.rs` font. Header `DFN1`, height, baseline, glyph count, then for each glyph its code point (4), width (1) and rows.
