# Docs

Working design documents for Desk, derived from a design-agent brainstorm that had no code to check against. The design in these documents has not been built or measured; only the toolchain has (see `TOOLCHAIN.md`). Numbers, versions and formats are targets until a spike confirms them, and when code and a document disagree, the code wins and the document is corrected in the same branch.

"Fizz DOS" below means [`ScottMorris/lieutenant-fizz-dos`](https://github.com/ScottMorris/lieutenant-fizz-dos), the Rust-on-DOS port this project learns from. Its code is a reference, not something to copy.

| Document | What it covers |
|---|---|
| [`SPEC.md`](SPEC.md) | What Desk is: goals, targets, display, input, the desktop, the apps, the handoff, packaging and milestones |
| [`SDK.md`](SDK.md) | The C ABI the core would export to apps |
| [`FORMATS.md`](FORMATS.md) | `APP.INF`, `.DAP`, `CATALOG.INF`, `.DIF`, `.ICO`, `DESK.INI`, `DESK.RUN`, `.DFN` and the others |
| [`HANDOFF.md`](HANDOFF.md) | How the Desk steps aside to run a DOS program with the whole machine and comes back |
| [`PALETTE.md`](PALETTE.md) | The 256-colour layout and the 16-colour fallback |
| [`HIG.md`](HIG.md) | The design language every app follows |
| [`OPEN-QUESTIONS.md`](OPEN-QUESTIONS.md) | What was decided before the repo existed, and what is still open |
| [`TOOLCHAIN.md`](TOOLCHAIN.md) | How Desk is built for DOS and checked headless: the Rust target, the object converter, the C glue, DOSBox, and what was verified |
| [`NAMING.md`](NAMING.md) | The working name and the candidates |
| [`FONTS.md`](FONTS.md) | The four bitmap faces: metrics, grid conventions, adding a glyph and building |

The prototypes and boards the documents point at are in [`../design/`](../design/), kept as reference only.
