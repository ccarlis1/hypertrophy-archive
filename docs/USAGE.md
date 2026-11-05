# Using Hypertrophy Archive

Hypertrophy Archive is two tools in one app:

1. **An exercise archive.** Each exercise is a *joint action* (target muscles, joints,
   resistance profile, plane of motion), saved as a JSON file.
2. **A workout logger.** Sessions, exercises and sets go in a local SQLite database,
   with last-time prefill and a history of everything you've logged. Old logs written in
   Apple Notes can be imported into the same database.

The logger links back to the archive: every logged exercise is a **variant** (one
specific machine or setup, e.g. "Hammer Strength Preacher Curl"), and a variant can be
linked to the joint action it trains.

```
joint action  (JSON in data/)        elbow_flexion_preacher
  └─ variant  (database)             Hammer Strength Preacher Curl
       └─ aliases                    "hammer preachers", "hammer strength preacher", …
```

## Contents
- [Install and run](#install-and-run)
- [Where your data lives](#where-your-data-lives)
- [The app window](#the-app-window)
  - [Editor: write a joint action](#editor-write-a-joint-action)
  - [Log: record a workout](#log-record-a-workout)
  - [History: look back](#history-look-back)
  - [Variants: manage machines and setups](#variants-manage-machines-and-setups)
  - [Progress: long-term charts](#progress-long-term-charts)
- [How progress is measured](#how-progress-is-measured)
- [The `ha` command line](#the-ha-command-line)
- [Importing Apple Notes history](#importing-apple-notes-history)
- [Backups, undo and safe testing](#backups-undo-and-safe-testing)
- [Troubleshooting](#troubleshooting)

## Install and run
You need [Rust](https://rustup.rs) (stable). It builds on macOS and Windows.

```bash
cargo run                 # open the app window
cargo install --path .    # optional: install `ha` and the app on your PATH
```

Without `cargo install`, run the command-line tool from the repo root as
`cargo run --bin ha -- <command>`. This guide writes it as `ha <command>`.

## Where your data lives
| What | Where |
|---|---|
| Joint actions | `data/<muscle>/<name>.json`, relative to where you run the app |
| Workout database | `log.db` in the app-data folder: `~/Library/Application Support/com.hypertrophy-archive.hypertrophy-archive/` on macOS, the matching `AppData` folder on Windows |
| Database backups | `log.db.bak-<date>-<time>` next to `log.db` |
| Notes export and override files | `exports/` and `overrides/` in the repo (gitignored, never committed) |

Two ways to use a different database:
- `HA_DATA_DIR=/some/folder` puts `log.db` in that folder. It works for both the app
  and `ha`.
- `ha --db /path/to/file.db …` picks a file for one command.

## The app window
The tabs across the top are **Editor**, **Log**, **History**, **Variants** and
**Progress**. The
status line shows the result of the last action ("saved", errors, and so on).

### Editor: write a joint action
The editor walks through five steps. Use the **Next** and **Previous** buttons, or click
the step names.

1. **Basic Info:** name, type (compound or isolation), description, plane of motion and
   resistance profile (ascending, descending, bell or constant).
2. **Target Muscles:** add muscle groups, pick which divisions are worked (e.g. the
   clavicular head of the chest), set each division's bias, and tick **Primary** on the
   main muscle.
3. **Joints Involved:** add each joint. A moving joint has a direction and a start and
   end angle; a fixed joint has one angle.
4. **Additional Info:** form tips and a technique video link.
5. **Preview:** check everything, then **Save Exercise** and enter a file name.

The file is saved to `data/<primary muscle>/<file name>.json` (the first muscle is used
if none is marked primary). Its `name` is the slug that variants link to.

### Log: record a workout
1. **Block:** pick the training block (one program, season or school year). To start a
   new one, open **New block** and enter a slug (e.g. `summer_2027`), a name and a split
   (upper/lower, full body, anterior/posterior or other), then **Create block**.
2. **Week and Day:** type the week number and a day label such as `Upper 1`, then click
   **New / load session**.
   - If that session exists, it loads and you can edit it.
   - If not, the screen says "new session (unsaved)". Nothing is written until you save,
     so a typo doesn't leave an empty session behind.
3. **Add exercise:** start typing. The search matches variant names, slugs and aliases.
   Click a result to add it.
   - The exercise shows what you did last time (`Last: 185×5 @1`), and its sets are
     prefilled from that session. A set you left blank last time comes back blank.
   - Each set has weight (`w`), reps (`r`), an RIR range (`rir` low – high), a note, and a
     **logged** tick. Untick it for a set you skipped.
   - **+ set** adds a set. 🗑 deletes a set or a whole exercise. Sets are renumbered
     when you save.
   - The **settings** box holds machine setup for this session (e.g. "seat 3").
4. **Untracked:** for warm-ups or anything you don't track, enter a number and a name
   ("2 sets of calves") and click **Add**.
5. Click **Save session**.

Weights are in pounds. RIR is stored as a range, so `1` is 1–1 and `1–2` is 1–2.

### History: look back
Sessions are listed by block and week. **Refresh** reloads the list.
- **Show** displays a session's exercises and sets.
- **Edit in Log** opens the session in the Log tab so you can change it and save again.

Imported sessions and sessions logged in the app appear side by side.

### Variants: manage machines and setups
A variant is one specific machine or setup. Two machines for the same movement are two
variants, because their weights aren't comparable.
- **Create:** enter a slug and a display name.
- Select a variant to edit its **display** name, **family** (e.g. "preacher curl"),
  **gym**, **joint_action** (the slug of a joint action from the Editor) and
  **default_settings** (e.g. "seat 3, pad 2"). Then click **Save variant**.

### Progress: long-term charts
The data loads when you open the tab. Click **Reload** after logging, importing or
linking variants. There are three views:
- **Variant:** e1RM over time, with e5RM next to it (untick **e5RM** to hide it). PRs are
  yellow diamonds and stalled sessions are red dots. Below the chart, **Sessions** lists
  the top set, best set and % change from baseline, and **Rep PRs** shows the most reps
  done at each weight. The filter box narrows the variant list.
- **Joint action:** one line per linked variant, as a % change from that variant's own
  baseline, so machines with different load schemes can be compared.
- **Sets per muscle:** pick a week and see its hard sets per muscle. Tick **Divisions**
  to split them by division. Hard sets from variants with no joint action are listed
  under the chart but not counted.

## How progress is measured
- **Reps to failure:** `r = reps + RIR midpoint`. A range like 1–2 counts as 1.5, "3+"
  counts as 3, and a set with no RIR counts as 0 (taken to failure).
- **e1RM** = `w × (1 + r/30)`, from the session's best set. **e5RM** =
  `e1RM / (1 + 5/30)`, so a set with `r = 5` gives e5RM = `w`. The constant 30 can be
  changed with `ha --epley-c`.
- **PR:** a session's e1RM beats every earlier session of that variant. The first
  session is the baseline, not a PR.
- **Stalled:** 4 or more sessions in a row without a new e1RM best.
- **Baseline:** the median e1RM of a variant's first 3 sessions.
- **Hard set:** a logged set with reps and an RIR midpoint of 2 or less. Untracked
  entries don't count. Each hard set counts 1 toward the joint action's primary muscle
  and 0.5 toward each other muscle, and the same to each active division.
- Sessions with no set that has both a weight and reps (bodyweight, all blank) have no
  strength estimate.

## The `ha` command line
| Command | What it does |
|---|---|
| `ha log show <session_id>` | Print a session with all its exercises and sets |
| `ha variants list` | List variants, with their linked joint action and status |
| `ha variants link <variant> <joint_action>` | Link a variant to a joint action |
| `ha variants seed exports/grouped.json` | Load variants and aliases without importing history |
| `ha import exports/grouped.json` | Import Apple Notes history (next section) |
| `ha stats <variant>` | Session-by-session e1RM, e5RM, top set, PRs, stall flag and rep PRs. Takes a slug or a search term that matches one variant |
| `ha stats <joint_action>` | % change from baseline for every variant linked to it |
| `ha stats <muscle>` | Weekly hard sets for a muscle (e.g. `"elbow flexors"`), with divisions |
| `ha cycle <block> <week>` | One week's hard sets per muscle, PRs and stalls |
| `ha export csv [-o file]` | One row per set, with the session's e1RM and e5RM for that variant, the session's `date_precision`, and `weight_outlier` (the importer's ±40%-of-median rule). Write it under `exports/` so it stays out of git |

Every command takes `--db <path>`, `--data <folder>` (the joint actions, default `data`)
and `--epley-c <c>`. `ha --help` and `ha <command> --help` list all options.

## Importing Apple Notes history
This is for workout logs written in Apple Notes, one note per week, with titles like
`Log Book Week 3` or `Full Body Week 2`. It runs on macOS.

### The four steps
Run them from the repo root:

```bash
# 1. Export matching notes to exports/notes.json
#    (the first run asks for permission to control Notes)
osascript -l JavaScript scripts/export_notes.js

# 2. Parse the notes into exports/parsed.json
python3 scripts/parse_notes.py

# 3. Match exercise names to variants: writes exports/grouped.json
python3 scripts/group_exercises.py

# 4. Load into the database. Try --dry-run first.
ha import exports/grouped.json --dry-run
ha import exports/grouped.json
```

`python3 scripts/notes_report.py` checks the export for missing, duplicate or
out-of-order weeks before you parse.

### Fixing what the scripts get wrong
Everything you correct by hand lives in `overrides/`, so a re-run reproduces it.

| File | What it's for |
|---|---|
| `overrides/titles.toml` | A note with an unusual or wrong title: set its block and week by note id |
| `overrides/lines.toml` | A line the parser misreads: give replacement text to parse, or `accept` it as is. These resolve items in the review list |
| `overrides/aliases.toml` | Exercise grouping. Start from `aliases.suggested.toml`, which step 3 writes. Mark each variant `confirmed`, `merge` it into another (`merge_into`), or `remove` it. `display` sets its name and `weight_factor` scales its weights when merging |

The parser lists lines it can't fully trust as **review items**, with the note title and
line number. The import report shows how many are still open.

### Re-importing
Run all four steps again whenever you've logged more in Notes or edited an override
file. Re-importing is safe:
- Notes that haven't changed are skipped. A changed note replaces only the rows it
  imported before. **Sessions logged in the app are never touched.**
- If a week's note was re-created in Notes (it has a new id), the new note replaces the
  old one's week.
- Variant names, families and aliases follow `overrides/aliases.toml`. A joint action or
  default settings you set in the app are kept.
- `--force` re-imports every note, even unchanged ones.

### Reading the report
| Line | Meaning |
|---|---|
| `Cycles: N new, N replaced, N unchanged` | Weeks added, re-imported, or skipped |
| `Variants: … created, … updated, … aliases moved` | Changes from `aliases.toml` |
| `Review: N open, N accepted, N fixed` | Parser review items; aim for 0 open |
| `Unmapped entries` | Tracked exercises with no variant. Add them to `aliases.toml` |
| `Outliers (±40% of median)` | Sets far from that variant's usual weight: often a typo, or per-side vs total plates |
| `Stale notes` | Notes in the database that the export no longer has. They're kept |
| `Re-created notes` | Weeks whose note got a new id |

`-v` also lists every weight the parser filled in from an earlier set.

## Backups, undo and safe testing
- Every `ha import` (except `--dry-run`) copies `log.db` to `log.db.bak-<date>-<time>`
  first and keeps the 10 newest copies.
- **To undo an import:** close the app, then copy the backup over the database:
  ```bash
  cp "<data folder>/log.db.bak-20261030-140803-893" "<data folder>/log.db"
  ```
- `--dry-run` imports into a temporary copy and prints the report. Your database isn't
  changed, or even created.
- To try anything without risk, use a throwaway folder:
  ```bash
  export HA_DATA_DIR=$(mktemp -d)
  ha import exports/grouped.json
  cargo run
  ```

## Troubleshooting
| Problem | Fix |
|---|---|
| `zsh: command not found: ha` | Run `cargo install --path .`, or use `cargo run --bin ha -- …` |
| The app says "Database not available." | Check that the data folder is writable, or set `HA_DATA_DIR` |
| An exercise doesn't appear in the Log search | Create it in **Variants**, or add it to `aliases.toml` and re-import |
| The import stops with "two notes in the file" for one week | Two notes claim the same block and week. Fix one note's title, or set it in `overrides/titles.toml` |
| `export_notes.js` fails with a permission error | Allow your terminal to control Notes in System Settings → Privacy & Security → Automation |
