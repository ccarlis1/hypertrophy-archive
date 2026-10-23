"""Snapshot tests for scripts/parse_notes.py.

Run from the repo root: python3 -m unittest discover tests

Each tests/fixtures/private/<name>.txt must parse to <name>.expected.json. The
fixtures are personal data and gitignored, so these tests skip when they're missing.
After an intended parser change, regenerate the snapshots with
`python3 scripts/parse_notes.py --fixtures` and review the diff.
"""

import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import parse_notes  # noqa: E402

FIXTURES = ROOT / "tests" / "fixtures" / "private"


class SnapshotTest(unittest.TestCase):
    def test_fixtures_match_snapshots(self):
        cases = sorted(FIXTURES.glob("*.txt"))
        if not cases:
            self.skipTest("no private fixtures")
        for path in cases:
            expected = path.with_suffix(".expected.json")
            with self.subTest(fixture=path.name):
                if not expected.exists():
                    self.fail(f"{expected.name} missing; run scripts/parse_notes.py --fixtures")
                text = path.read_text()
                sessions, review = parse_notes.parse_note(text, text.splitlines()[0])
                self.assertEqual(json.loads(expected.read_text()), {"sessions": sessions, "review": review})


class ParseSetTest(unittest.TestCase):
    """Line formats found in the notes, with made-up names and numbers, so they run without
    private fixtures."""

    def parse_line(self, line):
        issues = []
        entry = parse_notes.parse_entry(line, issues)
        return entry, issues

    def test_rir_range_and_comment(self):
        e, _ = self.parse_line("Leg Extension - set 1 - 60lbs - 8 reps 1-2 RIR- set 2 - 40lbs - 8 reps 1 RIR (slower tempo)")
        self.assertEqual(e["name"], "Leg Extension")
        s1, s2 = e["sets"]
        self.assertEqual((s1["weight"], s1["reps"], s1["rir_low"], s1["rir_high"]), (60, 8, 1, 2))
        self.assertEqual((s2["weight"], s2["comment"]), (40, "(slower tempo)"))

    def test_missing_weight_is_inferred(self):
        e, _ = self.parse_line("30 deg incline press - set 1 - 120 smth - 7 reps 1-2 RIR- set 2 - 8 reps 0 RIR")
        s1, s2 = e["sets"]
        self.assertEqual((s1["weight"], s1["weight_inferred"]), (120, False))
        self.assertEqual((s2["weight"], s2["weight_inferred"], s2["reps"]), (120, True, 8))

    def test_blank_last_set(self):
        e, _ = self.parse_line("Cable Curl - set 1 - 50lbs - 10 reps 1 RIR- set 2 - ")
        self.assertEqual([s["logged"] for s in e["sets"]], [True, False])

    def test_plates_per_side(self):
        e, _ = self.parse_line("Plate Row - set 1 - 1 + 25 - 8 reps 0 RIR- set 2 - 1.5 - 9 reps 0 RIR")
        self.assertEqual([(s["weight"], s["reps"]) for s in e["sets"]], [(70, 8), (67.5, 9)])

    def test_no_set_markers(self):
        e, _ = self.parse_line("Pulldown - 120lbs - 10 reps 1 RIR")
        self.assertEqual([(s["set_number"], s["weight"], s["reps"]) for s in e["sets"]], [(1, 120, 10)])

    def test_failure_and_half_reps(self):
        e, _ = self.parse_line("Chest Press - set 1 - 100lbs - 8 reps 0 RIR (better form) - set 2 - 6.5 reps failure (tired)")
        s2 = e["sets"][1]
        self.assertEqual((s2["weight"], s2["reps"], s2["rir_low"]), (100, 6.5, 0))

    def test_shorthand(self):
        e = parse_notes.parse_shorthand("2 sets Squat @ 135 for 8", [])
        self.assertEqual([(s["weight"], s["reps"]) for s in e["sets"]], [(135, 8), (135, 8)])
        e = parse_notes.parse_shorthand("2 sets hip warmup", [])
        self.assertEqual((e["tracked"], e["untracked_sets"]), (False, 2))

    def test_headers(self):
        self.assertEqual(parse_notes.parse_header("Day 1 - FB 1")["day_label"], "FB 1")
        self.assertEqual(parse_notes.parse_header("Day 2 Posterior")["day_label"], "Posterior")
        self.assertEqual(parse_notes.parse_header("Upper 1")["day_label"], "Upper 1")
        h = parse_notes.parse_header("Day 2 - short on time today")
        self.assertEqual((h["day_label"], h["comment"]), (None, "short on time today"))


if __name__ == "__main__":
    unittest.main()
