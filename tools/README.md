# tools

Scripts for working on the repository. Today these are the repo checks and the hero-banner builder. The DOS toolchain scripts (setup, fetch, headless DOSBox runner, the object converter) are written when the first spike needs them.

| Script | What it does |
|---|---|
| `check.sh` | Runs every check the CI runs, including the font build check in `fonts/` when Bun is set up |
| `check-headers.sh` | Checks the licence header on source and script files |
| `check-docs.py` | Checks the markdown house style and that relative links resolve |
| `build-hero.py` | Builds `assets/hero.svg`, the README banner |
