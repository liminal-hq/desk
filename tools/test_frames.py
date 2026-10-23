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
