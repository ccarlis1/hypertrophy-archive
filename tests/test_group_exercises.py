"""Tests for scripts/group_exercises.py entry exclusions."""

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import group_exercises  # noqa: E402


def entry(raw, name="Preacher Machine"):
    return {"name": name, "raw": raw, "tracked": True, "qualifiers": [],
            "sets": [{"weight": 120.0, "weight_note": None, "logged": True, "weight_inferred": False}]}


def parsed(*entries, note="n1"):
    return {"blocks": [{"slug": "b", "label": "uwo y2"}],
            "cycles": [{"block": "b", "note_id": note, "started_on": "2025-01-01",
                        "sessions": [{"entries": list(entries)}]}]}


class ExclusionTests(unittest.TestCase):
    def test_matching_entry_is_excluded_and_left_out(self):
        keep, drop = entry("Preacher Machine - 120lbs"), entry("Preacher Machine (half reps) - 120lbs")
        rows = group_exercises.collect(parsed(keep, drop), [{"raw": drop["raw"]}])
        self.assertEqual([r["entry"] for r in rows], [keep])
        self.assertTrue(drop["excluded"])
        self.assertIsNone(drop["variant"])

    def test_note_id_must_match_when_given(self):
        e = entry("Preacher Machine (half reps) - 120lbs")
        rows = group_exercises.collect(parsed(e, note="n1"), [{"raw": e["raw"], "note": "other"}])
        self.assertEqual(len(rows), 1)
        self.assertNotIn("excluded", e)


if __name__ == "__main__":
    unittest.main()
