//! Full-history import test against exports/grouped.json.
//!
//! Skips when the private export is missing (repo is public; exports/ is gitignored).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hypertrophy_archive::db::Db;
use hypertrophy_archive::import::{import_grouped, load_grouped, ImportOptions};

fn grouped_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("exports/grouped.json")
}

fn temp_db() -> Db {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ha_import_private_{nanos}.db"));
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
fn full_history_import_matches_grouped_counts() {
    let path = grouped_path();
    if !path.exists() {
        eprintln!("skipping: {} missing", path.display());
        return;
    }
    let data = load_grouped(&path).expect("parse grouped.json");
    let expected_cycles = data.cycles.len();
    let expected_sessions: usize = data.cycles.iter().map(|c| c.sessions.len()).sum();
    let expected_entries: usize = data
        .cycles
        .iter()
        .flat_map(|c| c.sessions.iter())
        .map(|s| s.entries.len())
        .sum();
    let expected_sets: usize = data
        .cycles
        .iter()
        .flat_map(|c| c.sessions.iter())
        .flat_map(|s| s.entries.iter())
        .map(|e| e.sets.len())
        .sum();

    let db = temp_db();
    let report = import_grouped(&db, &data, &ImportOptions::default()).expect("import");
    assert_eq!(report.cycles_new, expected_cycles);
    assert_eq!(report.sessions_written, expected_sessions);
    assert_eq!(report.entries_written, expected_entries);
    assert_eq!(report.sets_written, expected_sets);
    assert_eq!(report.review_open, 0, "all review items should be resolved");
    assert!(
        report.unmapped.is_empty(),
        "unmapped: {:?}",
        report.unmapped
    );
    assert!(
        report.unconfirmed_variants.is_empty(),
        "unconfirmed: {:?}",
        report.unconfirmed_variants
    );

    assert_eq!(expected_cycles, 59);
    assert_eq!(expected_sessions, 182);
    assert_eq!(expected_entries, 874);

    let dump1 = dump_tables(&db);
    let report2 = import_grouped(&db, &data, &ImportOptions::default()).expect("reimport");
    assert_eq!(report2.cycles_unchanged, expected_cycles);
    assert_eq!(report2.cycles_new, 0);
    assert_eq!(report2.cycles_replaced, 0);
    assert_eq!(dump_tables(&db), dump1);
}
