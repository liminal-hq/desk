#!/usr/bin/env python3
# Converts a frame buffer dump (one byte per pixel) and a DAC dump (6-bit RGB triples) into an indexed PNG.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tools/raw2png.py <pixels.raw> <palette.pal> <width> <height> <out.png> [scale]
#
# The DAC holds six bits per channel; each is widened to eight by repeating its top two bits (63 becomes 255), which is what a
# monitor does with the analogue level. `scale` repeats each pixel that many times each way (mode 13h at 2x is easier to look at).
import struct
import sys
import zlib


def chunk(kind, data):
    body = kind + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)


def main(argv):
    if len(argv) not in (6, 7):
        sys.exit(__doc__)
    raw_path, pal_path, width, height, out = argv[1], argv[2], int(argv[3]), int(argv[4]), argv[5]
    scale = int(argv[6]) if len(argv) == 7 else 1
    pixels = open(raw_path, "rb").read()
    pal = open(pal_path, "rb").read()
    if len(pixels) != width * height:
        sys.exit(f"raw2png: {raw_path} holds {len(pixels)} bytes, not {width}x{height}")
    if len(pal) % 3 or not pal:
        sys.exit(f"raw2png: {pal_path} is not a list of RGB triples")
    plte = bytearray()
    for i in range(0, len(pal), 3):
        for v in pal[i:i + 3]:
            v &= 63
            plte.append((v << 2) | (v >> 4))
    rows = bytearray()
    for y in range(height):
        row = pixels[y * width:(y + 1) * width]
        if scale > 1:
            row = bytes(b for b in row for _ in range(scale))
        for _ in range(scale):
            rows.append(0)  # filter: none
            rows.extend(row)
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width * scale, height * scale, 8, 3, 0, 0, 0))
    png += chunk(b"PLTE", bytes(plte))
    png += chunk(b"IDAT", zlib.compress(bytes(rows), 9))
    png += chunk(b"IEND", b"")
    open(out, "wb").write(png)


main(sys.argv)
