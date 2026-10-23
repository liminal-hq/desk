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
        return {f.lower(): os.path.join(directory, f) for f in os.listdir(directory)}
    except OSError as err:
        raise FramesError(f"{directory}: {err.strerror}") from None


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
            count, box = differences(load(gold), cap)
        except FramesError as err:
            print(f"  cannot diff: {err}")
            continue
        if count:
            out = os.path.join(args.dir, f"{want.name}.diff.png")
            write_png(out, diff_picture(load(gold), cap))
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
    try:
        text = slurp(path, "r", encoding="utf-8")
    except OSError as err:
        raise FramesError(f"{path}: {err.strerror}") from None
    names = []
    for number, raw in enumerate(text.splitlines(), 1):
        words = raw.split("#", 1)[0].split()
        if len(words) < 1 or words[0] != "frame":
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
