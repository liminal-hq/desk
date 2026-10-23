# Desk: specification

> **Status:** a working design, derived from a design-agent brainstorm before any code existed. Nothing here has been built or measured. Numbers, versions and file formats are targets until a spike confirms them; when code and this document disagree, the code wins and this document is updated in the same branch. "Desk" is a working name (see `NAMING.md`).

> **Scope:** Desk is now an operating system in Rust for old hardware, built as a hosted track on DOS and a native track in parallel (`DIRECTION.md`, decided 2026-10-22). This document is the detailed spec of the **hosted track** and of the **OS-independent parts** (the look, the palette, fonts, widgets, apps, the SDK's shape and the interaction design). Anything here about DOS, DJGPP, CWSDPMI, `DESK.COM`, `DESK.RUN`, the handoff, DXE3 modules, 8.3 names or INT 33h applies to the hosted track only. The native track's kernel, boot, drivers and filesystem are specified in `DIRECTION.md` until a native spec exists.

The hosted track is the part of Desk that runs on top of DOS: the thing you install after DOS that gives the machine windows, icons, menus and a mouse pointer, and lets you run programs without typing at a prompt. It follows the layering Lieutenant Fizz for DOS proved (C for the hardware, Rust for everything else, DJGPP and CWSDPMI underneath) and builds in CI on `ghcr.io/liminal-hq/ci-dos`.

The design boards are in `design/Afterglow Desk.dc.html`: the looks (1a Void, 1b 1993, 1c Dusk, 1d the chosen mix), the architecture (2a) and the app anatomy (3a). The working prototype is `design/Afterglow Desk Prototype v4.dc.html`. This file is the reference; `docs/` has the detail, and `docs/HIG.md` is the design language every app follows.

## Goals

- **A richer place than the prompt.** Move between files and programs with a mouse or the keyboard, run several small apps side by side, and come back to where you were.
- **Small core, everything else optional.** The core install is the shell, Files, Programs and Settings. Every other app is installed, and can be removed, on its own.
- **Runs every DOS program.** Desk-native apps run in windows. Any `.COM`, `.EXE` or `.BAT`, real mode or protected mode, runs full-screen with the whole machine and then returns to the Desk.
- **An SDK for apps.** Apps are independent modules that hook into the Desk through one C ABI.
- **Calm.** Nothing badgers you. No unread counts, no alarm red. Toasts are quiet and time out.

## Not goals

- Multitasking DOS programs, a DOS box in a window, or Windows 3.x compatibility.
- Networking, in the first version.
- Real-hardware support before the browser and DOSBox builds are solid. Real hardware is a later target.

## Targets

| Target | First version | Notes |
|---|---|---|
| Browser (js-dos 8) | Yes (spike s5-jsdos, `S5-JSDOS.md`) | The same DOS programs run unchanged in js-dos 8.5.3 with `CWSDPMI.EXE` beside them. VBE 2.0 with the linear frame buffer works: 640×480×8, DPMI 0800h and a pixel-exact picture in headless Chromium. Read the LFB address from `4F01h` (`0xC0000000` here, `0xE0000000` on the DOSBox-X backend), expect 2 MiB of video memory and a different mode list from DOSBox Staging, and keep the banked path; mode 12h is not needed for the browser. Shipped as a `.jsdos` bundle beside a separately served js-dos (GPL-2.0), like `lieutenant-fizz-dos/site`. Only Chromium and the skeleton programs were tried |
| DOSBox Staging 0.83 | Yes | `machine = svga_s3`, VBE 2.0 |
| 86Box / real 486 with VESA | Later | Mode 12h fallback covers plain VGA |
| 1.44 MB floppy | Core only | Target: the core install fits one disk; apps ship on a second |

Minimum machine: a 486 with an FPU, 8 MB of memory, VGA, a Microsoft-compatible mouse driver (or keyboard only), DOS 5 or FreeDOS. Sound: a Sound Blaster 16 (or compatible, read from `BLASTER`) for digital sound and OPL3 music; an AdLib, an MPU-401 or the PC speaker also work.

## Display

| Mode | Use | Colours |
|---|---|---|
| VESA 640 × 480 × 8 (VBE 2.0 linear frame buffer) | Default | 256 |
| VESA 800 × 600 × 8 | Optional in Settings | 256 |
| VGA mode 12h, 640 × 480 × 4 planar | Fallback when no VESA BIOS or no linear frame buffer | 16 |

- **The palette is owned by the Desk.** Slots 0–15 hold the 16-colour fallback set so any screen degrades cleanly; slots 16–96 hold the Afterglow ramps (nine steps each for orange, red, yellow, green, cyan, blue, purple, neutral, grey); 97–127 the look's surfaces; 128–223 icons; 224–239 are lent to the focused app; 240–255 are the cursor and reserved. `docs/PALETTE.md` lists every slot.
- **No alpha, no blur.** Surfaces are solid palette colours. Gradients are stepped bands of ramp colours (see the horizon in 1a).
- **Fades and dims go through the DAC**, never through the frame buffer (the lesson from Fizz's `PERFORMANCE.md`, item 4).
- **The cursor is drawn by the Desk**, not by the mouse driver, so it works in every mode and with no flicker under the compositor.

## Looks

Settings will offer two looks, switchable at any time, like Fizz DOS's options screen. Version 0.1 ships the Afterglow look only; 1993 comes later (`OPEN-QUESTIONS.md`).

- **Afterglow**: the brand, the void, the orange accent and grotesk titles. Chosen from 1a or 1c.
- **1993**: period-native bevels, a VGA 8 × 16 font, and Afterglow only in the accents (1b).

Both looks are themes over the same widgets; an app never draws its own chrome. Text is always ≥ 4.5:1 against its background in both looks and both colour depths.

**Direction (1d):** 1a's type, colours and icon grid, with 1c's bevelled launcher bar at the bottom and stepped-halo window borders.

## Fonts

Four bitmap faces (`.DFN`), each with ASCII, Latin-1 and CP437 box drawing: 8×16 fixed for Terminal, code and paths; an 11 px proportional UI face; a 12 px bold title face; a 6×8 small face for status bars. Specimen on board 4e.

The faces are bitmaps; reading real `.TTF` files so any font can be used is a later possibility (`OPEN-QUESTIONS.md`, "Reading TrueType fonts").

## Memory and files

- When memory runs short the Desk lets CWSDPMI swap to `CWSDPMI.SWP` and carries on, slower. The focused app's status bar says "Using disk for memory"; nothing else interrupts.
- An app that crashes is unloaded and its windows close. A toast offers to reopen it from its last autosave; Inspector keeps a one-line log (board 4c).
- Names are 8.3. If DOSLFN is loaded, Files shows long names and Properties shows the 8.3 name beside them.

## Input

Mouse and keyboard are equal. Everything the mouse can do has a key.

| Key | Action |
|---|---|
| Alt + Tab | Window switcher overlay; Tab moves, release picks (board 4b) |
| Alt + F4 | Close the window |
| Alt + Space | Window menu (move, size, minimise, maximise, close) |
| Ctrl + Esc or the Windows key | Open the Desk menu |
| F10 or Alt | Focus the window's menu bar; arrows move, Enter opens |
| Tab / Shift + Tab | Next / previous control, with a 2 px orange focus ring |
| Arrows, Enter, Esc | Move, activate, cancel |
| Ctrl + X, C, V | Clipboard |
| Ctrl + Alt + Del in the Desk | Asks first, defaults to no |

The mouse is read through INT 33h. The keyboard is read through a Fizz-style IRQ 1 handler, extended for text entry and the extended-key prefix.

## The desktop

- **Icons**: the core apps, installed apps, drives (A:, C:) and any file or program you drag out of Files. Positions are kept in `DESK.INI`.
- **Launcher bar**: the Desk menu, one button per open window, desktop widgets (clock, Jax) and the date in `en-CA` ("Thu 22 Oct · 3:06").
- **The Desk menu** is built every time it opens, so installing or removing an app changes it straight away. Files and Terminal sit at the top; **Accessories**, **Entertainment** and **System** are submenus filled from each installed app's `APP.INF` (`group=`); **DOS programs** lists every program with a `.DIF` (or a desktop icon), with *Add a DOS program…* at the end; then Settings, About and Exit to DOS. Ctrl+Esc or the Windows key opens it; arrows move, Right opens a submenu, Left closes it. Right-click menus use the same submenus (Wallpaper, Window border and Display on the desktop).
- **Windows** overlap, move, size, minimise to the bar and maximise. Focus is shown by the accent (an orange outline in Afterglow, a plum title bar in 1993).
- **Wallpaper**: built-in palette wallpapers (Horizon, Ember, Dusk sky, Plum, Void) or your own `.BMP` / `.PCX`, centred, tiled or stretched, and reduced to palette slots 128–223 when loaded. Set in Settings → Desktop.
- **Window borders**: the default is the stepped halo. The focused window gets three palette steps of orange around it, a glow without alpha, and unfocused windows keep a 1px hairline. Hairline, gradient rule, dark bevel, chunky 3D and double line are available as themes (board 1d).
- **Toasts** appear bottom-right, above the bar, for four seconds; never more than one at a time.

## Apps

| App | Install | Purpose | Has |
|---|---|---|---|
| Files | Core | Tabs; tree, list or icons, preview; search with wildcards; copy, move, rename, new folder; Deleted with restore; drag and drop; Properties; Open with | Menu, toolbar, tabs, status |
| Programs | Core | Installed, On A: and DOS programs; install from A: or a `.DAP`; remove (asks first); run, set up and add desktop icons for DOS programs | Menu, toolbar, status |
| Settings | Core | Desktop (wallpaper), Windows (border), Display (mode, motion), Sound, Jax | Menu, section tabs |
| Audio Player | Optional | Music (`.MID` on OPL3, OPL2 or MPU-401) and sounds (`.WAV`, `.VOC` on the Sound Blaster at up to 44.1 kHz 16-bit stereo); library sidebar, playlist you build by dropping files, channel or level view, loop off/all/one, shuffle, `.M3U` playlists | Menu, toolbar, status |
| Notes | Optional | Text files in tabs, autosave, find and replace, word wrap, `[[NAME]]` links to other notes beside it | Menu, toolbar, tabs, status |
| Paint | Optional | 16-colour pictures in tabs; pencil, line, rectangle, fill, picker; zoom 1–6×; undo and redo; autosave; use as wallpaper | Menu, toolbar, tabs, status |
| Calculator | Optional | Standard, and programmer (32-bit; hex, decimal, octal, binary; AND OR XOR NOT shifts MOD); history tape | Menu |
| Clock | Optional | Time, month calendar, quiet reminders per day (a toast at the time; no sound, no snooze) | Menu |
| Inspector | Optional | System, Memory (by app), Drives, Sound (test tones); copy or save a report | Menu, section tabs, status |
| Terminal | Optional | A command line in a window over the Desk's files: DIR, CD, TYPE, COPY, MOVE, REN, DEL (to Deleted), MD, TREE, OPEN, MEM; tabs, history, Tab completion. Typing a program's name runs it full-screen and the tab picks up after | Menu, tabs, status |
| Mines | Optional | Clear a minefield: beginner, intermediate, expert; first click safe; right-click marks, double-click opens around a number; keyboard play; best times. The face is Jax's | Menu, status |
| Jax | Optional | The desk companion: a launcher-bar mini dock and a floating box | Right-click menu only |

Every app has the menu bar (Help ends with *About*), right-click menus, keyboard control and the shared Open/Save picker; the other parts appear only where they earn their place (`docs/HIG.md`).

Lieutenant Fizz Episode 1 ships on the apps disk as an ordinary DOS program with a `.DIF` file.

## Window or full-screen?

The kind of file decides, not the app, and nothing ever has to ask:

| It is | How it runs | How the Desk knows |
|---|---|---|
| A Desk app | In a window, inside the Desk | It's a `.DXE` module in `C:\DESK\APPS\<ID>\` with an `APP.INF`. Only the Desk can load one. |
| A DOS program (`.EXE`, `.COM`, `.BAT`) | Full-screen, with the whole machine | Anything DOS itself can run. Its `.DIF` (if any) adds a name, memory and sound needs, never "windowed". |

There is no third case. DOS has no way to run a program in a window: it would need the 386's virtual-8086 mode and a monitor that fakes the screen, keyboard and sound card for it, which is what Windows 3.x in enhanced mode and DESQview/X were. That's a different, much larger project, and it would break the very games people most want to run. The Terminal app is a Desk app, so it lives in a window; the programs it starts are DOS programs, so they go full-screen.

### Example: KEEN4.EXE

Commander Keen 4 is a 16-bit real-mode game: EGA graphics, AdLib or Sound Blaster sound, its own keyboard and timer handlers, and about 580 KB of conventional memory.

1. You double-click `C:\GAMES\KEEN4\KEEN4.EXE` in Files (or type `KEEN4` in Terminal). It's an `.EXE`, so it's a DOS program; `KEEN4.DIF` says it wants 580 KB and the Sound Blaster.
2. The Desk saves everything, writes `DESK.RUN` and exits. CWSDPMI unloads, the VESA mode goes back to text, and every interrupt the Desk hooked is handed back.
3. `DESK.COM` shrinks to about 4 KB and runs `KEEN4.EXE` with roughly 631 KB free. As far as Keen can tell, it was started from a DOS prompt.
4. You quit Keen. `DESK.COM` restarts the Desk with `/RESUME`: same windows, same Terminal tab, which prints that Keen finished.

If `KEEN4.DIF` asks for more memory than is free, the Desk says so before handing off instead of letting the game fail. A game that needs expanded memory says `ems=yes` in its `.DIF`; DESK.COM then loads JEMMEX before it and unloads it after.

## Running DOS programs: the handoff

The Desk is a protected-mode program under CWSDPMI. To give a DOS program the whole machine, it steps out of the way:

1. `DESK.COM` (real mode, a few KB) starts `DESKCORE.EXE` and waits.
2. To run a program the core writes `C:\DESK\DESK.RUN` (program, arguments, folder, the window layout to restore), fades the palette out, puts the screen back to text mode, unhooks its interrupts and exits. CWSDPMI and all its memory go with it.
3. `DESK.COM` reads `DESK.RUN` and runs the program.
4. When the program exits, `DESK.COM` restarts the core with `/RESUME`. The windows come back as they were and a toast says "Welcome back".

Exit to DOS writes no `DESK.RUN`; `DESK.COM` then exits too. `docs/HANDOFF.md` has the file format and the failure cases.

## Apps and the SDK

- A Desk app is a **DJGPP DXE3 module** (`APP.DXE`) loaded into the core's address space and run in windows.
- Apps talk to the Desk only through the **Desk SDK**, a C ABI the core exports (`desk_*`): windows; the shared menu bar, toolbar, tabs and status bar; widgets and dialogs; the Open/Save picker; files dropped on a window; the clipboard; sound; toasts; file-type associations; desktop widgets; settings storage; About boxes. `docs/SDK.md` has the surface.
- The prototype mirrors this: each app is its own component (`NotesApp`, `MidiApp`, `PaintApp`, `CalcApp`, `ClockApp`, `InspectorApp`, `ProgramsApp`, `SettingsApp`, `DeskFiles`, `DeskProperties`) that only talks to the Desk through one `ops` object, built from the shared `DeskMenuBar`, `DeskToolbar`, `DeskTabs`, `DeskPicker` and `FileIcon`.
- Apps can be written in Rust (`no_std` + `alloc`, the `dos.json` target from Fizz) or C.
- Only integers, pointers and lengths cross the boundary, never structs passed by value (the rule from Fizz's `RUST_ON_DOS.md`).
- An app that panics is unloaded and its windows closed, with a toast. The Desk keeps running. The core wraps every call into an app in `setjmp`; a panic, or a CPU fault caught by a signal handler, jumps back to that frame, and the core unloads the module and frees the memory it held. It is not a sandbox: a stack overflow in an app takes the Desk down, and apps share the core's address space.

**Risk (spike `s1-dxe3`, done; `docs/S1-DXE3.md`):** Rust objects reach DXE3 through `coff2go32.py` and DJGPP's `dxe3gen`, and it works: a Rust `no_std` module loads with `dlopen`, calls back into the core through names registered with `dlregsym`, panics without taking the core down, and loads and unloads 200 times with memory unchanged. The limits: LTO is required, one `.DXE` is one instance, a stack overflow in an app cannot be recovered, and page faults could be run only under QEMU (DOSBox Staging aborts on them), not on hardware. If real hardware disagrees, apps are compiled into the core and installing an app becomes enabling it.

## Installing the Desk

`INSTALL.EXE` (text mode, any PC) or the graphical installer (when VESA works) copies the Desk to `C:\DESK`, reads `BLASTER` and plays a test sound, finds the mouse (keyboard-only if there is none), and asks whether to start the Desk at boot. It edits `AUTOEXEC.BAT` only after showing the change, and keeps `AUTOEXEC.BAK` (board 4d).

## Packages

Programs accepts either:

- **A folder** with `APP.DXE`, `APP.INF` and its icons, on a disk or anywhere on C:.
- **A `.DAP` archive**, one file per app, unpacked by Programs.

Programs also reads `CATALOG.INF` from the root of a disk, which lists every package on that disk. Installed apps live in `C:\DESK\APPS\<NAME>\`. `docs/FORMATS.md` defines `APP.INF`, `.DAP`, `CATALOG.INF`, `.DIF`, `DESK.INI` and `DESK.RUN`.

## Files on disk

```
C:\DESK\
  DESK.COM          real-mode launcher, the thing you type
  DESKCORE.EXE      the shell
  CWSDPMI.EXE       DPMI host
  DESK.INI          settings and desktop layout
  DESK.RUN          handoff, present only while a DOS program runs
  FONTS\            bitmap fonts (.DFN)
  APPS\FILES\ …     one folder per app
  DESKTOP\          whatever is on the desktop
  WALLPAPR\         built-in wallpapers (.BMP)
C:\DELETED\         things deleted from Files (hidden), until emptied
C:\NOTES\           Notes' documents
C:\PAINT\           Paint's pictures
C:\MUSIC\           music and .M3U playlists
```

All names are 8.3.

## Build and test

- Built in CI on `ghcr.io/liminal-hq/ci-dos`, pinned by digest. The tool versions (Rust, DJGPP, DOSBox Staging, CWSDPMI) are whatever the image carries; `rust-toolchain.toml` pins Rust.
- The pipeline proved by Fizz DOS and re-derived in `docs/TOOLCHAIN.md`: `cargo build -Zbuild-std` for `dos.json`, `coff2go32.py`, link with `i586-pc-msdosdjgpp-gcc`. Apps add `dxe3gen`, with the exact commands in `docs/S1-DXE3.md`.
- **Headless tests** through `dosrun.sh`: the core starts, loads each app, opens and closes its window, and writes a frame checksum of the desktop in both colour depths. A scripted input file drives the mouse and keyboard.
- **A native bench**, as Fizz DOS's `m5/bench`: the compositor and widgets built for `i586-unknown-linux-gnu` to count instructions per redraw.
- **Release artefacts**: `-core.img` and `-apps.img` floppy images, the same as zips, a DOSBox demo zip, a `.jsdos` bundle, `SHA256SUMS`.

## Milestones

1. **Spikes**: VBE 2.0 linear frame buffer and mode 12h from Rust; INT 33h; a Rust DXE3 module that calls back into the core (done, with the limits above; `docs/S1-DXE3.md`).
2. **Shell**: compositor, windows, launcher bar, cursor, keyboard focus, both looks.
3. **Handoff**: `DESK.COM`, `DESK.RUN`, resume; run FIZZ.EXE and come back.
4. **Core apps**: Files, Programs (folders and `.DAP`), Settings.
5. **Optional apps**: Audio Player and Notes first, then the rest.
6. **Release**: floppy images, browser page, real-hardware notes.
