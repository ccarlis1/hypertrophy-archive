use std::path::PathBuf;

use clap::{Parser, Subcommand};
use hypertrophy_archive::db::{self, Db};
use hypertrophy_archive::import::{format_report, import_grouped, load_grouped, ImportOptions};

#[derive(Parser)]
#[command(name = "ha", about = "Hypertrophy Archive workout log CLI")]
struct Cli {
    /// Override database path (default: Application Support / HA_DATA_DIR)
    #[arg(long, global = true)]
    db: Option<PathBuf>,

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
    }
    Ok(())
}
