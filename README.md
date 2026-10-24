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
cargo run          # GUI (joint-action editor + Log / History / Variants)
cargo run --bin ha # CLI
```

The workout database defaults to Application Support. Override with `HA_DATA_DIR`
(useful for throwaway testing — never point experiments at your real database).

## Notes import pipeline
Personal Apple Notes history is exported and normalized locally, then imported
into SQLite. `exports/` and `overrides/` are gitignored (the repo is public).

```bash
# 1. Export notes (macOS; needs Automation permission)
osascript -l JavaScript scripts/export_notes.js

# 2. Parse into exports/parsed.json (applies overrides/titles.toml and
#    overrides/lines.toml for line fixes / accepts)
python3 scripts/parse_notes.py

# 3. Resolve exercise names → variants (reads overrides/aliases.toml)
python3 scripts/group_exercises.py
# writes exports/grouped.json and overrides/variants.toml

# 4. Import into the local database (idempotent on note_id)
ha import exports/grouped.json
ha import exports/grouped.json --dry-run   # report only
ha import exports/grouped.json -v          # include inferred-weight list
```

`ha import` backs up `log.db` before writing (keeping the 10 newest backups), replaces only `source='notes_import'`
rows for each note, and never touches `source='app'` sessions. Seed variants
without importing history with `ha variants seed exports/grouped.json`.

Other CLI commands: `ha log show <session_id>`, `ha variants list`,
`ha variants link <variant> <joint_action>`.

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