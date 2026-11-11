# The shell design

What the full shell design holds, part by part, and how it compares with the documents and with the code. The design is the one the shell is working towards: the owner's design canvas, a claude.ai artifact (<https://claude.ai/artifact/63Qww66fRXY2yE6TVAVgxe>), captured on 2026-11-11 into [`../design/shell-og/`](../design/shell-og/) with screenshots of every board, app and dialog.

> **Status:** a description of a design, not of software. The design is a browser prototype written by a design agent before any code existed; nothing in it has been built or measured on a 486. Where this document says what the code does, it cites the crate or the roadmap item ([`ROADMAP.md`](ROADMAP.md)). Readings of the prototype that are uncertain are marked as such.

## Contents

- [Where it came from](#where-it-came-from)
- [How it relates to design/ and the documents](#how-it-relates-to-design-and-the-documents)
- [The pages and artboards](#the-pages-and-artboards)
- [The shell's parts](#the-shells-parts)
- [The apps](#the-apps)
- [The visual system](#the-visual-system)
- [Interactions](#interactions)
- [What is built](#what-is-built)
- [Differences from the documents](#differences-from-the-documents)
- [Open questions for the owner](#open-questions-for-the-owner)

## Where it came from

The canvas is a Claude Design project titled "DOS Project Brainstorm", moved to claude.ai on 2026-11-11 (its `compat.json` records the move and the original project). It is the brainstorm that produced the repository's first documents: its own drafts of the spec, SDK, formats, handoff, palette, guidelines and open questions are in the canvas too, and the repository's `docs/` grew from them. The brainstorm called the project "Afterglow Desk"; the working name is now "Desk" ([`NAMING.md`](NAMING.md)), and the prototype's strings keep the old name.

The link the owner shared ends in `#artboard-6b84bc2503e7`. The editor names an artboard by a 53-bit hash of its file name, and that hash is the one for `Afterglow Desk Prototype v4.dc.html`: the working prototype, which is also the canvas's entry page. Most of this document is about that page.

## How it relates to design/ and the documents

**The same design as `design/`, and a superset of it.** Every page and component already in [`../design/`](../design/) is byte-identical to its copy in the canvas: the v4 and v3 prototypes, the boards, the ten app components, the six shared components, the design system and the images. The canvas adds the first two prototypes (`Afterglow Desk Prototype.dc.html` and `v2`), its page list, and the brainstorm's markdown drafts. Nothing in the canvas is newer than `design/`'s v4.

**The documents follow it closely.** [`SPEC.md`](SPEC.md), [`HIG.md`](HIG.md) and [`PALETTE.md`](PALETTE.md) were written from this design (SPEC names `design/Afterglow Desk Prototype v4.dc.html` as the working prototype), so most of what follows agrees with them. The canvas's own drafts (`design/shell-og/SPEC.md`, `design/shell-og/docs/`) are the older versions: they name v3 as the prototype, predate the native track and the spike results, and are superseded by `docs/`. The differences that remain between the prototype and the documents are listed under [Differences from the documents](#differences-from-the-documents).

## The pages and artboards

The canvas has 22 pages. Five are full pages; seventeen are components, each an artboard of its own that the prototypes import.

| Page | Kind | What it shows |
|---|---|---|
| `Afterglow Desk Prototype v4.dc.html` | Page (entry) | The working prototype: a 640 × 480 desktop drawn at a zoom that fits the browser, with a hint line under it. Tweaks for window border, wallpaper and colours (256, 800 × 600, 16) |
| `Afterglow Desk Prototype v3.dc.html` | Page | v4 without the colour-depth switch, the 16-colour remapping and 800 × 600; otherwise the same apps and components |
| `Afterglow Desk Prototype v2.dc.html` | Page | Files and Properties as components, drag and drop, the file-copy job and the drag ghost; the other apps (MIDI Player, Notes, Paint, Calculator, Clock, Inspector) still drawn inline; no Terminal, Mines or picker |
| `Afterglow Desk Prototype.dc.html` | Page | The first prototype: every app drawn inline, no components |
| `Afterglow Desk.dc.html` | Page (boards) | Five sections: Concepts (1a, 1b, 1c), 1d, Architecture (2a), App anatomy (3a), Before the repo (4a to 4e) |
| `DeskFiles`, `DeskProperties` | Component | The Files app and the Properties window |
| `DeskMenuBar`, `DeskToolbar`, `DeskTabs`, `DeskPicker`, `FileIcon` | Component | The shared parts every app is built from, and the file-type icons |
| `CalcApp`, `ClockApp`, `InspectorApp`, `MidiApp`, `MinesApp`, `NotesApp`, `PaintApp`, `ProgramsApp`, `SettingsApp`, `TerminalApp` | Component | One per app |

### The boards (`Afterglow Desk.dc.html`)

All the boards are drawn at 640 × 480 in solid palette colours, shown at 1.25× or 1.5×.

- **Concepts: 1a Void.** "Faithful Afterglow": the void, a stepped horizon glow of seven palette bands with no dither, 1 px chrome, a free icon grid, grotesk type, the bar at the bottom, and an orange outline on the focused window. Files at `C:\FIZZ` and the MIDI Player over it; the bar shows Desk, the open windows, Jax and "Thu 22 Oct · 3:06".
- **Concepts: 1b 1993.** Period-native bevels, the bar across the top with "◆ Desk", chunky 3D chrome, an icon column, VGA 8 × 16 type; a plum title bar on the active window. Shows Programs installing from A: (a checklist with sizes) and Settings › Display with three modes.
- **Concepts: 1c Dusk.** A hybrid: dark bevels, the bar at the bottom, an icon column, grotesk titles and VGA content, a three-step gradient rule under the focused title. Shows Notes, the System Inspector and the "Welcome back" toast after Lt. Fizz closes.
- **1d: the chosen mix.** "Void + Dusk bar": 1a's type, colours and grid with 1c's bevelled bar and swappable window borders; tweaks for border and wallpaper. SPEC's "Direction (1d)" is this board.
- **2a Architecture.** The layer diagram (apps as DXE3 modules, the `desk_*` SDK, the Rust core, the DJGPP C layer, DOS and CWSDPMI), the four-step handoff, what carries over from Lt. Fizz, the DXE3 risk, and a screen map (start-up check, desktop, Files, Programs, Settings, handoff and return, optional apps).
- **3a App anatomy.** Notes annotated with the six parts of a window (title bar and border, menu bar, toolbar, tabs, content, status bar) and a table of which app has a toolbar, tabs, a sidebar and a status bar. [`HIG.md`](HIG.md) is this board in prose.
- **4a 16 colours, same look.** The desktop in mode 12h with the Desk's own 16 colours and the swatch table with roles; [`PALETTE.md`](PALETTE.md)'s 16-colour set.
- **4b Keyboard only.** The focus ring, F10 on the menu bar, a confirm dialog, the Alt+Tab switcher overlay ("Hold Alt, Tab to move, let go to switch · Esc cancels") and Alt+Space keyboard move ("arrows move, Shift+arrows size, Enter drops").
- **4c When an app crashes.** Paint stops; its window closes; a toast offers "Not now" or "Reopen" from the autosave; Inspector logs one line.
- **4d Installing.** `INSTALL.EXE` in text mode with box-drawn panes and the graphical installer: five steps (Where, Mouse, Sound, Start at boot, Copy files), the `AUTOEXEC.BAT` change shown as a diff, `AUTOEXEC.BAK` kept.
- **4e Bitmap faces.** The four `.DFN` faces (title 12 px bold, UI 11 px, fixed 8 × 16, small 6 × 8) with sample text.

## The shell's parts

The prototype draws a 640 × 480 screen (800 × 600 at the larger mode). The parts below are read from the v4 page and its components; the screenshots in `design/shell-og/screenshots/` show each one.

### The desktop

- **The wallpaper** fills the screen above the bar. Five built in: Horizon (the default, a band of stepped colours along the bottom 150 px), Ember (stepped rings rising from the bottom centre), Dusk sky (stepped bands top to bottom), Plum (flat `#252038`) and Void (flat `#050507`). A picture from Paint or a `.BMP` can be used instead ("Your picture"), drawn to cover the screen. In 16 colours each built-in wallpaper has its own three-band version.
- **The icons** sit in a grid from the top left: 66 × 56 px cells, filled column by column, 7 rows at 640 × 480 and 9 at 800 × 600. In order: Files, Programs, Settings, Lt. Fizz, each installed optional app (except Jax), desktop shortcuts to DOS programs, whatever is in `C:\DESK\DESKTOP`, then C:, A: and Deleted. App icons are a 16 px glyph on a coloured tile; files use `FileIcon` at 32 px. A selected icon gets a `#583b1a` label background. Icons are not dragged to new places: dragging a file icon drags the file. When every app is installed the grid runs off the bottom of the screen at 640 × 480 (`v4-02-desktop-all-apps.png`).
- **Right-click on the desktop:** New folder on desktop, Paste, then Wallpaper, Window border and Display as submenus (each with a tick on the current choice), Show desktop folder, Settings…, Exit to DOS.
- **Right-click on an icon:** for an app, Open (bold) and Remove {app}… for optional apps; for a drive or Deleted, Open, Open in new window, Empty Deleted, Paste here, Properties; for a file, its open action (bold), Open with…, Use as wallpaper (pictures), Cut, Copy, Show in Files, Move to Deleted, Properties; for a DOS-program shortcut, Run full-screen, Program settings…, Show in Files, Remove desktop icon.

### The launcher bar

A 30 px bar along the bottom (`#14161d` with a `#333847` line on top), always above the windows.

- **Desk**, in bold orange grotesk on a raised button that looks pressed while the Desk menu is open.
- **One button per window**, in the order they were opened, up to 110 px wide with an ellipsis. The focused window's button is sunken with orange text; the others are raised. Clicking the focused window's button minimises it; clicking another focuses and restores it. Right-click gives the window menu.
- A spacer, then **Jax's mini dock** ("●‿● jax 🦦" in rose monospace; 🎉 when petted): click lets Jax out on the desktop, right-click gives Pet Jax, Let Jax out or Tuck Jax away, and Hide Jax.
- **The clock**, sunken: "Wed 11 Nov · 10:52 a.m." (weekday, day, month, then a 12-hour time with a.m. or p.m.). Double-click opens Clock; right-click gives Open Clock (or Install Clock…) and Copy date and time.

### The Desk menu

Opened by the Desk button, Ctrl+Esc or the Windows key; it rises from the bar's left end, 200 px wide with 22 px rows. A header reads "Afterglow Desk" in an orange-to-rose-to-purple gradient over "Programs on this PC". Then: Files and Terminal; a separator; Accessories, Entertainment and System as submenus, filled from the installed apps by group and sorted by name (Entertainment also lists Lt. Fizz); DOS programs as a submenu listing every program that has a `.DIF` or a desktop shortcut, with its file name on the right and "Add a DOS program…" (which opens Programs) last; a separator; Show Jax or Hide Jax (when Jax is installed); Settings; About Afterglow Desk; a separator; Exit to DOS. Every item has its app's icon in its colour. Arrows move, Right or Enter opens a submenu, Left closes it, Enter runs, Escape closes.

The menu is rebuilt each time it opens, so installing or removing an app changes it at once. The page also carries an older flat menu (`menuItems`) that the v4 bar no longer opens; it is read here as left over from v3.

### Windows and their chrome

- **Title bar**, 22 px, `#0c0c10`: the title in the grotesk face, white when focused and `#b4bfce` when not, then three 10 px control glyphs for minimise, maximise (restore when maximised) and close. The close box's hover is `#541824`, the others' `#1b1e28`.
- **Titles** name the app and what it shows: "Files · C:\FIZZ", "Notes · TODAY.TXT", "Paint · UNTITLED.BMP", "Audio Player · Cocoa Caper overture", "Terminal · C:\", "Properties · FIZZ.EXE".
- **The border** is one of six (below); the default is the stepped halo.
- **Move** by dragging the title bar; **double-click** it to maximise or restore; **right-click** it for the window menu: Maximise or Restore, Minimise, New Files window (Files only), Close (Alt+F4).
- **Resize** from three grips: the right edge, the bottom edge and the bottom-right corner (drawn as a small hatched corner). Maximised windows have no grips and fill the screen above the bar.
- **Minimised** windows leave only their bar button. Focus passes to the topmost window that is not minimised.
- **New windows** cascade 22 px down and right from (90, 14), at a size each app sets (Files 560 × 330 down to Mines 208 × 276), kept on the screen.
- Each app opens one window and focuses it if it is already open, except Files, which can have several.

### The window borders

From Settings › Windows, the desktop's Window border submenu, or the tweak. The focused window wears the chosen border; unfocused windows keep a quiet line.

| Border | Focused | Unfocused |
|---|---|---|
| Hairline | 1 px orange | 1 px `#262a37` |
| Gradient rule | 1 px `#333847`, plus a 2 px orange, rose and purple rule under the title | 1 px `#262a37` |
| Stepped halo (default) | Three rings outside the window: `#ffaa40`, `#7d5422`, `#372612` | 1 px `#262a37` ring |
| Dark bevel | Inner bevel lit orange, `#7d5422` shadow, 1 px `#050507` outline, 3 px padding | Inner bevel `#333847` and `#050507` |
| Chunky 3D | Two-step bevel in light orange and orange, browns in shadow, 4 px padding | Two-step grey bevel |
| Double line | 3 px double orange | 3 px double `#333847` |

### Menus, toolbars and tabs (the shared components)

- **`DeskMenuBar`**: 18 px, `#0c0c10`, menus as plain labels that open the Desk's menu on mouse-down beneath them.
- **Menus** (the one menu component for the Desk menu, menu bars and right-click): `#16161c` with a 1 px `#333847` outline, 20 px rows, a 10 px tick column (✓), an optional 12 px icon, the label, and a right-hand hint (a shortcut, or › for a submenu); separators; disabled items greyed and kept; the default item bold; the highlighted row orange on `#372612`. Menus keep inside the screen and open upwards near the bottom.
- **`DeskToolbar`**: 26 px, `#14161d`. Items are glyph buttons (20 × 18, a label when the glyph is unclear), 1 px separators, a spacer that pushes the rest right, and muted text. On: orange on `#372612`; disabled: `#333847`.
- **`DeskTabs`**: 22 px, `#0c0c10`. Tabs up to 140 px wide with an optional file icon, an orange square when unsaved, and a close box; the active tab matches the content and has a 2 px orange line on top; middle-click closes; `+` opens a new tab; tabs are drop targets. Section tabs have no close box.
- **Status bars**: 18 px, muted 10 px text, the subject on the left and the context on the right.

### Dialogs

All modal dialogs dim nothing (no alpha) but sit above everything on `#16161c`, wear the stepped halo, and are centred.

- **Confirm (`ask`)**: 260 px wide: a title naming the thing ("Move NOTES.TXT to Deleted?"), one line of consequence, a secondary button and the safe default in orange ("Move to Deleted" / **Keep**). Enter and Escape both dismiss it without acting (as read from the key handler).
- **Open with**: "Open README.TXT with", the installed apps that can open files (Notes, Audio Player, Paint), "Always use this for .TXT files" as a tickbox, Cancel and Open. With no app: "No installed program can open this. Programs can add one from A:."
- **The picker (`DeskPicker`)**: 440 × 316: a title, back, up, the path in a sunken field and "+ New folder"; places on the left (Desktop, the app's own places, then the drives); a list with Name, Size, Modified; a name field in upper case; an error line; type filters as chips ("Pictures", "All files"); Cancel and Open, Save or "Pick this folder". Save asks before replacing ("Replace SUNSET.BMP?", **Keep it**) and checks 8.3 names. Escape cancels.
- **About**: the app's tile, name and version, a one-line purpose, size and licence, and OK. The Desk's own: "Version 0.1 · SDK 1", "Rust + DJGPP · Apache-2.0 OR MIT · Liminal HQ 🍁".
- **File job**: a progress box for long copies and moves (title, the file now, a bar, bytes, Cancel); cancelling says "Stopped. Nothing was changed."

### Toasts

One at a time, bottom right (10 px from the edge, 40 px from the bottom, so above the bar), 240 px wide on `#16161c` with a raised bevel: a title in the grotesk face and one line. Four seconds, then gone; a new toast replaces the old. Reminders from Clock arrive as toasts ("Reminder · 17:30"), and so does "Welcome back" after a DOS program.

### Jax

The Liminal HQ otter, an optional 12 KB app that is a widget, not a window: the mini dock in the bar and a floating box (250 × 170) on the desktop with a mood (chill, or party for three seconds when petted). Jax stands still when Motion is off and never covers a dialog (from Settings' wording).

### The handoff

Running a DOS program fades the screen out (0.3 s, none when Motion is off) and replaces the desktop with a text screen: "DESK.COM: the Desk has closed and CWSDPMI has gone with it.", the free conventional memory, the command, the program's output, then "Press any key to return to the Desk". Lt. Fizz shows its own start-up check and game screens. Exit to DOS shows a DOS prompt ("Afterglow Desk has closed. Your windows are saved. Type DESK to start it again."). Coming back fades in and shows "Welcome back"; a Terminal tab that started the program is told it finished. Starting `DESK.COM` from inside the Desk only says "Already running".

### Colour depth and screen size

The tweak, the desktop's Display submenu and Settings › Display offer 640 × 480 in 256 colours ("The Afterglow look. Needs a VESA card."), 640 × 480 in 16 colours ("Works on any VGA card.") and 800 × 600 in 256 colours ("More room. Needs a VESA card with 1 MB."). In the prototype the right-click and the tweak switch at once (16 colours by remapping every colour to the nearest of the Desk's 16, with a toast "Now in 16 colours … nothing moved"), while Settings says the change takes effect when the Desk starts again.

## The apps

Every app is a component that talks to the shell only through one `ops` object (files, clipboard, toasts, menus, dialogs, the picker, settings, sound names, reminders, window titles). It has a menu bar whose Help menu ends with About, right-click menus, keyboard control while focused and no dialog is open, and the shared picker. Optional apps have sizes on the apps disk.

| App | Group, size | Menus | Toolbar, tabs, sidebar, status |
|---|---|---|---|
| Files | Core | File, Edit, View, Go, Help | Toolbar, document tabs, tree, status |
| Programs | System, core | File, Edit, View, Help | Toolbar, section sidebar, details pane, status |
| Settings | System, core | File, View, Help | Section tabs |
| Audio Player | Entertainment, 96 KB | File, Play, View, Help | Toolbar, library sidebar, status |
| Notes | Accessories, 52 KB | File, Edit, View, Help | Toolbar, document tabs, links pane, status |
| Paint | Accessories, 118 KB | File, Edit, View, Tools, Help | Toolbar, document tabs, palette sidebar, status |
| Calculator | Accessories, 21 KB | Edit, View, Help | History tape |
| Clock | Accessories, 30 KB | File, View, Help | Calendar sidebar |
| Inspector | System, 40 KB | File, View, Help | Section tabs, status |
| Terminal | System, 36 KB | File, Edit, Help | Tabs, status |
| Mines | Entertainment, 28 KB | Game, Help | Status |
| Jax | Widget, 12 KB | Right-click only | None |
| Properties | Part of the shell | None | A scrolling form with Close |

- **Files.** Browse, move and open everything on the drives, in tabs. Back, forward and up, a path field (Ctrl+L), a search field with wildcards ("Search *.MID", Ctrl+F), and three view switches (details, large icons, preview pane). Places (Desktop, Deleted) and a drive and folder tree on the left; a list with Name, Size, Modified and Attr; a preview pane with the icon, name, type and size, facts (for a program, its title and needs) and actions. Status: the selection and the drive's free space with a bar. Cut, copy, paste, rename (F2), Move to Deleted (Del), Properties (Alt+Enter), new tab (Ctrl+T), close tab (Ctrl+W), Alt+arrows for history, Backspace for up; Deleted can be restored from and emptied; hidden and system files on request; sort by name, size, date or type. Opening a file goes to its app; a program runs full-screen; anything else asks Open with.
- **Properties.** For a file, folder or drive: icon, name, type, location, size in bytes, modified; attributes as tickboxes (read-only, hidden, system, archive); for a drive, used and free with a bar; Opens with and Change…; for a program, Program settings from its `.DIF` (title, arguments, memory in KB, sound, wait for a key before returning) with Make a desktop icon and Save settings; Run full-screen and Close.
- **Programs.** Install and remove Desk apps, and set up DOS programs. Sections: Installed, On A: and DOS programs, with counts. A list with sizes ("Core" for the three core apps); a details pane ("Pick a program to see what it does."); toolbar Install, Remove, Run, program settings and "From a package…" (a `.DAP`). Ticking apps on A: and installing copies them with a progress line; removing asks first.
- **Settings.** Sections Desktop (the wallpapers as previews, "Use a picture…", "Show wallpapers in Files"), Windows (the six borders as previews), Display (screen mode, Motion: "Fade between the Desk and DOS programs, and let Jax move"), Sound (Desk sounds on the Sound Blaster, music on OPL3 or another device, "Play a test sound") and Jax (show in the bar, let out on the desktop). File has Reset look to defaults and Open `DESK.INI`.
- **Audio Player.** Music (`.MID` on the FM chip or MIDI out) and sounds (`.WAV`, `.VOC` on the Sound Blaster). Toolbar: previous, play or pause, stop, next, Loop (off, all, one), Shuffle, and two view switches (playlist, channels and levels). A library sidebar of music folders; the current track's title, file, device and time with a stepped progress bar; a playlist (Title, File, Time) with the current row cyan on `#0b2e35`; files dropped on the window join the playlist; `.M3U` playlists saved and opened. Keys: Space, N, P, S, Del. Called "MIDI Player" on the boards and in v1 and v2.
- **Notes.** Plain-text notes in tabs, saved as you type (Ctrl+S on a saved note says so). Toolbar New, Open, Save a copy as, Find, Replace, Wrap. A find and replace bar; `[[NAME]]` links to notes beside it, listed in a Links pane and opened from the right-click menu; Insert date; status with line, column, characters, links, wrap and path. Opens the text types it knows (`TXT`, `MD`, `INI`, `BAT`, `C`, `RS` and others).
- **Paint.** 16-colour pictures in tabs, 200 × 120 by default, saved as you draw once named (an untitled picture shows the orange square). Tools Pencil (P), Line (L), Rectangle (R), Fill (F), Pick colour (I); undo and redo; zoom from 1× to 6× (2× to start); the 16 colours as a palette sidebar; "Wallpaper" uses the picture as the wallpaper. Status: size, colours, zoom and save state.
- **Calculator.** Standard (a 4 × 5 keypad) and Programmer (32-bit; HEX, DEC, OCT, BIN rows showing the value in each base; AND, OR, XOR, NOT, shifts, MOD; digits A to F; keys outside the base disabled). A history tape on the right ("Results land here. Click one to use it again."). Keyboard: digits, operators, Enter, Backspace, Escape; copy and paste numbers with a toast when the clipboard is not a number.
- **Clock.** The time large, the date, a month calendar with today in orange and a dot on days with reminders; the chosen day's reminders ("Reminders show a quiet note at their time. No sound, no snooze."), each with a done box, time, text and remove; a new-reminder row (24-hour time and text). Arrows move by day and week, PgUp and PgDn by month, T for today. The empty state: "Nothing on this day. That's everything."
- **Inspector.** Sections System (processor, memory, video, mouse, sound, DPMI, drive C:), Memory, Drives (free space bars, file system, Properties) and Sound (test tone and test sample). Status: "Redraw 10 ms · 2 dirty rects" and when it was checked. Copy report and Save report…; F5 refreshes.
- **Terminal.** A command line in a window over the Desk's files, in tabs: HELP, CLS, VER, DATE, TIME, ECHO, MEM, CD, DIR, TREE, TYPE, MD, REN, DEL (to Deleted), RD, COPY, MOVE, XCOPY, OPEN or START, EXIT, with history and Tab completion. Typing a program's name runs it full-screen and the tab picks up after. Status: the folder and "Desk shell · DOS programs run full-screen".
- **Mines.** Beginner (9 × 9, 10 mines), Intermediate (16 × 16, 40) and Expert (30 × 16, 99); the first click is safe; right-click marks (question marks optional), double-click opens around a number; keyboard play; best times per level; F2 for a new game. The face is Jax's.
- **Lt. Fizz.** Not a Desk app: `C:\FIZZ\FIZZ.EXE`, a DOS program with a `.DIF`, shown on the desktop and in the Desk menu, which runs full-screen through the handoff.

## The visual system

The prototype is drawn with the Liminal HQ design system (`design/shell-og/_ds/`) and fixed colours inline.

- **Surfaces:** void `#050507`, chrome `#0c0c10` (title bars, menu bars, sidebars, status bars), content `#111116`, toolbar and launcher bar `#14161d`, menus, toasts and dialogs `#16161c`, raised `#1b1e28`, lines `#1f2230` and `#262a37`, outlines and bevel light `#333847`.
- **Text:** `#e0e0e0`, white for titles and focused text, secondary `#b4bfce`, muted `#7c8796`, faint `#5a6273`.
- **Roles:** accent `#ffaa40` (orange, with `#7d5422` and `#372612` as the halo's outer steps and `#ffd096` for hover), selection orange on `#372612`, current cyan `#22d3ee` on `#0b2e35`, links `#60a5fa`, primary buttons orange with near-black text, secondary buttons a 1 px `#333847` outline. Each app has a tint and a tile colour (Files orange, Programs purple, Settings grey, Audio Player cyan, Notes blue, Paint yellow, Calculator pale blue, Clock pink, Inspector and Terminal green, Mines and Lt. Fizz rose, Jax pink).
- **Sunken fields:** `#050507` with a 1 px inset bevel (`#050507` above and left, `#333847` below and right) and an orange caret.
- **Type:** Inter at 11 px for everything, Space Grotesk (600 or 700) for titles, headings and the Desk button, JetBrains Mono for paths, times, sizes and Terminal, and VT323 for the DOS screens. These are web stand-ins for the four bitmap faces of board 4e.
- **Rules:** no alpha and no blur; gradients are stepped bands; text keeps 4.5:1. The 16-colour set is [`PALETTE.md`](PALETTE.md)'s, and the prototype's 16-colour mode maps every colour on screen to it.

## Interactions

- **Mouse and keyboard together.** Ctrl+Esc or the Windows key opens the Desk menu; Escape closes menus, dialogs and the picker; arrows, Enter, Left and Right drive every menu. Apps take their own keys only while focused and no dialog is open.
- **Right-click everywhere**: the desktop, icons, title bars, window contents, bar buttons, the clock, Jax, rows, tabs and tree nodes. The default action is first and bold. Right-clicking an unselected row selects it.
- **Drag and drop** between Files' tabs, windows, the tree and the desktop; a ghost under the pointer says what will happen ("→ MUSIC", copy with Ctrl or across drives); dropping a file on an app's window gives it to that app.
- **Open by kind**: folders open in Files, programs run full-screen, documents go to the app that claims their type (or the one picked with "Always use this"), and anything else asks.
- **Calm**: no counts on icons, no alarm red, one toast at a time, autosave instead of Save, deletion to Deleted, questions that name the thing and default to the safe answer, empty states that say what goes there.
- **Keyboard-only use**: board 4b draws an Alt+Tab overlay, F10 for the menu bar, a 2 px focus ring and Alt+Space keyboard move and size. The v4 prototype does not implement these (see the differences below).

## What is built

The code is the shared core's first window and live input ([`FIRST-WINDOW.md`](FIRST-WINDOW.md), [`LIVE-INPUT.md`](LIVE-INPUT.md)), running on both tracks. Against the design:

| Part | Status | Evidence |
|---|---|---|
| The palette's 16-colour set and the Afterglow surfaces, both depths | Partly built | `desk-ui`'s `DESK16` and `AFTERGLOW_SURFACES`, the planar and linear presenters in `desk-present`, goldens at 8 and 4 bits (R20, R21). The hue ramps (slots 16 to 96) have no values yet |
| Window chrome: 22 px title bar, three control boxes, stepped halo, unfocused hairline | Built for the halo only | `desk-ui`'s `paint_chrome` (`TITLE_HEIGHT` 22, `BOX` 18, `HALO` 3) (R20). The other five borders are not built |
| Windows: z-order, focus, title-bar drag, close | Built | `desk-wm` (R20 to R24) |
| Minimise, maximise, resize | Not built | The boxes are drawn but inert ([`FIRST-WINDOW.md`](FIRST-WINDOW.md), out of scope) |
| Keyboard window management | Partly built | Alt+Tab with release-to-pick and no overlay, Shift+Alt+Tab, Alt+F4, Tab and Shift+Tab in `desk-wm` (R28); the overlay is R67; Alt+Space, F10 and the focus ring are not built |
| The cursor | Built | Drawn by the shell, with coalesced damage (R20, R27) |
| Widgets: label and one-line text field with a caret | Partly built | `desk-ui`'s `Label` and `TextLine` (R20). Buttons, lists, trees, tickboxes, menus, toolbars, tabs and status bars are not built |
| Fonts | Partly built | The four bitmap faces and Desk Sans, Mono and Serif (R1), read by `desk-font`; the first window uses the UI and title faces |
| Wallpaper | Partly built | Void only; Horizon and the other wallpapers need the ramps |
| Desktop icons, launcher bar, Desk menu, right-click menus, toasts, dialogs, the picker | Not built | Out of the first window's scope |
| Apps, Properties, the `ops`-like interface | Not built | The SDK is design only ([`SDK.md`](SDK.md)); making it real is R79; the module loader is R48; DXE3 loading was proved by R6 |
| Files, Open with, Deleted, drag and drop | Not built | Need a filesystem: the hosted track has DOS's; the native track's is R75 |
| The handoff and Exit to DOS | Not built | Hosted track, R61 ([`HANDOFF.md`](HANDOFF.md)) |
| 16-colour mode at run time, 800 × 600 | Partly built | Both depths render the same scene through two presenters (R21, R24); switching at run time and 800 × 600 are not built |
| The 1993 look, the installer, crash recovery | Not built | Later per [`SPEC.md`](SPEC.md) and [`OPEN-QUESTIONS.md`](OPEN-QUESTIONS.md) |

## Differences from the documents

Each difference quotes both sides briefly and says which the code follows today, where the code reaches that far.

1. **The clock's format.** SPEC: the date in `en-CA`, "Thu 22 Oct · 3:06". The v4 prototype: "Wed 11 Nov · 10:52 a.m.", a 12-hour time with a.m. or p.m.; boards 1a and 1d agree with SPEC, board 4a with the prototype ("3:31 p.m."). The code has no clock.
2. **Keyboard window management.** SPEC's input table: Alt+Tab with a switcher overlay, Alt+F4, Alt+Space, F10 or Alt for the menu bar, Tab with a focus ring. The v4 prototype handles only Ctrl+Esc, the Windows key, Escape and menu keys; Alt+F4 appears only as a hint in the window menu; the rest is on board 4b. The code follows SPEC in part: Alt+Tab (no overlay), Alt+F4 and Tab (R28).
3. **The Desk menu's contents.** SPEC: "Files and Terminal sit at the top; Accessories, Entertainment and System … DOS programs … then Settings, About and Exit to DOS." The prototype also has Show Jax or Hide Jax before Settings, lists Lt. Fizz under Entertainment, and carries the header "Programs on this PC". Not built.
4. **Desktop icon positions.** SPEC: "Positions are kept in `DESK.INI`." The prototype lays icons out in a fixed column-first grid and never moves them; a dragged icon is a dragged file. Not built.
5. **Custom wallpaper placement.** SPEC: "your own `.BMP` / `.PCX`, centred, tiled or stretched". The prototype offers one placement (the picture covers the screen) and gets its pictures from Paint or Files. Not built.
6. **A leftover Settings in the v4 page.** The `SettingsApp` component lists all three modes, as SPEC does ("VESA 800 × 600 × 8 · Optional in Settings"), but the v4 page still builds an older inline set of display options with only the two 640 × 480 modes; it is read here as left over from v3 and not shown on screen.
7. **When a display change takes effect.** Settings in the prototype: "Display changes next start … takes effect when the Desk starts again." The desktop's Display submenu in the same prototype switches at once with a toast. SPEC does not say. The code fixes the depth at start.
8. **The launcher bar's size and window-button behaviour.** SPEC lists what the bar holds but not its height or behaviour; the prototype's bar is 30 px, maximised windows stop above it, and clicking the focused window's button minimises it. HIG's anatomy table has no row for the bar. Not built.
9. **Resizing.** SPEC: windows "overlap, move, size, minimise to the bar and maximise". The prototype sizes from three grips only (right, bottom, bottom-right), not from the left or top. Not built; the code moves and closes only.
10. **Dialog keys.** HIG: destructive questions default to the safe button. The prototype's confirm draws the safe button as the primary, but Enter dismisses the dialog without acting rather than pressing that button (a reading of the key handler; the result is the same safe outcome).
11. **The file-copy progress dialog** (title, file, bar, bytes, Cancel) is in the prototype but not in SPEC or HIG.
12. **Program settings live in Properties.** SPEC says Programs can "set up" DOS programs; the prototype puts the `.DIF` form (title, arguments, memory, sound, wait for a key, desktop icon) in Properties, reached from Programs, Files, the desktop and the Desk menu.
13. **Lt. Fizz on the desktop.** SPEC: Lt. Fizz "ships on the apps disk as an ordinary DOS program". The prototype gives it a fixed desktop icon beside the core apps and a place in the Desk menu.
14. **Fonts.** SPEC: four bitmap faces. The prototype draws with Inter, Space Grotesk, JetBrains Mono and VT323 as stand-ins (board 4e says so). The code uses the bitmap faces (R1) and the modern families exist as outline fonts; which family the shell uses at which size is not decided by the design.
15. **The default wallpaper.** SPEC lists five wallpapers but no default; the prototype's default is Horizon. The code draws Void, because the ramps Horizon needs have no values yet ([`PALETTE.md`](PALETTE.md), [`FIRST-WINDOW.md`](FIRST-WINDOW.md) question 5).
16. **App names.** The boards and the first two prototypes say "MIDI Player", "Clock and Calendar" and "System Inspector"; v3, v4 and the documents say Audio Player, Clock and Inspector.
17. **Toast and menu surface.** PALETTE lists `#16161c` among the surfaces; the prototype uses it for menus, toasts and dialogs. The code loads it into slot 101 but draws nothing with it yet.
18. **Ctrl+Alt+Del.** SPEC: "in the Desk asks first, defaults to no". Not in the prototype.

## Open questions for the owner

1. **The clock:** "3:06" or "3:06 p.m."? And is the bar's date always shown, or only the time when space is short?
2. **The launcher bar:** is 30 px right for the bar at 640 × 480, and should HIG gain a row for it?
3. **Icon positions:** a fixed grid (the prototype) or free positions kept in `DESK.INI` (SPEC)? The prototype's grid overflows the screen when every app is installed.
4. **Custom wallpapers:** one placement, or centred, tiled and stretched as SPEC says?
5. **Display changes:** at once (the prototype's right-click) or at the next start (the prototype's Settings)?
6. **Keyboard model:** is board 4b (Alt+Tab overlay, F10, Alt+Space move and size, focus ring) still the target, and in what order relative to the launcher bar and menus?
7. **Resizing:** right, bottom and corner grips only, or every edge?
8. **Lt. Fizz:** a fixed icon on every desktop, or a shortcut the user makes?
9. **Jax:** core of the shell's look (it is in the bar of every board) or an optional app as SPEC and the prototype say?
10. **Which faces:** the four bitmap faces of board 4e, or Desk Sans and Desk Mono at the prototype's sizes (11 px UI, 12 px titles)?
11. **The older prototypes and drafts:** keep `design/shell-og/` beside `design/`, or replace `design/` with it, since it is a superset?
