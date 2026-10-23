#!/usr/bin/env python3
# Writes, reads, digests, checks and diffs the shell's frame captures (PNG, PPM, and raw index dumps with their DAC dumps), and writes the
# QEMU monitor script for the native run, so the hosted and native tracks are held to the same goldens.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage (docs/FIRST-WINDOW.md, "Goldens and frame digests"):
#   python3 -I tools/frames.py png RAW PAL WIDTH HEIGHT OUT.png     a raw index dump and a 768-byte 6-bit DAC dump to an indexed PNG
#   python3 -I tools/frames.py digest FILE [PAL WIDTH HEIGHT]       "WIDTHxHEIGHT INDEX COLOUR" of a PNG, a PPM (INDEX is "-"), or a
#                                                                   RAW with its PAL and size
#   python3 -I tools/frames.py check SUMS DEPTH DIR [--golden DIR]  every frame of SUMS at DEPTH has a capture in DIR (NAME.ppm, NAME.png,
#                                                                   or NAME.RAW with NAME.PAL; names compared without case) with the
#                                                                   same COLOUR digest, and the same INDEX where it has indices; on a
#                                                                   mismatch writes DIR/NAME.diff.png against the golden PNG (default
#                                                                   golden/SUMSNAME/NAME-DEPTH.png beside SUMS) and prints the box
#   python3 -I tools/frames.py transcript SUMS DEPTH FILE          the "shell: frame NAME ..." lines of a transcript carry the SUMS digests
#   python3 -I tools/frames.py goldens SUMS DIR                    every PNG in DIR matches its SUMS line and every SUMS line has a PNG
#   python3 -I tools/frames.py same DIR_A DIR_B [--size WxH]       two capture sets hold the same frames with equal COLOUR digests (the
#                                                                   size is for RAW dumps, 640x480 unless given)
#   python3 -I tools/frames.py diff A B OUT.png                    the differing pixels in red over a dimmed A
#   python3 -I tools/frames.py inject SCENARIO > FILE              the QEMU monitor script tools/native-inject.py reads
#   python3 -I tools/frames.py live SCENARIO --qemu > FILE         a live scenario's steps after its first frame as input for a running
#                                                                   emulator: the monitor script for native-inject.py (--qemu), the arguments
#   python3 -I tools/frames.py live SCENARIO --autotype             for DOSBox's AUTOTYPE (--autotype), or the xdotool lines for
#   python3 -I tools/frames.py live SCENARIO --xdotool [--quick]    tools/xvfb-dos.sh (--xdotool; --quick makes an adjacent press and
#                                                                   release one click). A step the emulator cannot express is refused
#                                                                   with its line number
#   python3 -I tools/frames.py marks SUMS SCENARIO DEPTH TRANSCRIPT [--dumps DIR]
#                                                                   the transcript's `shell: mark NN ...` lines, in order, carry the digests
#                                                                   of the scenario's frames (mark k is the k-th frame); with --dumps,
#                                                                   renames DIR/Mkk.RAW and .PAL to the frame names for `check`
#
# A SUMS file has one line per frame and depth, "NAME DEPTH WIDTHxHEIGHT INDEX COLOUR" (hex), and ignores blank lines and lines starting
# with #. The INDEX digest is the CRC-32 of the palette indices row by row; the COLOUR digest is the CRC-32 of every pixel's six-bit
# "r g b" bytes. A PPM's 8-bit channels are shifted right by two to get the DAC's six bits back, which is exact for any expansion that
# keeps the six bits on top, so the comparison does not depend on how an emulator expands 6-bit colour. Exit status 0 on success, 1 on
# a failed check or an unreadable input, 2 on a usage error.
import argparse
import os
import re
import struct
import sys
import zlib

NAME_RE = re.compile(r"^[a-z0-9-]{1,8}$")
TRANSCRIPT_RE = re.compile(r"shell: frame (\S+) (\d+) (\d+)x(\d+) ([0-9a-fA-F]{8}) ([0-9a-fA-F]{8})")


class FramesError(Exception):
    """An input that cannot be read or does not mean anything; reported on stderr, exit status 1."""


def slurp(path, mode="rb", **kw):
    with open(path, mode, **kw) as f:
        return f.read()


def crc32(data):
    return zlib.crc32(data) & 0xFFFFFFFF


def expand6(v):
    """A DAC value as 8 bits, the top two bits repeated into the bottom two (`Rgb6::to_rgb8`'s rule)."""
    return (v << 2) | (v >> 4)


SIX_BIT = bytes(i >> 2 for i in range(256))
EXPAND = bytes(expand6(i) if i < 64 else 0 for i in range(256))


class Capture:
    """A picture: six-bit colours for every pixel, and the palette indices where the capture has them."""

    def __init__(self, width, height, rgb6=None, indices=None, pal6=None):
        self.width, self.height = width, height
        self.indices, self.pal6 = indices, pal6
        if rgb6 is None:
            rgb6 = self._expand(indices, pal6)
        if len(rgb6) != width * height * 3:
            raise FramesError(f"{len(rgb6)} colour bytes for a {width}x{height} picture")
        self.rgb6 = rgb6

    @staticmethod
    def _expand(indices, pal6):
        if max(indices, default=0) >= len(pal6):
            raise FramesError(f"index {max(indices)} is past the {len(pal6)}-entry palette")
        out = bytearray(len(indices) * 3)
        for channel in range(3):
            table = bytes(c[channel] for c in pal6) + bytes(256 - len(pal6))
            out[channel::3] = indices.translate(table)
        return bytes(out)

    def index_digest(self):
        return crc32(self.indices) if self.indices is not None else None

    def colour_digest(self):
        return crc32(self.rgb6)

    def digest_text(self):
        index = "-" if self.indices is None else f"{self.index_digest():08x}"
        return f"{self.width}x{self.height} {index} {self.colour_digest():08x}"


def read_pal(data, source="PAL"):
    """A DAC dump (three six-bit bytes per entry) as a list of (r, g, b)."""
    if not data or len(data) % 3 or len(data) > 768:
        raise FramesError(f"{source}: {len(data)} bytes, not a DAC dump of up to 256 entries")
    if max(data) > 63:
        raise FramesError(f"{source}: a value above 63, so not a 6-bit DAC dump")
    return [tuple(data[i : i + 3]) for i in range(0, len(data), 3)]


def pal8(pal6):
    return [tuple(expand6(v) for v in c) for c in pal6]


# PNG ------------------------------------------------------------------------------------------------------------------------------


def chunk(kind, body):
    raw = kind + body
    return struct.pack(">I", len(body)) + raw + struct.pack(">I", crc32(raw))


def _png(width, height, colour_type, rows, plte=None):
    head = struct.pack(">IIBBBBB", width, height, 8, colour_type, 0, 0, 0)
    body = b"".join(b"\0" + row for row in rows)  # filter 0 on every row
    out = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", head)
    if plte is not None:
        out += chunk(b"PLTE", b"".join(bytes(c) for c in plte))
    return out + chunk(b"IDAT", zlib.compress(body, 9)) + chunk(b"IEND", b"")


def png_indexed(width, height, indices, pal6):
    """An 8-bit indexed PNG: the palette expanded to 8 bits with `v << 2 | v >> 4`, filter 0 on every row."""
    rows = [indices[y * width : (y + 1) * width] for y in range(height)]
    return _png(width, height, 3, rows, pal8(pal6))


def png_rgb(width, height, rgb8):
    rows = [rgb8[y * width * 3 : (y + 1) * width * 3] for y in range(height)]
    return _png(width, height, 2, rows)


def write_png(path, data):
    with open(path, "wb") as f:
        f.write(data)


def unfilter(data, width, height, bpp, source):
    """Undoes the PNG row filters (all five) of `height` rows of `width * bpp` bytes."""
    stride = width * bpp
    if len(data) != height * (stride + 1):
        raise FramesError(f"{source}: {len(data)} bytes of image data, expected {height * (stride + 1)}")
    out = bytearray(height * stride)
    prev = bytes(stride)
    pos = 0
    for y in range(height):
        kind = data[pos]
        line = data[pos + 1 : pos + 1 + stride]
        pos += stride + 1
        if kind == 0:
            row = bytearray(line)
        elif kind == 1:
            row = bytearray(line)
            for i in range(bpp, stride):
                row[i] = (row[i] + row[i - bpp]) & 255
        elif kind == 2:
            row = bytearray((line[i] + prev[i]) & 255 for i in range(stride))
        elif kind == 3:
            row = bytearray(line)
            for i in range(stride):
                left = row[i - bpp] if i >= bpp else 0
                row[i] = (row[i] + ((left + prev[i]) >> 1)) & 255
        elif kind == 4:
            row = bytearray(line)
            for i in range(stride):
                a = row[i - bpp] if i >= bpp else 0
                b = prev[i]
                c = prev[i - bpp] if i >= bpp else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pred = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                row[i] = (row[i] + pred) & 255
        else:
            raise FramesError(f"{source}: row {y} has filter type {kind}")
        out[y * stride : (y + 1) * stride] = row
        prev = bytes(row)
    return bytes(out)


def read_png(path):
    """An 8-bit indexed or 8-bit RGB PNG, not interlaced, with any row filters."""
    data = slurp(path)
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise FramesError(f"{path}: not a PNG")
    pos, head, plte, idat = 8, None, None, []
    while pos < len(data):
        if pos + 8 > len(data):
            raise FramesError(f"{path}: truncated")
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        kind = data[pos + 4 : pos + 8]
        body = data[pos + 8 : pos + 8 + length]
        crc = data[pos + 8 + length : pos + 12 + length]
        if len(body) != length or len(crc) != 4 or struct.unpack(">I", crc)[0] != crc32(kind + body):
            raise FramesError(f"{path}: bad {kind.decode('latin-1')} chunk")
        pos += 12 + length
        if kind == b"IHDR":
            head = struct.unpack(">IIBBBBB", body)
        elif kind == b"PLTE":
            plte = body
        elif kind == b"IDAT":
            idat.append(body)
        elif kind == b"IEND":
            break
    if head is None or not idat:
        raise FramesError(f"{path}: no image data")
    width, height, depth, colour, _, _, interlace = head
    if depth != 8 or colour not in (2, 3) or interlace != 0:
        raise FramesError(f"{path}: only 8-bit indexed and RGB PNGs without interlacing are read")
    try:
        raw = zlib.decompress(b"".join(idat))
    except zlib.error as err:
        raise FramesError(f"{path}: {err}") from None
    pixels = unfilter(raw, width, height, 1 if colour == 3 else 3, path)
    if colour == 2:
        return Capture(width, height, rgb6=pixels.translate(SIX_BIT))
    if plte is None or len(plte) % 3:
        raise FramesError(f"{path}: an indexed PNG without a usable palette")
    if len(plte) > 768:
        raise FramesError(f"{path}: a palette of more than 256 entries")
    pal6 = [tuple(v >> 2 for v in plte[i : i + 3]) for i in range(0, len(plte), 3)]
    return Capture(width, height, indices=pixels, pal6=pal6)


# PPM and raw dumps ----------------------------------------------------------------------------------------------------------------


def read_ppm(path):
    """A binary PPM (P6, maxval 255), as QEMU's `screendump` writes it."""
    data = slurp(path)
    fields, pos = [], 0
    while len(fields) < 4:
        while data[pos : pos + 1].isspace():
            pos += 1
        if data[pos : pos + 1] == b"#":
            end = data.find(b"\n", pos)
            if end < 0:
                break
            pos = end
            continue
        end = pos
        while end < len(data) and not data[end : end + 1].isspace():
            end += 1
        if end == pos:
            break
        fields.append(data[pos:end])
        pos = end
    if len(fields) < 4 or fields[0] != b"P6" or fields[3] != b"255":
        raise FramesError(f"{path}: not an 8-bit P6 PPM")
    try:
        width, height = int(fields[1]), int(fields[2])
    except ValueError:
        raise FramesError(f"{path}: bad PPM size") from None
    pixels = data[pos + 1 :]
    if len(pixels) != width * height * 3:
        raise FramesError(f"{path}: {len(pixels)} pixel bytes, expected {width * height * 3}")
    return Capture(width, height, rgb6=pixels.translate(SIX_BIT))


def read_raw(raw_path, pal_path, width, height):
    indices = slurp(raw_path)
    if len(indices) != width * height:
        raise FramesError(f"{raw_path}: {len(indices)} bytes, expected {width * height} for {width}x{height}")
    return Capture(width, height, indices=indices, pal6=read_pal(slurp(pal_path), pal_path))


def parse_size(text):
    m = re.fullmatch(r"(\d+)x(\d+)", text)
    if not m:
        raise FramesError(f"bad size {text!r}, expected WIDTHxHEIGHT")
    return int(m.group(1)), int(m.group(2))


def lookup(directory):
    """The files of `directory` by lower-cased name, so NAME.RAW and name.raw are the same file."""
    try:
        names = os.listdir(directory)
    except OSError as err:
        raise FramesError(f"{directory}: {err.strerror}") from None
    found = {}
    for f in names:
        if f.lower() in found:
            raise FramesError(f"{directory}: {f} and {os.path.basename(found[f.lower()])} differ only in case")
        found[f.lower()] = os.path.join(directory, f)
    return found


def load(path):
    low = path.lower()
    try:
        if low.endswith(".png"):
            return read_png(path)
        if low.endswith(".ppm"):
            return read_ppm(path)
    except OSError as err:
        raise FramesError(f"{path}: {err.strerror}") from None
    raise FramesError(f"{path}: a picture file is .png or .ppm (a RAW needs its PAL and size)")


def find_capture(files, name, width, height):
    """The capture of frame `name` among `files` (from `lookup`): NAME.ppm, NAME.png, or NAME.RAW with NAME.PAL."""
    stem = name.lower()
    for ext in (".ppm", ".png"):
        if stem + ext in files:
            return load(files[stem + ext])
    if stem + ".raw" in files:
        if stem + ".pal" not in files:
            raise FramesError(f"{files[stem + '.raw']}: no {name}.PAL beside it")
        try:
            return read_raw(files[stem + ".raw"], files[stem + ".pal"], width, height)
        except OSError as err:
            raise FramesError(f"{name}: {err.strerror}") from None
    return None


# SUMS -----------------------------------------------------------------------------------------------------------------------------


class Sum:
    def __init__(self, name, depth, width, height, index, colour):
        self.name, self.depth, self.width, self.height, self.index, self.colour = name, depth, width, height, index, colour

    def digest_text(self):
        return f"{self.width}x{self.height} {self.index:08x} {self.colour:08x}"


def read_sums(path):
    try:
        text = slurp(path, "r", encoding="utf-8")
    except OSError as err:
        raise FramesError(f"{path}: {err.strerror}") from None
    sums = []
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        m = re.fullmatch(r"(\S+) (\d+) (\d+)x(\d+) ([0-9a-fA-F]{8}) ([0-9a-fA-F]{8})", line)
        if not m:
            raise FramesError(f"{path}:{number}: not a SUMS line (NAME DEPTH WIDTHxHEIGHT INDEX COLOUR): {line}")
        name, depth, w, h, index, colour = m.groups()
        if any(s.name.lower() == name.lower() and s.depth == int(depth) for s in sums):
            raise FramesError(f"{path}:{number}: {name} at depth {depth} is listed twice")
        sums.append(Sum(name, int(depth), int(w), int(h), int(index, 16), int(colour, 16)))
    return sums


def mismatch(cap, want):
    """Why `cap` differs from the SUMS line `want`, or None when it matches."""
    if (cap.width, cap.height) != (want.width, want.height):
        return f"size {cap.width}x{cap.height}, expected {want.width}x{want.height}"
    if cap.colour_digest() != want.colour:
        return f"colour digest {cap.colour_digest():08x}, expected {want.colour:08x}"
    if cap.indices is not None and cap.index_digest() != want.index:
        return f"index digest {cap.index_digest():08x}, expected {want.index:08x}"
    return None


# Comparing pictures ---------------------------------------------------------------------------------------------------------------


def differences(a, b):
    """(count, box) of the pixels whose six-bit colours differ; box is (x0, y0, x1, y1), inclusive, or None."""
    if (a.width, a.height) != (b.width, b.height):
        raise FramesError(f"sizes differ: {a.width}x{a.height} and {b.width}x{b.height}")
    count, x0, y0, x1, y1 = 0, a.width, a.height, -1, -1
    stride = a.width * 3
    for y in range(a.height):
        ra, rb = a.rgb6[y * stride : (y + 1) * stride], b.rgb6[y * stride : (y + 1) * stride]
        if ra == rb:
            continue
        for x in range(a.width):
            if ra[x * 3 : x * 3 + 3] != rb[x * 3 : x * 3 + 3]:
                count += 1
                x0, x1, y0, y1 = min(x0, x), max(x1, x), min(y0, y), max(y1, y)
    return count, ((x0, y0, x1, y1) if count else None)


def diff_picture(a, b):
    """An RGB PNG of the differing pixels in red over `a` dimmed to half."""
    out = bytearray(v >> 1 for v in a.rgb6.translate(EXPAND))
    stride = a.width * 3
    for y in range(a.height):
        ra, rb = a.rgb6[y * stride : (y + 1) * stride], b.rgb6[y * stride : (y + 1) * stride]
        if ra == rb:
            continue
        for x in range(a.width):
            if ra[x * 3 : x * 3 + 3] != rb[x * 3 : x * 3 + 3]:
                out[y * stride + x * 3 : y * stride + x * 3 + 3] = b"\xff\x00\x00"
    return png_rgb(a.width, a.height, bytes(out))


def describe(count, box):
    x0, y0, x1, y1 = box
    return f"{count} differing pixel{'s' if count != 1 else ''}, box x {x0}..{x1}, y {y0}..{y1}"


# Commands -------------------------------------------------------------------------------------------------------------------------


def cmd_png(args):
    width, height = args.width, args.height
    cap = read_raw(args.raw, args.pal, width, height)
    write_png(args.out, png_indexed(width, height, cap.indices, cap.pal6))
    return 0


def cmd_digest(args):
    low = args.file.lower()
    if low.endswith((".png", ".ppm")):
        if args.pal or args.width or args.height:
            raise UsageError("a PNG or PPM takes no PAL, WIDTH or HEIGHT")
        cap = load(args.file)
    else:
        if not (args.pal and args.width and args.height):
            raise UsageError("a RAW dump needs PAL WIDTH HEIGHT")
        cap = read_raw(args.file, args.pal, args.width, args.height)
    print(cap.digest_text())
    return 0


def cmd_check(args):
    sums = [s for s in read_sums(args.sums) if s.depth == args.depth]
    if not sums:
        raise FramesError(f"{args.sums}: no frames at depth {args.depth}")
    files = lookup(args.dir)
    golden = args.golden
    if golden is None:
        base = os.path.splitext(os.path.basename(args.sums))[0]
        golden = os.path.join(os.path.dirname(args.sums), "golden", base)
    ok = True
    for want in sums:
        cap = find_capture(files, want.name, want.width, want.height)
        if cap is None:
            print(f"FAIL {want.name}: no capture in {args.dir}")
            ok = False
            continue
        why = mismatch(cap, want)
        if why is None:
            continue
        ok = False
        print(f"FAIL {want.name}: {why}")
        golden_path = os.path.join(golden, f"{want.name}-{want.depth}.png")
        gold_files = lookup(golden) if os.path.isdir(golden) else {}
        gold = gold_files.get(os.path.basename(golden_path).lower())
        if gold is None:
            print(f"  no golden PNG at {golden_path} to diff against")
            continue
        try:
            gold_cap = load(gold)
            count, box = differences(gold_cap, cap)
        except FramesError as err:
            print(f"  cannot diff: {err}")
            continue
        if count:
            out = os.path.join(args.dir, f"{want.name}.diff.png")
            write_png(out, diff_picture(gold_cap, cap))
            print(f"  {describe(count, box)}; wrote {out}")
        else:
            print("  the colours equal the golden's; only the index digest differs")
    if ok:
        print(f"ok: {len(sums)} frames at depth {args.depth} match {args.sums}")
    return 0 if ok else 1


def cmd_transcript(args):
    sums = {s.name.lower(): s for s in read_sums(args.sums) if s.depth == args.depth}
    if not sums:
        raise FramesError(f"{args.sums}: no frames at depth {args.depth}")
    try:
        text = slurp(args.file, "r", encoding="utf-8", errors="replace")
    except OSError as err:
        raise FramesError(f"{args.file}: {err.strerror}") from None
    seen, ok = set(), True
    for m in TRANSCRIPT_RE.finditer(text):
        name, depth, w, h, index, colour = m.groups()
        key = name.lower()
        if key in seen:
            print(f"FAIL {name}: appears twice")
            ok = False
        seen.add(key)
        want = sums.get(key)
        if want is None:
            print(f"FAIL {name}: not a frame of {args.sums} at depth {args.depth}")
            ok = False
            continue
        got = (int(depth), int(w), int(h), int(index, 16), int(colour, 16))
        if got != (want.depth, want.width, want.height, want.index, want.colour):
            print(f"FAIL {name}: transcript has depth {depth} {w}x{h} {index.lower()} {colour.lower()}, expected {want.depth} {want.digest_text()}")
            ok = False
    for key, want in sums.items():
        if key not in seen:
            print(f"FAIL {want.name}: not in the transcript")
            ok = False
    if ok:
        print(f"ok: {len(sums)} frames in {args.file} carry the digests of {args.sums} at depth {args.depth}")
    return 0 if ok else 1


def cmd_goldens(args):
    sums = read_sums(args.sums)
    if not sums:
        raise FramesError(f"{args.sums}: no frames")
    wanted = {(s.name.lower(), s.depth): s for s in sums}
    ok, seen = True, set()
    for low, path in sorted(lookup(args.dir).items()):
        if not low.endswith(".png") or low.endswith(".diff.png"):
            continue
        m = re.fullmatch(r"(.+)-(\d+)\.png", low)
        key = (m.group(1), int(m.group(2))) if m else None
        want = wanted.get(key)
        if want is None:
            print(f"FAIL {os.path.basename(path)}: no line for it in {args.sums}")
            ok = False
            continue
        seen.add(key)
        why = mismatch(read_png(path), want)
        if why:
            print(f"FAIL {os.path.basename(path)}: {why}")
            ok = False
    for key, want in wanted.items():
        if key not in seen:
            print(f"FAIL {want.name} depth {want.depth}: no golden PNG in {args.dir}")
            ok = False
    if ok:
        print(f"ok: {len(wanted)} golden PNGs match {args.sums}")
    return 0 if ok else 1


def frames_in(directory, size):
    """{name: Capture} of the frames in `directory`: NAME.ppm, NAME.png (not .diff.png), or NAME.RAW with NAME.PAL."""
    files = lookup(directory)
    names = set()
    for low in files:
        for ext in (".ppm", ".png", ".raw"):
            if low.endswith(ext) and not low.endswith(".diff.png"):
                names.add(low[: -len(ext)])
    return {n: find_capture(files, n, *size) for n in sorted(names)}


def cmd_same(args):
    size = parse_size(args.size)
    a, b = frames_in(args.dir_a, size), frames_in(args.dir_b, size)
    if not a and not b:
        raise FramesError("neither directory holds a capture")
    ok = True
    for name in sorted(set(a) | set(b)):
        if name not in a or name not in b:
            print(f"FAIL {name}: only in {args.dir_b if name in b else args.dir_a}")
            ok = False
            continue
        ca, cb = a[name], b[name]
        if (ca.width, ca.height) != (cb.width, cb.height) or ca.colour_digest() != cb.colour_digest():
            print(f"FAIL {name}: {ca.width}x{ca.height} {ca.colour_digest():08x} against {cb.width}x{cb.height} {cb.colour_digest():08x}")
            ok = False
    if ok:
        print(f"ok: {len(a)} frames equal in {args.dir_a} and {args.dir_b}")
    return 0 if ok else 1


def cmd_diff(args):
    a, b = load(args.a), load(args.b)
    count, box = differences(a, b)
    write_png(args.out, diff_picture(a, b))
    print(describe(count, box) if count else "no differing pixels")
    return 0


def scenario_frames(path):
    """The frame names of a scenario file in order, validated as the scenario parser does."""
    names = []
    for number, words in scenario_lines(path):
        if words[0] != "frame":
            continue
        if len(words) != 2 or not NAME_RE.match(words[1]):
            raise FramesError(f"{path}:{number}: a frame name is 1 to 8 characters of a-z, 0-9 and -")
        for seen in names:
            if seen.startswith(words[1]) or words[1].startswith(seen):
                raise FramesError(f"{path}:{number}: frame name {words[1]} repeats or is a prefix of {seen}")
        names.append(words[1])
    if not names:
        raise FramesError(f"{path}: no frame steps")
    return names


def cmd_inject(args):
    """The monitor script: once the kernel's input is live, for each frame wait for its ready line, take the screendump, press a key."""
    lines = ["wait input: ready"]
    for name in scenario_frames(args.scenario):
        lines += [f"wait shell: ready {name}", f"screendump {{out}}/{name}.ppm", "sendkey spc"]
    sys.stdout.write("\n".join(lines) + "\n")
    return 0


# Live scenarios ---------------------------------------------------------------------------------------------------------------------
#
# A live scenario is a .scn file whose steps before the first `frame` are the first window's set-up (the platform's live mode performs
# them itself) and whose first frame is mark 00. Every later `frame` is a mark: the injector presses F12 and the guest prints
# `shell: mark NN ...`. docs/LIVE-INPUT.md ("Live scenarios") is the design.

# KeyCode name -> (QEMU `sendkey` name, DOSBox Staging AUTOTYPE name, xdotool keysym). QEMU's are its QKeyCode names; AUTOTYPE's are the
# mapper names `autotype -list` prints in DOSBox Staging 0.83 (None where it has no such key); xdotool's are X keysyms. A modifier's
# xdotool name is the alias xdotool accepts for the left key ("alt") or the keysym for the right one.
def _keys():
    t = {}
    for c in "abcdefghijklmnopqrstuvwxyz":
        t[c.upper()] = (c, c, c)
    for d in "0123456789":
        t["Num" + d] = (d, d, d)
        t["Kp" + d] = ("kp_" + d, "kp_" + d, "KP_" + d)
    for n in range(1, 13):
        t[f"F{n}"] = (f"f{n}", f"f{n}", f"F{n}")
    t.update(
        {
            "Escape": ("esc", "esc", "Escape"),
            "Backspace": ("backspace", "bspace", "BackSpace"),
            "Tab": ("tab", "tab", "Tab"),
            "Enter": ("ret", "enter", "Return"),
            "Space": ("spc", "space", "space"),
            "Minus": ("minus", "minus", "minus"),
            "Equals": ("equal", "equals", "equal"),
            "LeftBracket": ("bracket_left", "lbracket", "bracketleft"),
            "RightBracket": ("bracket_right", "rbracket", "bracketright"),
            "Backslash": ("backslash", "backslash", "backslash"),
            "Semicolon": ("semicolon", "semicolon", "semicolon"),
            "Apostrophe": ("apostrophe", "quote", "apostrophe"),
            "Grave": ("grave_accent", "grave", "grave"),
            "Comma": ("comma", "comma", "comma"),
            "Period": ("dot", "period", "period"),
            "Slash": ("slash", "slash", "slash"),
            "IntlBackslash": ("less", "oem102", "less"),
            "CapsLock": ("caps_lock", "capslock", "Caps_Lock"),
            "NumLock": ("num_lock", "numlock", "Num_Lock"),
            "ScrollLock": ("scroll_lock", "scrolllock", "Scroll_Lock"),
            "LeftShift": ("shift", "lshift", "shift"),
            "RightShift": ("shift_r", "rshift", "Shift_R"),
            "LeftCtrl": ("ctrl", "lctrl", "ctrl"),
            "RightCtrl": ("ctrl_r", "rctrl", "Control_R"),
            "LeftAlt": ("alt", "lalt", "alt"),
            "RightAlt": ("alt_r", "ralt", "Alt_R"),
            "LeftGui": ("meta_l", "lgui", "super"),
            "RightGui": ("meta_r", "rgui", "Super_R"),
            "Menu": ("menu", None, "Menu"),
            "Up": ("up", "up", "Up"),
            "Down": ("down", "down", "Down"),
            "Left": ("left", "left", "Left"),
            "Right": ("right", "right", "Right"),
            "Insert": ("insert", "insert", "Insert"),
            "Delete": ("delete", "delete", "Delete"),
            "Home": ("home", "home", "Home"),
            "End": ("end", "end", "End"),
            "PageUp": ("pgup", "pageup", "Prior"),
            "PageDown": ("pgdn", "pagedown", "Next"),
            "PrintScreen": ("print", "printscreen", "Print"),
            "SysRq": ("sysrq", None, "Sys_Req"),
            "Pause": ("pause", "pause", "Pause"),
            "KpPeriod": ("kp_decimal", "kp_period", "KP_Decimal"),
            "KpPlus": ("kp_add", "kp_plus", "KP_Add"),
            "KpMinus": ("kp_subtract", "kp_minus", "KP_Subtract"),
            "KpStar": ("kp_multiply", "kp_multiply", "KP_Multiply"),
            "KpSlash": ("kp_divide", "kp_divide", "KP_Divide"),
            "KpEnter": ("kp_enter", "kp_enter", "KP_Enter"),
        }
    )
    return t


KEY_NAMES = _keys()
MODIFIERS = {"LeftShift", "RightShift", "LeftCtrl", "RightCtrl", "LeftAlt", "RightAlt", "LeftGui", "RightGui"}
EMULATORS = {"qemu": 0, "autotype": 1, "xdotool": 2}

# The character a US-layout key types: unshifted, then shifted.
_SHIFTED = {c: ("Num" + d) for c, d in zip("!@#$%^&*()", "1234567890")}
_SHIFTED.update({"_": "Minus", "+": "Equals", "{": "LeftBracket", "}": "RightBracket", "|": "Backslash", ":": "Semicolon",
                 '"': "Apostrophe", "~": "Grave", "<": "Comma", ">": "Period", "?": "Slash"})
_PLAIN = {" ": "Space", "-": "Minus", "=": "Equals", "[": "LeftBracket", "]": "RightBracket", "\\": "Backslash", ";": "Semicolon",
          "'": "Apostrophe", "`": "Grave", ",": "Comma", ".": "Period", "/": "Slash"}


def key_for_char(c):
    """(KeyCode name, shifted) that types `c` on a US layout, or None."""
    if "a" <= c <= "z" or c.isdigit() and c.isascii():
        return (c.upper() if c.isalpha() else "Num" + c), False
    if "A" <= c <= "Z":
        return c, True
    if c in _PLAIN:
        return _PLAIN[c], False
    if c in _SHIFTED:
        return _SHIFTED[c], True
    return None


MOUSE_PAUSE = "0.1"  # after each mouse command: QEMU folds a press and release that arrive before the guest reads the last packet
SWITCH_PAUSE = "0.3"  # between a mouse step and a key step either way round, and before an F12: each ring is drained before the next device speaks
XDOTOOL_FRAME_PAUSE = "0.5"
XDOTOOL_BUTTON_PAUSE = "0.3"
MOVE_CHUNK = 100  # the most one `mouse_move` carries
MAX_MARKS = 100


def tokenize(text, where):
    """The words of one scenario line: `#` starts a comment outside a string; a quoted string is one word (with its quotes kept)."""
    words, cur, i, quoted = [], "", 0, False
    while i < len(text):
        c = text[i]
        if quoted:
            cur += c
            if c == "\\" and i + 1 < len(text):
                i += 1
                cur += text[i]
            elif c == '"':
                quoted = False
        elif c == '"':
            quoted, cur = True, cur + c
        elif c == "#":
            break
        elif c.isspace():
            if cur:
                words.append(cur)
                cur = ""
        else:
            cur += c
        i += 1
    if quoted:
        raise FramesError(f"{where}: an unterminated string")
    if cur:
        words.append(cur)
    return words


def unquote(word, where):
    if len(word) < 2 or word[0] != '"' or word[-1] != '"':
        raise FramesError(f"{where}: expected a quoted string")
    out, i, body = "", 0, word[1:-1]
    while i < len(body):
        if body[i] == "\\" and i + 1 < len(body):
            i += 1
        out += body[i]
        i += 1
    return out


def scenario_lines(path):
    """[(line number, words)] of the scenario's non-blank, non-comment lines."""
    try:
        text = slurp(path, "r", encoding="utf-8")
    except OSError as err:
        raise FramesError(f"{path}: {err.strerror}") from None
    out = []
    for number, raw in enumerate(text.splitlines(), 1):
        words = tokenize(raw, f"{path}:{number}")
        if words:
            out.append((number, words))
    return out


class Live:
    """A parsed live scenario: its screen, its frames and its input actions after the first frame, with the pointer tracked."""

    def __init__(self, path, emulator, first_window=None):
        self.path, self.emulator = path, emulator
        lines = scenario_lines(path)
        first = next((i for i, (_, w) in enumerate(lines) if w[0] == "frame"), None)
        if first is None:
            raise FramesError(f"{path}: no frame steps")
        reference = first_window or os.path.join(os.path.dirname(os.path.abspath(path)), "first-window.scn")
        ref = scenario_lines(reference)
        ref_first = next((i for i, (_, w) in enumerate(ref) if w[0] == "frame"), None)
        want = [w for _, w in ref[:ref_first]] if ref_first is not None else None
        got = [w for _, w in lines[:first]]
        if want != got:
            at = next((n for (n, w), v in zip(lines, want or []) if w != v), lines[min(first, len(lines) - 1)][0])
            raise FramesError(f"{path}:{at}: the steps before the first frame are not the first window's set-up ({os.path.basename(reference)})")
        self.screen, self.pointer = (640, 480), None
        for _, w in lines[:first]:
            if w[0] == "screen":
                self.screen = (int(w[1]), int(w[2]))
            elif w[0] == "pointer":
                self.pointer = (int(w[1]), int(w[2]))
        if self.pointer is None:
            self.pointer = (self.screen[0] // 2, self.screen[1] // 2)
        self._arity(lines[first:])
        self.frames = [lines[first][1][1]]
        self.actions = []
        self._steps(lines[first + 1 :])
        if len(self.frames) > MAX_MARKS:
            raise FramesError(f"{path}: {len(self.frames)} frames, but a session prints at most {MAX_MARKS} marks")

    def _arity(self, lines):
        """Refuses, with its line, a step with the wrong number of words, a number that is not one, or an unknown key state."""
        arity = {"move": 3, "pointer": 3, "press": 2, "release": 2, "type": 2, "key": 3, "frame": 2}
        for number, w in lines:
            if w[0] in arity and len(w) != arity[w[0]]:
                self.refuse(number, f"{w[0]} takes {arity[w[0]] - 1} argument{'s' if arity[w[0]] > 2 else ''}")
            if w[0] in ("move", "pointer") and not all(re.fullmatch(r"-?\d+", v) for v in w[1:]):
                self.refuse(number, f"{w[0]} takes whole numbers")
            if w[0] == "key" and w[2] not in ("down", "up"):
                self.refuse(number, f"a key state is down or up, not {w[2]}")
            if w[0] == "frame" and not NAME_RE.match(w[1]):
                self.refuse(number, "a frame name is 1 to 8 characters of a-z, 0-9 and -")

    def refuse(self, number, why):
        raise FramesError(f"{self.path}:{number}: {why}")

    def _clamp(self, x, y):
        return max(0, min(self.screen[0] - 1, x)), max(0, min(self.screen[1] - 1, y))

    def _key_name(self, number, name):
        if name not in KEY_NAMES:
            self.refuse(number, f"{name} is not a key a live scenario can send")
        if KEY_NAMES[name][EMULATORS[self.emulator]] is None:
            self.refuse(number, f"{self.emulator} has no key for {name}")
        return name

    def _steps(self, lines):
        held = []  # [(modifier name, line number, keys pressed under it)]
        pressed_at = None  # the line of a left press with no release yet
        i = 0
        while i < len(lines):
            number, w = lines[i]
            cmd = w[0]
            if held and cmd not in ("key",):
                self.refuse(number, f"{cmd} while {held[-1][0]} is held; a modifier is released before anything else")
            if cmd == "move":
                dx, dy = int(w[1]), int(w[2])
                if self.emulator == "autotype":
                    self.refuse(number, "AUTOTYPE cannot move the mouse")
                self.pointer = self._clamp(self.pointer[0] + dx, self.pointer[1] + dy)
                self.actions.append(("move", dx, dy))
            elif cmd == "pointer":
                if self.emulator == "autotype":
                    self.refuse(number, "AUTOTYPE cannot move the mouse")
                x, y = self._clamp(int(w[1]), int(w[2]))
                dx, dy = x - self.pointer[0], y - self.pointer[1]
                self.pointer = (x, y)
                if dx or dy:
                    self.actions.append(("move", dx, dy))
            elif cmd in ("press", "release"):
                if w[1:] != ["left"]:
                    self.refuse(number, f"only the left button can be injected, not {' '.join(w[1:])}")
                if self.emulator == "autotype":
                    self.refuse(number, "AUTOTYPE cannot press a mouse button")
                if cmd == "press":
                    if pressed_at is not None:
                        self.refuse(number, "press left while the button is already down")
                    pressed_at = number
                else:
                    if pressed_at is None:
                        self.refuse(number, "release left while the button is up")
                    pressed_at = None
                self.actions.append(("button", cmd == "press"))
            elif cmd == "type":
                for c in unquote(w[1], f"{self.path}:{number}"):
                    k = key_for_char(c)
                    if k is None:
                        self.refuse(number, f"no key types {c!r}")
                    name, shifted = k
                    self._key_name(number, name)
                    if shifted:
                        self._key_name(number, "LeftShift")
                        if self.emulator == "autotype":
                            self.refuse(number, f"AUTOTYPE has no chords, so it cannot type {c!r}")
                    self.actions.append(("press", ["LeftShift"] if shifted else [], name))
            elif cmd == "key":
                name, state = w[1], w[2]
                self._key_name(number, name)
                nxt = lines[i + 1][1] if i + 1 < len(lines) else None
                if state == "down":
                    adjacent_up = nxt == ["key", name, "up"]
                    if name in MODIFIERS and not adjacent_up:
                        if any(h[0] == name for h in held):
                            self.refuse(number, f"key {name} down while it is already held")
                        held.append([name, number, 0])
                    elif adjacent_up:
                        self._press(number, [h[0] for h in held], name)
                        for h in held:
                            h[2] += 1
                        i += 1
                    else:
                        self.refuse(number, f"key {name} down is never released straight away; a lone key down cannot be injected")
                else:
                    if not held or held[-1][0] != name:
                        self.refuse(number, f"key {name} up without its down (modifiers are released in reverse order)")
                    mod = held.pop()
                    if mod[2] == 0:
                        self.refuse(mod[1], f"{name} is held with no key pressed under it")
            elif cmd == "frame":
                self.frames.append(w[1])
                self.actions.append(("frame", w[1], len(self.frames) - 1))
            elif cmd == "expect-damage":
                pass
            else:
                self.refuse(number, f"{cmd} cannot be injected: a live scenario's steps after its first frame are move, pointer, press, release, type, key and frame")
            i += 1
        if held:
            self.refuse(held[-1][1], f"key {held[-1][0]} down is never released")
        if pressed_at is not None:
            self.refuse(pressed_at, "press left is never released")

    def _press(self, number, mods, name):
        if mods and self.emulator == "autotype":
            self.refuse(number, "AUTOTYPE has no chords, so it cannot press " + "+".join(mods + [name]))
        self.actions.append(("press", list(mods), name))

    def names(self, which):
        return lambda n: KEY_NAMES[n][EMULATORS[which]]


def _switches(actions):
    """The actions with the device they use: 'mouse' for motion and buttons, 'key' for keys, None for a frame."""
    for a in actions:
        yield a, ("mouse" if a[0] in ("move", "button", "click") else "key" if a[0] == "press" else None)


def live_qemu(lv):
    """The monitor script for tools/native-inject.py: wait for mark 00, then each step and a mark for each frame, ending with Escape."""
    k = lv.names("qemu")
    out = [f"# {os.path.basename(lv.path)} as input for a running QEMU (frames.py live --qemu)", "wait shell: mark 00",
           f"screendump {{out}}/{lv.frames[0]}.ppm"]
    last = None
    for a, dev in _switches(lv.actions):
        if dev and last and dev != last:
            out.append(f"sleep {SWITCH_PAUSE}")
        if dev:
            last = dev
        if a[0] == "move":
            dx, dy = a[1], a[2]
            while dx or dy:
                sx, sy = max(-MOVE_CHUNK, min(MOVE_CHUNK, dx)), max(-MOVE_CHUNK, min(MOVE_CHUNK, dy))
                out += [f"mouse_move {sx} {sy}", f"sleep {MOUSE_PAUSE}"]
                dx, dy = dx - sx, dy - sy
        elif a[0] == "button":
            out += [f"mouse_button {1 if a[1] else 0}", f"sleep {MOUSE_PAUSE}"]
        elif a[0] == "press":
            out.append("sendkey " + "-".join(k(m) for m in a[1] + [a[2]]))
        else:
            out += [f"sleep {SWITCH_PAUSE}", "sendkey f12", f"wait shell: mark {a[2]:02d}", f"screendump {{out}}/{a[1]}.ppm"]
            last = None
    out += [f"sleep {SWITCH_PAUSE}", "sendkey esc"]
    return "\n".join(out) + "\n"


def live_autotype(lv, wait, pace):
    """The arguments for DOSBox's AUTOTYPE: each key by name, F12 at each frame after the first, Escape at the end."""
    k = lv.names("autotype")
    keys = []
    for a in lv.actions:
        if a[0] == "press":
            keys.append(k(a[2]))
        elif a[0] == "frame":
            keys.append("f12")
    keys.append("esc")
    return " ".join(["-w", str(wait), "-p", str(pace)] + keys) + "\n"


def live_xdotool(lv, quick):
    """The lines for tools/xvfb-dos.sh, each the arguments of one `xdotool` command (`#` lines are comments)."""
    k = lv.names("xdotool")
    actions = []
    for a in lv.actions:
        if quick and a == ("button", False) and actions and actions[-1] == ("button", True):
            actions[-1] = ("click",)
        else:
            actions.append(a)
    out = [f"# {os.path.basename(lv.path)} as input for DOSBox's window under Xvfb (frames.py live --xdotool{' --quick' if quick else ''})"]
    last = None
    for a, dev in _switches(actions):
        if dev and last and dev != last:
            out.append(f"sleep {SWITCH_PAUSE}")
        if dev:
            last = dev
        if a[0] == "move":
            out.append(f"mousemove_relative -- {a[1]} {a[2]}")
        elif a[0] == "button":
            out += [f"{'mousedown' if a[1] else 'mouseup'} 1", f"sleep {XDOTOOL_BUTTON_PAUSE}"]
        elif a[0] == "click":
            out += ["click 1", f"sleep {XDOTOOL_BUTTON_PAUSE}"]
        elif a[0] == "press":
            out.append("key " + "+".join(k(m) for m in a[1] + [a[2]]))
        else:
            out += [f"sleep {SWITCH_PAUSE}", "key F12", f"sleep {XDOTOOL_FRAME_PAUSE}"]
            last = None
    out += [f"sleep {SWITCH_PAUSE}", "key Escape"]
    return "\n".join(out) + "\n"


def cmd_live(args):
    emulator = "qemu" if args.qemu else "autotype" if args.autotype else "xdotool"
    if args.quick and emulator != "xdotool":
        raise UsageError("--quick is for --xdotool")
    lv = Live(args.scenario, emulator)
    if emulator == "qemu":
        sys.stdout.write(live_qemu(lv))
    elif emulator == "autotype":
        sys.stdout.write(live_autotype(lv, args.wait, args.pace))
    else:
        sys.stdout.write(live_xdotool(lv, args.quick))
    return 0


MARK_RE = re.compile(r"^shell: mark (\d{2}) (\d+) (\d+)x(\d+) ([0-9a-fA-F]{8}|\.{8}) ([0-9a-fA-F]{8}|\.{8})\s*$", re.M)


def cmd_marks(args):
    names = scenario_frames(args.scenario)
    sums = {s.name.lower(): s for s in read_sums(args.sums) if s.depth == args.depth}
    if not sums:
        raise FramesError(f"{args.sums}: no frames at depth {args.depth}")
    try:
        text = slurp(args.transcript, "r", encoding="utf-8", errors="replace").replace("\r", "")
    except OSError as err:
        raise FramesError(f"{args.transcript}: {err.strerror}") from None
    marks = MARK_RE.findall(text)
    ok = True
    for k, name in enumerate(names):
        want = sums.get(name.lower())
        if want is None:
            raise FramesError(f"{args.sums}: no line for frame {name} at depth {args.depth}")
        if k >= len(marks):
            print(f"FAIL {name}: mark {k:02d} is missing from the transcript")
            ok = False
            continue
        n, depth, w, h, index, colour = marks[k]
        if int(n) != k:
            print(f"FAIL {name}: expected mark {k:02d}, the transcript has {n} in its place")
            ok = False
            continue
        got = (int(depth), int(w), int(h), index.lower(), colour.lower())
        exp = (want.depth, want.width, want.height, f"{want.index:08x}", f"{want.colour:08x}")
        if got != exp:
            print(f"FAIL {name}: mark {n} has depth {depth} {w}x{h} {got[3]} {got[4]}, expected {exp[0]} {want.digest_text()}")
            ok = False
    for extra in marks[len(names) :]:
        print(f"FAIL mark {extra[0]}: an extra mark, the scenario has {len(names)} frames")
        ok = False
    if args.dumps:
        files = lookup(args.dumps)
        moves, complete = [], True
        for k, name in enumerate(names):
            for ext in ("RAW", "PAL"):
                src = files.get(f"m{k:02d}.{ext.lower()}")
                if src is None:
                    print(f"FAIL {name}: no dump M{k:02d}.{ext} in {args.dumps}")
                    ok = complete = False
                else:
                    moves.append((src, os.path.join(args.dumps, f"{name.upper()}.{ext}")))
        if complete:  # rename only a complete set (matching or not, so `check` can show what differs), never half of one
            for src, dst in moves:
                os.replace(src, dst)
    if ok:
        print(f"ok: {len(names)} marks in {args.transcript} carry the digests of {args.sums} at depth {args.depth}")
    return 0 if ok else 1


class UsageError(Exception):
    pass


def build_parser():
    p = argparse.ArgumentParser(prog="frames.py", description="Frame captures, digests, checks and diffs for the shell.")
    sub = p.add_subparsers(dest="command", required=True)

    def add(name, fn, *positional):
        s = sub.add_parser(name)
        for arg, kind in positional:
            s.add_argument(arg, type=kind)
        s.set_defaults(fn=fn)
        return s

    add("png", cmd_png, ("raw", str), ("pal", str), ("width", int), ("height", int), ("out", str))
    s = add("digest", cmd_digest, ("file", str))
    s.add_argument("pal", nargs="?")
    s.add_argument("width", nargs="?", type=int)
    s.add_argument("height", nargs="?", type=int)
    s = add("check", cmd_check, ("sums", str), ("depth", int), ("dir", str))
    s.add_argument("--golden", metavar="DIR")
    add("transcript", cmd_transcript, ("sums", str), ("depth", int), ("file", str))
    add("goldens", cmd_goldens, ("sums", str), ("dir", str))
    s = add("same", cmd_same, ("dir_a", str), ("dir_b", str))
    s.add_argument("--size", default="640x480", metavar="WxH")
    add("diff", cmd_diff, ("a", str), ("b", str), ("out", str))
    add("inject", cmd_inject, ("scenario", str))
    s = add("live", cmd_live, ("scenario", str))
    group = s.add_mutually_exclusive_group(required=True)
    group.add_argument("--qemu", action="store_true")
    group.add_argument("--autotype", action="store_true")
    group.add_argument("--xdotool", action="store_true")
    s.add_argument("--quick", action="store_true")
    s.add_argument("--wait", type=int, default=3, help="--autotype: seconds before the first key (AUTOTYPE -w)")
    s.add_argument("--pace", type=float, default=0.3, help="--autotype: seconds between keys (AUTOTYPE -p)")
    s = add("marks", cmd_marks, ("sums", str), ("scenario", str), ("depth", int), ("transcript", str))
    s.add_argument("--dumps", metavar="DIR")
    return p


def main(argv=None):
    try:
        args = build_parser().parse_args(argv)
    except SystemExit as exit_:
        return 2 if exit_.code else 0
    try:
        return args.fn(args)
    except UsageError as err:
        print(f"frames.py: {err}", file=sys.stderr)
        return 2
    except (FramesError, OSError) as err:
        print(f"frames.py: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
