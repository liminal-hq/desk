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

## Still open

Hosted track:

- Which Rust/DXE route wins (after the spike).
- Whether the browser build uses mode 12h or waits for the LFB (after the js-dos spike).
- Who draws the four bitmap faces, and whether Fizz's `export-ui.ts` pipeline becomes the font tool.
- Whether JEMMEX loads and unloads cleanly from the command line on FreeDOS and MS-DOS 6.22 for each program (part of the handoff spike); if not, EMS games need a boot menu entry instead.

Native track and the shared core (details in `DIRECTION.md#open-questions`):

- Scheduling: cooperative in a single address space, or preemptive processes.
- Memory protection and how apps are isolated.
- Filesystem: FAT12 and FAT16 for floppies, and perhaps one of our own.
- Boot media, and whether the loader is ours.
- Early disk access: the BIOS or our own floppy and IDE drivers.
- Switching to real mode or V86 to call the BIOS, and whether it is worth it.
- Graphics driver strategy: VBE through the BIOS before switching, V86 calls, or native drivers for specific cards.
- Sound hardware detection without DOS or `BLASTER`.
- Input: our own PS/2 keyboard and mouse drivers.
- How DOS programs and FreeDOS are launched, and how Desk state is saved around them.
- Licensing of the bundled FreeDOS components.
- The shape of the hardware abstraction layer, and whether the module format becomes our own.
