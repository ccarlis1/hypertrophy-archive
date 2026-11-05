//! Progression metrics derived from logged sets.
//!
//! Everything here is a pure function over `SetRow`s from `Db::set_rows`, which come
//! in training order. Strength is estimated from each session's best set with reps to
//! failure r = reps + RIR midpoint: e1RM = w × (1 + r/c) and e5RM = e1RM / (1 + 5/c).

use std::collections::{BTreeMap, HashMap};

use crate::log::{SetLog, SetRow};
use crate::models::{Archive, Exercise};

/// Share of a hard set credited to a joint action's primary muscle.
pub const PRIMARY_SHARE: f64 = 1.0;
/// Share of a hard set credited to each secondary muscle.
pub const SECONDARY_SHARE: f64 = 0.5;

/// e1RM changes smaller than this (lb) are not a gain.
const GAIN_EPSILON: f64 = 1e-6;

#[derive(Debug, Clone, Copy)]
pub struct Params {
    /// Epley constant c in e1RM = w × (1 + r/c).
    pub epley_c: f64,
    /// Sessions without a new e1RM best before a variant counts as stalled.
    pub stall_sessions: usize,
    /// A variant's baseline is the median e1RM of its first this-many sessions.
    pub baseline_sessions: usize,
    /// A set is hard when its RIR midpoint is at most this.
    pub hard_set_max_rir: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            epley_c: 30.0,
            stall_sessions: 4,
            baseline_sessions: 3,
            hard_set_max_rir: 2.0,
        }
    }
}

/// RIR midpoint. A missing RIR counts as 0 (taken to failure), and "3+" counts as 3.
pub fn rir_mid(low: Option<f64>, high: Option<f64>) -> f64 {
    match (low, high) {
        (Some(lo), Some(hi)) => (lo + hi) / 2.0,
        (Some(v), None) | (None, Some(v)) => v,
        (None, None) => 0.0,
    }
}

pub fn e1rm(weight: f64, reps_to_failure: f64, c: f64) -> f64 {
    weight * (1.0 + reps_to_failure / c)
}

pub fn e5rm(e1rm: f64, c: f64) -> f64 {
    e1rm / (1.0 + 5.0 / c)
}

/// Reps plus RIR midpoint, for a logged set with reps.
pub fn reps_to_failure(set: &SetLog) -> Option<f64> {
    if !set.logged {
        return None;
    }
    set.reps
        .filter(|r| *r > 0.0)
        .map(|r| r + rir_mid(set.rir_low, set.rir_high))
}

/// e1RM of a logged set with a positive weight and reps.
pub fn set_e1rm(set: &SetLog, c: f64) -> Option<f64> {
    let w = set.weight.filter(|w| *w > 0.0)?;
    Some(e1rm(w, reps_to_failure(set)?, c))
}

/// A logged set with reps and an RIR midpoint at or under the hard-set cap.
pub fn is_hard_set(set: &SetLog, p: &Params) -> bool {
    set.logged
        && set.reps.is_some_and(|r| r > 0.0)
        && rir_mid(set.rir_low, set.rir_high) <= p.hard_set_max_rir
}

/// One session of one variant.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionStrength {
    pub session_id: i64,
    pub cycle_id: i64,
    pub block_slug: String,
    pub week: i32,
    pub performed_on: Option<String>,
    /// The set with the highest e1RM.
    pub best_weight: f64,
    pub best_reps: f64,
    pub best_rir: f64,
    pub e1rm: f64,
    pub e5rm: f64,
    /// The heaviest set, most reps on ties.
    pub top_weight: f64,
    pub top_reps: f64,
    pub hard_sets: usize,
    /// e1RM beat every earlier session. The first session is the baseline, not a PR.
    pub pr: bool,
    /// Sessions since the best e1RM so far, not counting the best one.
    pub since_best: usize,
    pub stalled: bool,
    /// e1RM as a % change from the variant's baseline.
    pub pct_from_baseline: f64,
}

/// Per-session strength for one variant, in training order. Sessions with no set that
/// has both a weight and reps (bodyweight, blank) are left out.
pub fn variant_history(rows: &[SetRow], variant_id: i64, p: &Params) -> Vec<SessionStrength> {
    let mut out: Vec<SessionStrength> = Vec::new();
    let mut best_so_far: Option<f64> = None;
    let mut since_best = 0;
    for group in sessions_of(rows, variant_id) {
        let first = group[0];
        let mut best: Option<(f64, &SetLog)> = None;
        let mut top: Option<&SetLog> = None;
        let mut hard_sets = 0;
        for row in &group {
            let set = &row.set;
            if is_hard_set(set, p) {
                hard_sets += 1;
            }
            let Some(e) = set_e1rm(set, p.epley_c) else {
                continue;
            };
            if best.is_none_or(|(b, _)| e > b) {
                best = Some((e, set));
            }
            if top.is_none_or(|t| (set.weight, set.reps) > (t.weight, t.reps)) {
                top = Some(set);
            }
        }
        let (Some((e1, best_set)), Some(top_set)) = (best, top) else {
            continue;
        };
        let pr = best_so_far.is_some_and(|b| e1 > b + GAIN_EPSILON);
        if best_so_far.is_none_or(|b| e1 > b + GAIN_EPSILON) {
            best_so_far = Some(e1);
            since_best = 0;
        } else {
            since_best += 1;
        }
        out.push(SessionStrength {
            session_id: first.session_id,
            cycle_id: first.cycle_id,
            block_slug: first.block_slug.clone(),
            week: first.week,
            performed_on: first.performed_on.clone(),
            best_weight: best_set.weight.unwrap_or(0.0),
            best_reps: best_set.reps.unwrap_or(0.0),
            best_rir: rir_mid(best_set.rir_low, best_set.rir_high),
            e1rm: e1,
            e5rm: e5rm(e1, p.epley_c),
            top_weight: top_set.weight.unwrap_or(0.0),
            top_reps: top_set.reps.unwrap_or(0.0),
            hard_sets,
            pr,
            since_best,
            stalled: since_best >= p.stall_sessions,
            pct_from_baseline: 0.0,
        });
    }
    if let Some(base) = baseline(&out, p) {
        for s in &mut out {
            s.pct_from_baseline = (s.e1rm / base - 1.0) * 100.0;
        }
    }
    out
}

/// Median e1RM of the first `baseline_sessions` sessions.
pub fn baseline(history: &[SessionStrength], p: &Params) -> Option<f64> {
    let n = p.baseline_sessions.max(1).min(history.len());
    if n == 0 {
        return None;
    }
    let mut first: Vec<f64> = history[..n].iter().map(|s| s.e1rm).collect();
    first.sort_by(f64::total_cmp);
    Some(if n % 2 == 1 {
        first[n / 2]
    } else {
        (first[n / 2 - 1] + first[n / 2]) / 2.0
    })
}

/// Rows of one variant, grouped by session in training order.
fn sessions_of(rows: &[SetRow], variant_id: i64) -> Vec<Vec<&SetRow>> {
    let mut groups: Vec<Vec<&SetRow>> = Vec::new();
    for row in rows.iter().filter(|r| r.variant_id == Some(variant_id)) {
        match groups.last_mut() {
            Some(g) if g[0].session_id == row.session_id => g.push(row),
            _ => groups.push(vec![row]),
        }
    }
    groups
}

/// The most reps ever done at one weight.
#[derive(Debug, Clone, PartialEq)]
pub struct RepPr {
    pub weight: f64,
    pub reps: f64,
    pub session_id: i64,
    pub performed_on: Option<String>,
}

/// Rep PRs at each weight for one variant, heaviest first. The earliest set wins ties.
pub fn rep_prs(rows: &[SetRow], variant_id: i64) -> Vec<RepPr> {
    let mut by_weight: BTreeMap<i64, RepPr> = BTreeMap::new();
    for row in rows.iter().filter(|r| r.variant_id == Some(variant_id)) {
        let set = &row.set;
        let (Some(w), Some(reps)) = (set.weight, set.reps) else {
            continue;
        };
        if !set.logged || w <= 0.0 || reps <= 0.0 {
            continue;
        }
        let key = (w * 100.0).round() as i64;
        if by_weight.get(&key).is_none_or(|pr| reps > pr.reps) {
            by_weight.insert(
                key,
                RepPr {
                    weight: w,
                    reps,
                    session_id: row.session_id,
                    performed_on: row.performed_on.clone(),
                },
            );
        }
    }
    by_weight.into_values().rev().collect()
}

/// A variant that appears in the rows.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantInfo {
    pub id: i64,
    pub slug: String,
    pub display: String,
    pub joint_action: Option<String>,
}

/// Variants in order of first appearance.
pub fn variants_in(rows: &[SetRow]) -> Vec<VariantInfo> {
    let mut seen = HashMap::new();
    let mut out = Vec::new();
    for row in rows {
        let Some(id) = row.variant_id else { continue };
        if seen.insert(id, ()).is_none() {
            out.push(VariantInfo {
                id,
                slug: row.variant_slug.clone().unwrap_or_default(),
                display: row.variant_display.clone().unwrap_or_default(),
                joint_action: row.joint_action.clone(),
            });
        }
    }
    out
}

/// One variant's history, for charts that compare variants.
#[derive(Debug, Clone)]
pub struct VariantSeries {
    pub variant: VariantInfo,
    pub sessions: Vec<SessionStrength>,
}

/// Every variant linked to a joint action, with its history. Compare them through
/// `pct_from_baseline`, not raw weight, since load schemes differ between machines.
pub fn joint_action_progress(rows: &[SetRow], joint_action: &str, p: &Params) -> Vec<VariantSeries> {
    variants_in(rows)
        .into_iter()
        .filter(|v| v.joint_action.as_deref() == Some(joint_action))
        .map(|v| VariantSeries {
            sessions: variant_history(rows, v.id, p),
            variant: v,
        })
        .filter(|s| !s.sessions.is_empty())
        .collect()
}

/// Hard sets credited to one muscle, in total and per active division.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MuscleVolume {
    pub sets: f64,
    pub divisions: BTreeMap<String, f64>,
}

/// Credit `sets` hard sets of a joint action to its muscles: the primary muscle gets
/// `PRIMARY_SHARE` per set, the others `SECONDARY_SHARE`. Each active division gets
/// its muscle's share.
pub fn add_volume(volume: &mut BTreeMap<String, MuscleVolume>, ja: &Exercise, sets: f64) {
    for target in &ja.target_muscle_groups {
        let share = if target.primary {
            PRIMARY_SHARE
        } else {
            SECONDARY_SHARE
        };
        let m = volume.entry(target.muscle_name.clone()).or_default();
        m.sets += sets * share;
        for d in target.muscle_divisions.iter().filter(|d| d.active) {
            *m.divisions.entry(d.name.clone()).or_default() += sets * share;
        }
    }
}

/// Hard sets per muscle for one week (cycle).
#[derive(Debug, Clone, PartialEq)]
pub struct WeekVolume {
    pub cycle_id: i64,
    pub block_slug: String,
    pub week: i32,
    /// Date of the week's first session.
    pub started_on: Option<String>,
    pub hard_sets: usize,
    pub muscles: BTreeMap<String, MuscleVolume>,
    /// Hard sets of variants with no joint action in the archive, by variant name.
    pub unmapped: BTreeMap<String, usize>,
}

/// Weekly hard sets per muscle and division, one entry per cycle in training order.
pub fn weekly_volume(rows: &[SetRow], archive: &Archive, p: &Params) -> Vec<WeekVolume> {
    let mut weeks: Vec<WeekVolume> = Vec::new();
    let mut index: HashMap<i64, usize> = HashMap::new();
    // Hard-set counts per (cycle, joint action), credited to muscles at the end.
    let mut ja_sets: BTreeMap<(usize, String), usize> = BTreeMap::new();
    for row in rows {
        let i = *index.entry(row.cycle_id).or_insert_with(|| {
            weeks.push(WeekVolume {
                cycle_id: row.cycle_id,
                block_slug: row.block_slug.clone(),
                week: row.week,
                started_on: row.performed_on.clone(),
                hard_sets: 0,
                muscles: BTreeMap::new(),
                unmapped: BTreeMap::new(),
            });
            weeks.len() - 1
        });
        if !is_hard_set(&row.set, p) {
            continue;
        }
        let w = &mut weeks[i];
        w.hard_sets += 1;
        match row.joint_action.as_deref().filter(|ja| archive.contains_key(*ja)) {
            Some(ja) => *ja_sets.entry((i, ja.to_string())).or_default() += 1,
            None => {
                let name = row
                    .variant_display
                    .clone()
                    .unwrap_or_else(|| "(no variant)".to_string());
                *w.unmapped.entry(name).or_default() += 1;
            }
        }
    }
    for ((i, ja), n) in ja_sets {
        add_volume(&mut weeks[i].muscles, &archive[&ja], n as f64);
    }
    weeks
}

/// One muscle's weekly hard sets, matched case-insensitively.
pub fn muscle_weekly<'a>(weeks: &'a [WeekVolume], muscle: &str) -> Vec<(&'a WeekVolume, f64)> {
    weeks
        .iter()
        .map(|w| {
            let sets = w
                .muscles
                .iter()
                .filter(|(name, _)| name.eq_ignore_ascii_case(muscle))
                .map(|(_, m)| m.sets)
                .sum();
            (w, sets)
        })
        .collect()
}

/// A variant's state at the end of one cycle.
#[derive(Debug, Clone)]
pub struct CycleVariant {
    pub variant: VariantInfo,
    /// The variant's last session in the cycle.
    pub last: SessionStrength,
    /// e1RM PRs set during the cycle.
    pub prs: usize,
}

#[derive(Debug, Clone)]
pub struct CycleSummary {
    pub volume: WeekVolume,
    pub variants: Vec<CycleVariant>,
}

impl CycleSummary {
    pub fn pr_count(&self) -> usize {
        self.variants.iter().map(|v| v.prs).sum()
    }

    pub fn stalled(&self) -> impl Iterator<Item = &CycleVariant> {
        self.variants.iter().filter(|v| v.last.stalled)
    }
}

/// Volume, PRs and stalls for one cycle. PRs and stalls use each variant's full history.
pub fn cycle_summary(
    rows: &[SetRow],
    cycle_id: i64,
    archive: &Archive,
    p: &Params,
) -> Option<CycleSummary> {
    let volume = weekly_volume(rows, archive, p)
        .into_iter()
        .find(|w| w.cycle_id == cycle_id)?;
    let in_cycle: Vec<SetRow> = rows.iter().filter(|r| r.cycle_id == cycle_id).cloned().collect();
    let variants = variants_in(&in_cycle)
        .into_iter()
        .filter_map(|v| {
            let history = variant_history(rows, v.id, p);
            let here: Vec<&SessionStrength> =
                history.iter().filter(|s| s.cycle_id == cycle_id).collect();
            let last = (*here.last()?).clone();
            Some(CycleVariant {
                prs: here.iter().filter(|s| s.pr).count(),
                last,
                variant: v,
            })
        })
        .collect();
    Some(CycleSummary { volume, variants })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::Split;
    use crate::models::{
        ExerciseType, JointsInvolved, MuscleDivision, PlaneOfMotion, ResistanceProfile,
        TargetMuscles,
    };

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    fn set(weight: Option<f64>, reps: Option<f64>, rir: (Option<f64>, Option<f64>)) -> SetLog {
        SetLog {
            id: 0,
            entry_id: 0,
            set_number: 1,
            weight,
            weight_inferred: false,
            weight_note: None,
            reps,
            rir_low: rir.0,
            rir_high: rir.1,
            logged: true,
            comment: None,
            raw: None,
        }
    }

    /// Variant 1 ("curl", linked to `elbow_flexion`) unless `variant` says otherwise.
    fn row(session: i64, cycle: i64, variant: i64, set: SetLog) -> SetRow {
        SetRow {
            block_slug: "b".into(),
            block_label: None,
            split: Split::Other,
            cycle_id: cycle,
            week: cycle as i32,
            session_id: session,
            performed_on: Some(format!("2026-01-{:02}", session)),
            day_label: None,
            position: 1,
            variant_id: Some(variant),
            variant_slug: Some(format!("v{variant}")),
            variant_display: Some(format!("V{variant}")),
            family: None,
            gym: None,
            joint_action: Some(if variant == 1 { "elbow_flexion" } else { "knee_extension" }.into()),
            set,
        }
    }

    fn ws(w: f64, reps: f64, rir: f64) -> SetLog {
        set(Some(w), Some(reps), (Some(rir), Some(rir)))
    }

    #[test]
    fn epley_with_rir() {
        // 100 × 8 @2 → r = 10.
        approx(e1rm(100.0, 10.0, 30.0), 100.0 * 40.0 / 30.0);
        approx(e5rm(e1rm(100.0, 10.0, 30.0), 30.0), 100.0 * 40.0 / 35.0);
        // r = 5 gives e5RM = w.
        approx(e5rm(e1rm(185.0, 5.0, 30.0), 30.0), 185.0);
        approx(set_e1rm(&ws(100.0, 8.0, 2.0), 30.0).unwrap(), 400.0 / 3.0);
    }

    #[test]
    fn rir_midpoint_rules() {
        approx(rir_mid(Some(1.0), Some(2.0)), 1.5);
        approx(rir_mid(Some(1.0), Some(1.0)), 1.0);
        approx(rir_mid(Some(3.0), None), 3.0);
        approx(rir_mid(None, None), 0.0);
        // Missing RIR is taken to failure.
        let s = set(Some(100.0), Some(10.0), (None, None));
        approx(set_e1rm(&s, 30.0).unwrap(), 100.0 * (1.0 + 10.0 / 30.0));
    }

    #[test]
    fn sets_without_weight_or_reps_have_no_e1rm() {
        assert!(set_e1rm(&set(None, Some(10.0), (None, None)), 30.0).is_none());
        assert!(set_e1rm(&set(Some(100.0), None, (None, None)), 30.0).is_none());
        let mut blank = ws(100.0, 5.0, 1.0);
        blank.logged = false;
        assert!(set_e1rm(&blank, 30.0).is_none());
    }

    #[test]
    fn hard_set_uses_rir_midpoint_cap() {
        let p = Params::default();
        assert!(is_hard_set(&ws(100.0, 8.0, 2.0), &p));
        assert!(!is_hard_set(&set(Some(100.0), Some(8.0), (Some(2.0), Some(3.0))), &p));
        assert!(is_hard_set(&set(Some(100.0), Some(8.0), (Some(1.0), Some(3.0))), &p));
        // Bodyweight with no RIR: taken to failure, so hard.
        assert!(is_hard_set(&set(None, Some(8.0), (None, None)), &p));
        assert!(!is_hard_set(&set(Some(100.0), None, (Some(0.0), Some(0.0))), &p));
    }

    #[test]
    fn history_picks_best_and_top_sets_and_flags_prs() {
        let rows = vec![
            // Session 1: top set 110×3 @0 (e1RM 121), best set 100×8 @2 (133.3).
            row(1, 1, 1, ws(110.0, 3.0, 0.0)),
            row(1, 1, 1, ws(100.0, 8.0, 2.0)),
            row(1, 1, 2, ws(300.0, 10.0, 0.0)),
            // Session 2: worse.
            row(2, 2, 1, ws(100.0, 6.0, 2.0)),
            // Session 3: PR.
            row(3, 3, 1, ws(105.0, 8.0, 2.0)),
        ];
        let h = variant_history(&rows, 1, &Params::default());
        assert_eq!(h.len(), 3);
        assert_eq!((h[0].top_weight, h[0].top_reps), (110.0, 3.0));
        assert_eq!((h[0].best_weight, h[0].best_reps, h[0].best_rir), (100.0, 8.0, 2.0));
        approx(h[0].e1rm, 400.0 / 3.0);
        approx(h[0].e5rm, 100.0 * 40.0 / 35.0);
        assert_eq!(h[0].hard_sets, 2);
        assert_eq!(h.iter().map(|s| s.pr).collect::<Vec<_>>(), [false, false, true]);
        assert_eq!(h.iter().map(|s| s.since_best).collect::<Vec<_>>(), [0, 1, 0]);
    }

    #[test]
    fn stall_flag_after_n_sessions_without_a_gain() {
        let p = Params {
            stall_sessions: 2,
            ..Params::default()
        };
        let rows: Vec<SetRow> = [100.0, 100.0, 95.0, 100.0, 101.0]
            .iter()
            .enumerate()
            .map(|(i, w)| row(i as i64 + 1, 1, 1, ws(*w, 5.0, 1.0)))
            .collect();
        let h = variant_history(&rows, 1, &p);
        // Matching the best is not a gain.
        assert_eq!(h.iter().map(|s| s.since_best).collect::<Vec<_>>(), [0, 1, 2, 3, 0]);
        assert_eq!(
            h.iter().map(|s| s.stalled).collect::<Vec<_>>(),
            [false, false, true, true, false]
        );
    }

    #[test]
    fn baseline_is_median_of_first_three_sessions() {
        let rows: Vec<SetRow> = [100.0, 90.0, 120.0, 150.0]
            .iter()
            .enumerate()
            .map(|(i, w)| row(i as i64 + 1, 1, 1, ws(*w, 5.0, 1.0)))
            .collect();
        let h = variant_history(&rows, 1, &Params::default());
        let base = baseline(&h, &Params::default()).unwrap();
        approx(base, e1rm(100.0, 6.0, 30.0));
        approx(h[3].pct_from_baseline, 50.0);
        approx(h[1].pct_from_baseline, -10.0);

        // Fewer sessions than the window: median of what there is.
        let h2 = variant_history(&rows[..2], 1, &Params::default());
        approx(baseline(&h2, &Params::default()).unwrap(), e1rm(95.0, 6.0, 30.0));
    }

    #[test]
    fn bodyweight_sessions_are_skipped() {
        let rows = vec![
            row(1, 1, 1, set(None, Some(10.0), (Some(1.0), Some(1.0)))),
            row(2, 1, 1, ws(50.0, 10.0, 1.0)),
        ];
        let h = variant_history(&rows, 1, &Params::default());
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].session_id, 2);
    }

    #[test]
    fn rep_prs_keep_most_reps_per_weight() {
        let rows = vec![
            row(1, 1, 1, ws(100.0, 8.0, 1.0)),
            row(2, 1, 1, ws(100.0, 10.0, 1.0)),
            row(3, 1, 1, ws(100.0, 10.0, 0.0)),
            row(3, 1, 1, ws(110.0, 6.0, 1.0)),
        ];
        let prs = rep_prs(&rows, 1);
        assert_eq!(
            prs.iter().map(|p| (p.weight, p.reps, p.session_id)).collect::<Vec<_>>(),
            [(110.0, 6.0, 3), (100.0, 10.0, 2)]
        );
    }

    #[test]
    fn joint_action_progress_is_relative_per_variant() {
        let mut rows = vec![
            row(1, 1, 1, ws(100.0, 5.0, 1.0)),
            row(2, 2, 1, ws(110.0, 5.0, 1.0)),
            row(1, 1, 3, ws(40.0, 5.0, 1.0)),
            row(2, 2, 3, ws(50.0, 5.0, 1.0)),
        ];
        for r in rows.iter_mut().filter(|r| r.variant_id == Some(3)) {
            r.joint_action = Some("elbow_flexion".into());
        }
        let p = Params {
            baseline_sessions: 1,
            ..Params::default()
        };
        let series = joint_action_progress(&rows, "elbow_flexion", &p);
        assert_eq!(series.len(), 2);
        approx(series[0].sessions[1].pct_from_baseline, 10.0);
        approx(series[1].sessions[1].pct_from_baseline, 25.0);
    }

    fn exercise(name: &str, muscles: &[(&str, bool, &[&str])]) -> Exercise {
        Exercise {
            name: name.into(),
            id: 0,
            r#type: ExerciseType::Isolation,
            description: String::new(),
            target_muscle_groups: muscles
                .iter()
                .map(|(m, primary, divs)| TargetMuscles {
                    muscle_name: m.to_string(),
                    primary: *primary,
                    muscle_divisions: divs
                        .iter()
                        .map(|d| MuscleDivision {
                            name: d.to_string(),
                            active: true,
                            bias: Default::default(),
                        })
                        .chain(std::iter::once(MuscleDivision {
                            name: "Inactive".into(),
                            active: false,
                            bias: Default::default(),
                        }))
                        .collect(),
                })
                .collect(),
            joints_involved: JointsInvolved { joints: Vec::new() },
            resistance_profile: ResistanceProfile::Constant,
            plane_of_motion: PlaneOfMotion::Sagittal,
            tips: String::new(),
            technique_video: String::new(),
        }
    }

    fn archive() -> Archive {
        let mut a = Archive::new();
        a.insert(
            "elbow_flexion".into(),
            exercise(
                "elbow_flexion",
                &[
                    ("Elbow Flexors", true, &["Biceps Brachii", "Brachialis"]),
                    ("Forearms", false, &["Wrist Flexors"]),
                ],
            ),
        );
        a
    }

    #[test]
    fn weekly_volume_weights_primary_and_secondary() {
        let mut soft = ws(100.0, 8.0, 3.0);
        soft.set_number = 3;
        let rows = vec![
            row(1, 1, 1, ws(100.0, 8.0, 1.0)),
            row(1, 1, 1, ws(100.0, 8.0, 2.0)),
            row(1, 1, 1, soft),
            // Variant 2 links to a joint action that isn't in the archive.
            row(1, 1, 2, ws(300.0, 10.0, 0.0)),
            row(2, 2, 1, ws(100.0, 8.0, 0.0)),
        ];
        let weeks = weekly_volume(&rows, &archive(), &Params::default());
        assert_eq!(weeks.len(), 2);
        let w1 = &weeks[0];
        assert_eq!(w1.hard_sets, 3);
        approx(w1.muscles["Elbow Flexors"].sets, 2.0);
        approx(w1.muscles["Elbow Flexors"].divisions["Brachialis"], 2.0);
        assert!(!w1.muscles["Elbow Flexors"].divisions.contains_key("Inactive"));
        approx(w1.muscles["Forearms"].sets, 1.0);
        approx(w1.muscles["Forearms"].divisions["Wrist Flexors"], 1.0);
        assert_eq!(w1.unmapped["V2"], 1);
        assert_eq!(w1.started_on.as_deref(), Some("2026-01-01"));

        let biceps = muscle_weekly(&weeks, "elbow flexors");
        assert_eq!(biceps.iter().map(|(_, s)| *s).collect::<Vec<_>>(), [2.0, 1.0]);
    }

    #[test]
    fn cycle_summary_counts_prs_and_stalls_from_full_history() {
        let p = Params {
            stall_sessions: 1,
            ..Params::default()
        };
        let rows = vec![
            row(1, 1, 1, ws(100.0, 5.0, 1.0)),
            row(1, 1, 2, ws(200.0, 5.0, 1.0)),
            row(2, 2, 1, ws(110.0, 5.0, 1.0)),
            row(2, 2, 2, ws(190.0, 5.0, 1.0)),
        ];
        let s = cycle_summary(&rows, 2, &archive(), &p).unwrap();
        assert_eq!(s.pr_count(), 1);
        assert_eq!(s.stalled().map(|v| v.variant.id).collect::<Vec<_>>(), [2]);
        assert_eq!(s.volume.hard_sets, 2);
        assert!(cycle_summary(&rows, 99, &archive(), &p).is_none());
    }
}
