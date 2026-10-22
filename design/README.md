# Design reference

The boards and prototypes from the design-agent brainstorm that preceded the code. They are reference only: they were produced without access to a code base, they are not built or tested, they carry no licence headers, and the code never imports from them. When the code is written, the code is the source of truth.

Open the `.dc.html` files in a browser from this folder; they load `support.js`, `image-slot.js`, `_ds/` and `site/img/` by relative path, so keep the folder together.

| File | What it is |
|---|---|
| `Afterglow Desk.dc.html` | The boards: looks (1a to 1d), architecture (2a), app anatomy (3a) and the design gaps (4a to 4e) |
| `Afterglow Desk Prototype v4.dc.html`, `v3` | The working prototype, latest and the version the docs were written against |
| `Desk*.dc.html`, `FileIcon`, and one file per app (`CalcApp`, `ClockApp`, `InspectorApp`, `MidiApp`, `MinesApp`, `NotesApp`, `PaintApp`, `ProgramsApp`, `SettingsApp`, `TerminalApp`) | The prototype's components; each app talks to the shell through one `ops` object |
| `_ds/` | The Liminal HQ design system the prototype was drawn with (tokens, fonts, colours) |
| `screenshots/`, `uploads/`, `site/img/` | Screenshots and a sketch the boards use |

"Afterglow Desk" is the brainstorm's name for the project. The working name is now "Desk" (see `docs/NAMING.md`); the file names here keep the original.
