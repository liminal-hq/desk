| Offset | Size | Field | Notes |
|---|---|---|---|
| 0 | u32 | state | 0 not tried, 1 a linear 8-bit mode is set, 2 VBE tried and not usable |
| 4 | u16 | VBE version | BCD, `0300h` for 3.0; with state 2, whatever the BIOS reported (0 if no VBE) |
| 6 | u16 | reason | with state 2: 1 no VBE BIOS, 2 older than 2.0, 3 no 8-bit linear mode, 4 the BIOS refused to set it; 0 otherwise |
| 8 | u16 | mode | VBE mode number |
| 10 | u16 | width | pixels |
| 12 | u16 | height | |
| 14 | u16 | pitch | bytes per scan line |
| 16 | u8 | bits per pixel | 8 |
| 17 | u8 | memory model | 4 is packed pixel |
| 18 | u16 | attributes | the mode attributes word |
| 20 | u32 | physical base | the linear frame buffer |
| 24 | u32 | video memory | KiB on the card |
| 28 | u32 | reserved | 0 |
