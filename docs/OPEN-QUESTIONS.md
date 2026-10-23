# Decisions before the repo

What was open before scaffolding `liminal-hq/desk`, and what was decided. The repo name is a working name; see `NAMING.md`. Each item says what it changes. Boards are in `design/Afterglow Desk.dc.html` (turn 4).

## Spikes that decide the architecture

| Question | Decision | Changes |
|---|---|---|
| Rust code as DXE3 modules (`coff2go32` → `dxe3gen` → `dlopen`, with calls back into the core) | **Yes, with limits (spike `s1-dxe3`, `S1-DXE3.md`).** Optional apps are `.DXE` modules that import `desk_*` by name from a table the core registers with `dlregsym` (route a). Function-pointer tables passed to `app_start` (route b) work too and stay the fallback. LTO is required; one file is one instance; a stack overflow in an app is not recoverable. The core's own apps may be linked in instead of loaded (a milestone 4 choice). Not needed: apps built into the core, C apps only, separate EXEs. | `apps/` is one Cargo workspace with a crate per app; `sdk/` holds the app-side crate (allocator, panic handler, `desk_app!`) and `desk.h`; `tools/build-app.sh` (to be added) generalises the spike's build. `SDK.md` and `FORMATS.md` corrected. |
| js-dos and the VESA linear frame buffer | **It works (spike `s5-jsdos`, `S5-JSDOS.md`).** js-dos 8.5.3 runs the repository's DOS programs unchanged, and VBE 2.0 640×480×8 with the linear frame buffer (DPMI 0800h) draws a pixel-exact picture in headless Chromium, as does the banked path. The browser build uses the same mode as DOSBox; mode 12h is not needed for it. | Release artefacts: a `.jsdos` bundle in 0.1, with js-dos served as a separate GPL-2.0 program. The video code reads the LFB address from `4F01h` and does not assume more than 2 MiB of video memory or a particular mode list. Only Chromium was tried; Firefox, Safari, audio you can hear, mobile and slow devices are not verified. |
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

## Direction (2026-10-22)

| Question | Decision | Changes |
|---|---|---|
| What is Desk | **An operating system, written in Rust, for old hardware** (486-class PCs: VGA and VBE graphics, Sound Blaster and OPL sound, PS/2 keyboard and mouse, floppy and IDE storage), graphical and calm. Not only a shell for MS-DOS. | Every document's framing; `DIRECTION.md` is the new reference. |
| Hosted or native | **Both, in parallel.** The hosted track runs Desk on DOS (DJGPP, CWSDPMI, DOSBox, js-dos later) for fast iteration; the native track is a bootable Rust kernel tested in QEMU, 86Box and Bochs (and v86 in the browser). They share an OS-independent core behind a small hardware abstraction layer. | The core must not call DOS or DJGPP directly. `SPEC.md` is the hosted track's spec until a native spec exists. |
| DOS programs on the native OS | **Bundle FreeDOS**, booting into it or running it in a V86 task (design later), rather than reimplementing DOS. A later milestone. | `HANDOFF.md` stays as designed for the hosted track and is reinterpreted for the native track. |
| Emulators | DOSBox and js-dos emulate DOS, not PC hardware, so they run only the hosted track. The native track needs QEMU, 86Box or Bochs. | `DIRECTION.md` has the test matrix. |

## Repo and process

| Question | Decision |
|---|---|
| Name and home | **`liminal-hq/desk`** (working name; the product name is open, see `NAMING.md`) |
| Licence | Apache-2.0 OR MIT, as Fizz; `NOTICE` lists CWSDPMI and JEMMEX, which ship as separate binaries |
| Test programs | Our own, never commercial or shareware games: `TONETEST.EXE` (sound), `REALMODE.COM` (a tiny real-mode program that sets mode 13h, reads the keyboard and exits), `NEEDEMS.EXE` (checks for EMS) |
| CI | `ghcr.io/liminal-hq/ci-dos`, pinned by digest. Frame checksums of the desktop at 256 and 16 colours, the handoff round trip with `REALMODE.COM`, and an EMS run with `NEEDEMS.EXE` |

## The walking skeleton

Done: `tools/check.sh` builds a program from Rust (`core/`, `no_std` with `alloc`) and C (`c/`, DJGPP), links it into `DESK.EXE` and runs it under headless DOSBox Staging, in CI and locally. It proves the target specification, the object converter, the allocator and panic glue, an x87 operation, exit-status propagation and the headless run, and it records what changed from Fizz DOS in `TOOLCHAIN.md`. It does not touch the open questions below: no module loading, no video, no input.

The first spike, `s1-dxe3`, is done: Rust runs as a DXE3 module that calls back into the core, and the converter needed no change. The result, the commands and the limits are in [`S1-DXE3.md`](S1-DXE3.md); the next spike is `s2-vbe`.

## Still open

Hosted track:

- Whether the page-fault recovery (`SIGSEGV` from a null write in an app) and the unrecoverable stack overflow behave on real hardware and on DPMI hosts other than CWSDPMI r7; DOSBox Staging cannot run them and QEMU was used (after the spike).
- Whether the browser build behaves the same in Firefox and Safari, on a phone and on a slow device, and whether the OPL music sounds right in js-dos (the spike measured pitch from the emulator's samples in Chromium only; see `S5-JSDOS.md`).
- Whether the four bitmap faces drafted in `fonts/` (see `FONTS.md`) are the ones that ship: they have not been seen on a DOS screen yet, and the 6×8 face is the one to check first.
- Whether Desk reads real TrueType files so any font can be used, and how (the section below).
- Whether JEMMEX loads and unloads cleanly from the command line on FreeDOS and MS-DOS 6.22 for each program (part of the handoff spike); if not, EMS games need a boot menu entry instead.

Native track and the shared core (details in `DIRECTION.md#open-questions`):

- Scheduling: cooperative in a single address space, or preemptive processes.
- Memory protection and how apps are isolated.
- Filesystem: FAT12 and FAT16 for floppies, and perhaps one of our own.
- Boot media, and whether the loader is ours.
- Early disk access: the BIOS or our own floppy and IDE drivers.
- Switching to real mode or V86 to call the BIOS, and whether it is worth it. A real-mode trampoline is built and works in QEMU and Bochs (spike n3, `N3-BIOS-PATH.md`); V86 is the later upgrade.
- Graphics driver strategy: VBE through the BIOS before switching, V86 calls, or native drivers for specific cards.
- Sound hardware detection without DOS or `BLASTER`.
- Input: our own PS/2 keyboard and mouse drivers.
- How DOS programs and FreeDOS are launched, and how Desk state is saved around them.
- Licensing of the bundled FreeDOS components.
- The shape of the hardware abstraction layer, and whether the module format becomes our own.

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
