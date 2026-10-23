# Docs

Working design documents for Desk, an operating system in Rust for old hardware, built hosted on DOS and native in parallel (see `DIRECTION.md`). They were derived from a design-agent brainstorm that had no code to check against. The design in these documents has not been built or measured; only the toolchain, the boot path and the first shared drivers have (see `TOOLCHAIN.md`, `NATIVE-BOOT.md` and `HAL.md`). Numbers, versions and formats are targets until a spike confirms them, and when code and a document disagree, the code wins and the document is corrected in the same branch.

"Fizz DOS" below means [`ScottMorris/lieutenant-fizz-dos`](https://github.com/ScottMorris/lieutenant-fizz-dos), the Rust-on-DOS port this project learns from. Its code is a reference, not something to copy.

| Document | What it covers |
|---|---|
| [`DIRECTION.md`](DIRECTION.md) | The direction: an operating system in Rust for old hardware, the hosted and native tracks, the shared core, FreeDOS, test matrix, open questions and milestones |
| [`SPEC.md`](SPEC.md) | The detailed spec of the hosted track and the OS-independent parts: goals, targets, display, input, the desktop, the apps, the handoff, packaging and milestones |
| [`SDK.md`](SDK.md) | The C ABI the core would export to apps |
| [`FORMATS.md`](FORMATS.md) | `APP.INF`, `.DAP`, `CATALOG.INF`, `.DIF`, `.ICO`, `DESK.INI`, `DESK.RUN`, `.DFN` and the others |
| [`HANDOFF.md`](HANDOFF.md) | How the Desk steps aside to run a DOS program with the whole machine and comes back |
| [`PALETTE.md`](PALETTE.md) | The 256-colour layout and the 16-colour fallback |
| [`HIG.md`](HIG.md) | The design language every app follows |
| [`OPEN-QUESTIONS.md`](OPEN-QUESTIONS.md) | What was decided before the repo existed, and what is still open |
| [`RUST-TARGETS.md`](RUST-TARGETS.md) | How Rust is built for each application space: target specifications, `core`, `alloc` and `std`, and the spikes that track them |
| [`TOOLCHAIN.md`](TOOLCHAIN.md) | How Desk is built for DOS and checked headless: the Rust target, the object converter, the C glue, DOSBox, and what was verified |
| [`S1-DXE3.md`](S1-DXE3.md) | The result of spike 1: Rust modules as DXE3 files loaded by the core, the commands that work, panics and faults, memory, and what it changes |
| [`D0-RUST-DOS.md`](D0-RUST-DOS.md) | Spike result: a DOS program in pure Rust with no C layer, its runtime contract, the `rt` crate, the packer, costs and limits |
| [`HAL.md`](HAL.md) | Spike H: the hardware abstraction layer and the shared drivers, the platforms, the concurrency and `unsafe` rules, the migration plan and the contracts for the next spikes; the reference crates are in `hal/` |
| [`A-VGA-VBE.md`](A-VGA-VBE.md) | Spike A result: VGA mode 13h and 12h by register programming, the DAC and fades, VBE 2.0 through a `Bios` trait, the DOS platform and the program that proves them in DOSBox, with screenshots, register tables and what native must implement |
| [`C-INPUT.md`](C-INPUT.md) | Spike C: the keyboard and PS/2 mouse in the native kernel, the 8042 tables and set-up sequences, the event contracts, the interrupt hand-off, the QEMU input tests and what real hardware may do differently |
| [`NATIVE-BOOT.md`](NATIVE-BOOT.md) | The native track's boot path and Rust kernel skeleton: boot flow, image layout, target, memory, interrupts and how QEMU checks it |
| [`NAMING.md`](NAMING.md) | The working name and the candidates |
| [`FONTS.md`](FONTS.md) | The four bitmap faces: metrics, grid conventions, adding a glyph and building |

The prototypes and boards the documents point at are in [`../design/`](../design/), kept as reference only.
