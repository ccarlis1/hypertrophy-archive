"""Suggest exercise variants from parsed notes, using names and weights (Phase 2d).

Usage:
    python3 scripts/group_exercises.py [exports/parsed.json]

Steps:
1. Normalize each logged name: lowercase, fix typos, expand brands
   (LF / LifeFit / Life Fitness -> lifefitness, FM / Free motion -> freemotion).
2. Put it in a movement family (preacher curl, lateral raise, leg extension, ...).
   Families are close to joint actions, so they map to archive entries later.
3. A variant is family + the name's distinguishing words + gym (home / uwo / travel;
   dumbbell and EZ-bar variants are the same at every gym). "DB Preachers" and
   "Lifefit Lateral Raise" are variants; so is a plain "Frontal Pulldown".
4. Weight correlation:
   - Split: if one variant's top-set weights fall into clearly separate groups
     (a jump of more than 30%), each group is matched to the sibling variant in the
     same family and gym with the closest median (within 30%), or kept as its own
     "~weight band" variant. This separates e.g. cable vs machine "Lateral Raise".
   - Merge hints: sibling variants with medians within 10% are printed as
     "possibly the same machine", for you to decide.
   - Unit check: a variant whose weights jump by about 2x may mix per-side and
     total weights (plate-loaded machines), or two different machines.

Writes overrides/aliases.suggested.toml (status = "suggested") and prints a report.
Copy it to overrides/aliases.toml and review it there. Fields read from each variant:
    status = "confirmed"                       reviewed and final (carried into variants.toml)
    status = "merge" + merge_into = "<slug>"   combine into another variant (chains are followed)
    status = "remove"                          drop these entries from the data
    weight_factor = 2.0                        multiply this variant's weights when merging
    display = "Lat Pulldown"                   name for the final variant
A merged variant goes to the gym most of its entries are from (ties: the most recent).

overrides/entries.toml (optional) excludes single entries without touching their name's
variant, e.g. one set of half reps:
    [[exclude]]
    note = "x-coredata://.../p123"             the week's note id (optional)
    raw = "Preacher Machine (half reps) - ..."  the entry's line, exactly as logged
    reason = "half reps"
An excluded entry stays in the data with "excluded": true and no variant.

When overrides/aliases.toml exists, also writes:
    exports/grouped.json      parsed.json with "variant" (or "excluded") on every tracked entry
    overrides/variants.toml   the final variants, for confirming
"""

import json
import re
import statistics
import sys
import tomllib
from collections import Counter, defaultdict
from pathlib import Path

TYPOS = {
    "prwafher": "preacher", "preaxhers": "preacher", "preachers": "preacher", "oress": "press",
    "cirl": "curl", "xurl": "curl", "curls": "curl", "mahine": "machine", "maxhine": "machine",
    "pulldonw": "pulldown", "pulkdown": "pulldown", "pulldow": "pulldown", "douvle": "dual",
    "double": "dual", "swuats": "squat", "squats": "squat", "99": "90", "dec": "deck",
    "ext": "extension", "extensions": "extension", "supp": "supported", "pullovers": "pullover",
    "rows": "row", "flies": "fly", "raises": "raise", "rotations": "rotation", "hangs": "hang",
    "calves": "calf", "lhs": "lbs", "dumbbells": "db", "dumbbell": "db", "freemotion": "freemotion",
}
PHRASES = [
    (r"\blife\s*fit(?:ness)?\b|\blifefit\b|\blf\b", "lifefitness"),
    (r"\bfree\s*motion\b|\bfm\b", "freemotion"),
    (r"\bhammer\s*strength\b", "hammer"),
    (r"\bgay\s*machine\b|\bgay\b", "gaymachine"),
    (r"\bsingle\s*leg\b|\bsl\b", "unilateral leg"),
    (r"\bsingle\s*arm\b|\bsingle\b|\bsa\b|\buni(?:lateral)?\b", "unilateral"),
    (r"\bplate\s*loaded\b", "plateloaded"),
    (r"\bbehind\s*neck\b|\bbehindneck\b", "behindneck"),
    (r"\bt\s*bar\b", "tbar"),
    (r"\bpull\s*ups?\b", "pullup"),
    (r"\by\s*raise\b", "yraise"),
    (r"\bpush\s*down\b", "pushdown"),
    (r"\bchest\s*supported\b", "chestsupported"),
    (r"\bstraight\s*arm\b", "straightarm"),
    (r"\bbent\s*arm\b", "bentarm"),
    (r"\bstraight\s*bar\b", "straightbar"),
    (r"\bez\s*bar\b", "ez"),
    (r"\bface\s*away\b", "faceaway"),
    (r"\bwide\s*grip\b", "widegrip"),
    (r"\b(\d+)\s*deg\b", r"\1deg"),
]
# Qualifiers in parentheses that name equipment, as opposed to comments.
EQUIPMENT_QUALIFIERS = {"matrix", "lifefitness", "freemotion", "dual", "seated", "unilateral",
                        "bench", "db", "ez", "gaymachine", "cable", "plateloaded", "upstairs"}

# (family, regex on the normalized name, core words that don't distinguish variants).
FAMILIES = [
    ("pullover", r"pullover", {"pullover"}),
    ("jefferson curl", r"jefferson", {"jefferson", "curl"}),
    ("preacher curl", r"preacher|curl machine|biceps curl|atlantis curl|brachialis curl", {"preacher", "curl", "biceps"}),
    ("incline curl", r"incline curl|recline curl", {"curl"}),
    ("cable curl", r"cable curl", {"cable", "curl"}),
    ("y raise", r"yraise", {"yraise"}),
    ("lateral raise", r"\blateral\b", {"lateral", "raise"}),
    ("straight-arm pulldown", r"straightarm pushdown", {"straightarm", "pushdown"}),
    ("overhead triceps extension", r"overhead extension", {"overhead", "extension"}),
    ("triceps extension", r"pushdown|triceps|tricep|baby extension|cross extension|carter extension|matrix extension",
     {"pushdown", "triceps", "tricep", "extension"}),
    ("leg extension", r"leg extension|unilateral leg extension", {"leg", "extension"}),
    ("leg curl", r"leg curl|hamstring curl|seated ham", {"leg", "curl"}),
    ("calf raise", r"\bcalf\b", {"calf", "press", "raise"}),
    ("stiff-leg deadlift", r"sldl|rdl", set()),
    ("back extension", r"back extension", {"back", "extension"}),
    ("hip press", r"hip press", {"hip", "press"}),
    ("leg press", r"leg press", {"leg", "press"}),
    ("hip thrust", r"hip thrust", {"hip", "thrust"}),
    ("squat", r"squat|pendulum", {"squat"}),
    ("incline press", r"incline (?:chest )?press|incline smith", {"incline", "press", "chest"}),
    ("pec fly", r"pec deck|\bfly\b", {"pec", "chest", "fly"}),
    ("flat press", r"chest press|flat press|flat bench|bench|smith", {"flat", "press", "bench", "chest", "variation"}),
    ("dip", r"\bdip\b", {"dip"}),
    ("shoulder press", r"shoulder press|behindneck press", {"shoulder", "press"}),
    ("vertical pull", r"pulldown|pullup|frontal pull|lat row down", {"pulldown", "pull"}),
    ("row", r"\brow\b|gaymachine$", {"row"}),
    ("hip rotation", r"hip rotation|external hip|torso rotation|golf swing", {"rotation"}),
    ("shoulder external rotation", r"external rotation|rotator cuff", {"rotation"}),
]
FILLER = {"the", "new", "a"}
# Equipment that's the same at every gym, so the gym doesn't split the variant.
PORTABLE = {"db", "ez"}


def normalize(name):
    words = [TYPOS.get(w, w) for w in re.findall(r"[a-z0-9]+", name.lower().replace("-", " "))]
    s = " ".join(words)
    for rx, rep in PHRASES:
        s = re.sub(rx, rep, s)
    return s


def family_of(norm):
    for fam, rx, core in FAMILIES:
        if re.search(rx, norm):
            return fam, core
    return norm, set(norm.split())


def gym_of(label):
    return (label or "unknown").split()[0]


def top_weight(entry):
    ws = [s["weight"] for s in entry["sets"] if s["logged"] and s["weight"] is not None and not s["weight_inferred"]]
    return max(ws) if ws else None


def load_exclusions(path=Path("overrides/entries.toml")):
    if not path.exists():
        return []
    with path.open("rb") as f:
        return tomllib.load(f).get("exclude", [])


def excluded_by(rule, cycle, entry):
    return rule["raw"] == entry["raw"] and rule.get("note", cycle["note_id"]) == cycle["note_id"]


def collect(parsed, exclusions=()):
    """Tracked entries as rows. Entries matching an exclusion are marked excluded and
    left out, so they never shape a variant."""
    labels = {b["slug"]: b["label"] for b in parsed["blocks"]}
    rows = []
    unused = list(exclusions)
    for c in parsed["cycles"]:
        for s in c["sessions"]:
            for e in s["entries"]:
                if not e["tracked"]:
                    continue
                hit = [r for r in exclusions if excluded_by(r, c, e)]
                if hit:
                    e["excluded"], e["variant"] = True, None
                    unused = [r for r in unused if r not in hit]
                    continue
                norm = normalize(e["name"])
                fam, core = family_of(norm)
                tokens = [w for w in norm.split() if w not in core and w not in FILLER]
                for q in e["qualifiers"]:
                    qn = normalize(q)
                    tokens += [w for w in qn.split() if w in EQUIPMENT_QUALIFIERS]
                    if qn == "hammer":  # "(hammer)" after a name is the grip, not Hammer Strength
                        tokens.append("hammergrip")
                tokens = tuple(sorted(set(tokens)))
                gym = "any" if set(tokens) & PORTABLE else gym_of(labels.get(c["block"]))
                rows.append({"name": e["name"], "family": fam, "key": (fam, tokens, gym),
                             "date": c["started_on"], "weight": top_weight(e), "entry": e,
                             "gym": gym_of(labels.get(c["block"])),
                             "plates": any("plate" in (x["weight_note"] or "") for x in e["sets"])})
    for r in unused:
        print(f"warning: overrides/entries.toml exclusion matched no entry: {r['raw']!r}")
    return rows


def cluster(weights, ratio=1.3):
    """Split sorted weights where one is more than `ratio` times the one before."""
    groups, cur = [], []
    for w in sorted(weights):
        if cur and w > cur[-1] * ratio:
            groups.append(cur)
            cur = []
        cur.append(w)
    if cur:
        groups.append(cur)
    return groups


def median_weight(members):
    ws = [r["weight"] for r in members if r["weight"]]
    return statistics.median(ws) if ws else None


CHECKS = defaultdict(list)


def assign(rows):
    """Return {variant key: [(row, basis)]}. Notes on suspicious splits go in CHECKS."""
    by_key = defaultdict(list)
    for r in rows:
        by_key[r["key"]].append(r)

    variants = defaultdict(list)
    for key, members in by_key.items():
        groups = cluster([r["weight"] for r in members if r["weight"]])
        if len(groups) <= 1:
            variants[key] += [(r, "name") for r in members]
            continue
        fam, tokens, gym = key
        # Only a more specific name can absorb a weight group: "Lateral Raise" can join
        # "Cable Lateral Raise", but "FM Pushdown" can't join "Matrix SA Pushdown".
        siblings = {k: median_weight(m) for k, m in by_key.items()
                    if k != key and k[0] == fam and k[2] in (gym, "any") and set(tokens) < set(k[1])
                    and len(cluster([r["weight"] for r in m if r["weight"]])) <= 1}
        siblings = {k: m for k, m in siblings.items() if m}
        largest = max(groups, key=len)
        lo, hi = statistics.median(groups[0]), statistics.median(groups[-1])
        if 1.8 <= hi / lo <= 3.6 and any(r["plates"] for r in members):
            CHECKS[key].append(f"split ~{lo:g} vs ~{hi:g}lb, and some weights are in plates: "
                               "per side vs total, rather than two machines?")
        for g in groups:
            gm = statistics.median(g)
            best = min(siblings.items(), key=lambda km: abs(km[1] - gm) / km[1], default=None)
            if best and abs(best[1] - gm) / best[1] <= 0.30:
                target, basis = best[0], f"weight {gm:g} matches median {best[1]:g}"
            elif g is largest:
                target, basis = key, "name"
            else:
                target, basis = (fam, tokens + (f"~{g[0]:g}-{g[-1]:g}lb",), gym), "own weight group"
            for r in members:
                if r["weight"] in g:
                    variants[target].append((r, basis))
        for r in members:
            if not r["weight"]:
                variants[key].append((r, "no weight"))
    return variants


def slug(key):
    fam, tokens, gym = key
    parts = [t.strip("~") for t in tokens] + [fam] + ([] if gym == "any" else [gym])
    return re.sub(r"[^a-z0-9]+", "_", " ".join(parts).lower()).strip("_")


ACRONYMS = {"db": "DB", "ez": "EZ", "fm": "FM", "sa": "SA", "sl": "SL", "lf": "LF", "rdl": "RDL",
            "sldl": "SLDL", "pjr": "PJR", "pc": "PC", "gaymachine": "Gay Machine", "freemotion": "Free Motion",
            "lifefitness": "Life Fitness", "straightbar": "Straight Bar", "unilateral": "Unilateral"}


def title(name):
    return " ".join(ACRONYMS.get(w.lower(), w.capitalize()) for w in name.split())


def resolve(variants, rules):
    """Apply the reviewed rules. Returns (final variants, removed entry count)."""
    by_slug = {slug(k): (k, members) for k, members in variants.items()}

    def target(s, seen=()):
        rule = rules.get(s, {})
        if rule.get("status") == "merge":
            if rule["merge_into"] in seen or rule["merge_into"] not in by_slug:
                raise SystemExit(f"bad merge_into for {s}: {rule['merge_into']}")
            return target(rule["merge_into"], seen + (s,))
        return s

    groups = defaultdict(list)
    removed = 0
    for s, (key, members) in by_slug.items():
        rule = rules.get(s, {})
        if rule.get("status") == "remove":
            for r, _ in members:
                r["entry"]["excluded"] = True
                r["entry"]["variant"] = None
            removed += len(members)
            continue
        groups[target(s)].append((s, key, members, rule.get("weight_factor", 1.0)))

    final = []
    for root, parts in groups.items():
        rows = [(r, f) for _, _, members, f in parts for r, _ in members]
        gyms = Counter("any" if key[2] == "any" else r["gym"] for _, key, members, _ in parts for r, _ in members)
        top = max(gyms.values())
        tied = {g for g, n in gyms.items() if n == top}
        gym = max((r for r, _ in rows if ("any" if r["key"][2] == "any" else r["gym"]) in tied), key=lambda r: r["date"])
        gym = "any" if gym["key"][2] == "any" else gym["gym"]
        names = Counter(r["name"].lower() for r, _ in rows)
        display = rules.get(root, {}).get("display") or title(names.most_common(1)[0][0])
        family, tokens, _ = by_slug[root][0]
        status = "confirmed" if rules.get(root, {}).get("status") == "confirmed" else "suggested"
        final.append({"display": display, "family": family, "gym": gym, "rows": rows, "tokens": tokens, "status": status,
                      "merged": sorted(s for s, *_ in parts if s != root), "root": root})

    # Two variants at one gym with the same name: add the words that tell them apart.
    dupes = Counter((v["display"].lower(), v["gym"]) for v in final)
    for v in final:
        if dupes[(v["display"].lower(), v["gym"])] > 1:
            squashed = re.sub(r"[^a-z0-9]", "", v["display"].lower())
            extra = [t for t in v["tokens"] if t.lower() not in squashed and not t.startswith("~")]
            band = [t.strip("~") for t in v["tokens"] if t.startswith("~")]
            if extra or band:
                v["display"] += f" ({title(' '.join(extra or band))})"

    # Slug from the display name, with the gym added only where two variants share a name.
    counts = Counter(re.sub(r"[^a-z0-9]+", "_", v["display"].lower()).strip("_") for v in final)
    for v in final:
        base = re.sub(r"[^a-z0-9]+", "_", v["display"].lower()).strip("_")
        v["slug"] = f"{base}_{v['gym']}" if counts[base] > 1 else base
    taken = Counter(v["slug"] for v in final)
    if any(n > 1 for n in taken.values()):
        raise SystemExit(f"duplicate variant slugs: {[s for s, n in taken.items() if n > 1]}; set display in overrides/aliases.toml")
    for v in final:
        for r, factor in v["rows"]:
            e = r["entry"]
            e["variant"], e["excluded"] = v["slug"], False
            if factor != 1.0:
                for x in e["sets"]:
                    if x["weight"] is not None:
                        x["weight_original"] = x["weight"]
                        x["weight"] = x["weight"] * factor
                        x["weight_note"] = "; ".join(filter(None, [x["weight_note"], f"x{factor:g} from review"]))
    return final, removed


def write_resolved(parsed, final, removed, rows):
    unassigned = sum(1 for r in rows if "variant" not in r["entry"])
    out = Path("exports/grouped.json")
    parsed = {**parsed, "variants": [
        {"slug": v["slug"], "display": v["display"], "family": v["family"], "gym": v["gym"], "status": v["status"],
         "names": sorted({r["name"].lower() for r, _ in v["rows"]})} for v in final]}
    out.write_text(json.dumps(parsed, indent=2, ensure_ascii=False) + "\n")

    lines = ["# Final exercise variants after review. Generated by scripts/group_exercises.py.",
             "# Don't edit: re-running the script overwrites this file. Make changes, including",
             "# status = \"confirmed\", in overrides/aliases.toml.", ""]
    confirmed = sum(v["status"] == "confirmed" for v in final)
    print(f"Resolved: {len(final)} variants ({confirmed} confirmed), {removed} entries removed, {unassigned} unassigned\n")
    for v in sorted(final, key=lambda v: (v["family"], v["display"])):
        ws = [r["weight"] * f for r, f in v["rows"] if r["weight"]]
        wtxt = f"{min(ws):g}-{max(ws):g}lb (median {statistics.median(ws):g})" if ws else "no weights"
        names = sorted({r["name"].lower() for r, _ in v["rows"]})
        merged = f", merged {len(v['merged'])}" if v["merged"] else ""
        print(f"  [{v['family']}] {v['display']} ({v['gym']}): {len(v['rows'])} entries, {wtxt}{merged}")
        lines += [f"[variants.{v['slug']}]", f'display = "{v["display"]}"', f'family = "{v["family"]}"',
                  f'gym = "{v["gym"]}"', f"names = {json.dumps(names)}", f'weights = "{wtxt}"',
                  f"merged_from = {json.dumps([v['root']] + v['merged'])}", f'status = "{v["status"]}"', ""]
    Path("overrides/variants.toml").write_text("\n".join(lines))
    print(f"\nwrote {out} and overrides/variants.toml")


def main(argv):
    parsed = json.loads(Path(argv[0] if argv else "exports/parsed.json").read_text())
    exclusions = load_exclusions()
    rows = collect(parsed, exclusions)
    if exclusions:
        print(f"{len(exclusions)} entries excluded by overrides/entries.toml")
    variants = assign(rows)
    rules_path = Path("overrides/aliases.toml")
    if rules_path.exists():
        with rules_path.open("rb") as f:
            rules = tomllib.load(f).get("variants", {})
        final, removed = resolve(variants, rules)
        write_resolved(parsed, final, removed, rows)
        return

    families = defaultdict(list)
    for key, members in variants.items():
        families[key[0]].append((key, members))

    lines = ["# Suggested exercise variants. Generated by scripts/group_exercises.py.",
             "# Review, edit, and save as overrides/aliases.toml with status = \"confirmed\".", ""]
    print(f"{len(rows)} tracked entries -> {len(families)} families, {len(variants)} variants\n")
    for fam in sorted(families, key=lambda f: -sum(len(m) for _, m in families[f])):
        vs = sorted(families[fam], key=lambda kv: -len(kv[1]))
        print(f"{fam} ({sum(len(m) for _, m in vs)} entries)")
        meds = {k: median_weight([r for r, _ in m]) for k, m in vs}
        for key, members in vs:
            rs = [r for r, _ in members]
            ws = [r["weight"] for r in rs if r["weight"]]
            names = sorted({r["name"].lower() for r in rs})
            dates = sorted(r["date"] for r in rs)
            wtxt = f"{min(ws):g}-{max(ws):g}lb (median {statistics.median(ws):g})" if ws else "no weights"
            print(f"  {slug(key)}: {len(rs)} entries, {wtxt}, {dates[0]} to {dates[-1]}")
            print(f"    names: {', '.join(names)}")
            moved = defaultdict(int)
            for r, b in members:
                if b.startswith("weight"):
                    moved[r["name"].lower()] += 1
            if moved:
                print("    added by weight: " + ", ".join(f"{n} x{c}" for n, c in sorted(moved.items())))
            for note in CHECKS.get(key, []):
                print(f"    CHECK: {note}")
            groups = cluster(ws, ratio=1.8)
            if len(groups) > 1:
                plates = " (some weights are in plates)" if any(r["plates"] for r in rs) else ""
                print(f"    CHECK: weights jump from ~{statistics.median(groups[0]):g} to ~{statistics.median(groups[-1]):g}lb{plates}; "
                      "per-side vs total, or two machines?")
            # A rarely used name with the same weights as a common one is often the
            # same machine under another name.
            m = meds[key]
            if m and len(rs) <= 5:
                common = [(abs(mk - m) / m, k) for k, mk in meds.items()
                          if k != key and mk and k[2] in (key[2], "any") and len(variants[k]) > len(rs)]
                close = sorted(c for c in common if c[0] <= 0.10)
                if close:
                    print(f"    possibly the same machine as: {slug(close[0][1])} (median {meds[close[0][1]]:g})")
            lines += [f"[variants.{slug(key)}]", f'family = "{fam}"', f'gym = "{key[2]}"',
                      f"names = {json.dumps(names)}", f'weights = "{wtxt}"', 'status = "suggested"', ""]
        print()
    out = Path("overrides/aliases.suggested.toml")
    out.parent.mkdir(exist_ok=True)
    out.write_text("\n".join(lines))
    print(f"wrote {out}")


if __name__ == "__main__":
    main(sys.argv[1:])
