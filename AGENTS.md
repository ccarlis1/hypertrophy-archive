# AGENTS.md — Hypertrophy Archive

Instructions for any AI agent working in this repo.

## What this is
A Rust desktop app (`eframe`/`egui` 0.22) for defining exercises as **joint actions**
(target muscles, joints, resistance profile, plane of motion) and saving them as JSON.
It is being extended into a personal workout logger: log sessions, migrate years of
Apple Notes workout logs, and model long-term progression. Each logged exercise maps
back to a joint action.

This is a personal, ad-hoc tool. Pick simple over robust.

## The plan
The owner keeps the implementation plan privately, outside this repo. Work one phase
at a time. For each phase:
1. Start in plan mode. Read the phase section you were given, then write a short spec:
   tasks, open questions, and the tests you will add.
2. Wait for approval of the spec.
3. Implement it on a branch named `phase-<n>-<slug>`.
4. Finish by checking the phase's "done when" criteria and reporting what passed.

## Privacy (the repo is public)
Never commit personal training data:
- workout logs, the SQLite database (`*.db`)
- Apple Notes exports (`exports/`)
- override files (`overrides/`)
- real fixtures (`tests/fixtures/private/`)

Tests that need private fixtures must skip, not fail, when the files are missing.

## Commands
- Build: `cargo build`
- Run the GUI: `cargo run`
- Before every commit: `cargo clippy` and `cargo test`

## Current state to know about
- The crate only builds for Windows (`winapi`, `winres`, mingw linker in
  `.cargo/config.toml`). Phase 0 makes it cross-platform.
- `/data/` (the joint-action JSON) is gitignored. Whether it should be tracked is an
  open decision. Don't change it without asking.

## Conventions
- One logical change per commit.
- Keep the egui version as it is unless an upgrade is needed. If it is, do the
  upgrade as its own commit.
- Don't push or open pull requests unless asked.
