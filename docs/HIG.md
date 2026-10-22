# Desk app guidelines

How a Desk app looks and behaves, so every app reads as one family. These come from Files, the first full app, and are what the other apps were rebuilt to. The anatomy board is in `design/Afterglow Desk.dc.html` (3a); the working examples are in `design/Afterglow Desk Prototype v4.dc.html`.

Same language, not the same features. Every app shares the parts below, but only takes the ones it needs. A calculator with a toolbar it doesn't need is worse, not more consistent.

## Anatomy

From the top of a window down:

| Part | Height | Who has it |
|---|---|---|
| Title bar (drawn by the Desk) | 22 px | Every window |
| Menu bar | 18 px | Every app |
| Toolbar | 26 px | Apps with frequent actions: Files, Notes, Audio Player, Paint, Programs |
| Tabs | 22 px | Apps with documents or sections: Files, Notes, Paint (documents); Inspector, Settings (sections, no close button) |
| Content | flexible | Every app. A sidebar (`#0c0c10`) on the left and a details pane on the right are optional |
| Status bar | 18 px | Only where it says something useful: Files, Notes, Paint, Audio Player, Programs, Inspector |

The Desk draws the title bar and the window's border; apps draw everything inside. Use the shared pieces (`DeskMenuBar`, `DeskToolbar`, `DeskTabs`, `DeskPicker`) rather than drawing your own.

## Menu bar

- Menus in this order, leaving out any an app has nothing for: **File, Edit, View**, then the app's own (**Go** in Files, **Play** in Audio Player, **Tools** in Paint), then **Help** last.
- **File** ends with *Close tab* (if the app has tabs) and *Close window*.
- **Help** always has *About {App}*, and nothing that needs a network.
- Items use sentence case; anything that opens a dialog ends in `…` ("Open…", "Save as…"). Shortcuts sit on the right, muted. Checkable items show `✓`.
- Disabled items stay in the menu, greyed out, so people learn where things live.

## Toolbar

- Only actions people use often, and every one also lives in a menu.
- Glyph buttons are 20 × 18. Add a short label when the glyph alone is unclear ("Find", "Shuffle", "Install").
- On state: orange text on `#372612`. Disabled: `#333847`, never hidden.
- Group with a 1 px separator. A spacer can push a group to the right (the view switches in Files and Audio Player).

## Tabs

- The active tab matches the content background and has a 2 px orange line on top. Others are muted.
- Document tabs show the file's icon and a close button; middle-click also closes. A small orange square means "not saved yet", which only an untitled Paint picture can be.
- `+` at the end opens a new tab. Section tabs (Inspector, Settings) have neither close nor `+`.
- Ctrl+Tab and Ctrl+Shift+Tab move between tabs; Ctrl+W closes one. Section tabs also answer to 1–9.

## Lists, trees and panes

- Rows are 18 px. Selected rows are orange on `#372612`; a playing or "current" row is cyan on `#0b2e35` (Audio Player).
- Sidebars hold places and folders (Files, the picker), a library (Audio Player) or sections (Programs).
- A details pane on the right (Files' preview, Programs' details) shows the selection's icon, name, a muted summary line, a few facts, then its actions as buttons, with the main one in orange.
- Empty states say what goes here and how: "The playlist is empty. Drop .MID files here, or double-click a track in the library." Always close with the caught-up state where there is one ("Nothing on this day. That's everything.").

## Status bar

Left: what you are looking at (counts, selection, line and column). Right: the context (drive free space, wrap on, loop mode). Muted text at 10 px; no buttons.

## Right-click menus

- Everything that can be acted on has one: rows, tabs, tree nodes, the desktop, desktop icons, title bars, launcher buttons, the clock, Jax, and inside every app.
- The default action is first and bold, and is what double-click or Enter does.
- Then, in order: open-like actions, clipboard (Cut, Copy, Paste), change (Rename, Delete), and **Properties** last.
- Right-clicking an unselected row selects it first.
- Text areas use the Desk's menu (Cut, Copy, Paste, Select all, then app extras), not the system's.

## Files in and out

- **Open and Save as** always go through the shared picker (`DeskPicker`): Desktop, the app's own places and the drives on the left; a type filter; an 8.3 name field. Saving over a file asks first, defaulting to *Keep it*.
- **Autosave.** Notes and Paint save as you type or draw. There is no Save command to forget; Ctrl+S on a saved document says so with a toast. Only a picture that has never been saved needs Save as.
- **Drag and drop.** Files dropped on an app's window go to that app: Notes opens them in tabs, Audio Player adds them to the playlist, Paint opens them as pictures. Dropping on a folder moves (same drive) or copies (other drive, or Ctrl held). The drag ghost says what will happen ("→ MUSIC", "+ copy to A:", "→ Notes").
- Apps open what they own from Files, and claim types with *Always use this* in *Open with…*.

## Dialogs

- Modal dialogs are rare: confirm (ask), Open with, the picker, About. They wear the stepped halo.
- Destructive questions name the thing and the consequence, and the default button is the safe one: "Move NOTES.TXT to Deleted?", *Move to Deleted* / **Keep**.
- Nothing is deleted outright from Files: it goes to Deleted, which can be restored from or emptied.
- About boxes show the app's tile, name, version, one-line purpose, size and licence, and an OK button.

## Keyboard

Every app is usable without a mouse.

| Keys | Everywhere |
|---|---|
| Ctrl+Esc | Desk menu |
| Esc | Close a menu, dialog, find bar or search |
| Ctrl+N, Ctrl+O, Ctrl+W | New, open, close tab |
| Ctrl+X, C, V, A | Clipboard and select all |
| Ctrl+Z, Ctrl+Y | Undo, redo (Paint) |
| Ctrl+F, F3, Ctrl+H | Find, find next, replace (Notes); search (Files) |
| Ctrl+Tab | Next tab |
| Arrows, Home, End, Enter | Move in lists, open |
| Delete, F2, Alt+Enter | Delete, rename, properties (Files) |
| Space | Play or pause (Audio Player), tick (Programs) |

Apps only act on keys while their window is focused and no dialog is open.

## Words

- Canadian English, sentence case for actions, title case only for app names.
- Say what happened and what to do next: "Not enough room on A:. That needs 935 KB; A: has 412 KB free."
- Toasts are one title and one line, four seconds, one at a time. No counts on icons, no red for ordinary errors.

## Colour roles

Apps use roles, not palette slots (`SDK.md`): content `#111116`, chrome `#0c0c10`, toolbar `#14161d`, fields `#050507` sunken, lines `#1f2230` and `#262a37`, text `#e0e0e0`, muted `#7c8796`, accent `#ffaa40`, selection `#372612`, current `#0b2e35` and `#22d3ee`, links `#60a5fa`. The primary button is orange with near-black text; secondary buttons are a 1 px `#333847` outline.
