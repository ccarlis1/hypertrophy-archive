"""Parse exported workout notes into structured data (Phase 2c).

Usage:
    python3 scripts/parse_notes.py [exports/notes.json] [-o exports/parsed.json]
    python3 scripts/parse_notes.py --fixtures   # write tests/fixtures/private/*.expected.json

Reads the export from scripts/export_notes.js and overrides/titles.toml, and writes
blocks -> cycles (weeks) -> sessions -> entries (exercises) -> sets. Lines the parser
can't fully trust go in a review list with the note, line number and reason. Nothing
is dropped: every entry and set keeps its raw text.

Conventions:
- Weights are lb. Plate-loaded weights are logged per side, so "3 and a 35" is
  3 x 45 + 35 = 170 lb, and the original text is kept in weight_note.
- "smth"/"something" after a weight means "at least this much" and is dropped.
- A logged set with no weight takes the previous set's weight (weight_inferred).
- RIR is a range (rir_low, rir_high). "failure" is 0.
- Reps can be fractional (4.5). A rep range like "6-7" keeps the low end and the
  range in the set comment.
"""

import argparse
import json
import re
import tomllib
from pathlib import Path

PARSER_VERSION = 1
PLATE = 45.0

BLOCKS = {
    "log_book": {"name": "Log Book", "split": "upper_lower", "label": "home"},
    "full_body": {"name": "Full Body", "split": "full_body", "label": "home"},
    "uwo_upper": {"name": "Uwo Upper/Lower", "split": "upper_lower", "label": "uwo y2"},
    "uwo_y3": {"name": "Uwo Y3 Full Body", "split": "full_body", "label": "uwo y3"},
    "uwo2_y3": {"name": "Uwo Y3 Anterior/Posterior", "split": "anterior_posterior", "label": "uwo y3"},
    "uwo_y4": {"name": "Uwo Y4 Upper/Lower", "split": "upper_lower", "label": "uwo y4"},
}

HEADER = re.compile(
    r"^day\s*(\d+)\s*[:.\-]?\s*"
    r"(?:(upper|lower|fb|full\s*body|anterior|posterior)\s*(\d+|[ab]\b)?)?"
    r"\s*[:.]?\s*(.*)$",
    re.I,
)
BARE_HEADER = re.compile(r"^(upper|lower)\s*(\d+)\s*$", re.I)
SHORTHAND = re.compile(r"^(\d+)\s*sets?\s+(.+)$", re.I)
SET_MARK = re.compile(r"\bset\s*(\d+)\b", re.I)
# A separator: a dash with whitespace on at least one side, a trailing dash, or a colon.
SEP = re.compile(r"\s+-\s*|-\s+|-$|\s*:\s*")

NUM = r"\d+(?:\.\d+)?"
ACTUAL = re.compile(rf"\(actually\s*({NUM})\s*lbs?\)", re.I)
PLATES = re.compile(rf"({NUM})\s*plates?(?:\s*total)?(?:\s*(?:and\s*a|and|\+)\s*({NUM}))?", re.I)
PLATE_COMBO = re.compile(rf"\b({NUM})\s*(?:and\s*a|and|\+)\s*({NUM})\b(?!\s*(?:reps?|rir))", re.I)
LBS = re.compile(rf"({NUM})\s*(?:lbs?|lhs|bs)\b", re.I)
AT = re.compile(rf"@\s*({NUM})\s*(?:lbs?)?", re.I)
# A bare number at the start of a set, followed by a separator (not "6-7 reps").
BARE = re.compile(rf"^({NUM})\s*(smth|something)?(?=\s+-|\s*-\s|\s*-?$|\s*\?)", re.I)
REPS = re.compile(rf"({NUM})\s*(?:ish\s*)?(?:(?:-|/|…|\.\.\.)\s*({NUM})\??)?\s*reps?\b\??", re.I)
FOR_REPS = re.compile(r"\bfor\s*(\d+)\b", re.I)
RIR = re.compile(rf"({NUM})\s*(?:-\s*({NUM}))?\s*(\+)?\s*\.?\s*(?:ish\s*)?r[io]r\b", re.I)
FAILURE = re.compile(r"\bfailure\b", re.I)
APPROX = re.compile(r"\b(smth|something)\b", re.I)
SETTING_WORDS = re.compile(r"\b(seat|lvl|level|height|notch|pad|backrest|foot\s*rest|cable|handle)\b", re.I)


def mask_parens(s):
    """Blank out text inside parentheses so separators there are ignored."""
    out, depth = [], 0
    for ch in s:
        if ch == "(":
            depth += 1
        elif ch == ")" and depth:
            depth -= 1
            out.append(ch)
            continue
        out.append("_" if depth and ch != "(" else ch)
    return "".join(out)


def clean(s):
    s = re.sub(r"\s+", " ", s or "").strip(" -:,.")
    return s or None


def has_data(s):
    m = mask_parens(s)
    return bool(LBS.search(m) or PLATES.search(m) or REPS.search(m) or RIR.search(m) or AT.search(m))


def parse_weight(t, masked, plate_context):
    """Return (weight, note, spans) from a set's text. spans are removed from the comment."""
    m = ACTUAL.search(t)
    if m:
        return float(m.group(1)), "logged as more, actually " + m.group(1), [m.span()]
    m = PLATES.search(masked)
    if m:
        w = float(m.group(1)) * PLATE + float(m.group(2) or 0)
        return w, t[m.start():m.end()].strip(), [m.span()]
    m = PLATE_COMBO.search(masked)
    if m and float(m.group(1)) <= 6:
        w = float(m.group(1)) * PLATE + float(m.group(2))
        return w, t[m.start():m.end()].strip() + " (plates)", [m.span()]
    m = LBS.search(masked)
    if m:
        return float(m.group(1)), None, [m.span()]
    m = AT.search(masked)
    if m:
        return float(m.group(1)), None, [m.span()]
    m = BARE.search(masked)
    if m:
        v = float(m.group(1))
        if plate_context and v <= 6:
            return v * PLATE, f"{m.group(1)} (plates)", [m.span()]
        if v >= 15:
            return v, None, [m.span()]
    return None, None, []


def parse_set(number, text, prev_weight, plate_context, issues):
    """Parse one set's text, e.g. '185lbs - 5 reps 1-2 RIR (slow)'."""
    raw = text.strip()
    t = raw.strip(" -:")
    s = {"set_number": number, "weight": None, "weight_inferred": False, "weight_note": None,
         "reps": None, "rir_low": None, "rir_high": None, "logged": False, "comment": None, "raw": raw}
    if not t:
        return s
    masked = mask_parens(t)
    spans = []

    weight, note, wspans = parse_weight(t, masked, plate_context)
    spans += wspans
    s["weight"], s["weight_note"] = weight, note
    # Blank out the weight so "2 + 35 - 5 reps" isn't read as a 35-5 rep range.
    for a, b in wspans:
        masked = masked[:a] + " " * (b - a) + masked[b:]

    reps = list(REPS.finditer(masked))
    if reps:
        r = reps[0]
        s["reps"] = float(r.group(1))
        if r.group(2):
            s["comment"] = f"{r.group(1)}-{r.group(2)} reps"
        spans += [x.span() for x in reps[:1]]
        if len(reps) > 1:
            issues.append("more than one rep count in a set; kept the first")
    elif "@" in t:  # "@ 245 for 6"; elsewhere "for 20 mins" isn't reps
        f = FOR_REPS.search(masked)
        if f:
            s["reps"] = float(f.group(1))
            spans.append(f.span())

    rir = RIR.search(masked)
    if rir:
        s["rir_low"] = float(rir.group(1))
        s["rir_high"] = None if rir.group(3) else float(rir.group(2) or rir.group(1))
        spans.append(rir.span())
    elif FAILURE.search(masked):
        s["rir_low"] = s["rir_high"] = 0.0

    rest = list(t)
    for a, b in spans:
        for i in range(a, b):
            rest[i] = " "
    rest = APPROX.sub(" ", "".join(rest))
    rest = re.sub(r"\b(?:\?\s*rir|rir)\b|@", " ", rest, flags=re.I)
    comment = clean(rest)
    if comment and s["comment"]:
        comment = s["comment"] + "; " + comment
    s["comment"] = comment or s["comment"]

    s["logged"] = s["reps"] is not None or s["rir_low"] is not None
    if s["logged"] and s["weight"] is None and prev_weight is not None:
        s["weight"], s["weight_inferred"] = prev_weight, True
    if s["rir_low"] is not None and s["reps"] is None:
        issues.append(f"set {number}: RIR but no rep count")
    if not s["logged"] and comment and re.search(r"\d", comment) and weight is None:
        issues.append(f"set {number}: has numbers the parser didn't use")
    return s


def split_name(line):
    """Split an exercise line into (name, rest)."""
    masked = mask_parens(line)
    cuts = [m.start() for m in (SEP.search(masked), SET_MARK.search(masked)) if m]
    for rx in (LBS, PLATES, REPS):
        m = rx.search(masked)
        if m:
            cuts.append(m.start())
    cut = min(cuts) if cuts else len(line)
    return line[:cut], line[cut:]


def parse_name(name_raw):
    name_raw = clean(name_raw.lstrip("*•")) or ""
    qualifiers = [clean(q) for q in re.findall(r"\(([^)]*)\)?", name_raw) if clean(q)]
    name = clean(re.sub(r"\([^)]*\)?", " ", name_raw)) or name_raw
    # "Cuffed Triceps (...), better warm up needed": text after a comma is a comment.
    name, _, extra = name.partition(",")
    if clean(extra):
        qualifiers.append(clean(extra))
    return clean(name) or name_raw, qualifiers, name_raw


def parse_entry(line, issues):
    name_part, rest = split_name(line)
    name, qualifiers, name_raw = parse_name(name_part)
    entry = {"name": name, "name_raw": name_raw, "qualifiers": qualifiers, "settings": None,
             "tracked": True, "untracked_sets": None, "comment": None, "raw": line, "sets": []}
    add_sets(entry, rest, issues)
    return entry


def add_sets(entry, rest, issues):
    """Parse the part of a line after the name (or a continuation line) into sets."""
    masked = mask_parens(rest)
    marks = list(SET_MARK.finditer(masked))
    chunks = []
    pre = rest[: marks[0].start()] if marks else rest
    if marks:
        if has_data(pre):
            chunks.append((None, pre))
        elif clean(pre):
            entry["settings"] = clean(" ".join(filter(None, [entry["settings"], clean(pre)])))
        for i, m in enumerate(marks):
            end = marks[i + 1].start() if i + 1 < len(marks) else len(rest)
            chunks.append((int(m.group(1)), rest[m.end():end]))
    elif has_data(rest):
        chunks.append((None, rest))
    elif clean(rest):
        # No sets on this line, e.g. "Chest press - 1 up from bottom seat height",
        # with the sets on the next line.
        key = "settings" if SETTING_WORDS.search(rest) else "comment"
        entry[key] = clean(" ".join(filter(None, [entry[key], clean(rest)])))

    sh = None
    if len(chunks) == 1 and chunks[0][0] is None:
        sh = re.match(r"^\s*[-:]?\s*(\d+)\s*sets?\b(.*)$", chunks[0][1], re.I)

    plate_context = bool(PLATES.search(mask_parens(entry["raw"])) or PLATE_COMBO.search(mask_parens(entry["raw"])))
    sets = entry["sets"]
    for number, text in chunks:
        prev = sets[-1] if sets else None
        if number is None:
            number = prev["set_number"] + 1 if prev else 1
        elif prev and number == prev["set_number"] and not prev["logged"]:
            # "set 2 - 154lbs - set 2 - 6 reps": the same set written twice.
            text = prev["raw"] + " - " + text
            sets.pop()
            prev = sets[-1] if sets else None
        elif prev and number <= prev["set_number"]:
            issues.append(f"set {number} repeated; renumbered to {prev['set_number'] + 1}")
            number = prev["set_number"] + 1
        prev_weight = next((x["weight"] for x in reversed(sets) if x["weight"] is not None), None)
        if sh:
            count, text = int(sh.group(1)), sh.group(2)
            template = parse_set(number, text, prev_weight, plate_context, issues)
            for k in range(count):
                sets.append({**template, "set_number": number + k, "comment": clean("; ".join(filter(None, [template["comment"], f"from '{count} sets'"])))})
        else:
            sets.append(parse_set(number, text, prev_weight, plate_context, issues))


def parse_shorthand(line, issues):
    """'2 sets calves' (untracked) or '2 sets SLDL @ 245 for 6' (tracked)."""
    m = SHORTHAND.match(line)
    count, body = int(m.group(1)), m.group(2)
    masked = mask_parens(body)
    at = AT.search(masked)
    name_part = body[: at.start()] if at else body
    name, qualifiers, name_raw = parse_name(name_part)
    entry = {"name": name, "name_raw": name_raw, "qualifiers": qualifiers, "settings": None,
             "tracked": bool(at), "untracked_sets": None if at else count, "comment": None,
             "raw": line, "sets": []}
    if at:
        template = parse_set(1, body[at.start():], None, False, issues)
        template["logged"] = True  # "2 sets X @ 95lbs" says the sets were done, even without reps
        for k in range(count):
            entry["sets"].append({**template, "set_number": k + 1, "comment": clean("; ".join(filter(None, [template["comment"], f"from '{count} sets'"])))})
    return entry


def parse_header(line):
    m = HEADER.match(line)
    if m:
        label = None
        if m.group(2):
            word = m.group(2).lower().replace(" ", "")
            word = {"fb": "FB", "fullbody": "FB"}.get(word, word.title())
            label = f"{word} {m.group(3).upper()}" if m.group(3) else word
        return {"day_number": int(m.group(1)), "day_label": label, "comment": clean(m.group(4)), "header_raw": line}
    m = BARE_HEADER.match(line)
    if m:
        return {"day_number": None, "day_label": f"{m.group(1).title()} {m.group(2)}", "comment": None, "header_raw": line}
    return None


def looks_like_exercise(line, known_names):
    masked = mask_parens(line)
    if SET_MARK.search(masked) or has_data(line):
        return True
    if re.search(r"-\s*$", line):
        return True
    return parse_name(split_name(line)[0])[0].lower() in known_names


def parse_note(text, title, known_names=frozenset()):
    """Parse one note's text into sessions. Returns (sessions, review items)."""
    sessions, review = [], []
    current = None

    def session():
        nonlocal current
        if current is None:
            current = {"day_number": None, "day_label": None, "comment": None, "header_raw": None,
                       "implicit": True, "notes": [], "entries": []}
            sessions.append(current)
        return current

    lines = text.splitlines()
    for line_no, raw in enumerate(lines[1:], start=2):
        line = raw.strip()
        if not line:
            continue
        issues = []
        header = parse_header(line)
        if header:
            current = {**header, "implicit": False, "notes": [], "entries": []}
            sessions.append(current)
            continue
        if re.match(r"^set\s*\d", line, re.I) and current and current["entries"]:
            entry = current["entries"][-1]
            entry["raw"] += "\n" + line
            add_sets(entry, line, issues)
        elif SHORTHAND.match(line):
            entry = parse_shorthand(line, issues)
            session()["entries"].append(entry)
        elif looks_like_exercise(line, known_names):
            entry = parse_entry(line, issues)
            session()["entries"].append(entry)
        else:
            session()["notes"].append(line)
            continue
        for issue in issues:
            review.append({"title": title, "line": line_no, "raw": line, "issue": issue})

    for i, s in enumerate(sessions):
        s["index"] = i + 1
        for j, e in enumerate(s["entries"]):
            e["position"] = j + 1
    return sessions, review


def load_overrides():
    path = Path("overrides/titles.toml")
    if not path.exists():
        return {}
    with path.open("rb") as f:
        return tomllib.load(f)


def parse_export(data):
    overrides = load_overrides()
    notes = []
    for n in data["notes"]:
        ov = overrides.get(n["id"], {})
        if ov.get("skip"):
            continue
        block, week = ov.get("block", n["block"]), ov.get("week", n["week"])
        if not block:
            continue
        notes.append((n, block, week, ov))

    # First pass: collect exercise names, so a bare name with no data ("Single Leg Ext.")
    # is still read as an exercise in the second pass.
    known = set()
    for n, *_ in notes:
        for s in parse_note(n["plaintext"], n["title"])[0]:
            known.update(e["name"].lower() for e in s["entries"])

    blocks, cycles, review = {}, [], []
    for n, block, week, ov in sorted(notes, key=lambda x: x[0]["created"]):
        meta = {**BLOCKS.get(block, {"name": block, "split": "other", "label": None})}
        meta.update({k: ov[k] for k in ("split", "label") if k in ov})
        b = blocks.setdefault(block, {"slug": block, **meta, "started_on": n["created"][:10]})
        sessions, rv = parse_note(n["plaintext"], n["title"], frozenset(known))
        review += rv
        cycles.append({"block": block, "week": week, "title": n["title"], "note_id": n["id"],
                       "started_on": n["created"][:10], "date_precision": "week",
                       "override": ov.get("reason"), "sessions": sessions})
        b["started_on"] = min(b["started_on"], n["created"][:10])
    return {"parser_version": PARSER_VERSION, "exported_at": data["exported_at"],
            "blocks": list(blocks.values()), "cycles": cycles, "review": review}


def summarize(out):
    sessions = [s for c in out["cycles"] for s in c["sessions"]]
    entries = [e for s in sessions for e in s["entries"]]
    sets = [x for e in entries for x in e["sets"]]
    logged = [x for x in sets if x["logged"]]
    print(f"{len(out['blocks'])} blocks, {len(out['cycles'])} weeks, {len(sessions)} sessions, "
          f"{len(entries)} exercise entries ({sum(not e['tracked'] for e in entries)} untracked), "
          f"{len(sets)} sets ({len(logged)} logged, {sum(x['weight_inferred'] for x in logged)} with inferred weight)")
    print(f"Logged sets missing weight: {sum(x['weight'] is None for x in logged)}, missing reps: {sum(x['reps'] is None for x in logged)}")
    print(f"Review items: {len(out['review'])}")


def write_fixtures():
    root = Path("tests/fixtures/private")
    for path in sorted(root.glob("*.txt")):
        text = path.read_text()
        sessions, review = parse_note(text, text.splitlines()[0])
        out = path.with_suffix(".expected.json")
        out.write_text(json.dumps({"sessions": sessions, "review": review}, indent=2, ensure_ascii=False) + "\n")
        print(f"wrote {out}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("export", nargs="?", default="exports/notes.json")
    ap.add_argument("-o", "--out", default="exports/parsed.json")
    ap.add_argument("--fixtures", action="store_true")
    args = ap.parse_args()
    if args.fixtures:
        write_fixtures()
        return
    out = parse_export(json.loads(Path(args.export).read_text()))
    Path(args.out).write_text(json.dumps(out, indent=2, ensure_ascii=False) + "\n")
    print(f"wrote {args.out}")
    summarize(out)


if __name__ == "__main__":
    main()
