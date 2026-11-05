//! Import of `exports/grouped.json` into the log database (Phase 2e).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::db::{CycleImportMeta, Db};
use crate::log::{
    AliasStatus, DatePrecision, GroupedCycle, GroupedFile, NewBlock, NewEntry, NewSession, NewSet,
    NewVariant, Source, Split,
};

#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    pub dry_run: bool,
    pub force: bool,
    pub verbose: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ImportReport {
    pub cycles_new: usize,
    pub cycles_replaced: usize,
    pub cycles_unchanged: usize,
    pub sessions_written: usize,
    pub entries_written: usize,
    pub sets_written: usize,
    pub app_sessions_preserved: usize,
    pub variants_created: usize,
    pub variants_updated: usize,
    pub aliases_moved: usize,
    /// Variants in the database that the file doesn't list. Kept, not deleted.
    pub variants_not_in_file: Vec<String>,
    pub excluded_entries: usize,
    pub inferred_weights: usize,
    pub unmapped: Vec<UnmappedEntry>,
    pub unconfirmed_variants: Vec<String>,
    pub review_open: usize,
    pub review_accepted: usize,
    pub review_fixed: usize,
    pub outliers: Vec<Outlier>,
    pub stale_notes: Vec<String>,
    /// (old note_id, new note_id) for weeks whose note was re-created.
    pub notes_replaced: Vec<(String, String)>,
    pub inferred_detail: Vec<InferredWeight>,
}

#[derive(Debug, Clone)]
pub struct UnmappedEntry {
    pub note_id: String,
    pub title: Option<String>,
    pub name_raw: String,
    pub variant_slug: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Outlier {
    pub variant_slug: String,
    pub note_id: String,
    pub weight: f64,
    pub median: f64,
}

#[derive(Debug, Clone)]
pub struct InferredWeight {
    pub note_id: String,
    pub name_raw: String,
    pub set_number: i32,
    pub weight: f64,
}

pub fn load_grouped(path: &Path) -> Result<GroupedFile, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))
}

pub fn cycle_source_hash(cycle: &GroupedCycle) -> Result<String, String> {
    let bytes = serde_json::to_vec(cycle).map_err(|e| format!("hash cycle: {e}"))?;
    let digest = Sha256::digest(&bytes);
    Ok(format!("{digest:x}"))
}

/// Import a grouped.json document. Never deletes or modifies `source='app'` rows.
pub fn import_grouped(
    db: &Db,
    data: &GroupedFile,
    opts: &ImportOptions,
) -> Result<ImportReport, String> {
    let tx = db
        .conn()
        .unchecked_transaction()
        .map_err(|e| format!("begin transaction: {e}"))?;

    let mut report = ImportReport::default();
    let incoming_notes: HashSet<String> = data.cycles.iter().map(|c| c.note_id.clone()).collect();

    // Blocks
    let mut block_ids: HashMap<String, i64> = HashMap::new();
    for b in &data.blocks {
        let split = Split::parse(&b.split).unwrap_or(Split::Other);
        let id = db.upsert_block(&NewBlock {
            slug: b.slug.clone(),
            name: b.name.clone(),
            split,
            label: b.label.clone(),
            started_on: b.started_on.clone(),
        })?;
        block_ids.insert(b.slug.clone(), id);
    }

    // Variants + aliases. The file is the source of truth for both, so re-imports pick up
    // renames and merges from aliases.toml. A name listed under several variants goes to
    // the first one in the file, the same choice a fresh import makes.
    let mut variant_ids: HashMap<String, i64> = HashMap::new();
    let current_aliases: HashMap<String, i64> = db
        .list_aliases()?
        .into_iter()
        .map(|a| (a.normalized, a.variant_id))
        .collect();
    let mut seen_names: HashSet<String> = HashSet::new();
    for v in &data.variants {
        let status = AliasStatus::parse(&v.status).unwrap_or(AliasStatus::Suggested);
        if status != AliasStatus::Confirmed {
            report.unconfirmed_variants.push(v.slug.clone());
        }
        let (id, created, updated) = db.upsert_import_variant(&NewVariant {
            slug: v.slug.clone(),
            display: v.display.clone(),
            family: v.family.clone(),
            gym: v.gym.clone(),
            status,
            joint_action: None,
            equipment: None,
            load_unit: "lb".to_string(),
            default_settings: None,
        })?;
        if created {
            report.variants_created += 1;
        }
        if updated {
            report.variants_updated += 1;
        }
        variant_ids.insert(v.slug.clone(), id);
        for name in &v.names {
            let normalized = name.trim().to_lowercase();
            if normalized.is_empty() || !seen_names.insert(normalized.clone()) {
                continue;
            }
            if current_aliases.get(&normalized).is_some_and(|&old| old != id) {
                report.aliases_moved += 1;
            }
            db.upsert_alias(&normalized, id, status)?;
        }
    }
    // Also index any variants already in the DB that the file references, and list the
    // ones the file no longer has (merged away, or created in the app). They are kept.
    let file_slugs: HashSet<&str> = data.variants.iter().map(|v| v.slug.as_str()).collect();
    for v in db.list_variants()? {
        if !file_slugs.contains(v.slug.as_str()) {
            report.variants_not_in_file.push(v.slug.clone());
        }
        variant_ids.entry(v.slug).or_insert(v.id);
    }
    report.variants_not_in_file.sort();

    for cycle in &data.cycles {
        let hash = cycle_source_hash(cycle)?;
        let block_id = *block_ids
            .get(&cycle.block)
            .ok_or_else(|| format!("cycle references unknown block {}", cycle.block))?;

        if let Some(existing) = db.get_cycle_by_note_id(&cycle.note_id)?
            && !opts.force
            && existing.source_hash.as_deref() == Some(hash.as_str())
        {
            report.cycles_unchanged += 1;
            continue;
        }

        // Replace prior notes_import rows for this note_id (may be a different week).
        let had_prior = db.get_cycle_by_note_id(&cycle.note_id)?.is_some();
        if let Some(prior) = db.get_cycle_by_note_id(&cycle.note_id)? {
            db.delete_notes_import_sessions(prior.id)?;
            let remaining = db.count_sessions_for_cycle(prior.id)?;
            let clear_note = CycleImportMeta {
                title: prior.title.as_deref(),
                note_id: None,
                started_on: prior.started_on.as_deref(),
                date_precision: prior.date_precision,
                override_flag: prior.override_flag,
                source: prior.source,
                source_hash: None,
            };
            if remaining == 0 && prior.source == Source::NotesImport {
                // Free the note_id before attaching it to another cycle.
                db.update_cycle_import_meta(prior.id, &clear_note)?;
                db.delete_cycle(prior.id)?;
            } else if prior.block_id != block_id || prior.week != cycle.week {
                // Note moved: clear note_id on the old cycle so the unique index allows reuse.
                db.update_cycle_import_meta(prior.id, &clear_note)?;
            }
        }

        let target = match db.get_cycle_by_block_week(block_id, cycle.week)? {
            Some(c) => c,
            None => {
                let id = db.create_cycle(&crate::log::NewCycle {
                    block_id,
                    week: cycle.week,
                    title: cycle.title.clone(),
                    note_id: None,
                    started_on: cycle.started_on.clone(),
                    date_precision: DatePrecision::parse(
                        cycle.date_precision.as_deref().unwrap_or("week"),
                    )
                    .unwrap_or(DatePrecision::Week),
                    override_flag: cycle.override_reason.is_some(),
                    source: Source::NotesImport,
                    source_hash: None,
                })?;
                db.get_cycle(id)?
                    .ok_or_else(|| "cycle missing after insert".to_string())?
            }
        };

        // Preserve app sessions already on this cycle.
        let app_on_cycle: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM session WHERE cycle_id = ?1 AND source = 'app'",
                rusqlite::params![target.id],
                |row| row.get(0),
            )
            .map_err(|e| format!("count app sessions: {e}"))?;
        report.app_sessions_preserved += app_on_cycle as usize;

        // If target already had this note (same week), imported sessions were deleted above.
        // If target is a different cycle that somehow still has notes_import rows for a
        // previous import of this week under another note, leave them — note_id is the key.
        if target.note_id.as_deref() == Some(cycle.note_id.as_str()) {
            // already cleared above
        } else if let Some(old_note) = target.note_id.as_deref() {
            // Week occupied by a different note. Two notes for one week in the same file is
            // a data error; an old note missing from the file was re-created (new id), so
            // its import rows are replaced by the new note's.
            if incoming_notes.contains(old_note) {
                return Err(format!(
                    "block {} week {} has two notes in the file: {} and {}",
                    cycle.block, cycle.week, old_note, cycle.note_id
                ));
            }
            db.delete_notes_import_sessions(target.id)?;
            report
                .notes_replaced
                .push((old_note.to_string(), cycle.note_id.clone()));
        } else {
            // Fresh week or app-created cycle: remove any leftover notes_import sessions
            // from a prior partial import of this week without a note_id.
            db.delete_notes_import_sessions(target.id)?;
        }

        let precision = DatePrecision::parse(cycle.date_precision.as_deref().unwrap_or("week"))
            .unwrap_or(DatePrecision::Week);
        db.update_cycle_import_meta(
            target.id,
            &CycleImportMeta {
                title: cycle.title.as_deref(),
                note_id: Some(&cycle.note_id),
                started_on: cycle.started_on.as_deref(),
                date_precision: precision,
                override_flag: cycle.override_reason.is_some(),
                source: Source::NotesImport,
                source_hash: Some(&hash),
            },
        )?;

        if had_prior {
            report.cycles_replaced += 1;
        } else {
            report.cycles_new += 1;
        }

        for (sess_i, sess) in cycle.sessions.iter().enumerate() {
            let index = if sess.index > 0 {
                sess.index
            } else {
                (sess_i as i32) + 1
            };
            let session_id = db.create_session(&NewSession {
                cycle_id: target.id,
                day_number: sess.day_number,
                day_label: sess.day_label.clone(),
                comment: sess.comment.clone(),
                header_raw: sess.header_raw.clone(),
                implicit: sess.implicit,
                index,
                notes: sess.notes.clone(),
                performed_on: cycle.started_on.clone(),
                date_precision: DatePrecision::Week,
                source: Source::NotesImport,
            })?;
            report.sessions_written += 1;

            for entry in &sess.entries {
                let variant_id = if entry.excluded {
                    report.excluded_entries += 1;
                    None
                } else if let Some(slug) = entry.variant.as_deref() {
                    match variant_ids.get(slug) {
                        Some(id) => Some(*id),
                        None => {
                            report.unmapped.push(UnmappedEntry {
                                note_id: cycle.note_id.clone(),
                                title: cycle.title.clone(),
                                name_raw: entry.name_raw.clone(),
                                variant_slug: Some(slug.to_string()),
                            });
                            None
                        }
                    }
                } else if entry.tracked {
                    report.unmapped.push(UnmappedEntry {
                        note_id: cycle.note_id.clone(),
                        title: cycle.title.clone(),
                        name_raw: entry.name_raw.clone(),
                        variant_slug: None,
                    });
                    None
                } else {
                    None
                };

                let sets: Vec<NewSet> = entry
                    .sets
                    .iter()
                    .map(|s| {
                        if s.weight_inferred {
                            report.inferred_weights += 1;
                            if let Some(w) = s.weight {
                                report.inferred_detail.push(InferredWeight {
                                    note_id: cycle.note_id.clone(),
                                    name_raw: entry.name_raw.clone(),
                                    set_number: s.set_number,
                                    weight: w,
                                });
                            }
                        }
                        NewSet {
                            set_number: s.set_number,
                            weight: s.weight,
                            weight_inferred: s.weight_inferred,
                            weight_note: s.weight_note.clone(),
                            reps: s.reps,
                            rir_low: s.rir_low,
                            rir_high: s.rir_high,
                            logged: s.logged,
                            comment: s.comment.clone(),
                            raw: s.raw.clone(),
                        }
                    })
                    .collect();
                report.sets_written += sets.len();
                report.entries_written += 1;

                db.add_entry_with_sets(
                    &NewEntry {
                        session_id,
                        variant_id,
                        position: entry.position,
                        tracked: entry.tracked,
                        name: entry.name.clone(),
                        name_raw: entry.name_raw.clone(),
                        qualifiers: entry.qualifiers.clone(),
                        untracked_sets: entry.untracked_sets,
                        settings: entry.settings.clone(),
                        comment: entry.comment.clone(),
                        raw: entry.raw.clone(),
                        excluded: entry.excluded,
                    },
                    &sets,
                )?;
            }
        }
    }

    for item in &data.review {
        match item.resolution.as_deref().unwrap_or("open") {
            "fixed" => report.review_fixed += 1,
            "accepted" => report.review_accepted += 1,
            _ => report.review_open += 1,
        }
    }

    // Stale notes: in DB but not in file.
    for (note_id, _) in db.list_imported_note_ids()? {
        if !incoming_notes.contains(&note_id) {
            report.stale_notes.push(note_id);
        }
    }

    report.outliers = find_outliers(db, crate::metrics::OUTLIER_RATIO)?;

    if opts.dry_run {
        tx.rollback()
            .map_err(|e| format!("rollback dry-run: {e}"))?;
    } else {
        tx.commit().map_err(|e| format!("commit import: {e}"))?;
    }
    Ok(report)
}

/// Flag logged, non-inferred set weights more than `ratio` away from the variant median.
pub fn find_outliers(db: &Db, ratio: f64) -> Result<Vec<Outlier>, String> {
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT v.slug, c.note_id, s.weight
             FROM set_log s
             JOIN entry e ON e.id = s.entry_id
             JOIN session sess ON sess.id = e.session_id
             JOIN cycle c ON c.id = sess.cycle_id
             JOIN variant v ON v.id = e.variant_id
             WHERE s.logged = 1 AND s.weight_inferred = 0 AND s.weight IS NOT NULL
               AND e.excluded = 0 AND e.variant_id IS NOT NULL",
        )
        .map_err(|e| format!("outlier query: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, f64>(2)?,
            ))
        })
        .map_err(|e| format!("outlier query: {e}"))?;

    let mut by_variant: HashMap<String, Vec<(Option<String>, f64)>> = HashMap::new();
    for row in rows {
        let (slug, note_id, weight) = row.map_err(|e| format!("outlier row: {e}"))?;
        by_variant.entry(slug).or_default().push((note_id, weight));
    }

    let mut outliers = Vec::new();
    for (slug, weights) in by_variant {
        if weights.is_empty() {
            continue;
        }
        let mut sorted: Vec<f64> = weights.iter().map(|(_, w)| *w).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = percentile_median(&sorted);
        if median <= 0.0 {
            continue;
        }
        for (note_id, w) in weights {
            if (w - median).abs() / median > ratio {
                outliers.push(Outlier {
                    variant_slug: slug.clone(),
                    note_id: note_id.unwrap_or_default(),
                    weight: w,
                    median,
                });
            }
        }
    }
    outliers.sort_by(|a, b| a.variant_slug.cmp(&b.variant_slug));
    Ok(outliers)
}

fn percentile_median(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return 0.0;
    }
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

pub fn format_report(report: &ImportReport, verbose: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Cycles: {} new, {} replaced, {} unchanged\n",
        report.cycles_new, report.cycles_replaced, report.cycles_unchanged
    ));
    out.push_str(&format!(
        "Wrote {} sessions, {} entries, {} sets\n",
        report.sessions_written, report.entries_written, report.sets_written
    ));
    out.push_str(&format!(
        "App sessions preserved (counted on touched cycles): {}\n",
        report.app_sessions_preserved
    ));
    out.push_str(&format!(
        "Variants: {} created, {} updated, {} aliases moved; excluded entries: {}; inferred weights: {}\n",
        report.variants_created,
        report.variants_updated,
        report.aliases_moved,
        report.excluded_entries,
        report.inferred_weights
    ));
    if !report.variants_not_in_file.is_empty() {
        out.push_str(&format!(
            "Variants in the database but not the file (kept): {}\n",
            report.variants_not_in_file.join(", ")
        ));
    }
    out.push_str(&format!(
        "Review: {} open, {} accepted, {} fixed\n",
        report.review_open, report.review_accepted, report.review_fixed
    ));
    out.push_str(&format!("Unmapped entries: {}\n", report.unmapped.len()));
    for u in &report.unmapped {
        out.push_str(&format!(
            "  {} {:?} variant={:?}\n",
            u.name_raw, u.title, u.variant_slug
        ));
    }
    if !report.unconfirmed_variants.is_empty() {
        out.push_str(&format!(
            "Unconfirmed variants: {}\n",
            report.unconfirmed_variants.join(", ")
        ));
    }
    out.push_str(&format!("Outliers (±40% of median): {}\n", report.outliers.len()));
    for o in &report.outliers {
        out.push_str(&format!(
            "  {} note={} weight={} median={:.1}\n",
            o.variant_slug, o.note_id, o.weight, o.median
        ));
    }
    if !report.stale_notes.is_empty() {
        out.push_str(&format!(
            "Stale notes (in DB, not in file): {}\n",
            report.stale_notes.len()
        ));
        for n in &report.stale_notes {
            out.push_str(&format!("  {n}\n"));
        }
    }
    if !report.notes_replaced.is_empty() {
        out.push_str(&format!(
            "Re-created notes (old note replaced): {}\n",
            report.notes_replaced.len()
        ));
        for (old, new) in &report.notes_replaced {
            out.push_str(&format!("  {old} -> {new}\n"));
        }
    }
    if verbose && !report.inferred_detail.is_empty() {
        out.push_str("Inferred weights:\n");
        for i in &report.inferred_detail {
            out.push_str(&format!(
                "  {} {} set {} = {}\n",
                i.note_id, i.name_raw, i.set_number, i.weight
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_and_outlier_rule() {
        assert!((percentile_median(&[1.0, 2.0, 3.0]) - 2.0).abs() < f64::EPSILON);
        assert!((percentile_median(&[1.0, 3.0]) - 2.0).abs() < f64::EPSILON);
        // 10 vs median 100 is > 40%
        let sorted = [10.0_f64, 100.0, 100.0, 100.0];
        let med = percentile_median(&sorted);
        assert!((10.0 - med).abs() / med > 0.40);
        assert!((100.0 - med).abs() / med < 0.40);
    }
}
