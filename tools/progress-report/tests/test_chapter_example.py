# An example of a chapter's own checks, picked up by test_report.py: the placeholder shows the shape, and each chapter that needs its own
# checks adds a file like this one (self-contained, plain unittest).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.normpath(os.path.join(HERE, "..")))

import manual  # noqa: E402


class ExampleChapterTests(unittest.TestCase):
    def test_the_plan_names_the_chapter_this_file_is_an_example_for(self):
        self.assertIn("m-map", [c["id"] for c in manual.load_plan()["chapters"]])
