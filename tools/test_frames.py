#!/usr/bin/env python3
# Tests for tools/frames.py: PNG and PPM reading and writing, the two digests, check/transcript/goldens/same/diff, and the monitor script.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tools/test_frames.py
import contextlib
import importlib.util
import io
import os
import re
import struct
import tempfile
import unittest
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
spec = importlib.util.spec_from_file_location("frames", os.path.join(HERE, "frames.py"))
frames = importlib.util.module_from_spec(spec)
spec.loader.exec_module(frames)


def slurp(path, mode="r"):
    with open(path, mode, **({} if "b" in mode else {"encoding": "utf-8"})) as f:
        return f.read()


def run(*argv):
    """(exit status, standard output) of `frames.py ARGV`; standard error is kept out of the way."""
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        status = frames.main([str(a) for a in argv])
    return status, out.getvalue()


def lcg(seed):
    """A fixed-seed byte generator, so the tests need no randomness."""
    state = seed
    while True:
        state = (state * 1103515245 + 12345) & 0x7FFFFFFF
        yield (state >> 16) & 0xFF


def write(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)


def ppm(width, height, rgb8):
    return b"P6\n%d %d\n255\n" % (width, height) + bytes(rgb8)


def six_bit_dac(entries):
    return bytes(v for c in entries for v in c)


# The 2 x 2 picture of shell/gfx/src/digest.rs: indices 0 1 2 3, palette 000000 3f0000 003f00 010203 (six-bit).
PAL4 = [(0, 0, 0), (63, 0, 0), (0, 63, 0), (1, 2, 3)]
INDICES4 = bytes([0, 1, 2, 3])
EXPANSIONS = {
    "v<<2|v>>4": lambda v: (v << 2) | (v >> 4),
    "v<<2": lambda v: v << 2,
    "v<<2|3": lambda v: (v << 2) | 3,
    "v*255/63": lambda v: v * 255 // 63,
}


class Pictures(unittest.TestCase):
    """A scratch directory, and a palette picture that uses many colours so a one-pixel change is a real change."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.dir = self.tmp.name

    def path(self, *parts):
        return os.path.join(self.dir, *parts)

    @staticmethod
    def picture(width=40, height=24, tweak=None):
        gen = lcg(7)
        indices = bytearray(next(gen) % 16 for _ in range(width * height))
        if tweak:
            x, y = tweak
            indices[y * width + x] = (indices[y * width + x] + 1) % 16
        pal6 = [((i * 5) % 64, (i * 11 + 3) % 64, (i * 17 + 9) % 64) for i in range(16)]
        return frames.Capture(width, height, indices=bytes(indices), pal6=pal6)

    @staticmethod
    def sums_line(name, depth, cap):
        return f"{name} {depth} {cap.digest_text()}\n"

    def make_set(self, names=("open", "moved"), depth=8):
        """A SUMS file with a golden PNG per frame, and the captures as PPMs in `dumps`; returns the pictures."""
        pics = {n: self.picture(tweak=(i + 1, i + 2)) for i, n in enumerate(names)}
        sums = ""
        for n, cap in pics.items():
            sums += self.sums_line(n, depth, cap)
            write(self.path("tests", "golden", "set", f"{n}-{depth}.png"), frames.png_indexed(cap.width, cap.height, cap.indices, cap.pal6))
            write(self.path("dumps", f"{n}.ppm"), self.as_ppm(cap))
        write(self.path("tests", "set.sums"), sums.encode())
        return pics

    @staticmethod
    def as_ppm(cap, expand=EXPANSIONS["v<<2|v>>4"]):
        return ppm(cap.width, cap.height, bytes(expand(v) for v in cap.rgb6))


class Digests(Pictures):
    def test_crc32_is_zlib_and_matches_the_rust_vectors(self):
        rust = slurp(os.path.join(ROOT, "shell", "gfx", "src", "digest.rs"))
        text_vectors = re.findall(r'assert_eq!\(crc32\(b"([^"]*)"\), (0x[0-9A-Fa-f_]+)\)', rust)
        byte_vectors = re.findall(r"assert_eq!\(crc32\(&\[([0-9, ]*)\]\), (0x[0-9A-Fa-f_]+)\)", rust)
        self.assertIn("123456789", [t for t, _ in text_vectors], "the Rust test vector moved; update this test")
        self.assertGreaterEqual(len(byte_vectors), 1)
        for text, value in text_vectors:
            self.assertEqual(frames.crc32(text.encode()), int(value.replace("_", ""), 16), text)
        for values, value in byte_vectors:
            data = bytes(int(v) for v in values.split(",")) if values.strip() else b""
            self.assertEqual(frames.crc32(data), int(value.replace("_", ""), 16), values)
        self.assertEqual(frames.crc32(b"123456789"), 0xCBF43926)
        for data in (b"", b"\0", b"123456789", bytes(range(256))):
            self.assertEqual(frames.crc32(data), zlib.crc32(data) & 0xFFFFFFFF)

    def test_the_known_two_by_two_digest_equals_the_rust_constants(self):
        rust = slurp(os.path.join(ROOT, "shell", "gfx", "src", "digest.rs"))
        index = int(re.search(r"const INDEX_2X2: u32 = (0x[0-9A-Fa-f_]+);", rust).group(1).replace("_", ""), 16)
        colour = int(re.search(r"const COLOUR_2X2: u32 = (0x[0-9A-Fa-f_]+);", rust).group(1).replace("_", ""), 16)
        cap = frames.Capture(2, 2, indices=INDICES4, pal6=PAL4)
        self.assertEqual((cap.index_digest(), cap.colour_digest()), (index, colour))
        self.assertEqual(cap.digest_text(), f"2x2 {index:08x} {colour:08x}")

    def test_a_ppm_of_known_colours_gives_the_expected_colour_digest(self):
        colours = [c for i in INDICES4 for c in PAL4[i]]
        want = zlib.crc32(bytes(colours)) & 0xFFFFFFFF
        for name, expand in EXPANSIONS.items():
            write(self.path("k.ppm"), ppm(2, 2, bytes(expand(v) for v in colours)))
            status, out = run("digest", self.path("k.ppm"))
            self.assertEqual((status, out), (0, f"2x2 - {want:08x}\n"), name)

    def test_raw_with_pal_gives_the_same_colour_digest_as_the_ppm_whatever_the_expansion(self):
        write(self.path("k.raw"), INDICES4)
        write(self.path("k.pal"), six_bit_dac(PAL4))
        status, out = run("digest", self.path("k.raw"), self.path("k.pal"), 2, 2)
        self.assertEqual(status, 0)
        index, colour = out.split()[1:]
        self.assertEqual(index, f"{frames.crc32(INDICES4):08x}")
        for name, expand in EXPANSIONS.items():
            write(self.path("k.ppm"), ppm(2, 2, bytes(expand(v) for i in INDICES4 for v in PAL4[i])))
            self.assertEqual(run("digest", self.path("k.ppm"))[1].split()[2], colour, name)

    def test_every_six_bit_value_survives_every_expansion(self):
        for name, expand in EXPANSIONS.items():
            row = bytes(expand(v) for v in range(64))
            write(self.path("all.ppm"), ppm(64, 1, bytes(b for v in row for b in (v, v, v))))
            cap = frames.read_ppm(self.path("all.ppm"))
            self.assertEqual(cap.rgb6, bytes(b for v in range(64) for b in (v, v, v)), name)

    def test_a_dac_dump_above_six_bits_or_the_wrong_size_is_refused(self):
        write(self.path("k.raw"), INDICES4)
        write(self.path("k.pal"), bytes([64]) + bytes(767))
        self.assertEqual(run("digest", self.path("k.raw"), self.path("k.pal"), 2, 2)[0], 1)
        write(self.path("k.pal"), six_bit_dac(PAL4))
        self.assertEqual(run("digest", self.path("k.raw"), self.path("k.pal"), 3, 2)[0], 1)
        write(self.path("k.raw"), bytes([0, 1, 2, 9]))
        self.assertEqual(run("digest", self.path("k.raw"), self.path("k.pal"), 2, 2)[0], 1, "an index past the palette")


class Png(Pictures):
    def test_a_png_written_then_read_gives_the_same_indices_and_palette(self):
        cap = self.picture()
        raw_path, pal_path, png_path = self.path("a.raw"), self.path("a.pal"), self.path("a.png")
        pal_bytes = six_bit_dac(cap.pal6) + bytes(768 - 3 * len(cap.pal6))
        write(raw_path, cap.indices)
        write(pal_path, pal_bytes)
        self.assertEqual(run("png", raw_path, pal_path, cap.width, cap.height, png_path), (0, ""))
        back = frames.read_png(png_path)
        self.assertEqual((back.width, back.height), (cap.width, cap.height))
        self.assertEqual(back.indices, cap.indices)
        self.assertEqual(back.pal6, [tuple(pal_bytes[i : i + 3]) for i in range(0, 768, 3)])
        self.assertEqual(back.rgb6, cap.rgb6)
        self.assertEqual(back.digest_text(), cap.digest_text())
        self.assertEqual(run("digest", png_path), (0, cap.digest_text() + "\n"))

    def test_the_png_palette_is_the_eight_bit_expansion_and_every_row_has_filter_zero(self):
        cap = self.picture(8, 4)
        data = frames.png_indexed(cap.width, cap.height, cap.indices, cap.pal6)
        plte = data[data.index(b"PLTE") + 4 :][: 3 * len(cap.pal6)]
        self.assertEqual(plte, bytes(v << 2 | v >> 4 for c in cap.pal6 for v in c))
        idat = data[data.index(b"IDAT") + 4 :]
        (length,) = struct.unpack(">I", data[data.index(b"IDAT") - 4 : data.index(b"IDAT")])
        rows = zlib.decompress(idat[:length])
        self.assertEqual([rows[y * 9] for y in range(4)], [0, 0, 0, 0])

    def test_the_writer_compresses(self):
        flat = frames.png_indexed(640, 480, bytes(640 * 480), [(0, 0, 0)])
        self.assertLess(len(flat), 2000)

    @staticmethod
    def filtered(kind, row, prev, bpp):
        out = bytearray()
        for i, x in enumerate(row):
            a = row[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            if kind == 0:
                pred = 0
            elif kind == 1:
                pred = a
            elif kind == 2:
                pred = b
            elif kind == 3:
                pred = (a + b) >> 1
            else:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pred = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
            out.append((x - pred) & 255)
        return bytes([kind]) + bytes(out)

    def encode(self, width, height, colour_type, pixels, kinds, plte=None, split_idat=False):
        bpp = 1 if colour_type == 3 else 3
        stride = width * bpp
        body, prev = b"", bytes(stride)
        for y in range(height):
            row = pixels[y * stride : (y + 1) * stride]
            body += self.filtered(kinds[y % len(kinds)], row, prev, bpp)
            prev = row
        packed = zlib.compress(body)
        parts = [packed[: len(packed) // 2], packed[len(packed) // 2 :]] if split_idat else [packed]
        out = b"\x89PNG\r\n\x1a\n" + frames.chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, colour_type, 0, 0, 0))
        if plte:
            out += frames.chunk(b"PLTE", plte)
        for part in parts:
            out += frames.chunk(b"IDAT", part)
        return out + frames.chunk(b"IEND", b"")

    def test_the_reader_handles_all_five_row_filters_indexed_and_rgb(self):
        gen = lcg(3)
        width, height = 13, 11
        indices = bytes(next(gen) % 16 for _ in range(width * height))
        plte = bytes(next(gen) for _ in range(48))
        rgb = bytes(next(gen) for _ in range(width * height * 3))
        for kind in range(5):
            data = self.encode(width, height, 3, indices, [kind], plte)
            write(self.path("i.png"), data)
            cap = frames.read_png(self.path("i.png"))
            self.assertEqual(cap.indices, indices, f"indexed, filter {kind}")
            self.assertEqual(cap.pal6[3], tuple(v >> 2 for v in plte[9:12]))
            data = self.encode(width, height, 2, rgb, [kind])
            write(self.path("c.png"), data)
            cap = frames.read_png(self.path("c.png"))
            self.assertIsNone(cap.indices)
            self.assertEqual(cap.rgb6, rgb.translate(frames.SIX_BIT), f"RGB, filter {kind}")
        mixed = self.encode(width, height, 3, indices, [0, 1, 2, 3, 4], plte, split_idat=True)
        write(self.path("m.png"), mixed)
        self.assertEqual(frames.read_png(self.path("m.png")).indices, indices, "all filters in one image, two IDAT chunks")

    def test_a_palette_of_more_than_256_entries_is_refused(self):
        write(self.path("big.png"), self.encode(2, 2, 3, bytes(4), [0], bytes(3 * 257)))
        self.assertEqual(run("digest", self.path("big.png"))[0], 1)

    def test_the_reader_refuses_what_it_does_not_read(self):
        good = self.encode(2, 2, 3, bytes(4), [0], bytes(6))
        write(self.path("good.png"), good)
        frames.read_png(self.path("good.png"))
        bad_crc = bytearray(good)
        bad_crc[-6] ^= 1
        interlaced = good.replace(struct.pack(">IIBBBBB", 2, 2, 8, 3, 0, 0, 0), struct.pack(">IIBBBBB", 2, 2, 8, 3, 0, 0, 1))
        sixteen = good.replace(struct.pack(">IIBBBBB", 2, 2, 8, 3, 0, 0, 0), struct.pack(">IIBBBBB", 2, 2, 16, 3, 0, 0, 0))
        for name, data in (("crc", bytes(bad_crc)), ("interlace", interlaced), ("16-bit", sixteen), ("not a png", b"GIF89a")):
            write(self.path("bad.png"), data)
            self.assertEqual(run("digest", self.path("bad.png"))[0], 1, name)


class Checks(Pictures):
    def test_check_passes_on_matching_captures(self):
        self.make_set()
        status, out = run("check", self.path("tests", "set.sums"), 8, self.path("dumps"))
        self.assertEqual(status, 0, out)

    def test_check_passes_for_every_expansion_of_a_ppm(self):
        pics = self.make_set()
        for name, expand in EXPANSIONS.items():
            for n, cap in pics.items():
                write(self.path("dumps", f"{n}.ppm"), self.as_ppm(cap, expand))
            self.assertEqual(run("check", self.path("tests", "set.sums"), 8, self.path("dumps"))[0], 0, name)

    def test_check_passes_on_a_raw_and_pal_and_on_a_png(self):
        pics = self.make_set()
        cap = pics["open"]
        os.remove(self.path("dumps", "open.ppm"))
        write(self.path("dumps", "open.raw"), cap.indices)
        write(self.path("dumps", "open.pal"), six_bit_dac(cap.pal6))
        cap = pics["moved"]
        os.remove(self.path("dumps", "moved.ppm"))
        write(self.path("dumps", "moved.png"), frames.png_indexed(cap.width, cap.height, cap.indices, cap.pal6))
        self.assertEqual(run("check", self.path("tests", "set.sums"), 8, self.path("dumps"))[0], 0)

    def test_names_match_without_case(self):
        pics = self.make_set()
        os.rename(self.path("dumps", "open.ppm"), self.path("dumps", "OPEN.PPM"))
        cap = pics["moved"]
        os.remove(self.path("dumps", "moved.ppm"))
        write(self.path("dumps", "MOVED.RAW"), cap.indices)
        write(self.path("dumps", "Moved.Pal"), six_bit_dac(cap.pal6))
        status, out = run("check", self.path("tests", "set.sums"), 8, self.path("dumps"))
        self.assertEqual(status, 0, out)
        # The SUMS line may be in capitals too.
        text = slurp(self.path("tests", "set.sums")).replace("open ", "OPEN ")
        write(self.path("tests", "set.sums"), text.encode())
        self.assertEqual(run("check", self.path("tests", "set.sums"), 8, self.path("dumps"))[0], 0)

    def test_a_one_pixel_change_fails_with_a_diff_png_and_the_box(self):
        pics = self.make_set()
        original = pics["open"]
        indices = bytearray(original.indices)
        indices[9 * original.width + 17] = (indices[9 * original.width + 17] + 1) % 16
        bumped = frames.Capture(original.width, original.height, indices=bytes(indices), pal6=original.pal6)
        self.assertNotEqual(bumped.rgb6, pics["open"].rgb6)
        write(self.path("dumps", "open.ppm"), self.as_ppm(bumped))
        status, out = run("check", self.path("tests", "set.sums"), 8, self.path("dumps"))
        self.assertEqual(status, 1)
        self.assertIn("FAIL open", out)
        self.assertIn("1 differing pixel,", out)
        self.assertIn("box x 17..17, y 9..9", out)
        diff = frames.read_png(self.path("dumps", "open.diff.png"))
        self.assertEqual((diff.width, diff.height), (40, 24))
        red = [i for i in range(40 * 24) if diff.rgb6[i * 3 : i * 3 + 3] == bytes([63, 0, 0])]
        self.assertEqual(red, [9 * 40 + 17])
        self.assertNotIn("FAIL moved", out)
        self.assertFalse(os.path.exists(self.path("dumps", "moved.diff.png")))

    def test_a_larger_change_reports_its_bounding_box(self):
        pics = self.make_set()
        cap = pics["moved"]
        indices = bytearray(cap.indices)
        for x, y in ((3, 4), (30, 20), (10, 7)):
            indices[y * cap.width + x] = (indices[y * cap.width + x] + 1) % 16
        write(self.path("dumps", "moved.ppm"), self.as_ppm(frames.Capture(cap.width, cap.height, indices=bytes(indices), pal6=cap.pal6)))
        status, out = run("check", self.path("tests", "set.sums"), 8, self.path("dumps"))
        self.assertEqual(status, 1)
        self.assertIn("3 differing pixels, box x 3..30, y 4..20", out)

    def test_an_index_only_change_fails_on_a_capture_with_indices(self):
        pics = self.make_set()
        cap = pics["open"]
        # Swap two palette entries' roles: the colours of every pixel stay, the indices do not.
        pal = [(1, 1, 1), (1, 1, 1)] + cap.pal6[2:]
        base = frames.Capture(cap.width, cap.height, indices=bytes(i % 2 for i in range(cap.width * cap.height)), pal6=pal)
        flipped = frames.Capture(cap.width, cap.height, indices=bytes(1 - (i % 2) for i in range(cap.width * cap.height)), pal6=pal)
        self.assertEqual(base.colour_digest(), flipped.colour_digest())
        write(self.path("tests", "set.sums"), self.sums_line("open", 8, base).encode())
        write(self.path("dumps", "open.ppm"), self.as_ppm(flipped))
        self.assertEqual(run("check", self.path("tests", "set.sums"), 8, self.path("dumps"))[0], 0, "a PPM has no indices to compare")
        os.remove(self.path("dumps", "open.ppm"))
        write(self.path("dumps", "open.raw"), flipped.indices)
        write(self.path("dumps", "open.pal"), six_bit_dac(pal))
        status, out = run("check", self.path("tests", "set.sums"), 8, self.path("dumps"))
        self.assertEqual(status, 1)
        self.assertIn("index digest", out)

    def test_a_missing_capture_other_depth_and_wrong_size_fail(self):
        self.make_set()
        os.remove(self.path("dumps", "moved.ppm"))
        status, out = run("check", self.path("tests", "set.sums"), 8, self.path("dumps"))
        self.assertEqual(status, 1)
        self.assertIn("FAIL moved: no capture", out)
        self.assertEqual(run("check", self.path("tests", "set.sums"), 4, self.path("dumps"))[0], 1, "no frames at depth 4")
        self.make_set()
        write(self.path("dumps", "open.ppm"), ppm(2, 2, bytes(12)))
        self.assertIn("size 2x2", run("check", self.path("tests", "set.sums"), 8, self.path("dumps"))[1])

    def test_depth_8_captures_fail_against_depth_4_sums(self):
        pics = self.make_set(depth=8)
        sums = "".join(self.sums_line(n, 4, self.picture(tweak=(9, 9))) for n in pics)
        write(self.path("tests", "set.sums"), sums.encode())
        self.assertEqual(run("check", self.path("tests", "set.sums"), 4, self.path("dumps"))[0], 1)

    def test_a_bad_sums_line_is_an_error_not_a_pass(self):
        write(self.path("bad.sums"), b"open 8 640x480 xyz\n")
        os.makedirs(self.path("d"))
        self.assertEqual(run("check", self.path("bad.sums"), 8, self.path("d"))[0], 1)


class Refusals(Pictures):
    def test_a_duplicate_sums_line_and_two_names_differing_in_case_are_refused(self):
        self.make_set()
        sums = self.path("tests", "set.sums")
        text = slurp(sums)
        write(sums, (text + text.splitlines()[0].replace("open", "OPEN") + "\n").encode())
        status, out = run("check", sums, 8, self.path("dumps"))
        self.assertEqual(status, 1)
        write(sums, text.encode())
        write(self.path("dumps", "OPEN.ppm"), slurp(self.path("dumps", "open.ppm"), "rb"))
        self.assertEqual(run("check", sums, 8, self.path("dumps"))[0], 1)


class Transcripts(Pictures):
    def setUp(self):
        super().setUp()
        self.pics = self.make_set()
        self.lines = [f"shell: frame {n} 8 {c.digest_text()}" for n, c in self.pics.items()]

    def transcript(self, lines):
        write(self.path("t.txt"), ("boot noise\r\n" + "\r\n".join(lines) + "\r\nshell: done\r\n").encode())
        return run("transcript", self.path("tests", "set.sums"), 8, self.path("t.txt"))

    def test_matching_digests_pass(self):
        self.assertEqual(self.transcript(self.lines)[0], 0)

    def test_a_changed_digest_is_found(self):
        lines = list(self.lines)
        lines[1] = lines[1][:-1] + ("0" if lines[1][-1] != "0" else "1")
        status, out = self.transcript(lines)
        self.assertEqual(status, 1)
        self.assertIn("FAIL moved", out)
        self.assertNotIn("FAIL open", out)

    def test_a_missing_extra_repeated_or_wrong_depth_frame_fails(self):
        self.assertIn("FAIL moved: not in the transcript", self.transcript(self.lines[:1])[1])
        self.assertIn("FAIL other", self.transcript(self.lines + ["shell: frame other 8 2x2 00000000 00000000"])[1])
        self.assertIn("appears twice", self.transcript(self.lines + self.lines[:1])[1])
        wrong = [line.replace(" 8 ", " 4 ", 1) for line in self.lines]
        self.assertEqual(self.transcript(wrong)[0], 1)
        self.assertEqual(run("transcript", self.path("tests", "set.sums"), 8, self.path("nothing.txt"))[0], 1)


class Goldens(Pictures):
    def test_goldens_pass_and_fail(self):
        self.make_set()
        sums, golden = self.path("tests", "set.sums"), self.path("tests", "golden", "set")
        self.assertEqual(run("goldens", sums, golden)[0], 0)
        # A PNG that disagrees with its line.
        other = self.picture(tweak=(5, 5))
        write(os.path.join(golden, "open-8.png"), frames.png_indexed(other.width, other.height, other.indices, other.pal6))
        status, out = run("goldens", sums, golden)
        self.assertEqual(status, 1)
        self.assertIn("FAIL open-8.png", out)

    def test_a_missing_png_and_an_extra_png_fail(self):
        pics = self.make_set()
        sums, golden = self.path("tests", "set.sums"), self.path("tests", "golden", "set")
        cap = pics["open"]
        png_bytes = slurp(os.path.join(golden, "open-8.png"), "rb")
        write(os.path.join(golden, "extra-8.png"), png_bytes)
        status, out = run("goldens", sums, golden)
        self.assertEqual(status, 1)
        self.assertIn("extra-8.png: no line", out)
        os.remove(os.path.join(golden, "extra-8.png"))
        os.remove(os.path.join(golden, "moved-8.png"))
        status, out = run("goldens", sums, golden)
        self.assertEqual(status, 1)
        self.assertIn("FAIL moved depth 8: no golden PNG", out)
        self.assertEqual(cap.width, 40)

    def test_golden_names_with_hyphens_and_capitals(self):
        cap = self.picture()
        write(self.path("g", "A-B-4.PNG"), frames.png_indexed(cap.width, cap.height, cap.indices, cap.pal6))
        write(self.path("s.sums"), self.sums_line("a-b", 4, cap).encode())
        self.assertEqual(run("goldens", self.path("s.sums"), self.path("g"))[0], 0)


class Same(Pictures):
    def test_same_and_not_same(self):
        pics = self.make_set()
        # Directory B holds the same frames as RAW plus PAL, and as PPMs with another expansion.
        cap = pics["open"]
        write(self.path("b", "OPEN.RAW"), cap.indices)
        write(self.path("b", "OPEN.PAL"), six_bit_dac(cap.pal6))
        write(self.path("b", "moved.ppm"), self.as_ppm(pics["moved"], EXPANSIONS["v<<2|3"]))
        status, out = run("same", self.path("dumps"), self.path("b"), "--size", "40x24")
        self.assertEqual(status, 0, out)
        write(self.path("b", "moved.ppm"), self.as_ppm(self.picture(tweak=(0, 0))))
        self.assertEqual(run("same", self.path("dumps"), self.path("b"), "--size", "40x24")[0], 1)
        os.remove(self.path("b", "moved.ppm"))
        status, out = run("same", self.path("dumps"), self.path("b"), "--size", "40x24")
        self.assertEqual(status, 1)
        self.assertIn("FAIL moved: only in", out)

    def test_the_default_raw_size_is_640_by_480(self):
        write(self.path("a", "f.raw"), bytes(640 * 480))
        write(self.path("a", "f.pal"), bytes(768))
        write(self.path("b", "f.ppm"), ppm(640, 480, bytes(640 * 480 * 3)))
        self.assertEqual(run("same", self.path("a"), self.path("b"))[0], 0)


class Diff(Pictures):
    def test_diff_marks_differences_in_red_over_a_dimmed_picture(self):
        a, b = self.picture(), self.picture(tweak=(4, 6))
        write(self.path("a.ppm"), self.as_ppm(a))
        write(self.path("b.png"), frames.png_indexed(b.width, b.height, b.indices, b.pal6))
        status, out = run("diff", self.path("a.ppm"), self.path("b.png"), self.path("d.png"))
        self.assertEqual(status, 0)
        self.assertIn("1 differing pixel, box x 4..4, y 6..6", out)
        d = frames.read_png(self.path("d.png"))
        at = (6 * a.width + 4) * 3
        self.assertEqual(d.rgb6[at : at + 3], bytes([63, 0, 0]))
        other = (6 * a.width + 5) * 3
        self.assertEqual(d.rgb6[other : other + 3], bytes(v >> 1 >> 2 for v in a.rgb6[other : other + 3].translate(frames.EXPAND)))
        write(self.path("small.ppm"), ppm(2, 2, bytes(12)))
        self.assertEqual(run("diff", self.path("a.ppm"), self.path("small.ppm"), self.path("d2.png"))[0], 1)


class Inject(unittest.TestCase):
    @staticmethod
    def example():
        text = slurp(os.path.join(ROOT, "docs", "FIRST-WINDOW.md"))
        m = re.search(r"```\n(desk-scenario 1\n.*?)```", text, re.S)
        assert m, "the example scenario moved in FIRST-WINDOW.md; update this test"
        return m.group(1)

    def script(self, text):
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "x.scn")
            write(path, text.encode())
            return run("inject", path)

    def test_inject_on_the_example_gives_one_triple_per_frame(self):
        status, out = self.script(self.example())
        self.assertEqual(status, 0)
        names = ["open", "moved", "focus", "dragged", "typed", "closed"]
        want = ["wait input: ready"]
        for n in names:
            want += [f"wait shell: ready {n}", f"screendump {{out}}/{n}.ppm", "sendkey spc"]
        self.assertEqual(out.splitlines(), want)
        self.assertTrue(out.endswith("\n"))

    def test_inject_output_is_in_the_format_native_inject_reads(self):
        source = slurp(os.path.join(HERE, "native-inject.py"))
        self.assertIn('line.startswith("wait ")', source)
        for line in self.script(self.example())[1].splitlines():
            self.assertTrue(line.startswith(("wait ", "screendump ", "sendkey ")), line)
            self.assertEqual(line, line.strip())

    def test_inject_ignores_comments_and_refuses_bad_or_overlapping_names(self):
        ok = "desk-scenario 1\nscreen 640 480\nframe a # a comment frame x\n# frame b\nframe b-c\n"
        status, out = self.script(ok)
        self.assertEqual(status, 0)
        self.assertEqual(out.count("wait shell: ready"), 2)
        for bad in ("frame Open", "frame abcdefghi", "frame", "frame a b"):
            self.assertEqual(self.script(f"desk-scenario 1\nscreen 1 1\n{bad}\n")[0], 1, bad)
        self.assertEqual(self.script("desk-scenario 1\nscreen 1 1\nframe ab\nframe a\n")[0], 1, "a prefix")
        self.assertEqual(self.script("desk-scenario 1\nscreen 1 1\nframe a\nframe a\n")[0], 1, "a repeat")
        self.assertEqual(self.script("desk-scenario 1\nscreen 1 1\n")[0], 1, "no frames")


SETUP = slurp(os.path.join(ROOT, "tests", "shell", "first-window.scn")).split("frame open")[0]
SETUP_LINES = len(SETUP.splitlines())


def live_scn(tmp, body, setup=SETUP, name="t.scn"):
    """A live scenario in `tmp`: the first window's set-up, `frame start`, then `body`; its directory also holds first-window.scn."""
    ref = os.path.join(tmp, "first-window.scn")
    if not os.path.exists(ref):
        write(ref, slurp(os.path.join(ROOT, "tests", "shell", "first-window.scn")).encode())
    path = os.path.join(tmp, name)
    write(path, (setup + "frame start\n" + body).encode())
    return path


def live_status(*argv):
    """(status, stdout, stderr) of `frames.py live ...`."""
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        status = frames.main([str(a) for a in argv])
    return status, out.getvalue(), err.getvalue()


class Live(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def scn(self, body, **kw):
        return live_scn(self.tmp.name, body, **kw)

    def lines(self, body, form, *extra):
        status, out, err = live_status("live", self.scn(body), form, *extra)
        self.assertEqual(status, 0, err)
        return [l for l in out.splitlines() if not l.startswith("#")]

    def refused(self, body, form, line, *extra, **kw):
        status, out, err = live_status("live", self.scn(body, **kw), form, *extra)
        self.assertEqual(status, 1, out)
        self.assertEqual(out, "", "a refused scenario prints no script")
        self.assertIn(f"t.scn:{SETUP_LINES + 1 + line}:", err)
        return err

    def test_the_qemu_script_waits_for_mark_zero_and_ends_with_escape(self):
        lines = self.lines("frame two\n", "--qemu")
        self.assertEqual(lines[:2], ["wait shell: mark 00", "screendump {out}/start.ppm"])
        self.assertEqual(lines[2:], ["sleep 0.3", "sendkey f12", "wait shell: mark 01", "screendump {out}/two.ppm", "sleep 0.3", "sendkey esc"])

    def test_qemu_moves_are_relative_to_the_tracked_pointer_and_split_at_100(self):
        # The set-up leaves the pointer at (320, 240).
        got = self.lines("pointer 120 70\nmove 250 -3\n", "--qemu")[2:]
        self.assertEqual(got[:4], ["mouse_move -100 -100", "sleep 0.1", "mouse_move -100 -70", "sleep 0.1"])
        self.assertEqual(got[4:], ["mouse_move 100 -3", "sleep 0.1", "mouse_move 100 0", "sleep 0.1", "mouse_move 50 0", "sleep 0.1"] + got[10:])

    def test_the_tracked_pointer_clamps_to_the_screen_as_the_shell_does(self):
        # Off the left edge: the shell clamps to x 0, so a later `pointer 10 240` is 10 to the right, not 410.
        got = self.lines("move -1000 0\npointer 10 240\n", "--qemu")[2:]
        self.assertEqual(got[0], "mouse_move -100 0")
        self.assertEqual(got.count("mouse_move -100 0"), 10)
        self.assertEqual(got[20:22], ["mouse_move 10 0", "sleep 0.1"])
        # A pointer step to where the pointer already is sends nothing.
        self.assertEqual(self.lines("pointer 320 240\n", "--qemu")[2:], ["sleep 0.3", "sendkey esc"])

    def test_qemu_buttons_and_the_pause_after_each_mouse_command(self):
        got = self.lines("press left\nrelease left\n", "--qemu")[2:]
        self.assertEqual(got, ["mouse_button 1", "sleep 0.1", "mouse_button 0", "sleep 0.1", "sleep 0.3", "sendkey esc"])

    def test_qemu_types_by_key_with_shift_for_capitals_and_symbols(self):
        got = self.lines('type "aZ 1!"\n', "--qemu")[2:-2]
        self.assertEqual(got, ["sendkey a", "sendkey shift-z", "sendkey spc", "sendkey 1", "sendkey shift-1"])

    def test_qemu_chords_and_plain_keys(self):
        body = "key LeftAlt down\nkey Tab down\nkey Tab up\nkey LeftAlt up\nkey F4 down\nkey F4 up\n" \
               "key LeftShift down\nkey LeftAlt down\nkey Tab down\nkey Tab up\nkey LeftAlt up\nkey LeftShift up\n"
        self.assertEqual(self.lines(body, "--qemu")[2:-2], ["sendkey alt-tab", "sendkey f4", "sendkey shift-alt-tab"])

    def test_a_mouse_step_and_a_key_step_are_a_third_of_a_second_apart_either_way_round(self):
        got = self.lines("press left\nrelease left\nkey A down\nkey A up\nmove 1 1\n", "--qemu")[2:]
        self.assertEqual(
            got,
            ["mouse_button 1", "sleep 0.1", "mouse_button 0", "sleep 0.1", "sleep 0.3", "sendkey a", "sleep 0.3", "mouse_move 1 1", "sleep 0.1", "sleep 0.3", "sendkey esc"],
        )

    def test_autotype_gives_the_arguments_with_f12_at_each_frame_but_the_first(self):
        status, out, err = live_status("live", self.scn('type "ok"\nframe a\nkey Backspace down\nkey Backspace up\nframe b\n'), "--autotype")
        self.assertEqual((status, out), (0, "-w 3 -p 0.3 o k f12 bspace f12 esc\n"))
        status, out, _ = live_status("live", self.scn("key Enter down\nkey Enter up\n"), "--autotype", "--wait", 5, "--pace", 0.5)
        self.assertEqual(out, "-w 5 -p 0.5 enter esc\n")

    def test_autotype_refuses_the_mouse_chords_and_shifted_characters(self):
        self.refused("move 1 1\n", "--autotype", 1)
        self.refused("pointer 1 1\n", "--autotype", 1)
        self.refused("press left\n", "--autotype", 1)
        self.refused("type \"a\"\nkey LeftAlt down\nkey Tab down\nkey Tab up\nkey LeftAlt up\n", "--autotype", 3)
        self.refused('type "A"\n', "--autotype", 1)

    def test_xdotool_lines(self):
        got = self.lines("move 5 -6\npress left\nrelease left\nkey LeftAlt down\nkey Tab down\nkey Tab up\nkey LeftAlt up\nframe a\n", "--xdotool")
        self.assertEqual(
            got,
            ["mousemove_relative -- 5 -6", "mousedown 1", "sleep 0.3", "mouseup 1", "sleep 0.3", "sleep 0.3", "key alt+Tab",
             "sleep 0.3", "key F12", "sleep 0.5", "sleep 0.3", "key Escape"],
        )

    def test_quick_makes_an_adjacent_press_and_release_one_click(self):
        got = self.lines("press left\nrelease left\npress left\nmove 1 1\nrelease left\n", "--xdotool", "--quick")
        self.assertEqual(got[:2], ["click 1", "sleep 0.3"])
        self.assertIn("mousedown 1", got)
        self.assertEqual(got.count("click 1"), 1, "a press and release with motion between them stay apart")
        self.assertEqual(self.lines("press left\nrelease left\n", "--xdotool").count("click 1"), 0)

    def test_quick_is_for_xdotool_only(self):
        status, _, err = live_status("live", self.scn("move 1 1\n"), "--qemu", "--quick")
        self.assertEqual(status, 2, err)

    def test_every_refusal_names_its_line(self):
        self.refused("open w 1 1 10 10 \"W\"\n", "--qemu", 1)
        self.refused("frame a\nclose welcome\n", "--qemu", 2)
        self.refused("focus notes\n", "--qemu", 1)
        self.refused("label notes 1 1 \"x\"\n", "--qemu", 1)
        self.refused("field notes 1 1 10\n", "--qemu", 1)
        self.refused("press right\n", "--qemu", 1)

    def test_a_lone_key_down_is_refused_with_its_line(self):
        self.refused("move 1 1\nkey LeftAlt down\n", "--qemu", 2)
        self.refused("key A down\nmove 1 1\n", "--qemu", 1)
        self.refused("key A up\n", "--qemu", 1)
        self.refused("key LeftAlt down\nkey Tab down\nkey Tab up\nframe a\nkey LeftAlt up\n", "--qemu", 4)
        self.refused("key LeftAlt down\nmove 1 1\nkey LeftAlt up\n", "--qemu", 2)
        self.refused("key LeftShift down\nkey LeftAlt down\nkey Tab down\nkey Tab up\nkey LeftShift up\nkey LeftAlt up\n", "--qemu", 5)

    def test_malformed_steps_are_refused_with_their_line_not_a_traceback(self):
        for body in ("key Tab\n", "move 1\n", "move a b\n", "frame\n", "press\n", "type\n", "key Tab dwon\n", "frame Open\n", "pointer 1 2 3\n"):
            status, out, err = live_status("live", self.scn(body), "--qemu")
            self.assertEqual((status, out), (1, ""), body)
            self.assertIn(f"t.scn:{SETUP_LINES + 2}:", err, body)
            self.assertNotIn("Traceback", err)

    def test_a_held_button_or_a_repeated_modifier_is_refused(self):
        self.refused("press left\n", "--qemu", 1)
        self.refused("press left\npress left\nrelease left\n", "--qemu", 2)
        self.refused("release left\n", "--qemu", 1)
        self.refused("key LeftAlt down\nkey LeftAlt down\nkey Tab down\nkey Tab up\nkey LeftAlt up\nkey LeftAlt up\n", "--qemu", 2)

    def test_unknown_keys_and_characters_are_refused(self):
        self.refused("key Nonsense down\n", "--qemu", 1)
        self.refused('type "é"\n', "--qemu", 1)
        self.refused("key Break down\nkey Break up\n", "--qemu", 1)

    def test_a_set_up_that_differs_from_the_first_windows_is_refused(self):
        bad = SETUP.replace("pointer 320 240", "pointer 300 240")
        status, out, err = live_status("live", self.scn("move 1 1\n", setup=bad), "--qemu")
        self.assertEqual((status, out), (1, ""))
        self.assertIn("not the first window's set-up", err)
        extra = SETUP + "label notes 1 1 \"extra\"\n"
        self.assertEqual(live_status("live", self.scn("move 1 1\n", setup=extra), "--xdotool")[0], 1)
        self.assertEqual(live_status("live", self.scn("move 1 1\n", setup=SETUP.replace("depths 8 4", "depths 8")), "--autotype")[0], 1)

    def test_a_scenario_with_no_frame_or_a_hundred_and_one_marks_is_refused(self):
        path = os.path.join(self.tmp.name, "n.scn")
        write(os.path.join(self.tmp.name, "first-window.scn"), slurp(os.path.join(ROOT, "tests", "shell", "first-window.scn")).encode())
        write(path, SETUP.encode())
        self.assertEqual(live_status("live", path, "--qemu")[0], 1)
        many = "".join(f"frame f{i}\n" for i in range(100))
        self.assertEqual(live_status("live", self.scn(many), "--qemu")[0], 1)

    def test_comments_and_strings_with_hashes(self):
        got = self.lines('# a comment\ntype "a#b" # trailing\nkey A down # c\nkey A up\n', "--qemu")[2:-2]
        self.assertEqual(got, ["sendkey a", "sendkey shift-3", "sendkey b", "sendkey a"])

    def test_the_key_table_covers_every_key_a_scenario_can_name(self):
        names = set(re.findall(r"^    (\w+),$", slurp(os.path.join(ROOT, "shell", "shell", "src", "scenario.rs")).split("key_table!(")[1].split(");")[0], re.M))
        self.assertTrue({"A", "Tab", "LeftAlt", "F12", "Kp9", "KpEnter"} <= names)
        missing = names - set(frames.KEY_NAMES) - {"Break"}
        self.assertEqual(missing, set(), "a key the scenarios can name has no emulator names")

    def test_every_typeable_character_has_a_key_on_each_emulator(self):
        for code in range(32, 127):
            c = chr(code)
            k = frames.key_for_char(c)
            self.assertIsNotNone(k, repr(c))
            self.assertIn(k[0], frames.KEY_NAMES)
        self.assertIsNone(frames.key_for_char("é"))

    def test_qemu_key_names_are_distinct_and_the_ones_the_design_names(self):
        q = {n: v[0] for n, v in frames.KEY_NAMES.items()}
        self.assertEqual(len(set(q.values())), len(q), "two keys share a QEMU name")
        for name, want in {"Space": "spc", "Enter": "ret", "Backspace": "backspace", "Tab": "tab", "F12": "f12", "Escape": "esc",
                           "LeftAlt": "alt", "LeftShift": "shift"}.items():
            self.assertEqual(q[name], want)

    def test_the_committed_live_scenarios_translate_in_the_forms_the_checks_use(self):
        shell = os.path.join(ROOT, "tests", "shell")
        for form in ("--qemu", "--xdotool"):
            self.assertEqual(live_status("live", os.path.join(shell, "live.scn"), form)[0], 0, form)
        for form in ("--qemu", "--autotype", "--xdotool"):
            self.assertEqual(live_status("live", os.path.join(shell, "live-keys.scn"), form)[0], 0, form)
        status, _, err = live_status("live", os.path.join(shell, "live.scn"), "--autotype")
        self.assertEqual(status, 1)
        self.assertIn("live.scn:13:", err)


class Marks(unittest.TestCase):
    """`frames.py marks` against a small scenario and its SUMS."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        d = self.tmp.name
        self.scn = live_scn(d, "frame b\nframe c\n")
        self.sums = os.path.join(d, "t.sums")
        write(self.sums, b"start 8 640x480 11111111 aaaaaaaa\nb 8 640x480 22222222 bbbbbbbb\nc 8 640x480 33333333 cccccccc\nstart 4 640x480 44444444 dddddddd\nb 4 640x480 55555555 eeeeeeee\nc 4 640x480 66666666 ffffffff\n")
        self.good = [
            "shell: mark 00 8 640x480 11111111 aaaaaaaa",
            "shell: mark 01 8 640x480 22222222 bbbbbbbb",
            "shell: mark 02 8 640x480 33333333 cccccccc",
        ]

    def transcript(self, lines):
        path = os.path.join(self.tmp.name, "serial.txt")
        write(path, ("boot\nshell: live, depth 8\n" + "\n".join(lines) + "\nshell: live ended by Escape: 3 keys\n").encode())
        return path

    def marks(self, lines, depth=8, *extra):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            status = frames.main(["marks", self.sums, self.scn, str(depth), self.transcript(lines), *extra])
        return status, out.getvalue()

    def test_matching_marks_pass_and_dos_line_ends_are_fine(self):
        status, out = self.marks(self.good)
        self.assertEqual(status, 0, out)
        self.assertEqual(self.marks([l + "\r" for l in self.good])[0], 0)

    def test_the_same_marks_fail_at_the_other_depth(self):
        status, out = self.marks(self.good, 4)
        self.assertEqual(status, 1)
        self.assertIn("FAIL start", out)

    def test_a_missing_mark_fails(self):
        status, out = self.marks(self.good[:2])
        self.assertEqual(status, 1)
        self.assertIn("FAIL c: mark 02 is missing", out)
        status, out = self.marks([self.good[0], self.good[2]])
        self.assertEqual(status, 1, "the second mark is missing even if a later one is there")
        self.assertIn("FAIL", out)

    def test_an_extra_mark_fails(self):
        status, out = self.marks(self.good + ["shell: mark 03 8 640x480 11111111 aaaaaaaa"])
        self.assertEqual(status, 1)
        self.assertIn("mark 03: an extra mark", out)

    def test_a_wrong_digest_names_the_frame_and_both_digests(self):
        bad = list(self.good)
        bad[1] = "shell: mark 01 8 640x480 22222222 bbbbbbb0"
        status, out = self.marks(bad)
        self.assertEqual(status, 1)
        self.assertIn("FAIL b", out)
        self.assertIn("bbbbbbb0", out)
        self.assertIn("bbbbbbbb", out)
        bad[1] = "shell: mark 01 8 640x480 22222223 bbbbbbbb"
        self.assertEqual(self.marks(bad)[0], 1, "the index digest counts")

    def test_a_transcript_with_no_marks_or_dots_for_digests_fails(self):
        self.assertEqual(self.marks([])[0], 1)
        bad = [self.good[0], "shell: mark 01 8 640x480 ........ ........", self.good[2]]
        self.assertEqual(self.marks(bad)[0], 1)

    def test_dumps_are_renamed_to_the_frame_names(self):
        dumps = os.path.join(self.tmp.name, "dumps")
        for k in range(3):
            write(os.path.join(dumps, f"M{k:02d}.RAW"), bytes([k]))
            write(os.path.join(dumps, f"M{k:02d}.PAL"), bytes([k + 10]))
        status, out = self.marks(self.good, 8, "--dumps", dumps)
        self.assertEqual(status, 0, out)
        self.assertEqual(sorted(os.listdir(dumps)), ["B.PAL", "B.RAW", "C.PAL", "C.RAW", "START.PAL", "START.RAW"])
        self.assertEqual(slurp(os.path.join(dumps, "C.PAL"), "rb"), bytes([12]))

    def test_a_wrong_digest_still_renames_a_complete_set_so_check_can_show_the_difference(self):
        dumps = os.path.join(self.tmp.name, "dumps")
        for k in range(3):
            write(os.path.join(dumps, f"M{k:02d}.RAW"), bytes([k]))
            write(os.path.join(dumps, f"M{k:02d}.PAL"), bytes([k]))
        bad = list(self.good)
        bad[2] = "shell: mark 02 8 640x480 33333333 cccccc00"
        self.assertEqual(self.marks(bad, 8, "--dumps", dumps)[0], 1)
        self.assertIn("C.RAW", os.listdir(dumps))

    def test_an_incomplete_set_of_dumps_is_left_alone(self):
        dumps = os.path.join(self.tmp.name, "dumps")
        for k in range(2):
            write(os.path.join(dumps, f"M{k:02d}.RAW"), bytes([k]))
            write(os.path.join(dumps, f"M{k:02d}.PAL"), bytes([k]))
        self.assertEqual(self.marks(self.good, 8, "--dumps", dumps)[0], 1)
        self.assertEqual(sorted(os.listdir(dumps)), ["M00.PAL", "M00.RAW", "M01.PAL", "M01.RAW"], "nothing is renamed")

    def test_the_inject_and_marks_commands_read_scenarios_as_live_does(self):
        path = live_scn(self.tmp.name, "frame abcdefghi\n")
        self.assertEqual(live_status("live", path, "--qemu")[0], 1)
        self.assertEqual(run("inject", path)[0], 1)

    def test_a_missing_dump_fails(self):
        dumps = os.path.join(self.tmp.name, "dumps")
        write(os.path.join(dumps, "M00.RAW"), b"x")
        status, out = self.marks(self.good, 8, "--dumps", dumps)
        self.assertEqual(status, 1)
        self.assertIn("no dump M00.PAL", out)
        self.assertIn("no dump M01.RAW", out)


class Usage(unittest.TestCase):
    def test_usage_errors_exit_with_status_2(self):
        for argv in ([], ["bogus"], ["check", "a", "b"], ["check", "a", "x", "c"], ["diff", "a"], ["same", "a"]):
            self.assertEqual(run(*argv)[0], 2, argv)

    def test_digest_arguments_are_checked(self):
        with tempfile.TemporaryDirectory() as tmp:
            write(os.path.join(tmp, "a.raw"), bytes(4))
            self.assertEqual(run("digest", os.path.join(tmp, "a.raw"))[0], 2, "a RAW needs its PAL and size")
            write(os.path.join(tmp, "a.ppm"), ppm(1, 1, bytes(3)))
            self.assertEqual(run("digest", os.path.join(tmp, "a.ppm"), "pal", 1, 1)[0], 2)
            self.assertEqual(run("digest", os.path.join(tmp, "missing.png"))[0], 1)


if __name__ == "__main__":
    unittest.main(verbosity=1)
