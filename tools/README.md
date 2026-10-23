# tools

Scripts for working on the repository: the repo checks, the DOS build pipeline and the hero-banner builder. `docs/TOOLCHAIN.md` explains the pipeline.

| Script | What it does |
|---|---|
| `check.sh` | Runs every check the CI runs: headers, docs, the font build check in `fonts/` when Bun is set up, Rust format and lints, the DOS build, headless runs of `DESK.EXE`, the s1-dxe3 module scenarios, the d0-rust-dos checks, and the native boot test |
| `setup.sh` | Checks host tools, installs the pinned Rust pieces, and links `vendor/` to `DOS_TOOLCHAIN` or downloads the tools |
| `fetch-tools.sh` | Downloads DJGPP, DOSBox Staging and CWSDPMI into `vendor/` with SHA-256 checks, using the versions of the CI image |
| `build.sh` | Cargo for `core/dos.json`, `coff2go32.py`, DJGPP's gcc for `c/`, then `dist/DESK.EXE` beside `CWSDPMI.EXE` |
| `coff2go32.py` | Converts rustc's COFF objects so DJGPP's linker lays them out and relocates them correctly |
| `dosrun.sh` | Runs a DOS command in headless DOSBox Staging; prints its output and returns its exit status |
| `check-headers.sh` | Checks the licence header on source and script files |
| `check-docs.py` | Checks the markdown house style and that relative links resolve |
| `build-hero.py` | Builds `assets/hero.svg`, the README banner |
| `native-run-bochs.sh`, `native-test-bochs.sh` | The same image in headless Bochs built as a 486, in Docker (`docker/Dockerfile.native-bochs`): the runner watches the serial transcript because Bochs has no exit device, and the test reruns `native-test.sh` against it plus instruction probes (`../tests/native/probe.asm`). Opt-in: `NATIVE_EXTRA=1 tools/check.sh` (see [`../docs/NATIVE-EMULATORS.md`](../docs/NATIVE-EMULATORS.md)) |
| `native-run-86box.sh`, `native-test-86box.sh` | The same image in 86Box on an AMIBIOS 486 machine, headless on Xvfb in Docker (`docker/Dockerfile.native-86box`, config in `86box/`); experimental and slow, with a screenshot kept in `dist/` |
| `native-build.sh`, `native-image.sh`, `native-run.sh`, `native-test.sh` | The native track: build the boot stages and the Rust kernel, assemble the floppy image, boot it headless in QEMU as a 486, and check the transcript and the negative cases (see [`../docs/NATIVE-BOOT.md`](../docs/NATIVE-BOOT.md)); `native-check.py` and `elf2bin.py` are their helpers |
