# tools

Scripts for working on the repository: the repo checks, the DOS build pipeline and the hero-banner builder. `docs/TOOLCHAIN.md` explains the pipeline.

| Script | What it does |
|---|---|
| `check.sh` | Runs every check the CI runs: headers, docs, the progress report generator's tests, the font build check in `fonts/` when Bun is set up, Rust format and lints, the HAL's host tests and `no_std` builds for the kernel and DOS targets (`hal/`), the DOS build, headless runs of `DESK.EXE`, the s1-dxe3 module scenarios, the d0-rust-dos checks, the a-vga-vbe checks, the b-dos-input checks (and with `INPUT_EXTRA=1` the Xvfb mouse run), and the native boot test (with its keyboard, mouse, graphics and BIOS-call sections) |
| `setup.sh` | Checks host tools, installs the pinned Rust pieces, and links `vendor/` to `DOS_TOOLCHAIN` or downloads the tools |
| `fetch-tools.sh` | Downloads DJGPP, DOSBox Staging and CWSDPMI into `vendor/` with SHA-256 checks, using the versions of the CI image |
| `build.sh` | Cargo for `core/dos.json`, `coff2go32.py`, DJGPP's gcc for `c/`, then `dist/DESK.EXE` beside `CWSDPMI.EXE` |
| `coff2go32.py` | Converts rustc's COFF objects so DJGPP's linker lays them out and relocates them correctly |
| `dosrun.sh` | Runs a DOS command in headless DOSBox Staging; prints its output and returns its exit status. `DOS_AUTOTYPE` types keys into the program with DOSBox's `AUTOTYPE`, `DOS_CONF` appends configuration, `DOS_X11=1` uses a real display; `DOS_MACHINE` picks the emulated card for one run (default `svga_s3`; `vgaonly` has no VBE) |
| `docker/Dockerfile.dos-xvfb` | Xvfb and `xdotool` for driving DOSBox's SDL window with a pointer; `spikes/b-dos-input/check-xvfb.sh` builds it once as `desk-dos-xvfb:1` |
| `raw2png.py` | Turns a frame buffer dump and a DAC dump into an indexed PNG (`docs/A-VGA-VBE.md`) |
| `check-headers.sh` | Checks the licence header on source and script files |
| `check-docs.py` | Checks the markdown house style and that relative links resolve |
| `build-hero.py` | Builds `assets/hero.svg`, the README banner |
| `native-build.sh`, `native-image.sh`, `native-run.sh`, `native-test.sh` | The native track: build the boot stages and the Rust kernel, assemble the floppy image, boot it headless in QEMU as a 486, and check the transcript and the negative cases (see [`../docs/NATIVE-BOOT.md`](../docs/NATIVE-BOOT.md)); `native-run.sh --inject SCRIPT` plays keys and mouse motion into QEMU's monitor with `native-inject.py` (and takes screenshots with `screendump {out}/NAME.ppm`, with `--vga CARD` and `--shots DIR`); `native-image.sh --test-mode gfx --video vbe` builds the graphics tour, `--test-mode bios` the BIOS-call scenarios (and `bios-solo` the same without typed keys; `native-build.sh` also assembles `kernel/bios16.asm`, the real-mode trampoline); `native-run.sh --hd FILE` attaches an IDE disk; `native-shot.py` converts, checksums and compares the screenshots; `native-check.py` and `elf2bin.py` are helpers |
| `progress-report/` | The generator for the Desk Progress Report, a multi-page archive built from the history of `main` and `gh`; see [`progress-report/README.md`](progress-report/README.md) |
