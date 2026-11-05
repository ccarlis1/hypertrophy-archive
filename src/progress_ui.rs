//! Progress tab: strength charts per variant and joint action, weekly sets per muscle.

use std::collections::HashMap;
use std::path::Path;

use chrono::NaiveDate;
use eframe::egui;
use egui::plot::{Bar, BarChart, Legend, Line, MarkerShape, Plot, PlotPoints, Points};
use egui::{Color32, RichText, Ui};
use hypertrophy_archive::db::Db;
use hypertrophy_archive::log::{DatePrecision, SetRow};
use hypertrophy_archive::metrics::{self, Params, SessionStrength, VariantInfo, WeekVolume};
use hypertrophy_archive::models::{load_archive, Archive};

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Variant,
    JointAction,
    Muscles,
}

struct Loaded {
    rows: Vec<SetRow>,
    archive: Archive,
    /// Variants with at least one session that has an e1RM, most sessions first.
    variants: Vec<(VariantInfo, usize)>,
    /// Joint actions with at least one linked, trained variant.
    joint_actions: Vec<String>,
    weeks: Vec<WeekVolume>,
}

pub struct ProgressUi {
    params: Params,
    data: Option<Loaded>,
    error: Option<String>,
    view: View,
    filter: String,
    variant_id: Option<i64>,
    joint_action: Option<String>,
    cycle_id: Option<i64>,
    show_e5rm: bool,
    show_divisions: bool,
}

impl Default for ProgressUi {
    fn default() -> Self {
        Self {
            params: Params::default(),
            data: None,
            error: None,
            view: View::Variant,
            filter: String::new(),
            variant_id: None,
            joint_action: None,
            cycle_id: None,
            show_e5rm: true,
            show_divisions: false,
        }
    }
}

const PR_COLOR: Color32 = Color32::from_rgb(255, 200, 60);
const STALL_COLOR: Color32 = Color32::from_rgb(230, 80, 80);

impl ProgressUi {
    fn load(&mut self, db: &Db, data_dir: &Path) {
        let result = (|| -> Result<Loaded, String> {
            let rows = db.set_rows()?;
            let archive = load_archive(data_dir)?;
            let mut variants: Vec<(VariantInfo, usize)> = metrics::variants_in(&rows)
                .into_iter()
                .map(|v| {
                    let n = metrics::variant_history(&rows, v.id, &self.params).len();
                    (v, n)
                })
                .filter(|(_, n)| *n > 0)
                .collect();
            variants.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.display.cmp(&b.0.display)));
            let mut joint_actions: Vec<String> = variants
                .iter()
                .filter_map(|(v, _)| v.joint_action.clone())
                .collect();
            joint_actions.sort();
            joint_actions.dedup();
            let weeks = metrics::weekly_volume(&rows, &archive, &self.params);
            Ok(Loaded {
                rows,
                archive,
                variants,
                joint_actions,
                weeks,
            })
        })();
        match result {
            Ok(d) => {
                if self.variant_id.is_none() {
                    self.variant_id = d.variants.first().map(|(v, _)| v.id);
                }
                if self.joint_action.is_none() {
                    self.joint_action = d.joint_actions.first().cloned();
                }
                if self.cycle_id.is_none() {
                    self.cycle_id = d.weeks.last().map(|w| w.cycle_id);
                }
                self.data = Some(d);
                self.error = None;
            }
            Err(e) => {
                self.data = None;
                self.error = Some(e);
            }
        }
    }

    pub fn show(&mut self, ui: &mut Ui, db: Option<&Db>, data_dir: &Path) {
        let Some(db) = db else {
            ui.label("Database not available.");
            return;
        };
        if self.data.is_none() && self.error.is_none() {
            self.load(db, data_dir);
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.view, View::Variant, "Variant");
            ui.selectable_value(&mut self.view, View::JointAction, "Joint action");
            ui.selectable_value(&mut self.view, View::Muscles, "Sets per muscle");
            ui.separator();
            if ui.button("Reload").clicked() {
                self.load(db, data_dir);
            }
        });
        if let Some(e) = &self.error {
            ui.colored_label(Color32::RED, e);
            return;
        }
        let Some(data) = &self.data else { return };
        ui.label(
            RichText::new(format!(
                "{} sets, {} joint actions in {}",
                data.rows.len(),
                data.archive.len(),
                data_dir.display()
            ))
            .small()
            .weak(),
        );
        ui.add_space(6.0);
        match self.view {
            View::Variant => self.show_variant(ui),
            View::JointAction => self.show_joint_action(ui),
            View::Muscles => self.show_muscles(ui),
        }
    }

    fn show_variant(&mut self, ui: &mut Ui) {
        let Some(data) = &self.data else { return };
        let selected = data.variants.iter().find(|(v, _)| Some(v.id) == self.variant_id);
        ui.horizontal(|ui| {
            ui.label("Variant");
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("filter").desired_width(140.0));
            let filter = self.filter.to_lowercase();
            egui::ComboBox::from_id_source("progress_variant")
                .width(320.0)
                .selected_text(selected.map_or("pick a variant".to_string(), |(v, n)| {
                    format!("{} ({n})", v.display)
                }))
                .show_ui(ui, |ui| {
                    for (v, n) in &data.variants {
                        let hay = format!("{} {}", v.display, v.slug).to_lowercase();
                        if !filter.is_empty() && !hay.contains(&filter) {
                            continue;
                        }
                        ui.selectable_value(&mut self.variant_id, Some(v.id), format!("{} ({n})", v.display));
                    }
                });
            ui.checkbox(&mut self.show_e5rm, "e5RM");
        });
        let Some(vid) = self.variant_id else { return };
        let history = metrics::variant_history(&data.rows, vid, &self.params);
        let (Some(first), Some(last)) = (history.first(), history.last()) else {
            ui.label("No sessions with a weight and reps.");
            return;
        };
        let best = history.iter().map(|s| s.e1rm).fold(f64::MIN, f64::max);
        let base = metrics::baseline(&history, &self.params).unwrap_or(first.e1rm);
        ui.label(format!(
            "{} sessions, {} to {}. e1RM baseline {base:.1}, best {best:.1}, latest {:.1} ({:+.1}%)",
            history.len(),
            date_of(first),
            date_of(last),
            last.e1rm,
            last.pct_from_baseline
        ));
        if last.stalled {
            ui.colored_label(
                STALL_COLOR,
                format!("Stalled: {} sessions since the last e1RM best", last.since_best),
            );
        }

        let days = plot_days(&history);
        let pts = |f: fn(&SessionStrength) -> f64, keep: fn(&SessionStrength) -> bool| -> Vec<[f64; 2]> {
            history
                .iter()
                .zip(&days)
                .filter(|(s, _)| keep(s))
                .filter_map(|(s, d)| Some([(*d)?, f(s)]))
                .collect()
        };
        let e1 = pts(|s| s.e1rm, |_| true);
        let e5 = pts(|s| s.e5rm, |_| true);
        let prs = pts(|s| s.e1rm, |s| s.pr);
        let stalls = pts(|s| s.e1rm, |s| s.stalled);
        let show_e5rm = self.show_e5rm;
        date_plot(("progress_variant_plot", vid))
            .y_axis_formatter(|y, _| format!("{y:.0} lb"))
            .show(ui, |plot_ui| {
                plot_ui.line(Line::new(PlotPoints::from(e1)).name("e1RM").width(2.0_f32));
                if show_e5rm {
                    plot_ui.line(Line::new(PlotPoints::from(e5)).name("e5RM"));
                }
                plot_ui.points(
                    Points::new(PlotPoints::from(stalls))
                        .name("stalled")
                        .shape(MarkerShape::Circle)
                        .radius(4.0_f32)
                        .color(STALL_COLOR),
                );
                plot_ui.points(
                    Points::new(PlotPoints::from(prs))
                        .name("PR")
                        .shape(MarkerShape::Diamond)
                        .radius(6.0_f32)
                        .filled(true)
                        .color(PR_COLOR),
                );
            });
        week_dates_note(ui);

        ui.collapsing("Sessions", |ui| {
            egui::Grid::new("progress_sessions").striped(true).show(ui, |ui| {
                for h in ["date", "block / week", "top set", "best set", "e1RM", "e5RM", "Δ baseline", ""] {
                    ui.strong(h);
                }
                ui.end_row();
                for s in history.iter().rev() {
                    ui.label(date_of(s));
                    ui.label(format!("{} w{}", s.block_slug, s.week));
                    ui.label(format!("{}×{}", s.top_weight, s.top_reps));
                    ui.label(format!("{}×{} @{}", s.best_weight, s.best_reps, s.best_rir));
                    ui.label(format!("{:.1}", s.e1rm));
                    ui.label(format!("{:.1}", s.e5rm));
                    ui.label(format!("{:+.1}%", s.pct_from_baseline));
                    if s.pr {
                        ui.colored_label(PR_COLOR, "PR");
                    } else if s.stalled {
                        ui.colored_label(STALL_COLOR, "stall");
                    } else {
                        ui.label("");
                    }
                    ui.end_row();
                }
            });
        });
        ui.collapsing("Rep PRs", |ui| {
            egui::Grid::new("progress_rep_prs").striped(true).show(ui, |ui| {
                for pr in metrics::rep_prs(&data.rows, vid) {
                    ui.label(format!("{} lb", pr.weight));
                    ui.label(format!("{} reps", pr.reps));
                    ui.label(pr.performed_on.as_deref().unwrap_or("?"));
                    ui.end_row();
                }
            });
        });
    }

    fn show_joint_action(&mut self, ui: &mut Ui) {
        let Some(data) = &self.data else { return };
        if data.joint_actions.is_empty() {
            ui.label("No variant is linked to a joint action yet. Link them in the Variants tab.");
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Joint action");
            egui::ComboBox::from_id_source("progress_ja")
                .width(320.0)
                .selected_text(self.joint_action.clone().unwrap_or_default())
                .show_ui(ui, |ui| {
                    for ja in &data.joint_actions {
                        ui.selectable_value(&mut self.joint_action, Some(ja.clone()), ja);
                    }
                });
        });
        let Some(ja) = self.joint_action.clone() else { return };
        ui.label("e1RM as % change from each variant's baseline (median of its first sessions).");
        let series = metrics::joint_action_progress(&data.rows, &ja, &self.params);
        date_plot(("progress_ja_plot", ja.as_str()))
            .y_axis_formatter(|y, _| format!("{y:+.0}%"))
            .show(ui, |plot_ui| {
                for s in &series {
                    let pts: Vec<[f64; 2]> = s
                        .sessions
                        .iter()
                        .zip(plot_days(&s.sessions))
                        .filter_map(|(x, d)| Some([d?, x.pct_from_baseline]))
                        .collect();
                    plot_ui.line(
                        Line::new(PlotPoints::from(pts))
                            .name(format!("{} ({})", s.variant.display, s.sessions.len())),
                    );
                }
            });
        week_dates_note(ui);
    }

    fn show_muscles(&mut self, ui: &mut Ui) {
        let Some(data) = &self.data else { return };
        let label = |w: &WeekVolume| {
            format!(
                "{} w{} ({})",
                w.block_slug,
                w.week,
                w.started_on.as_deref().unwrap_or("?")
            )
        };
        let selected = data.weeks.iter().find(|w| Some(w.cycle_id) == self.cycle_id);
        ui.horizontal(|ui| {
            ui.label("Week");
            egui::ComboBox::from_id_source("progress_week")
                .width(280.0)
                .selected_text(selected.map(label).unwrap_or_default())
                .show_ui(ui, |ui| {
                    for w in data.weeks.iter().rev() {
                        ui.selectable_value(&mut self.cycle_id, Some(w.cycle_id), label(w));
                    }
                });
            ui.checkbox(&mut self.show_divisions, "Divisions");
        });
        let Some(cid) = self.cycle_id else { return };
        let Some(summary) = metrics::cycle_summary(&data.rows, cid, &data.archive, &self.params)
        else {
            return;
        };
        let w = &summary.volume;
        ui.label(format!(
            "{} hard sets (RIR ≤ {}), {} PRs, {} stalled. Primary muscles count 1 per set, secondary {}.",
            w.hard_sets,
            self.params.hard_set_max_rir,
            summary.pr_count(),
            summary.stalled().count(),
            metrics::SECONDARY_SHARE
        ));

        let mut items: Vec<(String, f64)> = if self.show_divisions {
            w.muscles
                .iter()
                .flat_map(|(m, v)| v.divisions.iter().map(move |(d, n)| (format!("{m}: {d}"), *n)))
                .collect()
        } else {
            w.muscles.iter().map(|(m, v)| (m.clone(), v.sets)).collect()
        };
        items.sort_by(|a, b| b.1.total_cmp(&a.1));
        if items.is_empty() {
            ui.label("No hard sets map to a muscle this week. Link variants to joint actions in the Variants tab.");
        } else {
            let bars: Vec<Bar> = items
                .iter()
                .enumerate()
                .map(|(i, (name, n))| Bar::new(i as f64, *n).name(name).width(0.7))
                .collect();
            let names: Vec<String> = items.iter().map(|(n, _)| n.clone()).collect();
            Plot::new(("progress_muscle_plot", cid, self.show_divisions))
                .height(300.0)
                .allow_scroll(false)
                .allow_drag(false)
                .allow_zoom(false)
                .include_y(0.0)
                .x_axis_formatter(move |x, _| {
                    let i = x.round();
                    if (x - i).abs() > 1e-6 || i < 0.0 {
                        return String::new();
                    }
                    names.get(i as usize).cloned().unwrap_or_default()
                })
                .show(ui, |plot_ui| {
                    plot_ui.bar_chart(BarChart::new(bars).name("hard sets").color(Color32::from_rgb(120, 200, 255)));
                });
        }
        if !w.unmapped.is_empty() {
            let names: Vec<String> = w.unmapped.iter().map(|(v, n)| format!("{v} {n}")).collect();
            ui.label(
                RichText::new(format!("Not counted (no joint action): {}", names.join(", ")))
                    .small()
                    .weak(),
            );
        }
        ui.collapsing("PRs and stalls this week", |ui| {
            for v in &summary.variants {
                let l = &v.last;
                if v.prs > 0 {
                    ui.colored_label(PR_COLOR, format!("PR  {}  e1RM {:.1}", v.variant.display, l.e1rm));
                } else if l.stalled {
                    ui.colored_label(
                        STALL_COLOR,
                        format!(
                            "stalled  {}  {} sessions since best",
                            v.variant.display, l.since_best
                        ),
                    );
                }
            }
        });
    }
}

fn date_of(s: &SessionStrength) -> &str {
    s.performed_on.as_deref().unwrap_or("?")
}

fn week_dates_note(ui: &mut Ui) {
    ui.label(
        RichText::new("Sessions dated only to the week are spread across that week, so hover dates are approximate.")
            .small()
            .weak(),
    );
}

/// x position (days since 1970-01-01) of each session. Sessions dated only to the week
/// share their week's anchor date, so the k-th of n on one date moves k × 7/n days into
/// that week instead of stacking on one x.
fn plot_days(sessions: &[SessionStrength]) -> Vec<Option<f64>> {
    let week = |s: &SessionStrength| s.date_precision == DatePrecision::Week;
    let mut count: HashMap<&str, usize> = HashMap::new();
    for s in sessions.iter().filter(|s| week(s)) {
        if let Some(d) = s.performed_on.as_deref() {
            *count.entry(d).or_default() += 1;
        }
    }
    let mut seen: HashMap<&str, usize> = HashMap::new();
    sessions
        .iter()
        .map(|s| {
            let date = s.performed_on.as_deref()?;
            let day = day_of(date)?;
            if !week(s) {
                return Some(day);
            }
            let k = seen.entry(date).or_default();
            let offset = *k as f64 * 7.0 / count[date] as f64;
            *k += 1;
            Some(day + offset)
        })
        .collect()
}

/// Days since 1970-01-01, the x axis of the time charts.
fn day_of(date: &str) -> Option<f64> {
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)?;
    Some((d - epoch).num_days() as f64)
}

fn date_plot(id: impl std::hash::Hash) -> Plot {
    Plot::new(id)
        .height(320.0)
        .allow_scroll(false)
        .legend(Legend::default())
        .x_axis_formatter(|x, _| {
            NaiveDate::from_ymd_opt(1970, 1, 1)
                .and_then(|e| e.checked_add_signed(chrono::Duration::days(x.round() as i64)))
                .map(|d| d.format("%Y-%m").to_string())
                .unwrap_or_default()
        })
        .label_formatter(|name, p| {
            let day = NaiveDate::from_ymd_opt(1970, 1, 1)
                .and_then(|e| e.checked_add_signed(chrono::Duration::days(p.x.round() as i64)))
                .map(|d| d.to_string())
                .unwrap_or_default();
            if name.is_empty() {
                format!("{day}\n{:.1}", p.y)
            } else {
                format!("{name}\n{day}\n{:.1}", p.y)
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(date: &str, precision: DatePrecision) -> SessionStrength {
        SessionStrength {
            session_id: 0,
            cycle_id: 0,
            block_slug: "b".into(),
            week: 1,
            performed_on: Some(date.into()),
            date_precision: precision,
            best_weight: 100.0,
            best_reps: 5.0,
            best_rir: 1.0,
            e1rm: 120.0,
            e5rm: 102.9,
            top_weight: 100.0,
            top_reps: 5.0,
            hard_sets: 1,
            pr: false,
            since_best: 0,
            stalled: false,
            pct_from_baseline: 0.0,
        }
    }

    #[test]
    fn week_dated_sessions_spread_across_their_week() {
        let d = day_of("2026-01-05").unwrap();
        let days = plot_days(&[
            session("2026-01-05", DatePrecision::Week),
            session("2026-01-05", DatePrecision::Week),
            session("2026-01-12", DatePrecision::Week),
            session("2026-01-20", DatePrecision::Exact),
            session("2026-01-20", DatePrecision::Exact),
        ]);
        assert_eq!(
            days,
            [Some(d), Some(d + 3.5), Some(d + 7.0), Some(d + 15.0), Some(d + 15.0)]
        );
    }
}
