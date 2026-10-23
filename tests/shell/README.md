# Shell scenarios and goldens

The first window's scripted run and what it is expected to draw (`docs/FIRST-WINDOW.md`). The same scenario runs on the host in `cargo test`, and later in DOSBox and QEMU, so every platform is held to the same pictures.

## Files

- `NAME.scn` — a scenario: windows opened, the pointer moved, keys typed, and `frame` steps that capture the screen. The format is in `docs/FIRST-WINDOW.md`. `expect-damage` lines bound how many pixels a frame wrote, and are checked on the host only.
- `NAME.sums` — one line per frame and depth, `NAME DEPTH WIDTHxHEIGHT INDEX COLOUR`, the two CRC-32s of the frame in hex: `INDEX` of the palette indices, `COLOUR` of the six-bit `r g b` of every pixel. Every platform's capture must reproduce them.
- `golden/NAME/FRAME-DEPTH.png` — each frame as an indexed PNG (the platform's palette expanded to 8 bits), for people to look at and for `tools/frames.py diff`. They are checked against the `.sums` by `python3 -I tools/frames.py goldens tests/shell/NAME.sums tests/shell/golden/NAME`, which `tools/check.sh` runs.

## What the host test checks

`shell/shell/tests/scenarios.rs` runs every `.scn` here at every depth it lists, through `MemoryPresent` at both depths, `LinearPresent` over a `Vec`-backed region at depth 8 and `PlanarPresent` over a four-plane model at depth 4. All the presenters must give the same pixels, and their digests must equal the `.sums`.

When a frame differs, the test fails, writes the frames as `shell/target/frames/FRAME-DEPTH.RAW` and `.PAL`, and prints the `tools/frames.py png` and `tools/frames.py diff` commands that turn them into a picture of the differing pixels over the golden.

## Re-recording

A change that moves a pixel is deliberate: re-record in the same branch and look at the PNGs.

```bash
tools/shell-goldens.sh
git status tests/shell
```

The script runs the test with `DESK_FRAMES_RECORD=1`, which rewrites each `.sums`, starts from clean (stale PNGs are removed), and converts the dumps to the PNGs with `tools/frames.py png`. Running it twice leaves no diff. The PNGs depend on the font build (`fonts/generated/*.dfn`): rebuilding the fonts is a re-record, and the commit that does it says which build it used.
