# tools

Scripts for working on the repository. Today these are the repo checks and the hero-banner builder. The DOS toolchain scripts (setup, fetch, headless DOSBox runner, the object converter) are written when the first spike needs them.

| Script | What it does |
|---|---|
| `check.sh` | Runs every check the CI runs |
| `check-headers.sh` | Checks the licence header on source and script files |
| `check-docs.py` | Checks the markdown house style and that relative links resolve |
| `build-hero.py` | Builds `assets/hero.svg`, the README banner |
| `native-build.sh`, `native-image.sh`, `native-run.sh`, `native-test.sh` | The native track: build the boot stages and the Rust kernel, assemble the floppy image, boot it headless in QEMU as a 486, and check the transcript and the negative cases (see [`../docs/NATIVE-BOOT.md`](../docs/NATIVE-BOOT.md)); `native-check.py` and `elf2bin.py` are their helpers |
