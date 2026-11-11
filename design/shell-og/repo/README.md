# Afterglow Desk

> The place you come to after DOS boots.

A graphical shell for MS-DOS from Liminal HQ: windows, files, programs and a few calm apps on a 486, and any DOS program a double-click away (it gets the whole machine, then the Desk comes back). Rust for everything above the hardware, C for the hardware, DJGPP and CWSDPMI underneath, the way Lieutenant Fizz for DOS is built.

**Status:** design and spikes. Nothing runs yet. `docs/SPEC.md` is the reference; `docs/OPEN-QUESTIONS.md` says what is decided and what isn't.

## Layout

```
core/            Rust (no_std + alloc): window manager, compositor, palette, event loop, module loader, handoff
  dos.json       the i486 target, from lieutenant-fizz-dos
c/               DJGPP C: VBE 2.0 + mode 12h, INT 33h mouse, IRQ 1 keyboard, timer, OPL3/OPL2, SB16 DMA, MPU-401, files, dlopen
launcher/        DESK.COM: the real-mode launcher and handoff (and JEMMEX loading for EMS)
sdk/
  include/desk.h the C ABI apps use
  rust/          desk-sdk crate: safe wrappers over desk.h
apps/<id>/       one folder per app: src/, APP.INF, icons (files, programs, settings, term, audio, notes, paint, calc, clock, inspect, mines, jax)
install/         INSTALL.EXE (text mode) and the graphical installer
assets/
  fonts/         the four .DFN faces and their sources
  icons/         file-type and app icons (16 and 32 px, palette slots 128–223)
  wallpapers/    HORIZON.BMP, EMBER.BMP, DUSK.BMP, PLUM.BMP
tests/dos/       TONETEST.EXE, REALMODE.COM, NEEDEMS.EXE and scripted input
tools/           coff2go32.py, chkstk.c, dosrun.sh, dap pack and unpack, font and icon exporters, make-floppy.sh
docs/            SPEC, SDK, FORMATS, HANDOFF, PALETTE, HIG, OPEN-QUESTIONS
dist/            built floppies and zips (not committed)
```

## Build

On `ghcr.io/liminal-hq/ci-dos` (Rust 1.98.0, DJGPP 12.2, DOSBox Staging 0.83, CWSDPMI, Bun):

```bash
tools/setup.sh                 # check tools, fetch CWSDPMI and JEMMEX into vendor/ with checksums
tools/build.sh                 # core + C layer + launcher + apps → dist/desk/
tools/check.sh                 # frame checksums at 256 and 16 colours, handoff round trip, EMS run
tools/make-floppy.sh           # -core.img and -apps.img (1.44 MB each)
tools/dosrun.sh dist/desk "DESK.COM"   # run headless in DOSBox
```

## First milestones

1. **Spikes**: a Rust DXE3 module that calls back into the core; VBE 2.0 LFB and mode 12h from Rust; INT 33h; the DESK.COM handoff with `REALMODE.COM` and `NEEDEMS.EXE`.
2. **Shell**: compositor, windows, launcher bar, cursor, keyboard focus, the 16-colour fallback.
3. **Core apps**: Files, Programs, Settings.

## Licence

Apache-2.0 OR MIT. CWSDPMI and JEMMEX ship beside the Desk as separate programs under their own licences; see `NOTICE`.
