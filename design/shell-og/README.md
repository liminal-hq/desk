# The original shell design

The full shell design that Desk is working towards, captured as it stood on 2026-11-11 from the owner's design canvas, a claude.ai artifact: <https://claude.ai/artifact/63Qww66fRXY2yE6TVAVgxe>. The canvas is a Claude Design project ("DOS Project Brainstorm") moved to claude.ai; its page list is `compat.json`, which names the original project and its entry page, `Afterglow Desk Prototype v4.dc.html`.

This folder is reference only, like the rest of `design/`: the files were produced by a design agent without access to the code, they are not built or tested here, they carry no licence headers, and the code never imports from them. `docs/SHELL-DESIGN.md` describes what the design holds and how it compares with the documents and the code.

## What is here

Every file under the canvas's `project/` folder, unchanged, at the same relative paths, so the pages open in a browser from this folder (they load `support.js`, `_ds/`, `site/img/` and, from the internet, React 18.3.1, Babel and Google Fonts). The editor's own files (`index.html`, `SKILL.md`, `artifact-type/`) belong to the artifact type and are left out.

| File | What it is |
|---|---|
| `Afterglow Desk Prototype v4.dc.html` | The working prototype and the canvas's entry page: the desktop, the launcher bar, the Desk menu, windows, dialogs and every app. The link the owner shared points at this artboard (`#artboard-6b84bc2503e7` is the editor's hash of its file name) |
| `Afterglow Desk Prototype v3.dc.html`, `v2`, `Afterglow Desk Prototype.dc.html` | Earlier prototypes: v3 (the one the docs were first written against), v2 (Files and Properties as components, drag and drop, file jobs) and the first prototype (every app drawn inline) |
| `Afterglow Desk.dc.html` | The boards: Concepts (looks 1a Void, 1b 1993, 1c Dusk), 1d (the chosen mix), Architecture (2a), App anatomy (3a) and Before the repo (4a to 4e) |
| `Desk*.dc.html`, `FileIcon.dc.html` | The shared components: `DeskFiles`, `DeskProperties`, `DeskPicker`, `DeskMenuBar`, `DeskToolbar`, `DeskTabs` and the file-type icons |
| `CalcApp`, `ClockApp`, `InspectorApp`, `MidiApp`, `MinesApp`, `NotesApp`, `PaintApp`, `ProgramsApp`, `SettingsApp`, `TerminalApp` (`.dc.html`) | One component per app; each talks to the shell only through one `ops` object |
| `SPEC.md`, `docs/`, `repo/README.md`, `github.md` | The brainstorm's own drafts of the spec, SDK, formats, handoff, palette, guidelines and open questions, and its sketch of the repository. They are older than the documents in the repository's `docs/`, which supersede them |
| `_ds/` | The Liminal HQ design system the pages were drawn with (tokens, fonts, colours, icons, Jax) |
| `compat.json` | The canvas's page list and the record of its move from Claude Design |
| `site/img/`, `uploads/` | The Lieutenant Fizz screens the handoff shows, and a sketch |
| `screenshots/` | The brainstorm's own screenshots (the `.jpg` files and `files-resize.png`) and the captures listed below |

Apart from the four files listed next, every page and component here is byte-identical to its copy in `design/`: v4, v3, the boards, the components, `_ds/`, `support.js`, `image-slot.js` and the images. New to this capture: `Afterglow Desk Prototype.dc.html`, `Afterglow Desk Prototype v2.dc.html`, `compat.json` and the brainstorm's markdown drafts. `design/_ds/` also holds one file the canvas does not (`_adherence.oxlintrc.json`).

## Screenshots

Captured on 2026-11-11 by opening the pages in headless Chromium from this folder. The prototype's screens were reached by setting its state directly (installing every optional app, opening one app at a time, opening menus and dialogs), so they show what the prototype draws, not a recorded session. The clock shows the capture's date and time. Screens of the prototype are 640 × 480 shown at 1.5× (960 × 720), except where noted.

| Files | What they show |
|---|---|
| `boards-1-4-before-the-repo.png` to `boards-5-architecture.png` | The five sections of the boards page, one image each |
| `proto-v1-desktop.png`, `proto-v2-desktop.png`, `proto-v3-desktop.png` | The earlier prototypes as they open (shown at 1.75×) |
| `v4-01-desktop.png`, `v4-02-desktop-all-apps.png` | The v4 desktop as it opens, then with every optional app installed |
| `v4-03-desk-menu.png` to `v4-05-desk-menu-dos-programs.png` | The Desk menu, and its Accessories and DOS programs submenus |
| `v4-06-desktop-menu-borders.png` | The desktop's right-click menu with the Window border submenu |
| `v4-app-*.png` | Each app's window as it opens: Files, Programs, Settings (and its Windows, Display, Sound and Jax sections), Audio Player (`midi`), Notes, Paint, Calculator, Clock, Inspector (and its Memory, Drives and Sound sections), Mines, Terminal, and Properties for a program |
| `v4-dialog-*.png`, `v4-toast.png` | The confirm dialog, About, Open with, the Save picker and a toast |
| `v4-desktop-busy.png` | Four windows at once |
| `v4-wallpaper-*.png`, `v4-border-*.png` | The five wallpapers, and the six window borders on two windows |
| `v4-depth-16-colours.png`, `v4-depth-800x600.png` | The 16-colour mode (shown at 1.75×) and 800 × 600 (shown at 1.2×) |
| `v4-handoff-exit-to-dos.png`, `v4-handoff-fizz-check.png` | Exit to DOS, and running `FIZZ.EXE` full-screen |

Not captured as images: each component page on its own (they draw little without the prototype's `ops` object, and every one is shown inside a v4 window instead), drag and drop, the file-copy progress dialog, the Programs install flow and Jax's floating box.
