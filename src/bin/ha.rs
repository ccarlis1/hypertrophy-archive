use std::path::PathBuf;

use clap::{Parser, Subcommand};
use hypertrophy_archive::db::{self, Db};

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

fn open_db(path: Option<PathBuf>) -> Result<Db, String> {
    match path {
        Some(p) => Db::open(p),
        None => Db::open_default(),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let db = open_db(cli.db)?;

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
                    println!("{}. untracked: {n} sets of {label}", e.position);
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
            println!(
                "seeded {n} variants into {}",
                db.path().display()
            );
        }
    }
    Ok(())
}
