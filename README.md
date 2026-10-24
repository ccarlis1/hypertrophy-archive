# Hypertrophy Archive
A collection of the best exercises for muscle growth

## Description
This is an app is made to organize a list of, in theory, any exercise you would need in a hypertrophy training program. Each exercise is defined by characteristics of target muscle, resistance profile, single joint (isolation) or multi-joint (compound), and other minor characteristics.

## Tech Stack
- Written in Rust (`eframe`/`egui` GUI, `rusqlite` workout log, `ha` CLI)
- Joint actions as JSON; workout history in a local SQLite database

## Purpose
- This app is made mainly for personal interest. My goal with it is for people to have access to good exercises in general, and for developers to have access to them in a proper data format.
- Users are also able to download the app for themselves and enter their own exercises to store them locally or to contribute to the project.

## Build and run
```bash
cargo build
cargo run                     # GUI (joint-action editor + Log / History / Variants)
cargo run --bin ha -- --help  # CLI
cargo install --path .        # optional: puts `ha` on your PATH
```

The examples below write `ha …`. Without `cargo install`, use `cargo run --bin ha -- …`
from the repo root instead.

## Usage

### Where the data lives
- **Joint actions** (the exercise archive) are JSON files in `data/<muscle>/<name>.json`,
  edited in the GUI's **Editor** tab.
- **Workout history** is a SQLite database, `log.db`, in the platform's
  application-data folder (`~/Library/Application Support/com.hypertrophy-archive.hypertrophy-archive/`
  on macOS). Set `HA_DATA_DIR` to use another folder, or pass `--db <path>` to `ha`.

To try things without touching your real history, point both at a throwaway folder:
```bash
export HA_DATA_DIR=$(mktemp -d)
cargo run            # GUI on an empty database
ha import exports/grouped.json
```

### Logging a workout (GUI)
Open the **Log** tab:
1. Pick a **block** (a training block, e.g. one program or school year), or create one
   with a slug, name and split.
2. Set the **week** and **day** (e.g. `Upper 1`) and click **New / load session**. An
   existing session loads; a new one is only written when you save.
3. **Add exercise** by searching variants or their aliases. Sets are prefilled from last
   time (`Last: 185×5 @1`). Each set has weight, reps, an RIR range and a note. Use
   **+ set** to add one and 🗑 to delete a set or an exercise.
4. For warm-ups or anything you don't track, use **Untracked: N sets of …**.
5. Click **Save session**.

**History** lists sessions by block and week. **Show** displays a session's entries and sets, and **Edit in Log**
opens it in the Log tab. **Variants** is where you rename variants, link them to a joint
action, and set default machine settings.

### CLI
| Command | What it does |
|---|---|
| `ha log show <session_id>` | Print a session with all its entries and sets |
| `ha variants list` | List variants |
| `ha variants link <variant> <joint_action>` | Link a variant to a joint-action slug |
| `ha variants seed exports/grouped.json` | Load variants and aliases without importing history |
| `ha import exports/grouped.json` | Import the Apple Notes history (below) |

Every command takes `--db <path>` to use a different database.

### Importing Apple Notes history
Old workout logs written in Apple Notes are exported and normalized locally, then
imported into the database. `exports/` and `overrides/` hold personal data and are
gitignored (the repo is public).

```bash
# 1. Export notes (macOS; the first run asks for Automation permission)
osascript -l JavaScript scripts/export_notes.js

# 2. Parse into exports/parsed.json (applies overrides/titles.toml and
#    overrides/lines.toml for line fixes / accepts)
python3 scripts/parse_notes.py

# 3. Resolve exercise names → variants (reads overrides/aliases.toml)
python3 scripts/group_exercises.py
# writes exports/grouped.json and overrides/variants.toml

# 4. Import into the database
ha import exports/grouped.json --dry-run   # report only, writes nothing
ha import exports/grouped.json
```

Run all four steps again whenever you've logged more in Notes or edited an override
file. Re-importing is safe:
- Notes that haven't changed are skipped. A changed note replaces only the rows it
  imported before. Sessions logged in the app are never touched.
- A week whose note was re-created (new note id) replaces the old note's week.
- Variant names, families and aliases follow `overrides/aliases.toml`. A joint action or
  machine settings you set in the app are kept.
- `log.db` is backed up next to itself before every import, keeping the 10 newest
  backups. To undo an import, copy a `log.db.bak-…` file back over `log.db`.

The report lists counts, open review items, unmapped entries, inferred weights, and
outliers (sets more than 40% off their variant's median). `-v` also lists each inferred
weight, and `--force` re-imports every note even if it hasn't changed.

## Entries
- The goal is to cover every joint action from three types of resistance profiles. That is how it is organized. For example, I will not be listing every way to do one exercise (dumbbell preacher curls vs. machine preacher curls), it will instead be organized via target muscle, with the characteristics of the joints involved, resistance profile, etc. and how that will bias some vs. other muscles. For example, a 45 degree preacher curl with a descending resistance profile (hardest at the stretched position) will primarily target the biceps, whereas a preacher curl with an asceding resistance profile will primarily target the brachialis and brachioradidalis muscles. These discrepancies will be properly handled.

## Example Entry
```
{
	"name": "chest_press",
	"id": 0,
	"type": "compound",
	"description": "A press in the transverse plane.",
	"target_muscles": {
		"muscle_name": "chest",
		"muscle_division": {
			"name": "sternocostal",
			"active": true
		},
		"muscle_division": {
			"name": "clavicular",
			"active": true
		},
		"muscle_division": {
			"name": "abdominal",
			"active": true
		}
	},
	"joints_involved": {
		"joint": {
			"name": "shoulder",
			"dynamic": false,
			"angle": 45
		},
		"joint": {
			"name": "elbow",
			"dynamic": true,
			"direction": "extension",
			"angle_initial": 30,
			"angle_final": 180
		}
	},
	"resistance_profile": "descending",
	"plane_of_motion": "transverse",
	"tips": "Keep those shoulders tucked!",
	"technique_video": "https://youtube.com/shorts/hWbUlkb5Ms4?si=P89i2PXyGlX_q7XE"
}
```