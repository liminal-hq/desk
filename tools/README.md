# tools

Scripts for working on the repository: the repo checks, the DOS build pipeline and the hero-banner builder. `docs/TOOLCHAIN.md` explains the pipeline.

| Script | What it does |
|---|---|
| `check.sh` | Runs every check the CI runs: headers, docs, Rust format and lints, the DOS build, headless runs of `DESK.EXE`, and the s1-dxe3 module scenarios |
| `setup.sh` | Checks host tools, installs the pinned Rust pieces, and links `vendor/` to `DOS_TOOLCHAIN` or downloads the tools |
| `fetch-tools.sh` | Downloads DJGPP, DOSBox Staging and CWSDPMI into `vendor/` with SHA-256 checks, using the versions of the CI image |
| `build.sh` | Cargo for `core/dos.json`, `coff2go32.py`, DJGPP's gcc for `c/`, then `dist/DESK.EXE` beside `CWSDPMI.EXE` |
| `coff2go32.py` | Converts rustc's COFF objects so DJGPP's linker lays them out and relocates them correctly |
| `dosrun.sh` | Runs a DOS command in headless DOSBox Staging; prints its output and returns its exit status |
| `check-headers.sh` | Checks the licence header on source and script files |
| `check-docs.py` | Checks the markdown house style and that relative links resolve |
| `build-hero.py` | Builds `assets/hero.svg`, the README banner |
