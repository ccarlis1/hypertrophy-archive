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

## Features
- **Exercise archive:** write joint actions (target muscles and divisions, joints and
  angles, resistance profile, plane of motion) in a step-by-step editor, saved as JSON.
- **Workout logger:** log sessions by training block, week and day, with exercise search,
  last-time prefill, RIR ranges, untracked warm-up sets and machine settings.
- **History and variants:** browse and edit past sessions, and link each machine or setup
  (a variant) to the joint action it trains.
- **Apple Notes import:** turn years of workout notes into the same database. Re-running
  it is safe, with backups and a dry-run mode.
- **`ha` command line:** print sessions, manage variants, and run the import.

**See [docs/USAGE.md](docs/USAGE.md) for the full guide**: where data lives, each tab of
the app, every `ha` command, the Notes import, backups and troubleshooting.

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