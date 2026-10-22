# Desk

<p align="center">
  <img src="assets/hero.svg" alt="Desk — a pixel-art desktop with a Files window behind a focused Terminal window, a launcher bar and a pointer, beside the name and a DOS prompt running DESK.COM" width="100%">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/status-design%20and%20spikes-f97316" alt="Status: design and spikes">
  <img src="https://img.shields.io/badge/platform-DOS%20(VESA%2C%20VGA)-1f6feb" alt="Platform: DOS, VESA and VGA">
  <img src="https://img.shields.io/badge/licence-Apache--2.0%20OR%20MIT-3fb950" alt="Licence: Apache-2.0 OR MIT">
</p>

A calm graphical operating system in Rust for old hardware, from Liminal HQ: windows, files, programs and a few small apps on a 486-class PC with VGA or VBE graphics, Sound Blaster and OPL sound, a PS/2 keyboard and mouse, and floppy and IDE storage. It is built on two tracks in parallel that share one OS-independent core behind a small hardware abstraction layer: a **hosted track**, Desk running on top of DOS (DJGPP, CWSDPMI, tested in DOSBox), for fast iteration, and a **native track**, a bootable Rust kernel tested in QEMU, 86Box and Bochs. [`docs/DIRECTION.md`](docs/DIRECTION.md) has the plan.

It is the spiritual successor to [Lieutenant Fizz for DOS](https://github.com/ScottMorris/lieutenant-fizz-dos), which proved that Rust can drive a real DOS machine. This project starts from scratch with what that port taught us, and its toolchain is written fresh for this project rather than copied.

> **Status:** design, a toolchain skeleton, and fonts and emoji work. Nothing boots yet and no native code exists. [`docs/SPEC.md`](docs/SPEC.md) is the working design of the hosted track and the OS-independent parts, written before any code, so its numbers and formats are targets until a spike confirms them; the native track is described in [`docs/DIRECTION.md`](docs/DIRECTION.md). "Desk" is a working name; [`docs/NAMING.md`](docs/NAMING.md) has the candidates.

## What it will be

- **A richer place than the prompt.** Move between files and programs with a mouse or the keyboard, run several small apps side by side, and come back to where you were.
- **Small core, everything else optional.** The shell, Files, Programs and Settings are the core; every other app installs and removes on its own.
- **Runs DOS programs.** Desk apps run in windows. In the hosted track any `.COM`, `.EXE` or `.BAT` runs full-screen with the whole machine, then returns to the Desk ([`docs/HANDOFF.md`](docs/HANDOFF.md)). The native OS is planned to bundle FreeDOS for this, as a later milestone.
- **An operating system, not only a shell.** The native track is a Rust kernel with its own drivers, filesystem and module loader, so Desk can run on bare metal.
- **An SDK for apps.** Independent modules that talk to the Desk through one C ABI ([`docs/SDK.md`](docs/SDK.md)).
- **Calm.** Nothing badgers you: no unread counts, no alarm red, quiet toasts that time out.

## First milestones

**Hosted track** (on DOS)

1. **Spikes** that decide the architecture ([`spikes/`](spikes/)): a Rust module loaded into the core; VBE 2.0 and VGA mode 12h from Rust; the mouse; the handoff with a tiny real-mode program.
2. **Shell:** compositor, windows, launcher bar, cursor, keyboard focus.
3. **Handoff:** run a DOS program and come back.
4. **Core apps:** Files, Programs, Settings.

**Native track** (bare metal)

1. **n0 boot:** a Rust kernel that boots in QEMU.
2. Interrupts, memory, graphics and input, then a disk, a filesystem and a module loader.
3. The shared Desk core running on the native hardware layer.
4. Bundled FreeDOS for DOS programs.

The hosted list is in [`docs/SPEC.md`](docs/SPEC.md#milestones), the native one in [`docs/DIRECTION.md`](docs/DIRECTION.md#milestones), and [`docs/OPEN-QUESTIONS.md`](docs/OPEN-QUESTIONS.md) says what is decided and what is not.

## Repository

| Folder | What it holds |
|---|---|
| `docs` | The working design: direction, spec (the hosted track's), SDK, file formats, handoff, palette, app guidelines, open questions, naming |
| `design` | Boards and prototypes from the design brainstorm, kept as reference only |
| `core` | The OS-independent shell in Rust (empty until the spikes land) |
| `c` | The hosted track's hardware layer in C for DJGPP (empty until a spike needs it) |
| `launcher` | `DESK.COM`, the hosted track's real-mode launcher (empty until the handoff spike) |
| `sdk` | The app interface (empty until the first spike settles how apps load) |
| `spikes` | Experiments that decide the architecture |
| `tests/dos` | Test programs of our own for the headless DOS runs |
| `tools` | Repository checks and the banner builder; the DOS toolchain scripts arrive with the first spike |

## Working on it

```bash
tools/check.sh    # licence headers, markdown style and links: everything the CI runs today
python3 -I tools/build-hero.py > assets/hero.svg    # rebuild the banner
```

[`AGENTS.md`](AGENTS.md) has the conventions: Canadian English, unwrapped markdown, Conventional Commits, merge commits only.

## Licence

Apache-2.0 OR MIT (`LICENSE-APACHE`, `LICENSE-MIT`). In the hosted track CWSDPMI and JEMMEX will ship beside the Desk as separate programs under their own licences, and the native track plans to bundle FreeDOS, whose licensing is still to be checked; see `NOTICE`.
