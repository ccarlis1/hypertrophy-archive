//! Fixture-based import tests (Phase 2e).
//!
//! Uses tests/fixtures/import/small.json — synthetic data, safe to commit.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use hypertrophy_archive::db::Db;
use hypertrophy_archive::import::{
    cycle_source_hash, find_outliers, format_report, import_grouped, load_grouped, ImportOptions,
};
use hypertrophy_archive::log::{
    DatePrecision, GroupedFile, NewBlock, NewCycle, NewEntry, NewSession, NewSet, Source, Split,
};

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/import/small.json")
}

fn temp_db() -> Db {
    // Tests run in parallel and the clock can repeat, so add a counter.
    static N: AtomicUsize = AtomicUsize::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let n = N.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("ha_import_test_{nanos}_{n}.db"));
    Db::open(&path).expect("open temp db")
}

fn dump_tables(db: &Db) -> BTreeMap<String, Vec<Vec<String>>> {
    let tables = [
        "block", "cycle", "session", "variant", "alias", "entry", "set_log",
    ];
    let mut out = BTreeMap::new();
    for table in tables {
        let mut stmt = db
            .conn()
            .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
            .unwrap();
        let col_count = stmt.column_count();
        let rows = stmt
            .query_map([], |row| {
                let mut vals = Vec::new();
                for i in 0..col_count {
                    let v: rusqlite::types::Value = row.get(i)?;
                    vals.push(format!("{v:?}"));
                }
                Ok(vals)
            })
            .unwrap();
        out.insert(
            table.to_string(),
            rows.map(|r| r.unwrap()).collect::<Vec<_>>(),
        );
    }
    out
}

#[test]
fn import_round_trips_fixture_fields() {
    let db = temp_db();
    let data = load_grouped(&fixture_path()).unwrap();
    let report = import_grouped(&db, &data, &ImportOptions::default()).unwrap();
    assert_eq!(report.cycles_new, 3);
    assert_eq!(report.sessions_written, 3);
    assert_eq!(report.excluded_entries, 1);
    assert_eq!(report.inferred_weights, 1);
    assert_eq!(report.review_accepted, 1);
    assert_eq!(report.review_fixed, 1);
    assert_eq!(report.review_open, 0);
    assert!(report.unmapped.is_empty());

    let sessions = db.list_sessions().unwrap();
    assert_eq!(sessions.len(), 3);
    let w1 = sessions
        .iter()
        .find(|s| s.cycle_title.as_deref() == Some("Test Week 1"))
        .unwrap();
    let detail = db.get_session_detail(w1.session.id).unwrap().unwrap();
    assert_eq!(detail.session.day_label.as_deref(), Some("Upper 1"));
    assert_eq!(detail.session.notes, vec!["felt good".to_string()]);
    assert_eq!(detail.session.source, Source::NotesImport);
    assert_eq!(detail.session.performed_on.as_deref(), Some("2026-01-01"));
    assert_eq!(detail.entries.len(), 3);

    let curl = &detail.entries[0];
    assert_eq!(curl.variant_slug.as_deref(), Some("cable_curl"));
    assert_eq!(curl.entry.settings.as_deref(), Some("seat 2"));
    assert_eq!(curl.sets.len(), 3);
    assert_eq!(curl.sets[0].weight, Some(50.0));
    assert!(curl.sets[1].weight_inferred);
    assert!(!curl.sets[2].logged);

    let untracked = &detail.entries[1];
    assert!(!untracked.entry.tracked);
    assert_eq!(untracked.entry.untracked_sets, Some(2));

    let excluded = &detail.entries[2];
    assert!(excluded.entry.excluded);
    assert!(excluded.entry.variant_id.is_none());

    let cycle = db
        .get_cycle_by_note_id("note://test/w1")
        .unwrap()
        .unwrap();
    assert!(cycle.source_hash.is_some());
    assert_eq!(cycle.source, Source::NotesImport);
}

#[test]
fn import_twice_is_idempotent() {
    let db = temp_db();
    let data = load_grouped(&fixture_path()).unwrap();
    let r1 = import_grouped(&db, &data, &ImportOptions::default()).unwrap();
    let dump1 = dump_tables(&db);
    let r2 = import_grouped(&db, &data, &ImportOptions::default()).unwrap();
    let dump2 = dump_tables(&db);
    assert_eq!(r2.cycles_unchanged, 3);
    assert_eq!(r2.cycles_new, 0);
    assert_eq!(r2.cycles_replaced, 0);
    assert_eq!(dump1, dump2);
    assert_eq!(r1.sessions_written, 3);
    assert_eq!(r2.sessions_written, 0);
}

#[test]
fn app_logged_sessions_survive_reimport() {
    let db = temp_db();
    let data = load_grouped(&fixture_path()).unwrap();
    import_grouped(&db, &data, &ImportOptions::default()).unwrap();

    // App session inside an imported week (test_block week 1).
    let cycle = db.get_cycle_by_note_id("note://test/w1").unwrap().unwrap();
    let app_in_week = db
        .create_session(&NewSession {
            cycle_id: cycle.id,
            day_number: Some(2),
            day_label: Some("Lower 1".into()),
            comment: Some("app logged".into()),
            header_raw: None,
            implicit: false,
            index: 99,
            notes: vec![],
            performed_on: Some("2026-01-02".into()),
            date_precision: DatePrecision::Exact,
            source: Source::App,
        })
        .unwrap();
    let v = db.get_variant_by_slug("cable_curl").unwrap().unwrap();
    db.add_entry_with_sets(
        &NewEntry {
            session_id: app_in_week,
            variant_id: Some(v.id),
            position: 1,
            tracked: true,
            name: Some("Cable Curl".into()),
            name_raw: "Cable Curl".into(),
            qualifiers: vec![],
            untracked_sets: None,
            settings: None,
            comment: None,
            raw: None,
            excluded: false,
        },
        &[NewSet {
            set_number: 1,
            weight: Some(60.0),
            weight_inferred: false,
            weight_note: None,
            reps: Some(5.0),
            rir_low: Some(1.0),
            rir_high: Some(1.0),
            logged: true,
            comment: None,
            raw: None,
        }],
    )
    .unwrap();

    // App-only block with its own session.
    let app_block = db
        .create_block(&NewBlock {
            slug: "app_only".into(),
            name: "App Only".into(),
            split: Split::Other,
            label: Some("app".into()),
            started_on: None,
        })
        .unwrap();
    let app_cycle = db
        .create_cycle(&NewCycle {
            block_id: app_block,
            week: 1,
            title: Some("App Week".into()),
            note_id: None,
            started_on: None,
            date_precision: DatePrecision::Exact,
            override_flag: false,
            source: Source::App,
            source_hash: None,
        })
        .unwrap();
    let app_only_session = db
        .create_session(&NewSession {
            cycle_id: app_cycle,
            day_number: Some(1),
            day_label: Some("Day 1".into()),
            comment: None,
            header_raw: None,
            implicit: false,
            index: 1,
            notes: vec![],
            performed_on: Some("2026-03-01".into()),
            date_precision: DatePrecision::Exact,
            source: Source::App,
        })
        .unwrap();
    db.add_entry_with_sets(
        &NewEntry {
            session_id: app_only_session,
            variant_id: Some(v.id),
            position: 1,
            tracked: true,
            name: Some("Cable Curl".into()),
            name_raw: "Cable Curl".into(),
            qualifiers: vec![],
            untracked_sets: None,
            settings: None,
            comment: None,
            raw: None,
            excluded: false,
        },
        &[NewSet {
            set_number: 1,
            weight: Some(70.0),
            weight_inferred: false,
            weight_note: None,
            reps: Some(4.0),
            rir_low: None,
            rir_high: None,
            logged: true,
            comment: None,
            raw: None,
        }],
    )
    .unwrap();

    // Modify week 1 in the file so the import must replace notes_import rows.
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(fixture_path()).unwrap()).unwrap();
    value["cycles"][0]["sessions"][0]["entries"][0]["sets"][0]["weight"] =
        serde_json::json!(51.0);
    let modified: GroupedFile = serde_json::from_value(value).unwrap();

    let report = import_grouped(&db, &modified, &ImportOptions::default()).unwrap();
    assert!(report.cycles_replaced >= 1);

    let app_detail = db.get_session_detail(app_in_week).unwrap().unwrap();
    assert_eq!(app_detail.session.source, Source::App);
    assert_eq!(app_detail.session.comment.as_deref(), Some("app logged"));
    assert_eq!(app_detail.entries[0].sets[0].weight, Some(60.0));

    let only = db.get_session_detail(app_only_session).unwrap().unwrap();
    assert_eq!(only.session.source, Source::App);
    assert_eq!(only.entries[0].sets[0].weight, Some(70.0));

    // Imported week 1 session was replaced with the new weight.
    let sessions = db.list_sessions().unwrap();
    let imported: Vec<_> = sessions
        .iter()
        .filter(|s| {
            s.session.source == Source::NotesImport
                && s.cycle_title.as_deref() == Some("Test Week 1")
        })
        .collect();
    assert_eq!(imported.len(), 1);
    let d = db
        .get_session_detail(imported[0].session.id)
        .unwrap()
        .unwrap();
    assert_eq!(d.entries[0].sets[0].weight, Some(51.0));
}

#[test]
fn changing_one_cycle_replaces_only_that_cycle() {
    let db = temp_db();
    let data = load_grouped(&fixture_path()).unwrap();
    import_grouped(&db, &data, &ImportOptions::default()).unwrap();
    let w2_before = db.get_cycle_by_note_id("note://test/w2").unwrap().unwrap();
    let w2_sessions_before = db.count_sessions_for_cycle(w2_before.id).unwrap();
    let w2_hash = w2_before.source_hash.clone();

    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(fixture_path()).unwrap()).unwrap();
    value["cycles"][0]["sessions"][0]["notes"] = serde_json::json!(["changed"]);
    let modified: GroupedFile = serde_json::from_value(value).unwrap();
    let report = import_grouped(&db, &modified, &ImportOptions::default()).unwrap();
    assert_eq!(report.cycles_replaced, 1);
    assert_eq!(report.cycles_unchanged, 2);

    let w2_after = db.get_cycle_by_note_id("note://test/w2").unwrap().unwrap();
    assert_eq!(w2_after.id, w2_before.id);
    assert_eq!(w2_after.source_hash, w2_hash);
    assert_eq!(
        db.count_sessions_for_cycle(w2_after.id).unwrap(),
        w2_sessions_before
    );

    let w1 = db.get_cycle_by_note_id("note://test/w1").unwrap().unwrap();
    let sessions = db.list_sessions().unwrap();
    let w1s: Vec<_> = sessions
        .iter()
        .filter(|s| s.session.cycle_id == w1.id && s.session.source == Source::NotesImport)
        .collect();
    let d = db.get_session_detail(w1s[0].session.id).unwrap().unwrap();
    assert_eq!(d.session.notes, vec!["changed".to_string()]);
}

#[test]
fn moving_note_id_to_another_week_leaves_no_leftovers() {
    let db = temp_db();
    let data = load_grouped(&fixture_path()).unwrap();
    import_grouped(&db, &data, &ImportOptions::default()).unwrap();

    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(fixture_path()).unwrap()).unwrap();
    // Move note://test/w1 from week 1 to week 3 (new week).
    value["cycles"][0]["week"] = serde_json::json!(3);
    value["cycles"][0]["title"] = serde_json::json!("Test Week 3");
    let modified: GroupedFile = serde_json::from_value(value).unwrap();
    import_grouped(&db, &modified, &ImportOptions::default()).unwrap();

    let cycle = db.get_cycle_by_note_id("note://test/w1").unwrap().unwrap();
    assert_eq!(cycle.week, 3);

    // Old week 1 cycle should be gone (was notes_import-only).
    let block = db.get_block_by_slug("test_block").unwrap().unwrap();
    assert!(db.get_cycle_by_block_week(block.id, 1).unwrap().is_none());

    // No orphan notes_import sessions for week 1.
    let sessions = db.list_sessions().unwrap();
    assert!(!sessions.iter().any(|s| {
        s.block_slug == "test_block" && s.week == 1 && s.session.source == Source::NotesImport
    }));
}

#[test]
fn recreated_note_replaces_old_note_for_same_week() {
    let db = temp_db();
    let data = load_grouped(&fixture_path()).unwrap();
    import_grouped(&db, &data, &ImportOptions::default()).unwrap();
    let before = db.get_cycle_by_note_id("note://test/w1").unwrap().unwrap();
    let sessions_before = db.count_sessions_for_cycle(before.id).unwrap();

    // Same week, new note id (note duplicated or re-created in Apple Notes).
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(fixture_path()).unwrap()).unwrap();
    value["cycles"][0]["note_id"] = serde_json::json!("note://test/w1-recreated");
    let modified: GroupedFile = serde_json::from_value(value).unwrap();
    let report = import_grouped(&db, &modified, &ImportOptions::default()).unwrap();

    assert_eq!(
        report.notes_replaced,
        vec![("note://test/w1".to_string(), "note://test/w1-recreated".to_string())]
    );
    assert!(report.stale_notes.is_empty(), "{:?}", report.stale_notes);
    assert!(db.get_cycle_by_note_id("note://test/w1").unwrap().is_none());
    let after = db
        .get_cycle_by_note_id("note://test/w1-recreated")
        .unwrap()
        .unwrap();
    assert_eq!(after.id, before.id);
    assert_eq!(db.count_sessions_for_cycle(after.id).unwrap(), sessions_before);
}

#[test]
fn two_notes_for_one_week_in_the_file_is_an_error() {
    let db = temp_db();
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(fixture_path()).unwrap()).unwrap();
    let mut dup = value["cycles"][0].clone();
    dup["note_id"] = serde_json::json!("note://test/w1-dup");
    value["cycles"].as_array_mut().unwrap().push(dup);
    let data: GroupedFile = serde_json::from_value(value).unwrap();
    let err = import_grouped(&db, &data, &ImportOptions::default()).unwrap_err();
    assert!(err.contains("two notes"), "{err}");
}

#[test]
fn dry_run_writes_nothing() {
    let db = temp_db();
    let data = load_grouped(&fixture_path()).unwrap();
    let report = import_grouped(
        &db,
        &data,
        &ImportOptions {
            dry_run: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(report.cycles_new, 3);
    assert!(db.list_sessions().unwrap().is_empty());
    assert!(db.list_variants().unwrap().is_empty());
}

#[test]
fn outlier_detection_flags_far_weights() {
    let db = temp_db();
    let data = load_grouped(&fixture_path()).unwrap();
    import_grouped(&db, &data, &ImportOptions::default()).unwrap();
    // Fixture weights: 50, 55, 40 — median around 50; none >40% off.
    let outliers = find_outliers(&db, 0.40).unwrap();
    assert!(outliers.is_empty(), "{outliers:?}");

    // Add an extreme set via app logging.
    let v = db.get_variant_by_slug("cable_curl").unwrap().unwrap();
    let block = db.get_block_by_slug("test_block").unwrap().unwrap();
    let cycle = db.get_cycle_by_block_week(block.id, 2).unwrap().unwrap();
    let sid = db
        .create_session(&NewSession {
            cycle_id: cycle.id,
            day_number: Some(9),
            day_label: Some("X".into()),
            comment: None,
            header_raw: None,
            implicit: false,
            index: 50,
            notes: vec![],
            performed_on: Some("2026-01-09".into()),
            date_precision: DatePrecision::Exact,
            source: Source::App,
        })
        .unwrap();
    db.add_entry_with_sets(
        &NewEntry {
            session_id: sid,
            variant_id: Some(v.id),
            position: 1,
            tracked: true,
            name: Some("Cable Curl".into()),
            name_raw: "Cable Curl".into(),
            qualifiers: vec![],
            untracked_sets: None,
            settings: None,
            comment: None,
            raw: None,
            excluded: false,
        },
        &[NewSet {
            set_number: 1,
            weight: Some(200.0),
            weight_inferred: false,
            weight_note: None,
            reps: Some(1.0),
            rir_low: None,
            rir_high: None,
            logged: true,
            comment: None,
            raw: None,
        }],
    )
    .unwrap();
    let outliers = find_outliers(&db, 0.40).unwrap();
    assert!(outliers.iter().any(|o| o.weight == 200.0), "{outliers:?}");
}

#[test]
fn cycle_hash_changes_when_content_changes() {
    let data = load_grouped(&fixture_path()).unwrap();
    let h1 = cycle_source_hash(&data.cycles[0]).unwrap();
    let mut c = data.cycles[0].clone();
    c.title = Some("changed".into());
    let h2 = cycle_source_hash(&c).unwrap();
    assert_ne!(h1, h2);
}

#[test]
fn format_report_mentions_counts() {
    let db = temp_db();
    let data = load_grouped(&fixture_path()).unwrap();
    let report = import_grouped(&db, &data, &ImportOptions::default()).unwrap();
    let text = format_report(&report, true);
    assert!(text.contains("3 new"));
    assert!(text.contains("inferred"));
    assert!(text.contains("accepted"));
}
