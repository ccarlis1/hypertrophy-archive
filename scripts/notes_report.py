"""Coverage report for an Apple Notes export, and fixture picking (Phase 2a).

Usage:
    python3 scripts/notes_report.py [exports/notes.json] [--fixtures]

Reads the JSON written by scripts/export_notes.js, applies overrides/titles.toml
if it exists, then reports per block: week counts, missing and duplicate weeks,
weeks whose created date is out of order, and notes whose day headers don't match
the block's split. With --fixtures, it writes the most awkward week of each block
to tests/fixtures/private/<block>.txt.

overrides/titles.toml maps a note id to the block and week it really belongs to:

    ["x-coredata://.../p123"]
    block = "uwo2_y3"
    week = 8
    reason = "titled Uwo Y3 Wk 8, but the headers are anterior/posterior"

An override can define a new block by adding `split` and `label` (free text).
A note can also be left out on purpose with `skip = true`.
"""

import json
import re
import sys
import tomllib
from collections import defaultdict
from pathlib import Path

SPLITS = {
    "log_book": "upper_lower",
    "full_body": "full_body",
    "uwo_upper": "upper_lower",
    "uwo_y3": "full_body",
    "uwo2_y3": "anterior_posterior",
    "uwo_y4": "upper_lower",
}

WORKOUT_LINE = re.compile(r"set\s*\d|\d\s*reps?\b|\brir\b", re.I)

# Features the parser has to handle. The fixture for a block is the week that
# shows the most of them.
FEATURES = {
    "rir_range": re.compile(r"\d\s*-\s*\d\s*rir", re.I),
    "untracked": re.compile(r"^\s*\d+\s*sets?\s+[a-z]", re.I | re.M),
    "blank_set": re.compile(r"set\s*\d+\s*-\s*$", re.I | re.M),
    "missing_weight": re.compile(r"set\s*\d+\s*-\s*\d+\s*reps", re.I),
    "comment_after_rir": re.compile(r"rir\s*[(:a-z]", re.I),
    "tight_dash": re.compile(r"[a-z)]-\s|\s-[a-z]", re.I),
    "no_set_marker": re.compile(r"^(?!.*set\s*\d).*\d+\s*reps", re.I | re.M),
}


def header_split(text):
    """Guess a note's split from its day headers. None if it has no clear headers."""
    votes = defaultdict(int)
    for line in text.splitlines()[1:]:
        line = line.strip()
        if not line or len(line) > 40 or WORKOUT_LINE.search(line):
            continue
        if re.search(r"\b(anterior|posterior)\b", line, re.I):
            votes["anterior_posterior"] += 1
        elif re.match(r"^(upper|lower)\s*\d*$", line, re.I):
            votes["upper_lower"] += 1
        elif re.match(r"^day\s*\d+(\s*-?\s*fb\s*\d*)?$", line, re.I):
            votes["full_body"] += 1
    return max(votes, key=votes.get) if votes else None


def load_overrides(path):
    if not path.exists():
        return {}
    with path.open("rb") as f:
        return tomllib.load(f)


def main(argv):
    args = [a for a in argv if not a.startswith("--")]
    export_path = Path(args[0] if args else "exports/notes.json")
    data = json.loads(export_path.read_text())
    overrides = load_overrides(Path("overrides/titles.toml"))

    splits = dict(SPLITS)
    labels = {}
    blocks = defaultdict(list)
    candidates, skipped = [], []
    for note in data["notes"]:
        ov = overrides.get(note["id"])
        if ov:
            if ov.get("skip"):
                skipped.append((note, ov.get("reason", "")))
                continue
            note = {**note, "block": ov["block"], "week": ov["week"], "override": ov.get("reason", "")}
            if "split" in ov:
                splits[ov["block"]] = ov["split"]
            if "label" in ov:
                labels[ov["block"]] = ov["label"]
        if note["block"]:
            blocks[note["block"]].append(note)
        else:
            candidates.append(note)

    total = sum(len(v) for v in blocks.values())
    print(f"Scanned {data['total_notes']} notes, exported {export_path} at {data['exported_at'][:16]}")
    print(f"Workout weeks: {total} across {len(blocks)} blocks")
    print(f"Locked: {len(data['locked'])}  Unreadable: {len(data['unreadable'])}")

    for block in sorted(blocks, key=lambda b: min(n["created"] for n in blocks[b])):
        notes = sorted(blocks[block], key=lambda n: (n["week"], n["created"]))
        weeks = [n["week"] for n in notes]
        first, last = notes[0]["created"][:10], notes[-1]["created"][:10]
        label = f" [{labels[block]}]" if block in labels else ""
        print(f"\n{block}{label}: {len(notes)} weeks (wk {min(weeks)}-{max(weeks)}, {first} to {last})")

        missing = sorted(set(range(1, max(weeks) + 1)) - set(weeks))
        if missing:
            print(f"  missing weeks: {missing}")
        dupes = sorted({w for w in weeks if weeks.count(w) > 1})
        for w in dupes:
            print(f"  duplicate week {w}: " + ", ".join(n["created"][:10] for n in notes if n["week"] == w))
        for prev, cur in zip(notes, notes[1:]):
            if cur["created"] < prev["created"]:
                print(f"  out of order: wk {cur['week']} ({cur['created'][:10]}) created before wk {prev['week']} ({prev['created'][:10]})")
        for n in notes:
            split = header_split(n["plaintext"])
            if split and split != splits.get(block):
                print(f"  split mismatch: wk {n['week']} ({n['created'][:10]}) headers look {split}")
            if block == "uwo_upper" and re.match(r"^uwo\s*w", n["title"], re.I) and n["week"] < 7:
                print(f"  title format: {n['title']!r} uses the week-7+ format")
            if n.get("override"):
                print(f"  override: wk {n['week']} from {n['title']!r} ({n['override']})")

    if candidates:
        print("\nPossible workout notes with no matching title:")
        for n in candidates:
            print(f"  {n['title']!r} ({n['created'][:10]}, {n['workout_lines']} workout lines)")
    if skipped:
        print("\nSkipped by override:")
        for n, reason in skipped:
            print(f"  {n['title']!r} ({n['created'][:10]}) {reason}")
    if data["related_titles_without_workout_lines"]:
        print("\nRelated-looking titles with no workout lines (not exported):")
        for t in data["related_titles_without_workout_lines"]:
            print(f"  {t!r}")
    if data["locked"]:
        print("\nLocked notes (unlock them in Notes and re-run to include):")
        for t in data["locked"]:
            print(f"  {t!r}")

    if "--fixtures" in argv:
        write_fixtures(blocks)


def write_fixtures(blocks):
    out = Path("tests/fixtures/private")
    out.mkdir(parents=True, exist_ok=True)
    print("\nFixtures:")
    for block, notes in sorted(blocks.items()):
        def score(n):
            hits = {k: len(r.findall(n["plaintext"])) for k, r in FEATURES.items()}
            return (sum(1 for v in hits.values() if v), sum(min(v, 3) for v in hits.values()), n["created"])

        best = max(notes, key=score)
        text = best["plaintext"]
        if text.splitlines()[0].strip() != best["title"]:
            text = best["title"] + "\n" + text
        (out / f"{block}.txt").write_text(text if text.endswith("\n") else text + "\n")
        found = [k for k, r in FEATURES.items() if r.search(best["plaintext"])]
        print(f"  {block}.txt <- {best['title']!r} ({best['created'][:10]}): {', '.join(found) or 'no awkward cases'}")


if __name__ == "__main__":
    main(sys.argv[1:])
