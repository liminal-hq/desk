# Checks of the hosted runtime chapter (M17) and the toolchain chapter (M19): the numbers and tables they quote, against the sources.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import json
import os
import re
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


def num(n):
    return "{:,}".format(n)


class RuntimeChapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        cls.rt = cls.chapters["m-rt"]["raw"]
        cls.d0 = rd("docs", "D0-RUST-DOS.md")
        cls.src = {f: rd("spikes", "d0-rust-dos", "rt", "src", f) for f in os.listdir(os.path.join(gather.ROOT, "spikes", "d0-rust-dos", "rt", "src"))}

    def test_sizes_quoted_from_a_document_are_in_that_document(self):
        for size in (49664, 70144, 21325, 236072, 115712):
            self.assertIn(num(size), self.d0, size)
            self.assertIn(num(size), self.rt, size)
        s1 = rd("docs", "S1-DXE3.md")
        for size in (7734, 22853, 8157):
            self.assertIn(str(size), s1)
            self.assertIn(num(size), self.rt)
        self.assertIn("102,400", rd("docs", "FIRST-WINDOW.md"))
        self.assertIn("102,400", self.rt)

    def test_constants_the_prose_states(self):
        self.assertIn("TB_PARAGRAPHS: u16 = 0x800;", self.src["dos.rs"])
        self.assertEqual(0x800 * 16, 32768)
        self.assertIn("const CHUNK: usize = 256 * 1024;", self.src["heap.rs"])
        self.assertIn("const HEADER: usize = 8;", self.src["heap.rs"])
        self.assertIn("const MIN_NODE: usize = 16;", self.src["heap.rs"])
        self.assertIn("const SLOTS: usize = 8;", self.src["cleanup.rs"])
        self.assertIn("STACK_SIZE: usize = 256 * 1024;", self.src["start.rs"])
        self.assertIn("const STACK_SIZE: usize = 8192;", self.src["irq.rs"])
        self.assertIn("mov ecx, 21", self.src["start.rs"])
        self.assertEqual(21 * 4, 0x54)
        self.assertIn("const BUFFER_PARAGRAPHS: u16 = 32;", self.src["hal.rs"])
        self.assertEqual(rd("hal", "hal", "src", "bios.rs").count("pub const BIOS_BUFFER_MAX: usize = 512;"), 1)
        self.assertIn("exit_now(3)", self.src["panic.rs"])
        self.assertIn("exit_now(255)", self.src["dos.rs"])
        regs = re.search(r"pub struct EntryRegs \{(.*?)\n\}", self.src["start.rs"], re.S).group(1)
        self.assertEqual(len(re.findall(r"pub \w+: u32", regs)), 17)

    def test_the_worked_example_arithmetic(self):
        size = 900 << 20
        self.assertEqual(size, 943718400)
        need = size + max(1, 8) + 8 + 16
        self.assertEqual(need, 943718432)
        want = (max(need, 256 * 1024) + 4095) & ~4095
        self.assertEqual(want, 943722496)
        self.assertEqual(hex(want), "0x38401000")
        self.assertEqual((want >> 16, want & 0xFFFF), (0x3840, 0x1000))
        self.assertEqual(size // 4096, 230400)
        self.assertEqual(256 * 1024, 0x40000)
        self.assertIn("Vec::with_capacity(900 << 20)", rd("spikes", "d0-rust-dos", "progs", "src", "hello.rs"))
        self.assertIn("memory allocation of 943718400 bytes failed", rd("spikes", "d0-rust-dos", "check.sh"))
        for needle in ("0x3840", "0x1000", "943,722,496", "943,718,432", "0x38401000"):
            self.assertIn(needle, self.rt)

    def test_the_shell_exe_pieces_add_up(self):
        text, data, bss = 0x18200, 0x200, 0x42400
        self.assertEqual((text, data, bss), (98816, 512, 271360))
        self.assertEqual(512 + text + data, 99840)
        self.assertEqual(2048 + 99840, 101888)
        end = 0x19400 + bss
        self.assertEqual(end, 0x5B800)
        self.assertEqual((end + 65535) // 65536 * 65536 // 1024, 384)
        self.assertEqual(bss - 256 * 1024, 9216)
        self.assertEqual(0x42600 - 0x200, bss)
        for needle in ("101,888", "99,840", "98,816", "271,360", "384 KiB", "374,784"):
            self.assertIn(needle, self.rt)

    def test_the_contract_values_are_the_transcripts(self):
        ctx = rd("spikes", "d0-rust-dos", "expected", "ctx.txt")
        for needle in ("cs=00a7", "ds=00af", "fs=008f", "gs=0000", "eflags=00003212", "limit 0x4ffff", "initial_size 0x50000", "go32stub, v 2.05"):
            self.assertIn(needle, ctx)
        self.assertEqual(0x50000 // 1024, 320)
        for needle in ("00a7", "00af", "008f", "00003212", "0x4ffff", "0x50000"):
            self.assertIn(needle, self.rt)
        self.assertIn("display offers 37 modes", rd("tests", "shell", "expected-dos-8.txt"))

    def test_the_legacy_section_exists_and_is_marked(self):
        heads = [b["text"] for b in self.chapters["m-rt"]["blocks"] if b["type"] == "h" and b["level"] == 3]
        legacy = [h for h in heads if h.startswith("Legacy")]
        self.assertEqual(len(legacy), 1, heads)
        after = self.rt.split("### " + legacy[0])[1]
        self.assertIn("is not the live one", after.split("\n\n")[1])
        for needle in ("core/src/lib.rs", "coff2go32.py", "c/", "DXE3", "<!-- explainer modules -->"):
            self.assertIn(needle, after)
        # the live story names none of the legacy pieces before the marked section
        before = self.rt.split("### " + legacy[0])[0]
        self.assertNotIn("<!-- explainer modules -->", before)

    def test_legacy_line_counts(self):
        def lines(*p):
            return len(rd(*p).splitlines())

        self.assertEqual(lines("core", "src", "lib.rs"), 125)
        self.assertEqual(lines("c", "main.c") + lines("c", "out.c") + lines("c", "panic.c"), 88)
        self.assertEqual(lines("tools", "coff2go32.py"), 112)
        self.assertEqual(lines("tools", "elf2go32.py"), 139)
        total = sum(len(t.splitlines()) for t in self.src.values())
        self.assertEqual((total, len(self.src)), (2304, 16))

    def test_the_build_script_steps(self):
        sh = rd("shell", "dos", "build.sh")
        for needle in ('"$STUBIFY" -g', "tools/elf2go32.py", "CWSDPMI.EXE", "RUSTC_BOOTSTRAP=1 cargo build --release --locked", "UI.DFN"):
            self.assertIn(needle, sh)
        py = rd("tools", "elf2go32.py")
        self.assertIn("assert len(out) == 168", py)
        self.assertEqual(20 + 28 + 3 * 40, 168)
        self.assertIn("stub[0x244:0x254]", py)
        self.assertEqual(0x200 + 0x44, 0x244)


class ToolchainChapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.chapters = {c["id"]: c for c in manual.load_chapters(manual.load_plan())}
        cls.ch = cls.chapters["m-toolchain"]
        cls.raw = cls.ch["raw"]
        files = {
            "kernel": ("kernel", "i486-desk-none.json"),
            "dos-elf": ("spikes", "d0-rust-dos", "dos-elf.json"),
            "dos": ("core", "dos.json"),
        }
        cls.spec = {k: json.loads(rd(*v)) for k, v in files.items()}

    def table_rows(self):
        rows = {}
        for b in self.ch["blocks"]:
            if b["type"] == "table" and b["rows"] and len(b["rows"][0]) == 5 and "`arch`" in b["rows"][0][0]:
                for row in b["rows"]:
                    rows[row[0].strip("`")] = row
        return rows

    def test_the_table_has_every_field_of_the_three_files(self):
        rows = self.table_rows()
        fields = set().union(*[set(s) for s in self.spec.values()])
        self.assertEqual(len(fields), 26)
        self.assertEqual(set(rows), fields)

    def test_the_table_says_same_exactly_where_the_files_agree(self):
        rows = self.table_rows()
        same = set()
        for f, row in rows.items():
            vals = {json.dumps(s.get(f)) for s in self.spec.values()}
            says_same = row[4].startswith("Same")
            self.assertEqual(says_same, len(vals) == 1, f)
            if says_same:
                same.add(f)
        self.assertEqual(same, {"arch", "cpu", "max-atomic-width", "panic-strategy", "relocation-model", "target-pointer-width",
                                "disable-redzone", "dynamic-linking"})

    def test_the_cells_quote_the_files(self):
        rows = self.table_rows()
        names = ["kernel", "dos-elf", "dos"]
        for f, row in rows.items():
            for i, n in enumerate(names):
                v = self.spec[n].get(f)
                cell = row[i + 1]
                if f in ("data-layout", "metadata"):
                    continue
                if v is None:
                    self.assertEqual(cell, "—", (f, n))
                elif isinstance(v, bool):
                    self.assertEqual(cell, "`%s`" % str(v).lower(), (f, n))
                elif v == "":
                    self.assertEqual(cell, "`\"\"`", (f, n))
                elif isinstance(v, (str, int)):
                    self.assertEqual(cell, "`%s`" % v, (f, n))
                else:
                    self.assertEqual(cell, "`%s`" % json.dumps(v), (f, n))
        k, e, d = (self.spec[n]["data-layout"] for n in ("kernel", "dos-elf", "dos"))
        self.assertEqual(k, e)
        self.assertNotEqual(k, d)
        self.assertIn("-m:x-", d.replace("e-m:x", "-m:x"))
        self.assertTrue(k.endswith("S128") and d.endswith("S32"))
        meta = self.spec["kernel"]["metadata"]
        self.assertEqual((meta["host_tools"], meta["std"], meta["tier"]), (False, False, 3))

    def test_the_pin_and_the_configs(self):
        self.assertIn('channel = "1.98.0"', rd("rust-toolchain.toml"))
        for c in ('"rustfmt"', '"clippy"', '"rust-src"'):
            self.assertIn(c, rd("rust-toolchain.toml"))
        build_std = 'build-std = ["core", "alloc", "compiler_builtins"]'
        mem = 'build-std-features = ["compiler-builtins-mem"]'
        for p in (("kernel", ".cargo", "config.toml"), ("spikes", "d0-rust-dos", ".cargo", "config.toml"), ("shell", "dos", ".cargo", "config.toml")):
            self.assertIn(build_std, rd(*p))
            self.assertIn(mem, rd(*p))
        legacy = rd("core", ".cargo", "config.toml")
        self.assertIn(build_std, legacy)
        self.assertNotIn("compiler-builtins-mem", legacy)
        self.assertIn("-zmax-page-size=0x1000", rd("kernel", ".cargo", "config.toml"))
        self.assertIn(". = 0x100000;", rd("kernel", "link.ld"))
        self.assertIn(". = 0x1000;", rd("spikes", "d0-rust-dos", "link.ld"))

    def test_the_numbers_in_the_worked_example(self):
        # the kernel packer's arithmetic: the third segment's file part ends the flat image
        self.assertEqual(0x128790 + 0x2000 - 0x100000, 173968)
        self.assertEqual(0x42600 - 0x200, 271360)
        for needle in ("127,232", "795,956", "173968 bytes, load address 0x100000", "101888", "102,400", "173,344", "174,032"):
            self.assertIn(needle, self.raw)
        self.assertIn("102,400", rd("docs", "FIRST-WINDOW.md"))
        self.assertIn("173,344", rd("docs", "FIRST-WINDOW.md"))

    def test_std_numbers_are_the_spikes(self):
        e = rd("docs", "E-STD-STUBS.md")
        for needle in ("21 errors", "55 added lines in 12 existing files", "about 340 lines", "15 `desk_*` C functions"):
            self.assertIn(needle, e)
        for needle in ("21 errors", "55 added lines in 12", "about 340 lines", "15 `desk_*`"):
            self.assertIn(needle, self.raw)

    def test_fetch_tools_versions(self):
        f = rd("tools", "fetch-tools.sh")
        for needle in ("djgpp-linux64-gcc1220", "v0.83.0", "csdpmi7b.zip", "build-djgpp/releases/download/v3.4"):
            self.assertIn(needle, f)
        for needle in ("build-djgpp v3.4", "0.83.0", "CWSDPMI r7"):
            self.assertIn(needle, self.raw)

    def test_the_target_comparison_is_a_table_until_the_explainer_lands(self):
        self.assertNotIn("<!-- explainer m-targets -->", self.raw)
        self.assertIn("figure std-sizes", self.raw)


class NewTermsTests(unittest.TestCase):
    def test_each_new_term_is_defined_once_and_used(self):
        chapters = manual.load_chapters(manual.load_plan())
        defined = {}
        for c in chapters:
            for t in c.get("glossary", []):
                defined.setdefault(t["id"], []).append(c["id"])
        want = {"dos-extender": "m-rt", "dpmi": "m-rt", "stub": "m-rt", "transfer-buffer": "m-rt",
                "target-specification": "m-toolchain", "build-std": "m-toolchain", "linker-script": "m-toolchain"}
        for t, cid in want.items():
            self.assertEqual(defined.get(t), [cid], t)
            self.assertTrue(any("](term:%s)" % t in c["raw"] for c in chapters), t)


if __name__ == "__main__":
    unittest.main()
