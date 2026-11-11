---
id: shelldesign
n: 21
title: The owner's shell
date: 2026-11-11
summary: The original design of the whole shell is captured into the repository and read against the code: a launcher bar, menus, a dozen apps and an installer on paper, one window and a cursor in software, and a roadmap of forty-seven items between them.
prs: [68, 69]
---
Chapter 0 described the donation that started the archive: a design brainstorm in a zip. This chapter is about the day it was catalogued properly. On 11 November the owner's design canvas, a browser prototype that a design agent wrote before any code existed, was captured into `design/shell-og/` with a screenshot of every board, app and dialog, and `docs/SHELL-DESIGN.md` (#68, {{pr68.merge}}) describes it part by part and compares it with the documents and with the code. A second pull request, #69 ({{pr69.merge}}), turned every unit of the design into a roadmap item, R84 to R130.

**What the design holds.** A 640 × 480 desktop (and 800 × 600, and sixteen colours) with a wallpaper and a grid of icons; a 30 px launcher bar with a Desk menu, one button per window, a mini dock for Jax the otter and a clock; windows with a 22 px title bar, three control boxes and one of six borders (the default a stepped three-ring orange halo); menus, toolbars, tabs and status bars shared by every app; and the apps themselves, thirteen entries in the document's table: Files, Programs, Settings, Audio Player, Notes, Paint, Calculator, Clock, Inspector, Terminal, Mines, Jax and Properties. The canvas has 22 pages, five of them full pages and seventeen components; the boards cover concepts, the chosen mix, the architecture, app anatomy, sixteen colours, keyboard-only use, a crashing app, the installer and the bitmap faces. It is a superset of what `design/` already held, and the documents (`SPEC.md`, `HIG.md`, `PALETTE.md`) were written from it, so most of it agrees with them. The document lists eighteen places where it does not, and eleven questions for the owner.

<!-- figure repo-images img-sd-desktop.png img-sd-menu.png img-sd-halo.png -->

> **From the stacks — Alt+Tab.** The design's board 4b draws a switcher overlay for it: "Hold Alt, Tab to move, let go to switch · Esc cancels". Wikipedia says the shortcut "has been in Microsoft Windows since Windows 1.0 (1985)", so the Desk is borrowing a habit that is older than almost any hardware it targets. The first window ships the habit without the overlay: release picks the window (chapter 19); the overlay is roadmap item R67.
> Source: [Alt-Tab, Wikipedia](https://en.wikipedia.org/wiki/Alt-Tab)

**What is built against it.** The document's own table, which I have checked against the roadmap rather than the code (I did not run anything for this chapter):

| Part of the design | Where it stands |
|---|---|
| Windows with z-order, focus, title-bar drag and close | Built (`desk-wm`; chapters 18 and 19) |
| The stepped halo and the unfocused hairline | Built; the other five borders are not |
| The cursor, a label and a one-line text field | Built |
| Alt+Tab (no overlay), Shift+Alt+Tab, Alt+F4, Tab | Built (chapter 19); Alt+Space, F10 and the focus ring are not |
| Palette's 16-colour set, both presenters | Partly built; the colour ramps have no values yet, so the wallpaper is Void, not Horizon |
| Minimise, maximise, resize | Drawn boxes, inert |
| Launcher bar, Desk menu, right-click menus, toasts, dialogs, the picker | Not built |
| Every app, Properties, the shell's `ops` interface | Not built; the SDK is design only, the module loader is R48 and the app model R79 |
| Files, Open with, Deleted, drag and drop | Not built; they need a filesystem (R75 on the native track) |
| The handoff and Exit to DOS | Not built (R61, hosted track) |

**The roadmap grows.** `docs/ROADMAP.md` now lists 140 items. R84 to R130 are the full shell and apps: forty-seven items: one done (R127, the transcript fix of chapter 20), eight in progress (parts that have some merged code) and thirty-eight planned. They fall into shared widgets and dialogs, the shell's parts, the hosted handoff and installer, the four core apps, the optional apps, two design proposals for the owner (R125 and R126: a path and mount model, and shared commands) and the four findings of chapter 20. The chart below is counted from the file itself when this page is built, so it moves when the roadmap does.

<!-- figure roadmap-status -->

**An honest reading of the numbers.** The roadmap's `done` column is the foundations, the spikes, the native track, the first window, live input and the Manual, with the findings fixed since: fifty things, each merged, each with its limits stated beside it. The long planned bars are the distance between a window and a desktop. None of the planned work depends on a pull request being open; it depends on the app model, a filesystem and a loader, which the roadmap puts first (R48, R125, R126, R74, R75, R77, R79, R80).

## The shelves

### Catalogued
- The capture: `design/shell-og/` holds the pages, components and screenshots, and `docs/SHELL-DESIGN.md` describes them (#68, {{pr68.merge}}). The document says every page and component already in `design/` is byte-identical to its copy in the canvas.
- The counts above are read from `docs/ROADMAP.md` by the build, which refuses a row with an unknown status or a repeated id.

### Uncatalogued
- Everything the design describes. It is a browser prototype: nothing in it was built, measured or drawn on a 486, and the screenshots above were rendered by a browser with web fonts at 1.5 times the 640 × 480 screen, not by the Desk.
- The document's reading of the prototype's code where it is marked uncertain, and its statement of what the code has built, which I took from the roadmap and did not re-derive.

### Wish list
- All of R84 to R124 (the shell's parts and the apps), after the app model and the filesystem; the eleven questions for the owner, from the clock's format to which bitmap faces the shell uses.
- The 1993 look, the installer and crash recovery, which `SPEC.md` places later.

```figures
id: design-shots
kind: screenshot
track: design
title: The original shell design
alt: Three screenshots of the owner's browser prototype: the desktop with a Files window over an Audio Player and a bar along the bottom, the Desk menu open with its submenus, and the stepped halo border on a Calculator window.
caption: The prototype's desktop, Desk menu and halo border, from design/shell-og/screenshots.
at: figure repo-images img-sd-desktop.png img-sd-menu.png img-sd-halo.png
thumb: img:img/img-sd-desktop.png
date: 2026-11-11

id: roadmap-status
kind: chart
track: shared
title: The roadmap by section and status
alt: Stacked bars of the 140 roadmap items by the section they sit in, split into done, in progress, planned, idea and blocked.
caption: Counted from docs/ROADMAP.md.
at: figure roadmap-status
thumb: svg
date: 2026-11-11
```
