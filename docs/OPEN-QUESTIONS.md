# Decisions before the repo

What was open before scaffolding `liminal-hq/desk`, and what was decided. The repo name is a working name; see `NAMING.md`. Each item says what it changes. Boards are in `design/Afterglow Desk.dc.html` (turn 4).

## Spikes that decide the architecture

| Question | Decision | Changes |
|---|---|---|
| Rust code as DXE3 modules (`coff2go32` → `dxe3gen` → `dlopen`, with calls back into the core) | **Decide after the spike.** It is milestone 1, before any app code. The fallbacks on the table are: build apps into the core; write apps in C against the SDK; separate EXEs. | The `apps/` layout and the SDK crate. Keep both possible until the spike passes. |
| js-dos and the VESA linear frame buffer | **Ship DOSBox-only until it works.** The browser build waits; nothing is forked or patched yet. | Release artefacts: no `.jsdos` bundle in 0.1 unless the spike shows the LFB working. |
| Games that need EMS | **DESK.COM loads an EMS driver for that program, as its `.DIF` asks.** `[needs] ems=yes` makes DESK.COM run `JEMMEX` before the program and unload it after. | `.DIF` gains `ems=`; HANDOFF gains a step; `JEMMEX.EXE` (GPL, separate binary) ships beside DESK.COM. |

## Design gaps

| Question | Decision | Board |
|---|---|---|
| Bitmap fonts | Four `.DFN` faces for 0.1: **8×16 fixed** (Terminal, code, paths), **proportional UI** (Inter-like, 11 px), **title** (Space Grotesk-like, 12 px bold), **6×8 small** (status bars, badges). Each covers ASCII, Latin-1 (Canadian French needs it) and CP437 box drawing. | 4e |
| 16-colour fallback | Designed now, and it keeps the 1d look: the Desk programs all 16 VGA colour registers itself (8 greys, 8 hues), so the halo, selection, focus and text are unchanged. Icon tiles share one grey and the horizon has three bands. | 4a |
| The 1993 look | **Later**, not in 0.1. Settings shows only the Afterglow look until then. | — |
| Keyboard-only use | All five: **Alt+Tab** switcher overlay, **F10 or Alt** for the menu bar, **Tab** between controls with a visible 2 px focus ring, **Alt+Space** window menu with arrow-key move and size, **Ctrl+Esc or the Windows key** for the Desk menu. | 4b |
| An app crashes | **Close its windows, show a toast, and offer to reopen it from its last autosave.** Inspector keeps a one-line log. | 4c |
| Running out of memory | **Swap to disk through CWSDPMI and keep going**, slower. The only sign is a quiet note in the status bar of the focused app ("Using disk for memory"), and Inspector's Memory tab. | — |
| Long file names | **Read them when DOSLFN is loaded.** Files shows the long name; Properties shows the 8.3 name beside it. Without DOSLFN nothing changes. Renaming to a long name needs DOSLFN and says so if it isn't there. | — |
| First run and installation | **Both**: a text-mode `INSTALL.EXE` that works on any PC, and a graphical installer when VESA works. Both copy to `C:\DESK`, read `BLASTER` and test the sound, detect the mouse (keyboard-only if there is none), and ask whether to start the Desk at boot. They edit `AUTOEXEC.BAT` only after asking, and keep `AUTOEXEC.BAK`. | 4d |

## Repo and process

| Question | Decision |
|---|---|
| Name and home | **`liminal-hq/desk`** (working name; the product name is open, see `NAMING.md`) |
| Licence | Apache-2.0 OR MIT, as Fizz; `NOTICE` lists CWSDPMI and JEMMEX, which ship as separate binaries |
| Test programs | Our own, never commercial or shareware games: `TONETEST.EXE` (sound), `REALMODE.COM` (a tiny real-mode program that sets mode 13h, reads the keyboard and exits), `NEEDEMS.EXE` (checks for EMS) |
| CI | `ghcr.io/liminal-hq/ci-dos`, pinned by digest. Frame checksums of the desktop at 256 and 16 colours, the handoff round trip with `REALMODE.COM`, and an EMS run with `NEEDEMS.EXE` |

## The walking skeleton

Done: `tools/check.sh` builds a program from Rust (`core/`, `no_std` with `alloc`) and C (`c/`, DJGPP), links it into `DESK.EXE` and runs it under headless DOSBox Staging, in CI and locally. It proves the target specification, the object converter, the allocator and panic glue, an x87 operation, exit-status propagation and the headless run, and it records what changed from Fizz DOS in `TOOLCHAIN.md`. It does not touch the open questions below: no module loading, no video, no input.

The next spike is `s1-dxe3` (milestone 1): can Rust code run as a DXE3 module that calls back into the core? It starts from `tools/coff2go32.py`, which the module build will need to extend, and from the same `tools/check.sh` run.

## Still open

- Which Rust/DXE route wins (after the spike).
- Whether the browser build uses mode 12h or waits for the LFB (after the js-dos spike).
- Whether the four bitmap faces drafted in `fonts/` (see `FONTS.md`) are the ones that ship: they have not been seen on a DOS screen yet, and the 6×8 face is the one to check first.
- Whether Desk reads real TrueType files so any font can be used, and how (the section below).
- Whether JEMMEX loads and unloads cleanly from the command line on FreeDOS and MS-DOS 6.22 for each program (part of the handoff spike); if not, EMS games need a boot menu entry instead.

## Reading TrueType fonts

The idea: let Desk load `.TTF` files so apps can use any font, not only the four bitmap faces. This is what the operating systems of the era added (TrueType on System 7 in 1991 and Windows 3.1 in 1992, after Type 1 through Adobe Type Manager on Windows 3.0): a font stores outlines and the system draws them at any size with its own rasteriser. OpenType with CFF outlines (`.otf`) only arrived in 1996–97, so it is not part of that story.

It is not needed for 0.1. The bitmap faces stay the UI default, and the font build also writes `.ttf` files so the same faces install on Windows 3.1-era systems (`FONTS.md`).

**What it would take**

- **Parsing.** `cmap`, `glyf` (simple and composite glyphs), `loca`, `hmtx`, `head`, `hhea` and `maxp`; kerning later, if at all. A `no_std` parser crate (`ttf-parser`) exists, or this is small enough to write.
- **Rasterising.** Flatten the quadratic curves and fill scanlines in fixed-point integers (as FreeType does), because the SPEC keeps floating point out of pixel loops. Every glyph is cached per size, so a 486 draws each glyph's outline once.
- **TrueType only at first.** CFF (`.otf`) needs a second parser for Type 2 charstrings, so it waits.

**What makes it hard**

- **Hinting.** Windows 3.1 drew TrueType in plain black and white, and it looked good at small sizes because fonts carry hinting bytecode that snaps outlines to the pixel grid. Without it most fonts look lumpy at 11 px. A bytecode interpreter is a large job.
- **Smoothing without alpha.** The palette has no alpha (`SPEC.md`, Display). Smooth edges mean a short ramp of in-between shades for each text colour, so text would need a few reserved palette slots per foreground and background pair.

**Options, to decide after a spike**

| Option | What it costs | What it gives |
|---|---|---|
| Unhinted outlines with 4–5 step smoothing, for larger sizes only (Notes at 14 px and up, Paint's text tool, previews) | A rasteriser and a few palette slots per colour pair | Any font, at sizes where hinting matters less |
| Convert a TTF to a `.DFN` at one chosen size when it is installed | The same rasteriser, run once at install time (or off-line on a host machine) | Reuses the bitmap path and the palette as it is; the font is tied to that size |
| A bytecode interpreter for hinted, black-and-white text | A large piece of work | Period-accurate small text |

Spike `s6-ttf` (in `spikes/README.md`) decides between these. Its question is whether an integer rasteriser draws a real TrueType face at 14 px and 24 px on a 486 within the frame budget, and how it looks next to the bitmap faces.
