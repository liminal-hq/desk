#!/usr/bin/env python3
# Works with the screenshots QEMU's monitor writes (`screendump FILE.ppm`): converts them to PNG, checksums them against reference
# values and compares two of them, so the native graphics tests can say what is on the screen without a display.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage:
#   python3 -I tools/native-shot.py png IN.ppm OUT.png        convert (indexed colour when the picture has 256 colours or fewer)
#   python3 -I tools/native-shot.py sum IN.ppm                print "WIDTHxHEIGHT CRC32" of the pixel bytes
#   python3 -I tools/native-shot.py check REFERENCE DIR       every line "NAME WIDTHxHEIGHT CRC32" of REFERENCE must match DIR/NAME.ppm
#   python3 -I tools/native-shot.py record DIR NAME...        print REFERENCE lines for DIR/NAME.ppm
#   python3 -I tools/native-shot.py moved A.ppm B.ppm X0 Y0 X1 Y1
#                                                              the two pictures differ, and only inside the rectangle [X0,X1) x [Y0,Y1)
#   python3 -I tools/native-shot.py has-text A.ppm            the picture is a text-mode screen: fewer than 17 colours and some glyph pixels
# A REFERENCE file ignores blank lines and lines starting with #. Exit status 0 on success, 1 on a failed check, 2 on a usage error.
import os
import struct
import sys
import zlib


def read_ppm(path):
    data = open(path, "rb").read()
    fields, pos = [], 0
    while len(fields) < 4:
        while data[pos : pos + 1].isspace():
            pos += 1
        if data[pos : pos + 1] == b"#":
            pos = data.index(b"\n", pos)
            continue
        end = pos
        while not data[end : end + 1].isspace():
            end += 1
        fields.append(data[pos:end])
        pos = end
    pos += 1  # the single whitespace byte after maxval
    if fields[0] != b"P6" or fields[3] != b"255":
        raise ValueError(path + ": not an 8-bit P6 PPM")
    w, h = int(fields[1]), int(fields[2])
    pixels = data[pos:]
    if len(pixels) != w * h * 3:
        raise ValueError(f"{path}: {len(pixels)} pixel bytes, expected {w * h * 3}")
    return w, h, pixels


def chunk(kind, body):
    raw = kind + body
    return struct.pack(">I", len(body)) + raw + struct.pack(">I", zlib.crc32(raw) & 0xFFFFFFFF)


def write_png(path, w, h, pixels):
    colours = sorted({pixels[i : i + 3] for i in range(0, len(pixels), 3)})
    if len(colours) <= 256:
        index = {c: i for i, c in enumerate(colours)}
        rows = [b"\0" + bytes(index[pixels[o : o + 3]] for o in range(y * w * 3, (y + 1) * w * 3, 3)) for y in range(h)]
        head = struct.pack(">IIBBBBB", w, h, 8, 3, 0, 0, 0)
        extra = chunk(b"PLTE", b"".join(colours))
    else:
        rows = [b"\0" + pixels[y * w * 3 : (y + 1) * w * 3] for y in range(h)]
        head = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)
        extra = b""
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", head) + extra + chunk(b"IDAT", zlib.compress(b"".join(rows), 9)) + chunk(b"IEND", b"")
    open(path, "wb").write(png)


def summary(path):
    w, h, pixels = read_ppm(path)
    return f"{w}x{h} {zlib.crc32(pixels) & 0xFFFFFFFF:08x}"


def check(reference, directory):
    bad = 0
    for line in open(reference, encoding="utf-8"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        name, size, crc = line.split()
        path = os.path.join(directory, name + ".ppm")
        try:
            got = summary(path)
        except (OSError, ValueError) as err:
            print(f"native-shot: {name}: {err}", file=sys.stderr)
            bad += 1
            continue
        if got != f"{size} {crc}":
            print(f"native-shot: {name}: got {got}, reference {size} {crc}", file=sys.stderr)
            bad += 1
        else:
            print(f"native-shot: {name} matches ({size} {crc})")
    return bad == 0


def moved(a, b, rect):
    wa, ha, pa = read_ppm(a)
    wb, hb, pb = read_ppm(b)
    if (wa, ha) != (wb, hb):
        print("native-shot: the pictures differ in size", file=sys.stderr)
        return False
    x0, y0, x1, y1 = rect
    box = None
    for y in range(ha):
        ra, rb = pa[y * wa * 3 : (y + 1) * wa * 3], pb[y * wa * 3 : (y + 1) * wa * 3]
        if ra == rb:
            continue
        for x in range(wa):
            if ra[x * 3 : x * 3 + 3] != rb[x * 3 : x * 3 + 3]:
                box = (x, y, x, y) if box is None else (min(box[0], x), min(box[1], y), max(box[2], x), max(box[3], y))
    if box is None:
        print("native-shot: the two pictures are identical", file=sys.stderr)
        return False
    inside = x0 <= box[0] and y0 <= box[1] and box[2] < x1 and box[3] < y1
    print(f"native-shot: the pictures differ in x {box[0]}..{box[2]}, y {box[1]}..{box[3]}; allowed [{x0},{x1}) x [{y0},{y1}): {inside}")
    return inside


def has_text(path):
    w, h, pixels = read_ppm(path)
    colours = {pixels[i : i + 3] for i in range(0, len(pixels), 3)}
    ok = 2 <= len(colours) <= 16 and (w, h) == (720, 400)
    print(f"native-shot: {path}: {w}x{h}, {len(colours)} colours; a 720x400 text screen: {ok}")
    return ok


def main(argv):
    try:
        if argv[1] == "png" and len(argv) == 4:
            w, h, pixels = read_ppm(argv[2])
            write_png(argv[3], w, h, pixels)
        elif argv[1] == "sum" and len(argv) == 3:
            print(summary(argv[2]))
        elif argv[1] == "check" and len(argv) == 4:
            return 0 if check(argv[2], argv[3]) else 1
        elif argv[1] == "record" and len(argv) >= 4:
            for name in argv[3:]:
                print(name, summary(os.path.join(argv[2], name + ".ppm")))
        elif argv[1] == "moved" and len(argv) == 8:
            return 0 if moved(argv[2], argv[3], [int(v) for v in argv[4:8]]) else 1
        elif argv[1] == "has-text" and len(argv) == 3:
            return 0 if has_text(argv[2]) else 1
        else:
            print(__doc__, file=sys.stderr)
            return 2
    except (OSError, ValueError, IndexError) as err:
        print("native-shot: " + str(err), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
