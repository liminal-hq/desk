# Checks of the figures in since.py: the timeline and the CI strip draw from the gathered dates, the roadmap chart refuses a status it does
# not know, a diagram refuses a path that is not in the tree, and a copied screenshot must equal its source.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.normpath(os.path.join(HERE, "..")))

import since  # noqa: E402

DATA = {"sources": {"gh": "test"}, "head": "abc1234", "timeline": {
    "prs": [{"number": 19, "title": "A", "base": "main", "createdAt": "2026-10-23T08:26:00Z", "mergedAt": "2026-10-23T13:16:00Z", "state": "MERGED"},
            {"number": 33, "title": "B", "base": "docs/x", "createdAt": "2026-10-23T14:43:00Z", "mergedAt": "2026-11-11T06:26:00Z", "state": "MERGED"},
            {"number": 99, "title": "Open", "base": "main", "createdAt": "2026-11-11T06:26:00Z", "mergedAt": None, "state": "OPEN"}],
    "runs": [{"createdAt": "2026-11-11T15:27:00Z", "sha": "b928309", "conclusion": "failure", "title": "T"},
             {"createdAt": "2026-11-11T17:21:00Z", "sha": "77d9053", "conclusion": "success", "title": "U"},
             {"createdAt": "2026-10-23T17:21:00Z", "sha": "1111111", "conclusion": "success", "title": "earlier"}]}}


class SinceTests(unittest.TestCase):
    def test_the_timeline_draws_merged_pull_requests_only(self):
        html = since.pr_timeline(DATA)
        self.assertIn("#19", html)
        self.assertIn("#33", html)
        self.assertNotIn("#99", html)
        self.assertIn("The same dates as a table", html)

    def test_the_ci_strip_keeps_one_day_and_names_each_conclusion(self):
        html = since.ci_runs(DATA)
        self.assertIn("failed", html)
        self.assertIn("passed", html)
        self.assertNotIn("1111111", html)

    def test_the_roadmap_chart_counts_the_real_file_and_refuses_an_unknown_status(self):
        rows = since.roadmap_rows()
        self.assertGreaterEqual(len(rows), 140)
        self.assertIn("done", {r[1] for r in rows})
        html = since.roadmap_status(DATA)
        self.assertIn("%d items" % len(rows), html)
        real = since._read
        try:
            since._read = lambda rel: "## S\n\n| R1 | T | `mostly` | x | y |\n" if rel == "docs/ROADMAP.md" else real(rel)
            with self.assertRaises(ValueError):
                since.roadmap_status(DATA)
        finally:
            since._read = real

    def test_the_live_marks_come_from_the_sums_and_the_document(self):
        self.assertIn("c310363f", since.live_marks(DATA))
        real = since._read
        try:
            since._read = lambda rel: "no digests here" if rel == "docs/LIVE-INPUT.md" else real(rel)
            with self.assertRaises(ValueError):
                since.live_marks(DATA)
        finally:
            since._read = real

    def test_a_diagram_refuses_a_path_that_is_not_in_the_tree(self):
        self.assertIn("<svg", since.live_routes(DATA))
        with self.assertRaises(ValueError):
            since._chain([("Row", [("Box", "sub", "no/such/path.rs")])], "alt", "caption")

    def test_the_go32_layout_names_its_sources_and_the_diagrams_draw(self):
        self.assertIn("2,048 bytes", since.go32_layout(DATA))
        self.assertIn("16 KiB buffer", since.transcript_flow(DATA))

    def test_a_copied_screenshot_must_equal_its_source(self):
        html = since.repo_images(DATA, ["img-li-native-8-typed.png"])
        self.assertIn("docs/images/live-input/native-8-typed.png", html)
        saved = since.REPO_IMAGES["img-li-native-8-typed.png"]
        try:
            since.REPO_IMAGES["img-li-native-8-typed.png"] = ("docs/images/live-input/dos-8-typed.png",) + saved[1:]
            with self.assertRaises(ValueError):
                since.repo_images(DATA, ["img-li-native-8-typed.png"])
        finally:
            since.REPO_IMAGES["img-li-native-8-typed.png"] = saved


if __name__ == "__main__":
    unittest.main()
