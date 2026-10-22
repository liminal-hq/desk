# fonts

The four Desk bitmap faces as pixel grids, and the build that writes them as OpenType (CFF), TrueType and `.DFN` files, in Square and Round styles. [`docs/FONTS.md`](../docs/FONTS.md) is the reference: faces, metrics, grid conventions, adding a glyph.

| Path | What it holds |
|---|---|
| `src/faces/` | The glyph grids, one file per face |
| `src/face.ts` | Assembly: composed accents, derived bold, generated box drawing, placement in the cell |
| `src/boxes.ts`, `src/outline.ts`, `src/grid.ts` | Box drawing and blocks, the pixel-edge tracer, grid helpers |
| `src/round.ts` | Outlines for both styles, and the Round derivation |
| `src/otf.ts`, `src/ttf.ts`, `src/dfn.ts`, `src/names.ts` | The OpenType (CFF), TrueType and `.DFN` writers and their shared names |
| `src/specimen.ts`, `src/specimen-client.js` | The specimen page with glyph charts and the pixel editor |
| `build.ts` | The command line |
| `test/` | `bun test` |
| `validate.sh`, `validate.py` | Optional third-party validation (fontTools, OTS, FreeType) in Docker |
| `generated/` | The built files, committed, with `OFL.txt` |

```bash
bun install && bun build.ts && bun test
bun build.ts --check
```

The font files are under the SIL Open Font License 1.1 (`generated/OFL.txt`); the code is Apache-2.0 OR MIT.
