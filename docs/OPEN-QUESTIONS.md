# Decisions before the repo

What was open before scaffolding `liminal-hq/desk`, and what was decided. The repo name is a working name; see `NAMING.md`. Each item says what it changes. Boards are in `design/Afterglow Desk.dc.html` (turn 4).

## Spikes that decide the architecture

| Question | Decision | Changes |
|---|---|---|
| Rust code as DXE3 modules (`coff2go32` → `dxe3gen` → `dlopen`, with calls back into the core) | **Yes, with limits (spike `s1-dxe3`, `S1-DXE3.md`).** Optional apps are `.DXE` modules that import `desk_*` by name from a table the core registers with `dlregsym` (route a). Function-pointer tables passed to `app_start` (route b) work too and stay the fallback. LTO is required; one file is one instance; a stack overflow in an app is not recoverable. The core's own apps may be linked in instead of loaded (a milestone 4 choice). Not needed: apps built into the core, C apps only, separate EXEs. | `apps/` is one Cargo workspace with a crate per app; `sdk/` holds the app-side crate (allocator, panic handler, `desk_app!`) and `desk.h`; `tools/build-app.sh` (to be added) generalises the spike's build. `SDK.md` and `FORMATS.md` corrected. |
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

The first spike, `s1-dxe3`, is done: Rust runs as a DXE3 module that calls back into the core, and the converter needed no change. The result, the commands and the limits are in [`S1-DXE3.md`](S1-DXE3.md); the next spike is `s2-vbe`.

## Still open

- Whether the page-fault recovery (`SIGSEGV` from a null write in an app) and the unrecoverable stack overflow behave on real hardware and on DPMI hosts other than CWSDPMI r7; DOSBox Staging cannot run them and QEMU was used (after the spike).
- Whether the browser build uses mode 12h or waits for the LFB (after the js-dos spike).
- Who draws the four bitmap faces, and whether Fizz's `export-ui.ts` pipeline becomes the font tool.
- Whether JEMMEX loads and unloads cleanly from the command line on FreeDOS and MS-DOS 6.22 for each program (part of the handoff spike); if not, EMS games need a boot menu entry instead.
