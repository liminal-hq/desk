# Checks of the fonts chapter (M18) and the emulators chapter (M20): the numbers they quote from the font files and the emulator
# scripts and documents, against the files themselves.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import os
import re
import struct
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
TOOL = os.path.normpath(os.path.join(HERE, ".."))
sys.path.insert(0, TOOL)

import gather  # noqa: E402
import manual  # noqa: E402


def rd(*parts):
    with open(os.path.join(gather.ROOT, *parts), encoding="utf-8") as f:
        return f.read()


def rb(*parts):
    with open(os.path.join(gather.ROOT, *parts), "rb") as f:
        return f.read()


def walk_dfn(d):
    """(height, baseline, [(code point, width)]) of a .DFN file; the walk must end exactly at the end of the file."""
    assert d[:4] == b"DFN1"
    h, base, n = d[4], d[5], struct.unpack("<H", d[6:8])[0]
    at, glyphs = 8, []
    for _ in range(n):
        cp, w = struct.unpack("<I", d[at:at + 4])[0], d[at + 4]
        glyphs.append((cp, w))
        at += 5 + h * ((w + 7) // 8)
    assert at == len(d), "the walk ended at %d of %d" % (at, len(d))
    return h, base, glyphs


def dem_directory(d):
    n = struct.unpack("<H", d[6:8])[0]
    return [struct.unpack("<II", d[8 + 8 * i:16 + 8 * i]) for i in range(n)]


class FontsChapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        cls.fonts = cls.chapters["m-fonts"]["raw"]

    def test_dfn_files_have_the_size_the_formula_gives(self):
        want = {"ui": (14, 11, 289, 5639, 2, 11), "title": (14, 12, 289, 5975, 3, 12),
                "fixed": (16, 13, 332, 6980, 8, 8), "small": (8, 7, 289, 3765, 6, 6)}
        for name, (h, base, n, size, lo, hi) in want.items():
            d = rb("fonts", "generated", name + ".dfn")
            height, baseline, glyphs = walk_dfn(d)
            self.assertEqual((height, baseline, len(glyphs), len(d)), (h, base, n, size), name)
            widths = [w for _, w in glyphs]
            self.assertEqual((min(widths), max(widths)), (lo, hi), name)
            self.assertEqual(8 + sum(5 + h * ((w + 7) // 8) for w in widths), size)
            self.assertEqual(glyphs[-1][0], 0x1F4C1)
            self.assertEqual([c for c, _ in glyphs], sorted(set(c for c, _ in glyphs)))
            for number in (n, size):
                self.assertIn("{:,}".format(number), self.fonts, name)
        self.assertEqual(332 - 289, 43)
        self.assertIn("43 more glyphs", self.fonts)
        self.assertEqual(max(w for name in want for _, w in walk_dfn(rb("fonts", "generated", name + ".dfn"))[2]), 12)

    def test_the_cell_rows_agree_with_the_baseline_bytes(self):
        # "baseline under row R" in the table means R + 1 rows above it
        for face, under in (("ui", 10), ("title", 11), ("fixed", 12), ("small", 6)):
            self.assertEqual(walk_dfn(rb("fonts", "generated", face + ".dfn"))[1], under + 1)
            self.assertRegex(rd("fonts", "src", "faces", face + ".ts"), r"ascent: %d," % (under + 1))

    def test_dem_headers_match_the_tiers(self):
        manifest = rd("fonts", "src", "emoji", "manifest.ts")
        body = manifest.split("export const MANIFEST", 1)[1]
        tiers = {t: len(re.findall(r"'%s'\)" % t, body)) for t in ("core", "essential", "extended")}
        self.assertEqual(tiers, {"core": 40, "essential": 40, "extended": 400})
        counts = {}
        for size in (16, 12, 8):
            d = rb("fonts", "generated", "emoji", "emoji%d.dem" % size)
            self.assertEqual((d[:4], d[4], d[5]), (b"DEM1", size, 0))
            counts[size] = len(dem_directory(d))
        self.assertEqual(counts, {16: 480, 12: 480, 8: 40})
        self.assertEqual(sum(tiers.values()), 480)
        self.assertEqual(counts[8], tiers["core"])
        for needle in ("| Core | 40 |", "| Essential | 40 |", "| Extended | 400 |", "`0x0028` = 40"):
            self.assertIn(needle, self.fonts)

    def test_dem_records_have_the_length_the_formula_gives(self):
        layers = []
        for size in (16, 12, 8):
            d = rb("fonts", "generated", "emoji", "emoji%d.dem" % size)
            directory = dem_directory(d)
            rowb = (size + 7) // 8
            self.assertEqual(directory[0][1], 8 + 8 * len(directory))
            self.assertEqual([c for c, _ in directory], sorted(c for c, _ in directory))
            for i, (cp, off) in enumerate(directory):
                end = directory[i + 1][1] if i + 1 < len(directory) else len(d)
                n = d[off]
                layers.append(n)
                self.assertEqual(end - off, 1 + n * (1 + size * rowb) + size * rowb, hex(cp))
                if size == 16:
                    self.assertEqual(end - off, 1 + n * 33 + 32)
        self.assertEqual((min(layers), max(layers)), (1, 9))

    def test_the_worked_example_is_the_floppy_disk(self):
        d8 = rb("fonts", "generated", "emoji", "emoji8.dem")
        dire = dem_directory(d8)
        i = [c for c, _ in dire].index(0x1F4BE)
        self.assertEqual((i, dire[i][1], dire[i + 1][1] - dire[i][1], d8[dire[i][1]]), (23, 1030, 54, 5))
        self.assertEqual(8 + 8 * i, 192)
        self.assertEqual(d8[:8].hex(" "), "44 45 4d 31 08 00 28 00")
        self.assertEqual(d8[192:200].hex(" "), "be f4 01 00 06 04 00 00")
        self.assertEqual(1 + 5 * 9 + 8, 54)
        slots = []
        p = dire[i][1] + 1
        for _ in range(5):
            slots.append(d8[p])
            p += 9
        self.assertEqual(slots, [0, 15, 7, 6, 4])
        self.assertEqual(format(d8[dire[i][1] + 2], "08b"), "11111110")
        d16 = rb("fonts", "generated", "emoji", "emoji16.dem")
        dire16 = dem_directory(d16)
        j = [c for c, _ in dire16].index(0x1F4BE)
        self.assertEqual((j, dire16[j][1], dire16[j + 1][1] - dire16[j][1], d16[dire16[j][1]]), (240, 43844, 198, 5))
        self.assertEqual(1 + 5 * 33 + 32, 198)
        for needle in ("entry 240", "43,844", "198 bytes", "1,030"):
            self.assertIn(needle, self.fonts)

    def test_the_build_writes_the_files_the_chapter_counts(self):
        files = []
        for root, _, names in os.walk(os.path.join(gather.ROOT, "fonts", "generated")):
            files += [n for n in names if n != "OFL.txt"]
        self.assertEqual(len(files), 58)
        top = os.listdir(os.path.join(gather.ROOT, "fonts", "generated"))
        self.assertEqual(len([n for n in top if n.endswith((".otf", ".ttf"))]), 44)
        self.assertEqual(len([n for n in top if n.endswith(".dfn")]), 4)
        self.assertEqual(len(os.listdir(os.path.join(gather.ROOT, "fonts", "generated", "emoji"))), 10)
        self.assertEqual(4 * 2 * 2 + 2 * 2 + 12 * 2, 44)
        self.assertEqual(len([n for n in top if re.match(r"desk-(sans|mono|serif)", n)]), 24)
        self.assertIn("The result is 58 files", self.fonts)

    def test_the_encoders_are_as_the_chapter_describes(self):
        dfn = rd("fonts", "src", "dfn.ts")
        self.assertIn("5 + face.height * rowBytes(g.adv)", dfn)
        self.assertIn("if (g.adv > 255)", dfn)
        dem = rd("fonts", "src", "emoji", "dem.ts")
        self.assertIn("0x44, 0x45, 0x4d, 0x31", dem)
        self.assertIn("let offset = 8 + 8 * items.length;", dem)
        self.assertIn("1 + layers × 33 + 32", rd("docs", "FORMATS.md"))
        build = rd("fonts", "build.ts")
        for name in ("buildPixel", "buildModern", "buildEmoji", "dosGlyphs", "encodeDfn"):
            self.assertIn(name, build)
        self.assertIn("Round is outline-only", build)

    def test_every_planned_format_is_in_formats_md_and_nothing_reads_it(self):
        formats = rd("docs", "FORMATS.md")
        for head in ("APP.INF: one app", ".DAP: an app archive", "CATALOG.INF: a disk of apps", ".DIF: a DOS program's information",
                     ".ICO: an icon", "DESK.INI: settings and layout", "DESK.RUN: the handoff", ".M3U: a playlist",
                     ".BMP: pictures from Paint", ".WAV and .VOC: sounds", "Mines best times", "Reminders"):
            self.assertIn("## " + head, formats)
        planned = [r for r in self.chapters["m-fonts"]["status"] if r["state"] == "planned"]
        self.assertEqual(len(planned), 9)
        for r in planned:
            kind, rest = r["items"][0]
            self.assertEqual(kind, "doc")
            self.assertTrue(rest.startswith("docs/"))
        # nothing in the code names their magic strings or file names
        markers = ("APP.INF", "DAP1", "CATALOG.INF", "DIC1", "DESK.INI", "DESK.RUN", "EXTM3U")
        skip = {"node_modules", "dist", "target", "progress-report", ".git", ".claude", "docs", "vendor"}
        for root, dirs, names in os.walk(gather.ROOT):
            dirs[:] = [d for d in dirs if d not in skip]
            for n in names:
                if n.endswith((".rs", ".c", ".h", ".py", ".sh", ".ts", ".asm")) and n != "test_chapter_fonts_emulators.py":
                    with open(os.path.join(root, n), encoding="utf-8", errors="replace") as f:
                        text = f.read()
                    for m in markers:
                        self.assertNotIn(m, text, "%s names %s" % (os.path.join(root, n), m))


class EmulatorsChapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        cls.emu = cls.chapters["m-emulators"]["raw"]
        cls.doc = rd("docs", "NATIVE-EMULATORS.md")

    def table_rows(self, text, start_header):
        lines = text.splitlines()
        i = next(k for k, l in enumerate(lines) if l.startswith(start_header))
        rows = []
        for l in lines[i + 2:]:
            if not l.startswith("|"):
                break
            rows.append(l)
        return rows

    def test_the_results_table_is_copied_from_the_document(self):
        header = "| Emulator | BIOS | CPU model | Result | Differences from the QEMU transcript |"
        source = self.table_rows(self.doc, header)
        mine = self.table_rows(self.emu, header)
        self.assertEqual(len(source), 7)
        # every row of the chapter is a row of the document, word for word; only the Pentium-model Bochs row is left out
        self.assertEqual(len(mine), 6)
        for row in mine:
            self.assertIn(row, source)
        left_out = [r for r in source if r not in mine]
        self.assertEqual(len(left_out), 1)
        self.assertIn("Ubuntu's package", left_out[0])
        self.assertIn("docs/NATIVE-EMULATORS.md", self.emu)

    def test_the_cpu_probe_table_is_the_documents(self):
        header = "| Instruction | QEMU `-cpu 486` | Bochs 486 |"
        source = self.table_rows(self.doc, header)
        mine = self.table_rows(self.emu, header)
        self.assertEqual(mine, source)
        probe = rd("tests", "native", "probe.asm")
        self.assertIn("probe: #UD", probe)
        self.assertIn("probe: executed", probe)
        bochs_test = rd("tools", "native-test-bochs.sh")
        for spec in ("rdtsc", "rdmsr", "emms", "mov-from-cr4", "ud2"):
            self.assertIn(spec, bochs_test)

    def test_the_qemu_arguments_are_the_scripts(self):
        run = rd("tools", "native-run.sh")
        for arg in ("-cpu 486", "-m 16", "-display none", "-device isa-debug-exit,iobase=0xf4,iosize=0x04", "-no-reboot",
                    "-monitor none", "unix:$sockdir/monitor,server,nowait", "-serial \"file:$serial\""):
            self.assertIn(arg, run, arg)
        for status in ("33) echo", "35) echo", "124) echo"):
            self.assertIn(status, run)
        self.assertEqual((0x10 << 1) | 1, 33)
        self.assertEqual((0x11 << 1) | 1, 35)
        self.assertIn("(0x11 << 1) | 1 = 35", self.chapters["m-boot"]["raw"])
        self.assertIn("`(0x11 << 1) | 1 = 35`", self.emu)
        self.assertRegex(rd("kernel", "src", "exit.rs"), r"0x11")

    def test_the_bochs_and_86box_runners_decide_as_the_chapter_says(self):
        bochs = rd("tools", "native-run-bochs.sh")
        self.assertIn("sleep 0.2", bochs)
        self.assertIn('"$stable" -ge 5', bochs)
        self.assertIn("reset_on_triple_fault=0", bochs)
        self.assertIn("port_e9_hack: enabled=1", bochs)
        self.assertIn("display_library: nogui", bochs)
        for f in ("BIOS-bochs-legacy", "BIOS-bochs-latest", "bios.bin-1.13.0"):
            self.assertIn(f, bochs)
        self.assertIn("--enable-cpu-level=4 --with-nogui --disable-debugger --disable-plugins", rd("docs", "NATIVE-EMULATORS.md"))
        self.assertIn("enable-cpu-level=4", rd("tools", "docker", "Dockerfile.native-bochs"))
        cfg = rd("tools", "86box", "86box.cfg")
        for line in ("machine = ami471", "cpu_family = i486dx", "cpu_speed = 33333333", "mem_size = 16384", "fdd_01_type = 35_2hd"):
            self.assertIn(line, cfg)
        run = rd("tools", "86box", "run-in-container.sh")
        for piece in ("Xvfb :99", "xdotool key Return", "import -window root", "% 25"):
            self.assertIn(piece, run)
        self.assertEqual(len(bytes.fromhex(rd("tools", "86box", "ami471.nvr.hex").strip())), 128)

    def test_dosrun_is_as_the_chapter_says(self):
        dos = rd("tools", "dosrun.sh")
        for piece in ("SDL_VIDEODRIVER=dummy", "SDL_AUDIODRIVER=dummy", "output = texture", "cputype = 486", "core = normal",
                      "machine = ${DOS_MACHINE:-svga_s3}", "seq 1 255", "if errorlevel $n set RC=$n", "DOS_AUTOTYPE", "DOS_X11",
                      "${DOS_TIMEOUT:-60}", "exit 125", "exit 124"):
            self.assertIn(piece, dos, piece)

    def test_the_documents_agree_on_what_the_chapter_quotes(self):
        self.assertIn("emulate DOS, not PC hardware", rd("docs", "OPEN-QUESTIONS.md"))
        self.assertIn("Emulates DOS, not PC hardware", rd("docs", "DIRECTION.md"))
        s5 = rd("docs", "S5-JSDOS.md")
        for piece in ("js-dos 8.5.3", "18 scenarios", "Chromium 154", "DOSBox Staging 0.83.0"):
            self.assertIn(piece, s5)
        self.assertIn("0x60000033", self.doc)
        self.assertIn("0x60000033", self.emu)
        self.assertIn("about 20 seconds", self.doc)
        self.assertIn("This is the emulator that found a bug", self.emu)

    def test_the_status_records_say_what_was_not_run(self):
        recs = {r["id"]: r for r in self.chapters["m-emulators"]["status"]}
        for key in ("emu.bochs", "emu.86box", "emu.jsdos"):
            self.assertEqual(recs[key]["state"], "built-unverified")
        self.assertEqual(recs["emu.hardware"]["state"], "planned")
        self.assertIn("real hardware", recs["emu.hardware"]["note"])


if __name__ == "__main__":
    unittest.main()
