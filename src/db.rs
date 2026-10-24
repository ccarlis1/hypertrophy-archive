//! SQLite persistence for the workout log.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};

use crate::log::{
    Alias, AliasStatus, Block, Cycle, DatePrecision, Entry, EntryDetail, GroupedSeed, LastPerformance,
    NewBlock, NewCycle, NewEntry, NewSession, NewSet, NewVariant, Session, SessionDetail,
    SessionSummary, SetLog, Source, Split, Variant,
};

const MIGRATION_001: &str = include_str!("../migrations/001_init.sql");
const MIGRATION_002: &str = include_str!("../migrations/002_import.sql");

/// Fields written onto a cycle during Notes import.
#[derive(Debug, Clone)]
pub struct CycleImportMeta<'a> {
    pub title: Option<&'a str>,
    pub note_id: Option<&'a str>,
    pub started_on: Option<&'a str>,
    pub date_precision: DatePrecision,
    pub override_flag: bool,
    pub source: Source,
    pub source_hash: Option<&'a str>,
}

#[derive(Debug)]
pub struct Db {
    conn: Connection,
    path: PathBuf,
}

impl Db {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn open_default() -> Result<Self, String> {
        open(&default_db_path()?)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        open(path.as_ref())
    }
}

/// Resolve the data directory: `HA_DATA_DIR` or platform Application Support.
pub fn data_dir() -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("HA_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let base = directories::ProjectDirs::from("com", "hypertrophy-archive", "hypertrophy-archive")
        .ok_or_else(|| "could not resolve Application Support directory".to_string())?;
    Ok(base.data_dir().to_path_buf())
}

pub fn default_db_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("log.db"))
}

pub fn open(path: &Path) -> Result<Db, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create data dir: {e}"))?;
    }
    let conn = Connection::open(path).map_err(|e| format!("open db {}: {e}", path.display()))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| format!("pragma foreign_keys: {e}"))?;
    migrate(&conn)?;
    Ok(Db {
        conn,
        path: path.to_path_buf(),
    })
}

fn migrate(conn: &Connection) -> Result<(), String> {
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| format!("read user_version: {e}"))?;
    if version < 1 {
        conn.execute_batch(MIGRATION_001)
            .map_err(|e| format!("apply migration 001: {e}"))?;
        conn.execute_batch("PRAGMA user_version = 1;")
            .map_err(|e| format!("set user_version: {e}"))?;
    }
    if version < 2 {
        conn.execute_batch(MIGRATION_002)
            .map_err(|e| format!("apply migration 002: {e}"))?;
        conn.execute_batch("PRAGMA user_version = 2;")
            .map_err(|e| format!("set user_version: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
fn user_version(conn: &Connection) -> Result<i32, String> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| format!("read user_version: {e}"))
}

fn json_list(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}

fn parse_json_list(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn bool_i(v: bool) -> i32 {
    if v {
        1
    } else {
        0
    }
}

fn i_bool(v: i32) -> bool {
    v != 0
}

impl Db {
    pub fn create_block(&self, b: &NewBlock) -> Result<i64, String> {
        self.conn
            .execute(
                "INSERT INTO block (slug, name, split, label, started_on) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    b.slug,
                    b.name,
                    b.split.as_str(),
                    b.label,
                    b.started_on
                ],
            )
            .map_err(|e| format!("create block: {e}"))?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Insert or update a block by slug. Returns the block id.
    pub fn upsert_block(&self, b: &NewBlock) -> Result<i64, String> {
        if let Some(existing) = self.get_block_by_slug(&b.slug)? {
            self.conn
                .execute(
                    "UPDATE block SET name = ?1, split = ?2, label = ?3, started_on = ?4 WHERE id = ?5",
                    params![b.name, b.split.as_str(), b.label, b.started_on, existing.id],
                )
                .map_err(|e| format!("update block: {e}"))?;
            return Ok(existing.id);
        }
        self.create_block(b)
    }

    pub fn get_block(&self, id: i64) -> Result<Option<Block>, String> {
        self.conn
            .query_row(
                "SELECT id, slug, name, split, label, started_on FROM block WHERE id = ?1",
                params![id],
                |row| {
                    let split_s: String = row.get(3)?;
                    Ok(Block {
                        id: row.get(0)?,
                        slug: row.get(1)?,
                        name: row.get(2)?,
                        split: Split::parse(&split_s).unwrap_or(Split::Other),
                        label: row.get(4)?,
                        started_on: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(|e| format!("get block: {e}"))
    }

    pub fn get_block_by_slug(&self, slug: &str) -> Result<Option<Block>, String> {
        self.conn
            .query_row(
                "SELECT id, slug, name, split, label, started_on FROM block WHERE slug = ?1",
                params![slug],
                |row| {
                    let split_s: String = row.get(3)?;
                    Ok(Block {
                        id: row.get(0)?,
                        slug: row.get(1)?,
                        name: row.get(2)?,
                        split: Split::parse(&split_s).unwrap_or(Split::Other),
                        label: row.get(4)?,
                        started_on: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(|e| format!("get block by slug: {e}"))
    }

    pub fn list_blocks(&self) -> Result<Vec<Block>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, slug, name, split, label, started_on FROM block ORDER BY name")
            .map_err(|e| format!("list blocks: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                let split_s: String = row.get(3)?;
                Ok(Block {
                    id: row.get(0)?,
                    slug: row.get(1)?,
                    name: row.get(2)?,
                    split: Split::parse(&split_s).unwrap_or(Split::Other),
                    label: row.get(4)?,
                    started_on: row.get(5)?,
                })
            })
            .map_err(|e| format!("list blocks: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("list blocks: {e}"))
    }

    pub fn create_cycle(&self, c: &NewCycle) -> Result<i64, String> {
        self.conn
            .execute(
                "INSERT INTO cycle (block_id, week, title, note_id, started_on, date_precision, override, source, source_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    c.block_id,
                    c.week,
                    c.title,
                    c.note_id,
                    c.started_on,
                    c.date_precision.as_str(),
                    bool_i(c.override_flag),
                    c.source.as_str(),
                    c.source_hash
                ],
            )
            .map_err(|e| format!("create cycle: {e}"))?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_or_create_cycle(
        &self,
        block_id: i64,
        week: i32,
        title: Option<&str>,
        source: Source,
    ) -> Result<Cycle, String> {
        if let Some(existing) = self.get_cycle_by_block_week(block_id, week)? {
            return Ok(existing);
        }
        let id = self.create_cycle(&NewCycle {
            block_id,
            week,
            title: title.map(|s| s.to_string()),
            note_id: None,
            started_on: None,
            date_precision: DatePrecision::Unknown,
            override_flag: false,
            source,
            source_hash: None,
        })?;
        self.get_cycle(id)?
            .ok_or_else(|| "cycle missing after insert".to_string())
    }

    pub fn get_cycle(&self, id: i64) -> Result<Option<Cycle>, String> {
        self.conn
            .query_row(
                "SELECT id, block_id, week, title, note_id, started_on, date_precision, override, source, source_hash
                 FROM cycle WHERE id = ?1",
                params![id],
                map_cycle,
            )
            .optional()
            .map_err(|e| format!("get cycle: {e}"))
    }

    pub fn get_cycle_by_block_week(
        &self,
        block_id: i64,
        week: i32,
    ) -> Result<Option<Cycle>, String> {
        self.conn
            .query_row(
                "SELECT id, block_id, week, title, note_id, started_on, date_precision, override, source, source_hash
                 FROM cycle WHERE block_id = ?1 AND week = ?2",
                params![block_id, week],
                map_cycle,
            )
            .optional()
            .map_err(|e| format!("get cycle by week: {e}"))
    }

    pub fn list_cycles(&self, block_id: i64) -> Result<Vec<Cycle>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, block_id, week, title, note_id, started_on, date_precision, override, source, source_hash
                 FROM cycle WHERE block_id = ?1 ORDER BY week",
            )
            .map_err(|e| format!("list cycles: {e}"))?;
        let rows = stmt
            .query_map(params![block_id], map_cycle)
            .map_err(|e| format!("list cycles: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("list cycles: {e}"))
    }

    pub fn get_cycle_by_note_id(&self, note_id: &str) -> Result<Option<Cycle>, String> {
        self.conn
            .query_row(
                "SELECT id, block_id, week, title, note_id, started_on, date_precision, override, source, source_hash
                 FROM cycle WHERE note_id = ?1",
                params![note_id],
                map_cycle,
            )
            .optional()
            .map_err(|e| format!("get cycle by note_id: {e}"))
    }

    pub fn update_cycle_import_meta(&self, cycle_id: i64, meta: &CycleImportMeta<'_>) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE cycle SET title = ?1, note_id = ?2, started_on = ?3, date_precision = ?4,
                 override = ?5, source = ?6, source_hash = ?7 WHERE id = ?8",
                params![
                    meta.title,
                    meta.note_id,
                    meta.started_on,
                    meta.date_precision.as_str(),
                    bool_i(meta.override_flag),
                    meta.source.as_str(),
                    meta.source_hash,
                    cycle_id
                ],
            )
            .map_err(|e| format!("update cycle import meta: {e}"))?;
        Ok(())
    }

    /// Delete sessions with `source='notes_import'` for a cycle (entries/sets cascade).
    pub fn delete_notes_import_sessions(&self, cycle_id: i64) -> Result<usize, String> {
        let n = self
            .conn
            .execute(
                "DELETE FROM session WHERE cycle_id = ?1 AND source = 'notes_import'",
                params![cycle_id],
            )
            .map_err(|e| format!("delete notes_import sessions: {e}"))?;
        Ok(n)
    }

    pub fn count_sessions_for_cycle(&self, cycle_id: i64) -> Result<i64, String> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM session WHERE cycle_id = ?1",
                params![cycle_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("count sessions: {e}"))
    }

    pub fn delete_cycle(&self, cycle_id: i64) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM cycle WHERE id = ?1", params![cycle_id])
            .map_err(|e| format!("delete cycle: {e}"))?;
        Ok(())
    }

    /// List note_ids currently stored on cycles (for stale-note reporting).
    pub fn list_imported_note_ids(&self) -> Result<Vec<(String, Option<String>)>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT note_id, source_hash FROM cycle WHERE note_id IS NOT NULL ORDER BY note_id",
            )
            .map_err(|e| format!("list imported note_ids: {e}"))?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| format!("list imported note_ids: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("list imported note_ids: {e}"))
    }

    /// Insert an alias only if the normalized name is not already taken.
    pub fn insert_alias_if_absent(
        &self,
        normalized: &str,
        variant_id: i64,
        status: AliasStatus,
    ) -> Result<bool, String> {
        let n = self
            .conn
            .execute(
                "INSERT OR IGNORE INTO alias (normalized, variant_id, status) VALUES (?1, ?2, ?3)",
                params![normalized, variant_id, status.as_str()],
            )
            .map_err(|e| format!("insert alias if absent: {e}"))?;
        Ok(n > 0)
    }

    /// Create a variant only if the slug is missing. Returns (id, created).
    pub fn ensure_variant(&self, v: &NewVariant) -> Result<(i64, bool), String> {
        if let Some(existing) = self.get_variant_by_slug(&v.slug)? {
            return Ok((existing.id, false));
        }
        Ok((self.create_variant(v)?, true))
    }

    /// Exposed for import transactions (dry-run rollback / commit).
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn create_session(&self, s: &NewSession) -> Result<i64, String> {
        self.conn
            .execute(
                "INSERT INTO session (cycle_id, day_number, day_label, comment, header_raw, implicit, \"index\", notes, performed_on, date_precision, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    s.cycle_id,
                    s.day_number,
                    s.day_label,
                    s.comment,
                    s.header_raw,
                    bool_i(s.implicit),
                    s.index,
                    json_list(&s.notes),
                    s.performed_on,
                    s.date_precision.as_str(),
                    s.source.as_str()
                ],
            )
            .map_err(|e| format!("create session: {e}"))?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_session(&self, id: i64) -> Result<Option<Session>, String> {
        self.conn
            .query_row(
                "SELECT id, cycle_id, day_number, day_label, comment, header_raw, implicit, \"index\", notes, performed_on, date_precision, source
                 FROM session WHERE id = ?1",
                params![id],
                map_session,
            )
            .optional()
            .map_err(|e| format!("get session: {e}"))
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionSummary>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT s.id, s.cycle_id, s.day_number, s.day_label, s.comment, s.header_raw, s.implicit,
                        s.\"index\", s.notes, s.performed_on, s.date_precision, s.source,
                        b.slug, b.name, c.week, c.title,
                        (SELECT COUNT(*) FROM entry e WHERE e.session_id = s.id) AS entry_count
                 FROM session s
                 JOIN cycle c ON c.id = s.cycle_id
                 JOIN block b ON b.id = c.block_id
                 ORDER BY b.name, c.week, s.\"index\", s.id",
            )
            .map_err(|e| format!("list sessions: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                let session = map_session_row(row)?;
                Ok(SessionSummary {
                    session,
                    block_slug: row.get(12)?,
                    block_name: row.get(13)?,
                    week: row.get(14)?,
                    cycle_title: row.get(15)?,
                    entry_count: row.get(16)?,
                })
            })
            .map_err(|e| format!("list sessions: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("list sessions: {e}"))
    }

    pub fn add_entry_with_sets(
        &self,
        entry: &NewEntry,
        sets: &[NewSet],
    ) -> Result<i64, String> {
        self.conn
            .execute(
                "INSERT INTO entry (session_id, variant_id, position, tracked, name, name_raw, qualifiers, untracked_sets, settings, comment, raw, excluded)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    entry.session_id,
                    entry.variant_id,
                    entry.position,
                    bool_i(entry.tracked),
                    entry.name,
                    entry.name_raw,
                    json_list(&entry.qualifiers),
                    entry.untracked_sets,
                    entry.settings,
                    entry.comment,
                    entry.raw,
                    bool_i(entry.excluded)
                ],
            )
            .map_err(|e| format!("add entry: {e}"))?;
        let entry_id = self.conn.last_insert_rowid();
        for set in sets {
            self.conn
                .execute(
                    "INSERT INTO set_log (entry_id, set_number, weight, weight_inferred, weight_note, reps, rir_low, rir_high, logged, comment, raw)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        entry_id,
                        set.set_number,
                        set.weight,
                        bool_i(set.weight_inferred),
                        set.weight_note,
                        set.reps,
                        set.rir_low,
                        set.rir_high,
                        bool_i(set.logged),
                        set.comment,
                        set.raw
                    ],
                )
                .map_err(|e| format!("add set: {e}"))?;
        }
        Ok(entry_id)
    }

    pub fn clear_session_entries(&self, session_id: i64) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM entry WHERE session_id = ?1", params![session_id])
            .map_err(|e| format!("clear session entries: {e}"))?;
        Ok(())
    }

    pub fn get_session_detail(&self, session_id: i64) -> Result<Option<SessionDetail>, String> {
        let Some(session) = self.get_session(session_id)? else {
            return Ok(None);
        };
        let cycle = self
            .get_cycle(session.cycle_id)?
            .ok_or_else(|| "session cycle missing".to_string())?;
        let block = self
            .get_block(cycle.block_id)?
            .ok_or_else(|| "session block missing".to_string())?;

        let mut stmt = self
            .conn
            .prepare(
                "SELECT e.id, e.session_id, e.variant_id, e.position, e.tracked, e.name, e.name_raw,
                        e.qualifiers, e.untracked_sets, e.settings, e.comment, e.raw, e.excluded,
                        v.slug, v.display
                 FROM entry e
                 LEFT JOIN variant v ON v.id = e.variant_id
                 WHERE e.session_id = ?1
                 ORDER BY e.position, e.id",
            )
            .map_err(|e| format!("session entries: {e}"))?;
        let entry_rows = stmt
            .query_map(params![session_id], |row| {
                let entry = Entry {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    variant_id: row.get(2)?,
                    position: row.get(3)?,
                    tracked: i_bool(row.get(4)?),
                    name: row.get(5)?,
                    name_raw: row.get(6)?,
                    qualifiers: parse_json_list(&row.get::<_, String>(7)?),
                    untracked_sets: row.get(8)?,
                    settings: row.get(9)?,
                    comment: row.get(10)?,
                    raw: row.get(11)?,
                    excluded: i_bool(row.get(12)?),
                };
                let variant_slug: Option<String> = row.get(13)?;
                let variant_display: Option<String> = row.get(14)?;
                Ok((entry, variant_slug, variant_display))
            })
            .map_err(|e| format!("session entries: {e}"))?;

        let mut entries = Vec::new();
        for row in entry_rows {
            let (entry, variant_slug, variant_display) =
                row.map_err(|e| format!("session entries: {e}"))?;
            let sets = self.list_sets(entry.id)?;
            entries.push(EntryDetail {
                entry,
                variant_slug,
                variant_display,
                sets,
            });
        }

        Ok(Some(SessionDetail {
            session,
            block_slug: block.slug,
            block_name: block.name,
            week: cycle.week,
            cycle_title: cycle.title,
            entries,
        }))
    }

    pub fn list_sets(&self, entry_id: i64) -> Result<Vec<SetLog>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, entry_id, set_number, weight, weight_inferred, weight_note, reps, rir_low, rir_high, logged, comment, raw
                 FROM set_log WHERE entry_id = ?1 ORDER BY set_number, id",
            )
            .map_err(|e| format!("list sets: {e}"))?;
        let rows = stmt
            .query_map(params![entry_id], map_set)
            .map_err(|e| format!("list sets: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("list sets: {e}"))
    }

    pub fn last_performance(&self, variant_id: i64) -> Result<Option<LastPerformance>, String> {
        let row = self
            .conn
            .query_row(
                "SELECT e.id, e.session_id, e.settings, s.day_label, s.performed_on
                 FROM entry e
                 JOIN session s ON s.id = e.session_id
                 WHERE e.variant_id = ?1 AND e.tracked = 1 AND e.excluded = 0
                 ORDER BY COALESCE(s.performed_on, '') DESC, s.id DESC, e.id DESC
                 LIMIT 1",
                params![variant_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| format!("last performance: {e}"))?;

        let Some((entry_id, session_id, settings, day_label, performed_on)) = row else {
            return Ok(None);
        };
        let sets = self.list_sets(entry_id)?;
        Ok(Some(LastPerformance {
            session_id,
            entry_id,
            day_label,
            performed_on,
            settings,
            sets,
        }))
    }

    pub fn create_variant(&self, v: &NewVariant) -> Result<i64, String> {
        self.conn
            .execute(
                "INSERT INTO variant (slug, display, family, gym, status, joint_action, equipment, load_unit, default_settings)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    v.slug,
                    v.display,
                    v.family,
                    v.gym,
                    v.status.as_str(),
                    v.joint_action,
                    v.equipment,
                    v.load_unit,
                    v.default_settings
                ],
            )
            .map_err(|e| format!("create variant: {e}"))?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_variant(&self, v: &Variant) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE variant SET slug = ?1, display = ?2, family = ?3, gym = ?4, status = ?5,
                 joint_action = ?6, equipment = ?7, load_unit = ?8, default_settings = ?9
                 WHERE id = ?10",
                params![
                    v.slug,
                    v.display,
                    v.family,
                    v.gym,
                    v.status.as_str(),
                    v.joint_action,
                    v.equipment,
                    v.load_unit,
                    v.default_settings,
                    v.id
                ],
            )
            .map_err(|e| format!("update variant: {e}"))?;
        Ok(())
    }

    pub fn link_variant(&self, slug: &str, joint_action: &str) -> Result<(), String> {
        let n = self
            .conn
            .execute(
                "UPDATE variant SET joint_action = ?1 WHERE slug = ?2",
                params![joint_action, slug],
            )
            .map_err(|e| format!("link variant: {e}"))?;
        if n == 0 {
            return Err(format!("variant not found: {slug}"));
        }
        Ok(())
    }

    pub fn get_variant(&self, id: i64) -> Result<Option<Variant>, String> {
        self.conn
            .query_row(
                "SELECT id, slug, display, family, gym, status, joint_action, equipment, load_unit, default_settings
                 FROM variant WHERE id = ?1",
                params![id],
                map_variant,
            )
            .optional()
            .map_err(|e| format!("get variant: {e}"))
    }

    pub fn get_variant_by_slug(&self, slug: &str) -> Result<Option<Variant>, String> {
        self.conn
            .query_row(
                "SELECT id, slug, display, family, gym, status, joint_action, equipment, load_unit, default_settings
                 FROM variant WHERE slug = ?1",
                params![slug],
                map_variant,
            )
            .optional()
            .map_err(|e| format!("get variant by slug: {e}"))
    }

    pub fn list_variants(&self) -> Result<Vec<Variant>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, slug, display, family, gym, status, joint_action, equipment, load_unit, default_settings
                 FROM variant ORDER BY display",
            )
            .map_err(|e| format!("list variants: {e}"))?;
        let rows = stmt
            .query_map([], map_variant)
            .map_err(|e| format!("list variants: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("list variants: {e}"))
    }

    pub fn upsert_alias(
        &self,
        normalized: &str,
        variant_id: i64,
        status: AliasStatus,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO alias (normalized, variant_id, status) VALUES (?1, ?2, ?3)
                 ON CONFLICT(normalized) DO UPDATE SET variant_id = excluded.variant_id, status = excluded.status",
                params![normalized, variant_id, status.as_str()],
            )
            .map_err(|e| format!("upsert alias: {e}"))?;
        Ok(())
    }

    pub fn list_aliases(&self) -> Result<Vec<Alias>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT normalized, variant_id, status FROM alias ORDER BY normalized")
            .map_err(|e| format!("list aliases: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                let status_s: String = row.get(2)?;
                Ok(Alias {
                    normalized: row.get(0)?,
                    variant_id: row.get(1)?,
                    status: AliasStatus::parse(&status_s).unwrap_or(AliasStatus::Suggested),
                })
            })
            .map_err(|e| format!("list aliases: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("list aliases: {e}"))
    }

    /// Search variants by display, slug, or alias substring (case-insensitive).
    pub fn search_variants(&self, query: &str) -> Result<Vec<Variant>, String> {
        let q = format!("%{}%", query.to_lowercase());
        let mut stmt = self
            .conn
            .prepare(
                "SELECT DISTINCT v.id, v.slug, v.display, v.family, v.gym, v.status, v.joint_action, v.equipment, v.load_unit, v.default_settings
                 FROM variant v
                 LEFT JOIN alias a ON a.variant_id = v.id
                 WHERE lower(v.display) LIKE ?1
                    OR lower(v.slug) LIKE ?1
                    OR lower(a.normalized) LIKE ?1
                 ORDER BY v.display
                 LIMIT 40",
            )
            .map_err(|e| format!("search variants: {e}"))?;
        let rows = stmt
            .query_map(params![q], map_variant)
            .map_err(|e| format!("search variants: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("search variants: {e}"))
    }

    pub fn seed_variants_from_grouped(&self, path: &Path) -> Result<usize, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("read seed file: {e}"))?;
        let seed: GroupedSeed =
            serde_json::from_str(&text).map_err(|e| format!("parse seed file: {e}"))?;
        self.seed_variants(&seed)
    }

    pub fn seed_variants(&self, seed: &GroupedSeed) -> Result<usize, String> {
        let mut count = 0;
        for v in &seed.variants {
            let status = AliasStatus::parse(&v.status).unwrap_or(AliasStatus::Suggested);
            let id = if let Some(existing) = self.get_variant_by_slug(&v.slug)? {
                let mut updated = existing;
                updated.display = v.display.clone();
                updated.family = v.family.clone();
                updated.gym = v.gym.clone();
                updated.status = status;
                self.update_variant(&updated)?;
                updated.id
            } else {
                self.create_variant(&NewVariant {
                    slug: v.slug.clone(),
                    display: v.display.clone(),
                    family: v.family.clone(),
                    gym: v.gym.clone(),
                    status,
                    joint_action: None,
                    equipment: None,
                    load_unit: "lb".to_string(),
                    default_settings: None,
                })?
            };
            for name in &v.names {
                let normalized = name.trim().to_lowercase();
                if normalized.is_empty() {
                    continue;
                }
                self.upsert_alias(&normalized, id, status)?;
            }
            count += 1;
        }
        Ok(count)
    }
}

fn map_cycle(row: &rusqlite::Row<'_>) -> rusqlite::Result<Cycle> {
    let precision_s: String = row.get(6)?;
    let source_s: String = row.get(8)?;
    Ok(Cycle {
        id: row.get(0)?,
        block_id: row.get(1)?,
        week: row.get(2)?,
        title: row.get(3)?,
        note_id: row.get(4)?,
        started_on: row.get(5)?,
        date_precision: DatePrecision::parse(&precision_s).unwrap_or(DatePrecision::Unknown),
        override_flag: i_bool(row.get(7)?),
        source: Source::parse(&source_s).unwrap_or(Source::App),
        source_hash: row.get(9)?,
    })
}

fn map_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
    map_session_row(row)
}

fn map_session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
    let notes_s: String = row.get(8)?;
    let precision_s: String = row.get(10)?;
    let source_s: String = row.get(11)?;
    Ok(Session {
        id: row.get(0)?,
        cycle_id: row.get(1)?,
        day_number: row.get(2)?,
        day_label: row.get(3)?,
        comment: row.get(4)?,
        header_raw: row.get(5)?,
        implicit: i_bool(row.get(6)?),
        index: row.get(7)?,
        notes: parse_json_list(&notes_s),
        performed_on: row.get(9)?,
        date_precision: DatePrecision::parse(&precision_s).unwrap_or(DatePrecision::Unknown),
        source: Source::parse(&source_s).unwrap_or(Source::App),
    })
}

fn map_variant(row: &rusqlite::Row<'_>) -> rusqlite::Result<Variant> {
    let status_s: String = row.get(5)?;
    Ok(Variant {
        id: row.get(0)?,
        slug: row.get(1)?,
        display: row.get(2)?,
        family: row.get(3)?,
        gym: row.get(4)?,
        status: AliasStatus::parse(&status_s).unwrap_or(AliasStatus::Suggested),
        joint_action: row.get(6)?,
        equipment: row.get(7)?,
        load_unit: row.get(8)?,
        default_settings: row.get(9)?,
    })
}

fn map_set(row: &rusqlite::Row<'_>) -> rusqlite::Result<SetLog> {
    Ok(SetLog {
        id: row.get(0)?,
        entry_id: row.get(1)?,
        set_number: row.get(2)?,
        weight: row.get(3)?,
        weight_inferred: i_bool(row.get(4)?),
        weight_note: row.get(5)?,
        reps: row.get(6)?,
        rir_low: row.get(7)?,
        rir_high: row.get(8)?,
        logged: i_bool(row.get(9)?),
        comment: row.get(10)?,
        raw: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::GroupedVariant;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db() -> Db {
        // Tests run in parallel and the clock can repeat, so add a counter.
        static N: AtomicUsize = AtomicUsize::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let n = N.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("ha_test_{nanos}_{n}.db"));
        open(&path).expect("open temp db")
    }

    #[test]
    fn migrate_and_round_trip_example_week_shapes() {
        let db = temp_db();
        let block_id = db
            .create_block(&NewBlock {
                slug: "log_book".into(),
                name: "Log Book".into(),
                split: Split::UpperLower,
                label: Some("home".into()),
                started_on: None,
            })
            .unwrap();
        let cycle_id = db
            .create_cycle(&NewCycle {
                block_id,
                week: 1,
                title: Some("Week 1".into()),
                note_id: None,
                started_on: None,
                date_precision: DatePrecision::Week,
                override_flag: false,
                source: Source::App,
                source_hash: None,
            })
            .unwrap();
        let session_id = db
            .create_session(&NewSession {
                cycle_id,
                day_number: Some(1),
                day_label: Some("Upper 1".into()),
                comment: None,
                header_raw: Some("Upper 1".into()),
                implicit: false,
                index: 0,
                notes: vec!["session note".into()],
                performed_on: Some("2026-10-20".into()),
                date_precision: DatePrecision::Exact,
                source: Source::App,
            })
            .unwrap();

        let variant_id = db
            .create_variant(&NewVariant {
                slug: "flat_smith_press".into(),
                display: "Flat Smith Press".into(),
                family: Some("press".into()),
                gym: Some("home".into()),
                status: AliasStatus::Confirmed,
                joint_action: None,
                equipment: None,
                load_unit: "lb".into(),
                default_settings: None,
            })
            .unwrap();
        db.upsert_alias("flat smith press", variant_id, AliasStatus::Confirmed)
            .unwrap();

        // Tracked entry with RIR range + blank second set (like LF Preacher Curl)
        db.add_entry_with_sets(
            &NewEntry {
                session_id,
                variant_id: Some(variant_id),
                position: 1,
                tracked: true,
                name: Some("Flat Smith Press".into()),
                name_raw: "Flat Smith Press".into(),
                qualifiers: Vec::new(),
                untracked_sets: None,
                settings: None,
                comment: None,
                raw: None,
                excluded: false,
            },
            &[
                NewSet {
                    set_number: 1,
                    weight: Some(185.0),
                    weight_inferred: false,
                    weight_note: None,
                    reps: Some(5.0),
                    rir_low: Some(1.0),
                    rir_high: Some(1.0),
                    logged: true,
                    comment: None,
                    raw: None,
                },
                NewSet {
                    set_number: 2,
                    weight: Some(165.0),
                    weight_inferred: false,
                    weight_note: None,
                    reps: Some(6.0),
                    rir_low: Some(1.0),
                    rir_high: Some(2.0),
                    logged: true,
                    comment: None,
                    raw: None,
                },
            ],
        )
        .unwrap();

        let preacher = db
            .create_variant(&NewVariant {
                slug: "lf_preacher_curl".into(),
                display: "LF Preacher Curl".into(),
                family: None,
                gym: None,
                status: AliasStatus::Confirmed,
                joint_action: None,
                equipment: None,
                load_unit: "lb".into(),
                default_settings: None,
            })
            .unwrap();
        db.add_entry_with_sets(
            &NewEntry {
                session_id,
                variant_id: Some(preacher),
                position: 2,
                tracked: true,
                name: Some("LF Preacher Curl".into()),
                name_raw: "LF Preacher Curl".into(),
                qualifiers: Vec::new(),
                untracked_sets: None,
                settings: None,
                comment: None,
                raw: None,
                excluded: false,
            },
            &[
                NewSet {
                    set_number: 1,
                    weight: Some(105.0),
                    weight_inferred: false,
                    weight_note: None,
                    reps: Some(5.0),
                    rir_low: Some(1.0),
                    rir_high: Some(1.0),
                    logged: true,
                    comment: None,
                    raw: None,
                },
                NewSet {
                    set_number: 2,
                    weight: None,
                    weight_inferred: false,
                    weight_note: None,
                    reps: None,
                    rir_low: None,
                    rir_high: None,
                    logged: false,
                    comment: None,
                    raw: None,
                },
            ],
        )
        .unwrap();

        // Untracked: "2 sets abduction warmup"
        db.add_entry_with_sets(
            &NewEntry {
                session_id,
                variant_id: None,
                position: 3,
                tracked: false,
                name: Some("abduction warmup".into()),
                name_raw: "abduction warmup".into(),
                qualifiers: Vec::new(),
                untracked_sets: Some(2),
                settings: None,
                comment: None,
                raw: None,
                excluded: false,
            },
            &[],
        )
        .unwrap();

        let detail = db.get_session_detail(session_id).unwrap().unwrap();
        assert_eq!(detail.entries.len(), 3);
        assert_eq!(detail.entries[0].sets[1].rir_low, Some(1.0));
        assert_eq!(detail.entries[0].sets[1].rir_high, Some(2.0));
        assert!(!detail.entries[1].sets[1].logged);
        assert_eq!(detail.entries[2].entry.untracked_sets, Some(2));
        assert_eq!(detail.session.notes, vec!["session note".to_string()]);

        let last = db.last_performance(variant_id).unwrap().unwrap();
        assert_eq!(last.sets[0].weight, Some(185.0));

        let found = db.search_variants("smith").unwrap();
        assert!(found.iter().any(|v| v.slug == "flat_smith_press"));
    }

    #[test]
    fn seed_variants_from_inline_grouped() {
        let db = temp_db();
        let n = db
            .seed_variants(&GroupedSeed {
                variants: vec![GroupedVariant {
                    slug: "lateral_raise_db".into(),
                    display: "Lateral Raise".into(),
                    family: Some("lateral_raise".into()),
                    gym: Some("home".into()),
                    status: "confirmed".into(),
                    names: vec!["lateral raise".into(), "Lat Raise".into()],
                }],
            })
            .unwrap();
        assert_eq!(n, 1);
        let v = db.get_variant_by_slug("lateral_raise_db").unwrap().unwrap();
        assert_eq!(v.display, "Lateral Raise");
        let aliases = db.list_aliases().unwrap();
        assert!(aliases.iter().any(|a| a.normalized == "lateral raise"));
        assert!(aliases.iter().any(|a| a.normalized == "lat raise"));
    }

    #[test]
    fn ha_data_dir_env_override() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ha_datadir_{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        // SAFETY: test-only, single-threaded test process for this env key
        unsafe { std::env::set_var("HA_DATA_DIR", &dir) };
        let path = default_db_path().unwrap();
        assert_eq!(path, dir.join("log.db"));
        unsafe { std::env::remove_var("HA_DATA_DIR") };
        let _ = open(&path).unwrap();
    }

    #[test]
    fn fresh_db_lands_at_user_version_2() {
        let db = temp_db();
        assert_eq!(user_version(&db.conn).unwrap(), 2);
    }

    #[test]
    fn migrate_upgrades_v1_db_and_keeps_rows() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ha_v1_{nanos}.db"));
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
            conn.execute_batch(MIGRATION_001).unwrap();
            conn.execute_batch("PRAGMA user_version = 1;").unwrap();
            conn.execute(
                "INSERT INTO block (slug, name, split) VALUES ('b', 'B', 'other')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO cycle (block_id, week, note_id, source) VALUES (1, 1, 'note-a', 'notes_import')",
                [],
            )
            .unwrap();
            assert_eq!(user_version(&conn).unwrap(), 1);
        }
        let db = open(&path).unwrap();
        assert_eq!(user_version(&db.conn).unwrap(), 2);
        let cycle = db.get_cycle_by_block_week(1, 1).unwrap().unwrap();
        assert_eq!(cycle.note_id.as_deref(), Some("note-a"));
        // Unique note_id is enforced after upgrade.
        let err = db.create_cycle(&NewCycle {
            block_id: 1,
            week: 2,
            title: None,
            note_id: Some("note-a".into()),
            started_on: None,
            date_precision: DatePrecision::Week,
            override_flag: false,
            source: Source::NotesImport,
            source_hash: None,
        });
        assert!(err.is_err(), "duplicate note_id should fail: {err:?}");
    }
}
