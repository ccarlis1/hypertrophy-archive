use std::io::Write;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use hypertrophy_archive::db::{self, Db};
use hypertrophy_archive::export::write_csv;
use hypertrophy_archive::import::{format_report, import_grouped, load_grouped, ImportOptions};
use hypertrophy_archive::log::{SetRow, Variant};
use hypertrophy_archive::metrics::{self, Params, SessionStrength};
use hypertrophy_archive::models::{load_archive, Archive};

#[derive(Parser)]
#[command(name = "ha", about = "Hypertrophy Archive workout log CLI")]
struct Cli {
    /// Override database path (default: Application Support / HA_DATA_DIR)
    #[arg(long, global = true)]
    db: Option<PathBuf>,

    /// Joint-action archive folder
    #[arg(long, global = true, default_value = "data")]
    data: PathBuf,

    /// Epley constant c in e1RM = w × (1 + r/c)
    #[arg(long, global = true, default_value_t = 30.0)]
    epley_c: f64,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Session commands
    Log {
        #[command(subcommand)]
        command: LogCommands,
    },
    /// Variant commands
    Variants {
        #[command(subcommand)]
        command: VariantCommands,
    },
    /// Import a grouped.json Notes export into the database
    Import {
        /// Path to exports/grouped.json
        path: PathBuf,
        /// Parse and report without writing
        #[arg(long)]
        dry_run: bool,
        /// Re-import even when source_hash matches
        #[arg(long)]
        force: bool,
        /// Verbose report (list inferred weights)
        #[arg(short = 'v', long)]
        verbose: bool,
    },
    /// Progress for a variant, joint action or muscle
    Stats {
        /// Variant slug (or a unique search match), joint-action slug, or muscle name
        target: String,
    },
    /// Volume, PRs and stalls for one week of a block
    Cycle { block: String, week: i32 },
    /// Export the log
    Export {
        #[command(subcommand)]
        command: ExportCommands,
    },
}

#[derive(Subcommand)]
enum ExportCommands {
    /// One row per set, with the session's e1RM and e5RM for that variant
    Csv {
        /// Output file (default: stdout)
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum LogCommands {
    /// Print a session with all entries and sets
    Show { session_id: i64 },
}

#[derive(Subcommand)]
enum VariantCommands {
    /// List all variants
    List,
    /// Link a variant slug to a joint-action slug
    Link {
        variant_slug: String,
        joint_action_slug: String,
    },
    /// Seed variants and aliases from a grouped.json file
    Seed { path: PathBuf },
}


fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let db_path = match cli.db {
        Some(p) => p,
        None => db::default_db_path()?,
    };
    // A dry-run import works on a throwaway copy, so the real file isn't created or
    // migrated. Declared before `db` so the copy outlives the connection.
    let scratch = match cli.command {
        Commands::Import { dry_run: true, .. } => Some(db::ScratchCopy::new(&db_path)?),
        _ => None,
    };
    let db = Db::open(scratch.as_ref().map_or(db_path.as_path(), |s| s.path()))?;

    match cli.command {
        Commands::Log {
            command: LogCommands::Show { session_id },
        } => {
            let detail = db
                .get_session_detail(session_id)?
                .ok_or_else(|| format!("session {session_id} not found"))?;
            println!(
                "Session {} — {} / week {} — {}",
                detail.session.id,
                detail.block_name,
                detail.week,
                detail
                    .session
                    .day_label
                    .as_deref()
                    .unwrap_or("(no day label)")
            );
            if let Some(title) = &detail.cycle_title {
                println!("Cycle title: {title}");
            }
            if let Some(on) = &detail.session.performed_on {
                println!("Performed on: {on}");
            }
            if let Some(comment) = &detail.session.comment {
                println!("Session comment: {comment}");
            }
            for note in &detail.session.notes {
                println!("Note: {note}");
            }
            println!();
            for ed in &detail.entries {
                let e = &ed.entry;
                let label = ed
                    .variant_display
                    .as_deref()
                    .or(e.name.as_deref())
                    .unwrap_or(&e.name_raw);
                if e.tracked {
                    print!("{}. {label}", e.position);
                    if let Some(settings) = &e.settings {
                        print!("  [{settings}]");
                    }
                    println!();
                    for set in &ed.sets {
                        if !set.logged {
                            println!("   set {} — (blank)", set.set_number);
                            continue;
                        }
                        let weight = set
                            .weight
                            .map(|w| format!("{w}"))
                            .unwrap_or_else(|| "—".to_string());
                        let reps = set
                            .reps
                            .map(|r| format!("{r}"))
                            .unwrap_or_else(|| "—".to_string());
                        let rir = match (set.rir_low, set.rir_high) {
                            (Some(lo), Some(hi)) if (lo - hi).abs() < f64::EPSILON => {
                                format!("@{lo}")
                            }
                            (Some(lo), Some(hi)) => format!("@{lo}-{hi}"),
                            (Some(lo), None) => format!("@{lo}+"),
                            _ => String::new(),
                        };
                        print!("   set {} — {weight}×{reps} {rir}", set.set_number);
                        if set.weight_inferred {
                            print!(" (inferred)");
                        }
                        if let Some(c) = &set.comment {
                            print!("  // {c}");
                        }
                        println!();
                    }
                } else {
                    let n = e.untracked_sets.unwrap_or(0);
                    let sets = if n == 1 { "set" } else { "sets" };
                    println!("{}. untracked: {n} {sets} of {label}", e.position);
                }
                if let Some(c) = &e.comment {
                    println!("   entry comment: {c}");
                }
            }
        }
        Commands::Variants {
            command: VariantCommands::List,
        } => {
            for v in db.list_variants()? {
                let ja = v.joint_action.as_deref().unwrap_or("-");
                println!(
                    "{:<32} {:<40} ja={} status={}",
                    v.slug,
                    v.display,
                    ja,
                    v.status.as_str()
                );
            }
        }
        Commands::Variants {
            command: VariantCommands::Link {
                variant_slug,
                joint_action_slug,
            },
        } => {
            db.link_variant(&variant_slug, &joint_action_slug)?;
            println!("linked {variant_slug} -> {joint_action_slug}");
        }
        Commands::Variants {
            command: VariantCommands::Seed { path },
        } => {
            let n = db.seed_variants_from_grouped(&path)?;
            println!("seeded {n} variants into {}", db.path().display());
        }
        Commands::Import {
            path,
            dry_run,
            force,
            verbose,
        } => {
            if !dry_run
                && let Some(bak) = db::backup(db.path(), db::BACKUPS_KEPT)?
            {
                println!("backed up database to {}", bak.display());
            }
            let data = load_grouped(&path)?;
            let report = import_grouped(
                &db,
                &data,
                &ImportOptions {
                    dry_run,
                    force,
                    verbose,
                },
            )?;
            if dry_run {
                println!("dry-run (no changes written to {})", db_path.display());
            } else {
                println!("imported into {}", db.path().display());
            }
            print!("{}", format_report(&report, verbose));
        }
        Commands::Stats { target } => {
            let p = params(cli.epley_c);
            let rows = db.set_rows()?;
            let archive = load_archive(&cli.data)?;
            stats(&db, &rows, &archive, &target, &p)?;
        }
        Commands::Cycle { block, week } => {
            let p = params(cli.epley_c);
            let b = db
                .get_block_by_slug(&block)?
                .ok_or_else(|| format!("block not found: {block}"))?;
            let c = db
                .get_cycle_by_block_week(b.id, week)?
                .ok_or_else(|| format!("{block} has no week {week}"))?;
            let rows = db.set_rows()?;
            let archive = load_archive(&cli.data)?;
            let s = metrics::cycle_summary(&rows, c.id, &archive, &p)
                .ok_or_else(|| format!("{block} week {week} has no sets"))?;
            print_cycle(&b.name, week, &s);
        }
        Commands::Export {
            command: ExportCommands::Csv { out },
        } => {
            let p = params(cli.epley_c);
            let rows = db.set_rows()?;
            let n = match &out {
                Some(path) => {
                    let mut f = std::io::BufWriter::new(
                        std::fs::File::create(path)
                            .map_err(|e| format!("create {}: {e}", path.display()))?,
                    );
                    let n = write_csv(&rows, &p, &mut f).map_err(|e| e.to_string())?;
                    f.flush().map_err(|e| e.to_string())?;
                    n
                }
                None => write_csv(&rows, &p, &mut std::io::stdout().lock())
                    .map_err(|e| e.to_string())?,
            };
            if let Some(path) = out {
                eprintln!("wrote {n} sets to {}", path.display());
            }
        }
    }
    Ok(())
}

fn params(epley_c: f64) -> Params {
    Params {
        epley_c,
        ..Params::default()
    }
}

fn stats(db: &Db, rows: &[SetRow], archive: &Archive, target: &str, p: &Params) -> Result<(), String> {
    let muscle = archive
        .values()
        .flat_map(|ja| &ja.target_muscle_groups)
        .find(|m| m.muscle_name.eq_ignore_ascii_case(target));
    if let Some(v) = db.get_variant_by_slug(target)? {
        print_variant(&v, rows, p);
        return Ok(());
    } else if archive.contains_key(target) {
        print_joint_action(target, rows, p);
        return Ok(());
    } else if let Some(m) = muscle {
        print_muscle(&m.muscle_name, rows, archive, p);
        return Ok(());
    }
    match db.search_variants(target)?.as_slice() {
        [v] => {
            print_variant(v, rows, p);
            Ok(())
        }
        [] => Err(format!(
            "{target:?} is not a variant, joint action (in {} joint actions) or muscle",
            archive.len()
        )),
        many => Err(format!(
            "{target:?} matches several variants: {}",
            many.iter().map(|v| v.slug.as_str()).collect::<Vec<_>>().join(", ")
        )),
    }
}

fn date(s: &SessionStrength) -> &str {
    s.performed_on.as_deref().unwrap_or("?")
}

fn print_variant(v: &Variant, rows: &[SetRow], p: &Params) {
    let h = metrics::variant_history(rows, v.id, p);
    println!(
        "{} ({}) — joint action {}",
        v.display,
        v.slug,
        v.joint_action.as_deref().unwrap_or("-")
    );
    let (Some(first), Some(last)) = (h.first(), h.last()) else {
        println!("no sessions with a weight and reps");
        return;
    };
    let best = h.iter().map(|s| s.e1rm).fold(f64::MIN, f64::max);
    let base = metrics::baseline(&h, p).unwrap_or(first.e1rm);
    println!(
        "{} sessions, {} to {}. e1RM baseline {:.1} (median of first {}), best {:.1}, latest {:.1} ({:+.1}%)",
        h.len(),
        date(first),
        date(last),
        base,
        p.baseline_sessions.min(h.len()),
        best,
        last.e1rm,
        last.pct_from_baseline
    );
    if last.stalled {
        println!("STALLED: {} sessions since the last e1RM best", last.since_best);
    }
    println!();
    println!(
        "{:<11} {:<16} {:>10} {:>14} {:>7} {:>7} {:>7}  ",
        "date", "block/week", "top set", "best set", "e1RM", "e5RM", "Δbase"
    );
    for s in &h {
        let flag = match (s.pr, s.stalled) {
            (true, _) => "PR",
            (_, true) => "stall",
            _ => "",
        };
        println!(
            "{:<11} {:<16} {:>10} {:>14} {:>7.1} {:>7.1} {:>+6.1}%  {flag}",
            date(s),
            format!("{} w{}", s.block_slug, s.week),
            format!("{}×{}", s.top_weight, s.top_reps),
            format!("{}×{} @{}", s.best_weight, s.best_reps, s.best_rir),
            s.e1rm,
            s.e5rm,
            s.pct_from_baseline
        );
    }
    println!();
    println!("Rep PRs (most reps at each weight)");
    for pr in metrics::rep_prs(rows, v.id) {
        println!(
            "  {:>7} × {:<5} {}",
            pr.weight,
            pr.reps,
            pr.performed_on.as_deref().unwrap_or("?")
        );
    }
}

fn print_joint_action(ja: &str, rows: &[SetRow], p: &Params) {
    println!("Joint action {ja}: e1RM % change from each variant's baseline");
    let series = metrics::joint_action_progress(rows, ja, p);
    if series.is_empty() {
        println!("no linked variants with sessions");
    }
    for s in series {
        let last = s.sessions.last().expect("non-empty");
        let best = s
            .sessions
            .iter()
            .map(|x| x.pct_from_baseline)
            .fold(f64::MIN, f64::max);
        println!(
            "  {:<40} {:>3} sessions  {} to {}  latest {:+6.1}%  best {:+6.1}%{}",
            s.variant.display,
            s.sessions.len(),
            date(&s.sessions[0]),
            date(last),
            last.pct_from_baseline,
            best,
            if last.stalled { "  STALLED" } else { "" }
        );
    }
}

fn print_muscle(muscle: &str, rows: &[SetRow], archive: &Archive, p: &Params) {
    let weeks = metrics::weekly_volume(rows, archive, p);
    println!("{muscle}: weekly hard sets (RIR ≤ {})", p.hard_set_max_rir);
    for w in &weeks {
        let Some(m) = w.muscles.iter().find(|(n, _)| n.eq_ignore_ascii_case(muscle)).map(|(_, m)| m)
        else {
            continue;
        };
        let divs: Vec<String> = m.divisions.iter().map(|(d, n)| format!("{d} {n}")).collect();
        println!(
            "  {:<11} {:<16} {:>5.1}  {}",
            w.started_on.as_deref().unwrap_or("?"),
            format!("{} w{}", w.block_slug, w.week),
            m.sets,
            divs.join(", ")
        );
    }
}

fn print_cycle(block_name: &str, week: i32, s: &metrics::CycleSummary) {
    let w = &s.volume;
    println!(
        "{block_name} week {week} ({}): {} hard sets, {} PRs, {} stalled",
        w.started_on.as_deref().unwrap_or("?"),
        w.hard_sets,
        s.pr_count(),
        s.stalled().count()
    );
    println!();
    println!("Hard sets per muscle");
    for (name, m) in &w.muscles {
        let divs: Vec<String> = m.divisions.iter().map(|(d, n)| format!("{d} {n}")).collect();
        println!("  {name:<16} {:>5.1}  {}", m.sets, divs.join(", "));
    }
    if !w.unmapped.is_empty() {
        let names: Vec<String> = w.unmapped.iter().map(|(v, n)| format!("{v} {n}")).collect();
        println!("  (no joint action: {})", names.join(", "));
    }
    println!();
    println!("Variants");
    for v in &s.variants {
        let l = &v.last;
        let flag = match (v.prs, l.stalled) {
            (n, _) if n > 0 => "PR".to_string(),
            (_, true) => format!("stalled ({} sessions)", l.since_best),
            _ => String::new(),
        };
        println!(
            "  {:<40} e1RM {:>6.1}  e5RM {:>6.1}  {:>+6.1}%  {flag}",
            v.variant.display, l.e1rm, l.e5rm, l.pct_from_baseline
        );
    }
}
