| Offset | Size | Field | Notes |
|---|---|---|---|
| 0 | u32 | magic | `DESK` (`0x4B534544`) |
| 4 | u32 | boot drive | |
| 8 | u32 | test mode | patched into the image |
| 12 | u32 | map source | 1 E820, 2 E801 |
| 16 | u32 | map entries | |
| **20** | u32 | **version** | **1 now; 0 (or garbage, since the loader did not write it) before.** A kernel reads the next two fields only when this is at least 1 |
| **24** | u32 | **video block address** | `0x820`; 0 for none |
| **28** | u32 | **font address** | `0x2000`; 0 for none (4096 bytes, 256 glyphs of 16 rows) |
| 32 | 24 × n | memory map | up to 32 entries, so up to `0x820` |
