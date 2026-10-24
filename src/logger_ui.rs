//! Log / History / Variants UI state and panels.

use eframe::egui;
use egui::{Color32, RichText, Ui};
use hypertrophy_archive::db::Db;
use hypertrophy_archive::log::{
    AliasStatus, Block, DatePrecision, NewBlock, NewEntry, NewSession, NewSet, NewVariant, SessionSummary,
    Source, Split, Variant,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MainTab {
    Editor,
    Log,
    History,
    Variants,
}

pub struct LoggerUi {
    pub main_tab: MainTab,
    pub db: Option<Db>,
    pub db_error: Option<String>,
    pub db_path_display: String,

    // Log tab
    pub blocks: Vec<Block>,
    pub selected_block_id: Option<i64>,
    pub new_block_slug: String,
    pub new_block_name: String,
    pub new_block_split: Split,
    pub week: String,
    pub day_label: String,
    pub session_id: Option<i64>,
    pub search: String,
    pub search_results: Vec<Variant>,
    pub drafts: Vec<DraftEntry>,
    pub untracked_n: String,
    pub untracked_name: String,

    // History
    pub sessions: Vec<SessionSummary>,
    pub history_detail: Option<String>,

    // Variants
    pub variants: Vec<Variant>,
    pub selected_variant_id: Option<i64>,
    pub edit_display: String,
    pub edit_family: String,
    pub edit_gym: String,
    pub edit_joint_action: String,
    pub edit_settings: String,
    pub new_variant_slug: String,
    pub new_variant_display: String,
}

#[derive(Clone)]
pub struct DraftEntry {
    pub variant_id: Option<i64>,
    pub name_raw: String,
    pub display: String,
    pub settings: String,
    pub tracked: bool,
    pub untracked_sets: i32,
    pub last_label: String,
    pub sets: Vec<DraftSet>,
}

#[derive(Clone)]
pub struct DraftSet {
    pub weight: String,
    pub reps: String,
    pub rir_low: String,
    pub rir_high: String,
    pub comment: String,
    pub logged: bool,
}

impl Default for LoggerUi {
    fn default() -> Self {
        Self {
            main_tab: MainTab::Editor,
            db: None,
            db_error: None,
            db_path_display: String::new(),
            blocks: Vec::new(),
            selected_block_id: None,
            new_block_slug: String::new(),
            new_block_name: String::new(),
            new_block_split: Split::UpperLower,
            week: "1".to_string(),
            day_label: "Upper 1".to_string(),
            session_id: None,
            search: String::new(),
            search_results: Vec::new(),
            drafts: Vec::new(),
            untracked_n: "2".to_string(),
            untracked_name: String::new(),
            sessions: Vec::new(),
            history_detail: None,
            variants: Vec::new(),
            selected_variant_id: None,
            edit_display: String::new(),
            edit_family: String::new(),
            edit_gym: String::new(),
            edit_joint_action: String::new(),
            edit_settings: String::new(),
            new_variant_slug: String::new(),
            new_variant_display: String::new(),
        }
    }
}

impl LoggerUi {
    pub fn open_db(&mut self) {
        match Db::open_default() {
            Ok(db) => {
                self.db_path_display = db.path().display().to_string();
                self.db = Some(db);
                self.db_error = None;
                self.reload_blocks();
                self.reload_variants();
                self.reload_sessions();
            }
            Err(e) => {
                self.db = None;
                self.db_error = Some(e);
            }
        }
    }

    fn with_db<R>(&self, f: impl FnOnce(&Db) -> Result<R, String>) -> Result<R, String> {
        let db = self.db.as_ref().ok_or_else(|| "database not open".to_string())?;
        f(db)
    }

    pub fn reload_blocks(&mut self) {
        if let Ok(blocks) = self.with_db(|db| db.list_blocks()) {
            self.blocks = blocks;
            if self.selected_block_id.is_none() {
                self.selected_block_id = self.blocks.first().map(|b| b.id);
            }
        }
    }

    pub fn reload_variants(&mut self) {
        if let Ok(v) = self.with_db(|db| db.list_variants()) {
            self.variants = v;
        }
    }

    pub fn reload_sessions(&mut self) {
        if let Ok(s) = self.with_db(|db| db.list_sessions()) {
            self.sessions = s;
        }
    }

    pub fn show_main_tabs(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (label, tab) in [
                ("Editor", MainTab::Editor),
                ("Log", MainTab::Log),
                ("History", MainTab::History),
                ("Variants", MainTab::Variants),
            ] {
                let selected = self.main_tab == tab;
                let text = if selected {
                    RichText::new(label).strong().color(Color32::from_rgb(120, 200, 255))
                } else {
                    RichText::new(label)
                };
                if ui.button(text).clicked() {
                    self.main_tab = tab;
                    if tab == MainTab::History {
                        self.reload_sessions();
                    }
                    if tab == MainTab::Variants {
                        self.reload_variants();
                    }
                    if tab == MainTab::Log {
                        self.reload_blocks();
                    }
                }
            }
        });
        if let Some(err) = &self.db_error {
            ui.colored_label(Color32::RED, format!("DB error: {err}"));
        } else if !self.db_path_display.is_empty() {
            ui.label(
                RichText::new(format!("db: {}", self.db_path_display))
                    .small()
                    .weak(),
            );
        }
    }

    pub fn show_log_tab(&mut self, ui: &mut Ui, status: &mut String) {
        if self.db.is_none() {
            ui.label("Database not available.");
            return;
        }

        ui.heading("Log session");
        ui.add_space(8.0);

        // Block picker / create
        ui.horizontal(|ui| {
            ui.label("Block:");
            egui::ComboBox::from_id_source("log_block")
                .selected_text(
                    self.blocks
                        .iter()
                        .find(|b| Some(b.id) == self.selected_block_id)
                        .map(|b| b.name.as_str())
                        .unwrap_or("(none)"),
                )
                .show_ui(ui, |ui| {
                    for b in &self.blocks {
                        ui.selectable_value(&mut self.selected_block_id, Some(b.id), &b.name);
                    }
                });
        });

        ui.collapsing("New block", |ui| {
            ui.horizontal(|ui| {
                ui.label("slug");
                ui.text_edit_singleline(&mut self.new_block_slug);
                ui.label("name");
                ui.text_edit_singleline(&mut self.new_block_name);
            });
            ui.horizontal(|ui| {
                ui.label("split");
                for (label, split) in [
                    ("upper/lower", Split::UpperLower),
                    ("full body", Split::FullBody),
                    ("A/P", Split::AnteriorPosterior),
                    ("other", Split::Other),
                ] {
                    ui.radio_value(&mut self.new_block_split, split, label);
                }
                if ui.button("Create block").clicked() {
                    match self.create_block() {
                        Ok(msg) => *status = msg,
                        Err(e) => *status = e,
                    }
                }
            });
        });

        ui.horizontal(|ui| {
            ui.label("Week:");
            ui.add(egui::TextEdit::singleline(&mut self.week).desired_width(40.0));
            ui.label("Day:");
            ui.text_edit_singleline(&mut self.day_label);
            if ui.button("New / load session").clicked() {
                match self.ensure_session() {
                    Ok(msg) => *status = msg,
                    Err(e) => *status = e,
                }
            }
            if let Some(id) = self.session_id {
                ui.label(format!("session #{id}"));
            }
        });

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Add exercise:");
            let resp = ui.text_edit_singleline(&mut self.search);
            if resp.changed() {
                self.run_search();
            }
        });
        if !self.search_results.is_empty() {
            ui.group(|ui| {
                for v in self.search_results.clone() {
                    if ui.button(format!("{} ({})", v.display, v.slug)).clicked() {
                        self.add_variant_draft(&v);
                        self.search.clear();
                        self.search_results.clear();
                    }
                }
            });
        }

        ui.horizontal(|ui| {
            ui.label("Untracked:");
            ui.add(egui::TextEdit::singleline(&mut self.untracked_n).desired_width(30.0));
            ui.label("sets of");
            ui.text_edit_singleline(&mut self.untracked_name);
            if ui.button("Add").clicked()
                && let Ok(n) = self.untracked_n.parse::<i32>()
                && !self.untracked_name.trim().is_empty()
            {
                self.drafts.push(DraftEntry {
                    variant_id: None,
                    name_raw: self.untracked_name.trim().to_string(),
                    display: self.untracked_name.trim().to_string(),
                    settings: String::new(),
                    tracked: false,
                    untracked_sets: n,
                    last_label: String::new(),
                    sets: Vec::new(),
                });
                self.untracked_name.clear();
            }
        });

        ui.add_space(8.0);
        let mut remove_idx = None;
        for (i, draft) in self.drafts.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.strong(format!("{}. {}", i + 1, draft.display));
                    if !draft.last_label.is_empty() {
                        ui.label(
                            RichText::new(&draft.last_label)
                                .italics()
                                .color(Color32::LIGHT_GRAY),
                        );
                    }
                    if ui.small_button("✕").clicked() {
                        remove_idx = Some(i);
                    }
                });
                if draft.tracked {
                    ui.horizontal(|ui| {
                        ui.label("settings");
                        ui.text_edit_singleline(&mut draft.settings);
                    });
                    for (si, set) in draft.sets.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("set {}", si + 1));
                            ui.checkbox(&mut set.logged, "logged");
                            ui.label("w");
                            ui.add(egui::TextEdit::singleline(&mut set.weight).desired_width(50.0));
                            ui.label("r");
                            ui.add(egui::TextEdit::singleline(&mut set.reps).desired_width(40.0));
                            ui.label("rir");
                            ui.add(egui::TextEdit::singleline(&mut set.rir_low).desired_width(30.0));
                            ui.label("-");
                            ui.add(egui::TextEdit::singleline(&mut set.rir_high).desired_width(30.0));
                            ui.label("note");
                            ui.text_edit_singleline(&mut set.comment);
                        });
                    }
                    if ui.button("+ set").clicked() {
                        draft.sets.push(DraftSet {
                            weight: String::new(),
                            reps: String::new(),
                            rir_low: String::new(),
                            rir_high: String::new(),
                            comment: String::new(),
                            logged: true,
                        });
                    }
                } else {
                    ui.label(format!("untracked: {} sets", draft.untracked_sets));
                }
            });
        }
        if let Some(i) = remove_idx {
            self.drafts.remove(i);
        }

        ui.add_space(10.0);
        if ui.button(RichText::new("Save session").strong()).clicked() {
            match self.save_session() {
                Ok(msg) => {
                    *status = msg;
                    self.reload_sessions();
                }
                Err(e) => *status = e,
            }
        }
    }

    pub fn show_history_tab(&mut self, ui: &mut Ui, status: &mut String) {
        if self.db.is_none() {
            ui.label("Database not available.");
            return;
        }
        ui.heading("History");
        if ui.button("Refresh").clicked() {
            self.reload_sessions();
        }
        ui.add_space(8.0);

        let mut open_id = None;
        let mut edit_id = None;
        for s in &self.sessions {
            let label = format!(
                "{} · wk {} · {} · #{} ({} entries)",
                s.block_name,
                s.week,
                s.session.day_label.as_deref().unwrap_or("?"),
                s.session.id,
                s.entry_count
            );
            ui.horizontal(|ui| {
                ui.label(&label);
                if ui.button("Show").clicked() {
                    open_id = Some(s.session.id);
                }
                if ui.button("Edit in Log").clicked() {
                    edit_id = Some(s.session.id);
                }
            });
        }
        if let Some(id) = edit_id
            && let Some(summary) = self.sessions.iter().find(|s| s.session.id == id).cloned()
        {
            self.jump_to_log(&summary);
            *status = format!("loaded session #{id} into Log");
        }

        if let Some(id) = open_id {
            match self.with_db(|db| db.get_session_detail(id)) {
                Ok(Some(detail)) => {
                    let mut text = format!(
                        "Session {} — {} week {} — {}\n",
                        detail.session.id,
                        detail.block_name,
                        detail.week,
                        detail.session.day_label.as_deref().unwrap_or("?")
                    );
                    for ed in &detail.entries {
                        text.push_str(&format!(
                            "\n{}. {}\n",
                            ed.entry.position,
                            ed.variant_display
                                .as_deref()
                                .or(ed.entry.name.as_deref())
                                .unwrap_or(&ed.entry.name_raw)
                        ));
                        if !ed.entry.tracked {
                            text.push_str(&format!(
                                "  untracked {} sets\n",
                                ed.entry.untracked_sets.unwrap_or(0)
                            ));
                        }
                        for set in &ed.sets {
                            if set.logged {
                                text.push_str(&format!(
                                    "  set {} — {:?}×{:?} rir {:?}-{:?} {:?}\n",
                                    set.set_number,
                                    set.weight,
                                    set.reps,
                                    set.rir_low,
                                    set.rir_high,
                                    set.comment
                                ));
                            } else {
                                text.push_str(&format!("  set {} — (blank)\n", set.set_number));
                            }
                        }
                    }
                    self.history_detail = Some(text);
                }
                Ok(None) => *status = format!("session {id} not found"),
                Err(e) => *status = e,
            }
        }

        if let Some(detail) = &self.history_detail {
            ui.separator();
            ui.label(RichText::new(detail).monospace());
        }
    }

    pub fn show_variants_tab(&mut self, ui: &mut Ui, status: &mut String) {
        if self.db.is_none() {
            ui.label("Database not available.");
            return;
        }
        ui.heading("Variants");
        ui.collapsing("New variant", |ui| {
            ui.horizontal(|ui| {
                ui.label("slug");
                ui.text_edit_singleline(&mut self.new_variant_slug);
                ui.label("display");
                ui.text_edit_singleline(&mut self.new_variant_display);
                if ui.button("Create").clicked() {
                    match self.create_variant() {
                        Ok(msg) => {
                            *status = msg;
                            self.reload_variants();
                        }
                        Err(e) => *status = e,
                    }
                }
            });
        });

        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .id_source("variants_list")
            .max_height(200.0)
            .show(ui, |ui| {
                for v in &self.variants {
                    let selected = Some(v.id) == self.selected_variant_id;
                    if ui.selectable_label(selected, format!("{} — {}", v.display, v.slug)).clicked()
                    {
                        self.selected_variant_id = Some(v.id);
                        self.edit_display = v.display.clone();
                        self.edit_family = v.family.clone().unwrap_or_default();
                        self.edit_gym = v.gym.clone().unwrap_or_default();
                        self.edit_joint_action = v.joint_action.clone().unwrap_or_default();
                        self.edit_settings = v.default_settings.clone().unwrap_or_default();
                    }
                }
            });

        if self.selected_variant_id.is_some() {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("display");
                ui.text_edit_singleline(&mut self.edit_display);
            });
            ui.horizontal(|ui| {
                ui.label("family");
                ui.text_edit_singleline(&mut self.edit_family);
                ui.label("gym");
                ui.text_edit_singleline(&mut self.edit_gym);
            });
            ui.horizontal(|ui| {
                ui.label("joint_action");
                ui.text_edit_singleline(&mut self.edit_joint_action);
            });
            ui.horizontal(|ui| {
                ui.label("default_settings");
                ui.text_edit_singleline(&mut self.edit_settings);
            });
            if ui.button("Save variant").clicked() {
                match self.save_variant() {
                    Ok(msg) => {
                        *status = msg;
                        self.reload_variants();
                    }
                    Err(e) => *status = e,
                }
            }
        }
    }

    fn create_block(&mut self) -> Result<String, String> {
        let slug = self.new_block_slug.trim().to_string();
        let name = self.new_block_name.trim().to_string();
        if slug.is_empty() || name.is_empty() {
            return Err("block slug and name required".into());
        }
        let id = self.with_db(|db| {
            db.create_block(&NewBlock {
                slug: slug.clone(),
                name: name.clone(),
                split: self.new_block_split,
                label: None,
                started_on: None,
            })
        })?;
        self.new_block_slug.clear();
        self.new_block_name.clear();
        self.reload_blocks();
        self.selected_block_id = Some(id);
        Ok(format!("created block {name} (#{id})"))
    }

    fn ensure_session(&mut self) -> Result<String, String> {
        let block_id = self
            .selected_block_id
            .ok_or_else(|| "select a block first".to_string())?;
        let week: i32 = self
            .week
            .trim()
            .parse()
            .map_err(|_| "week must be an integer".to_string())?;
        let day = self.day_label.trim().to_string();
        if day.is_empty() {
            return Err("day label required".into());
        }

        let (cycle_id, session_id) = self.with_db(|db| {
            let cycle = db.get_or_create_cycle(block_id, week, Some(&format!("Week {week}")), Source::App)?;
            // Reuse existing session with same day_label in this cycle if present
            let sessions = db.list_sessions()?;
            if let Some(existing) = sessions.iter().find(|s| {
                s.session.cycle_id == cycle.id
                    && s.session.day_label.as_deref() == Some(day.as_str())
            }) {
                return Ok((cycle.id, existing.session.id));
            }
            let index = sessions
                .iter()
                .filter(|s| s.session.cycle_id == cycle.id)
                .count() as i32;
            let sid = db.create_session(&NewSession {
                cycle_id: cycle.id,
                day_number: None,
                day_label: Some(day.clone()),
                comment: None,
                header_raw: Some(day.clone()),
                implicit: false,
                index,
                notes: Vec::new(),
                performed_on: None,
                date_precision: DatePrecision::Unknown,
                source: Source::App,
            })?;
            Ok((cycle.id, sid))
        })?;

        let _ = cycle_id;
        self.session_id = Some(session_id);
        // Load existing entries into drafts
        self.load_session_drafts(session_id)?;
        Ok(format!("session #{session_id} ready"))
    }

    fn load_session_drafts(&mut self, session_id: i64) -> Result<(), String> {
        let detail = self
            .with_db(|db| db.get_session_detail(session_id))?
            .ok_or_else(|| "session not found".to_string())?;
        self.drafts.clear();
        for ed in detail.entries {
            let mut sets = Vec::new();
            for s in ed.sets {
                sets.push(DraftSet {
                    weight: s.weight.map(|w| w.to_string()).unwrap_or_default(),
                    reps: s.reps.map(|r| r.to_string()).unwrap_or_default(),
                    rir_low: s.rir_low.map(|r| r.to_string()).unwrap_or_default(),
                    rir_high: s.rir_high.map(|r| r.to_string()).unwrap_or_default(),
                    comment: s.comment.unwrap_or_default(),
                    logged: s.logged,
                });
            }
            self.drafts.push(DraftEntry {
                variant_id: ed.entry.variant_id,
                name_raw: ed.entry.name_raw,
                display: ed
                    .variant_display
                    .or(ed.entry.name)
                    .unwrap_or_else(|| "entry".into()),
                settings: ed.entry.settings.unwrap_or_default(),
                tracked: ed.entry.tracked,
                untracked_sets: ed.entry.untracked_sets.unwrap_or(0),
                last_label: String::new(),
                sets,
            });
        }
        Ok(())
    }

    fn jump_to_log(&mut self, summary: &SessionSummary) {
        self.main_tab = MainTab::Log;
        self.selected_block_id = self
            .blocks
            .iter()
            .find(|b| b.slug == summary.block_slug)
            .map(|b| b.id);
        self.week = summary.week.to_string();
        self.day_label = summary.session.day_label.clone().unwrap_or_default();
        self.session_id = Some(summary.session.id);
        let _ = self.load_session_drafts(summary.session.id);
    }

    fn run_search(&mut self) {
        let q = self.search.trim();
        if q.is_empty() {
            self.search_results.clear();
            return;
        }
        if let Ok(results) = self.with_db(|db| db.search_variants(q)) {
            self.search_results = results;
        }
    }

    fn add_variant_draft(&mut self, v: &Variant) {
        let last_label = self
            .with_db(|db| db.last_performance(v.id))
            .ok()
            .flatten()
            .map(|lp| format_last(&lp.sets))
            .unwrap_or_default();

        let mut sets = vec![
            DraftSet {
                weight: String::new(),
                reps: String::new(),
                rir_low: String::new(),
                rir_high: String::new(),
                comment: String::new(),
                logged: true,
            },
            DraftSet {
                weight: String::new(),
                reps: String::new(),
                rir_low: String::new(),
                rir_high: String::new(),
                comment: String::new(),
                logged: true,
            },
        ];

        if let Ok(Some(lp)) = self.with_db(|db| db.last_performance(v.id))
            && !lp.sets.is_empty()
        {
            // A set left blank last time stays blank, so saving untouched doesn't
            // record an empty set as logged.
            sets = lp
                .sets
                .iter()
                .map(|s| DraftSet {
                    weight: s.weight.filter(|_| s.logged).map(|w| w.to_string()).unwrap_or_default(),
                    reps: s.reps.filter(|_| s.logged).map(|r| r.to_string()).unwrap_or_default(),
                    rir_low: s.rir_low.filter(|_| s.logged).map(|r| r.to_string()).unwrap_or_default(),
                    rir_high: s.rir_high.filter(|_| s.logged).map(|r| r.to_string()).unwrap_or_default(),
                    comment: String::new(),
                    logged: s.logged,
                })
                .collect();
        }

        self.drafts.push(DraftEntry {
            variant_id: Some(v.id),
            name_raw: v.display.clone(),
            display: v.display.clone(),
            settings: v.default_settings.clone().unwrap_or_default(),
            tracked: true,
            untracked_sets: 0,
            last_label,
            sets,
        });
    }

    fn save_session(&mut self) -> Result<String, String> {
        let session_id = self
            .session_id
            .ok_or_else(|| "create or load a session first".to_string())?;
        self.with_db(|db| {
            db.clear_session_entries(session_id)?;
            for (i, draft) in self.drafts.iter().enumerate() {
                let sets: Vec<NewSet> = if draft.tracked {
                    draft
                        .sets
                        .iter()
                        .enumerate()
                        .map(|(si, s)| NewSet {
                            set_number: (si + 1) as i32,
                            weight: parse_f64(&s.weight),
                            weight_inferred: false,
                            weight_note: None,
                            reps: parse_f64(&s.reps),
                            rir_low: parse_f64(&s.rir_low),
                            rir_high: parse_f64(&s.rir_high).or_else(|| parse_f64(&s.rir_low)),
                            logged: s.logged,
                            comment: nonempty(&s.comment),
                            raw: None,
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                db.add_entry_with_sets(
                    &NewEntry {
                        session_id,
                        variant_id: draft.variant_id,
                        position: (i + 1) as i32,
                        tracked: draft.tracked,
                        name: Some(draft.display.clone()),
                        name_raw: draft.name_raw.clone(),
                        qualifiers: Vec::new(),
                        untracked_sets: if draft.tracked {
                            None
                        } else {
                            Some(draft.untracked_sets)
                        },
                        settings: nonempty(&draft.settings),
                        comment: None,
                        raw: None,
                        excluded: false,
                    },
                    &sets,
                )?;
            }
            Ok(())
        })?;
        Ok(format!(
            "saved {} entries to session #{session_id}",
            self.drafts.len()
        ))
    }

    fn create_variant(&mut self) -> Result<String, String> {
        let slug = self.new_variant_slug.trim().to_string();
        let display = self.new_variant_display.trim().to_string();
        if slug.is_empty() || display.is_empty() {
            return Err("slug and display required".into());
        }
        let id = self.with_db(|db| {
            db.create_variant(&NewVariant {
                slug: slug.clone(),
                display: display.clone(),
                family: None,
                gym: None,
                status: AliasStatus::Confirmed,
                joint_action: None,
                equipment: None,
                load_unit: "lb".to_string(),
                default_settings: None,
            })
        })?;
        self.with_db(|db| db.upsert_alias(&display.to_lowercase(), id, AliasStatus::Confirmed))?;
        self.new_variant_slug.clear();
        self.new_variant_display.clear();
        Ok(format!("created variant {display}"))
    }

    fn save_variant(&mut self) -> Result<String, String> {
        let id = self
            .selected_variant_id
            .ok_or_else(|| "no variant selected".to_string())?;
        let mut v = self
            .with_db(|db| db.get_variant(id))?
            .ok_or_else(|| "variant missing".to_string())?;
        v.display = self.edit_display.trim().to_string();
        v.family = nonempty(&self.edit_family);
        v.gym = nonempty(&self.edit_gym);
        v.joint_action = nonempty(&self.edit_joint_action);
        v.default_settings = nonempty(&self.edit_settings);
        self.with_db(|db| db.update_variant(&v))?;
        Ok(format!("updated {}", v.slug))
    }
}

fn parse_f64(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        t.parse().ok()
    }
}

fn nonempty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn format_last(sets: &[hypertrophy_archive::log::SetLog]) -> String {
    let Some(best) = sets.iter().find(|s| s.logged && s.weight.is_some()) else {
        return String::new();
    };
    let w = best.weight.unwrap();
    let reps = best.reps.map(|r| format!("{r}")).unwrap_or_else(|| "?".into());
    let rir = match (best.rir_low, best.rir_high) {
        (Some(lo), Some(hi)) if (lo - hi).abs() < f64::EPSILON => format!("@{lo}"),
        (Some(lo), Some(hi)) => format!("@{lo}-{hi}"),
        (Some(lo), None) => format!("@{lo}"),
        _ => String::new(),
    };
    format!("Last: {w}×{reps} {rir}")
}
