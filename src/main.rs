mod agents;
mod config;
mod helper;
mod init;
mod status;
mod sync;
mod tui;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;

use config::Config;
use init::InitOk;
use sync::SyncOk;

#[derive(Parser)]
#[command(name = "hana", version, about = "🌸 Sync AI coding agent skills & instructions from a single source")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create config file (.agents/hana.toml)
    Init {
        /// Target global config (~/.agents/hana.toml)
        #[arg(short, long)]
        global: bool,

        /// Overwrite existing files
        #[arg(short, long)]
        force: bool,

        /// Preview without making changes
        #[arg(short, long)]
        dry_run: bool,
    },

    /// Sync skills and instructions across agents
    Sync {
        /// Target global config (~/.agents/hana.toml)
        #[arg(short, long)]
        global: bool,

        /// Overwrite existing files
        #[arg(short, long)]
        force: bool,

        /// Preview without making changes
        #[arg(short, long)]
        dry_run: bool,
    },

    /// Show current sync status
    Status {
        /// Target global config (~/.agents/hana.toml)
        #[arg(short, long)]
        global: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Init {
            global,
            force,
            dry_run,
        } => run_init(init::InitOptions {
            global,
            force,
            dry_run,
        }),

        Commands::Sync {
            global,
            force,
            dry_run,
        } => run_sync(sync::SyncOptions {
            global,
            force,
            dry_run,
        }),

        Commands::Status { global } => run_status(global),
    };

    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

fn resolve_base_dir(global: bool) -> Result<PathBuf, String> {
    if global {
        dirs::home_dir().ok_or_else(|| "could not determine home directory".to_string())
    } else {
        Ok(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
}

// ── init ──

fn run_init(opts: init::InitOptions) -> i32 {
    let base_dir = match resolve_base_dir(opts.global) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{} {e}", "error:".red().bold());
            return 1;
        }
    };

    match init::run(&opts, &base_dir) {
        Ok(InitOk::Created { path }) => {
            print!("{}", tui::header("init", false));
            let rows = vec![format!(
                "{}  {}",
                "created".green(),
                path.display().to_string().bold()
            )];
            print!("{}", tui::section("Config", &rows));
            print!("{}", tui::footer_done());
            0
        }
        Ok(InitOk::DryRun { path, content }) => {
            print!("{}", tui::header("init", true));
            let rows = vec![format!(
                "{}  {}",
                "would create".cyan(),
                path.bold()
            )];
            print!("{}", tui::section("Config", &rows));
            println!("{}", content.dimmed());
            0
        }
        Err(e) => {
            eprintln!("{} {e}", "error:".red().bold());
            1
        }
    }
}

// ── sync ──

fn run_sync(opts: sync::SyncOptions) -> i32 {
    let base_dir = match resolve_base_dir(opts.global) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{} {e}", "error:".red().bold());
            return 1;
        }
    };

    let config_path = base_dir.join(".agents/hana.toml");
    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} {e}", "error:".red().bold());
            eprintln!(
                "  run {} to create the config first.",
                "hana init".bold()
            );
            return 1;
        }
    };

    let result = sync::run(&config, &base_dir, &opts);

    print!("{}", tui::header("sync", opts.dry_run));
    print_sync_result(&result);
    0
}

fn print_sync_result(result: &SyncOk) {
    let has_skills = !result.skills_collected.is_empty() || !result.skills_linked.is_empty();
    let has_instructions = result.instructions_collected.is_some()
        || !result.instructions_linked.is_empty()
        || !result.instructions_skipped.is_empty();
    let has_cleanup = !result.cleaned.is_empty();
    let has_warnings = !result.warnings.is_empty();

    // Skills
    if has_skills {
        let mut table_rows: Vec<Vec<String>> = Vec::new();

        for (name, agent) in &result.skills_collected {
            table_rows.push(vec![
                tui::label_collected("collected"),
                name.bold().to_string(),
                format!("← {}", agent),
            ]);
        }

        if !result.skills_linked.is_empty() {
            let mut by_skill: std::collections::HashMap<&str, Vec<&str>> =
                std::collections::HashMap::new();
            for (skill, agent) in &result.skills_linked {
                by_skill.entry(skill).or_default().push(agent);
            }
            let mut skills: Vec<_> = by_skill.into_iter().collect();
            skills.sort_by_key(|(name, _)| *name);
            for (skill, agents) in &skills {
                table_rows.push(vec![
                    tui::label_symlinked("symlinked"),
                    skill.bold().to_string(),
                    format!("→ {}", agents.join(", ")),
                ]);
            }
        }

        let rows = tui::table(&table_rows);
        print!("{}", tui::section("Skills", &rows));
    }

    // Instructions
    if has_instructions {
        let mut table_rows: Vec<Vec<String>> = Vec::new();

        if let Some((file, agent)) = &result.instructions_collected {
            table_rows.push(vec![
                tui::label_collected("collected"),
                file.bold().to_string(),
                format!("→ {} (from {})", "AGENTS.md".bold(), agent),
            ]);
        }

        for agent in &result.instructions_linked {
            table_rows.push(vec![
                tui::label_symlinked("symlinked"),
                "AGENTS.md".bold().to_string(),
                format!("→ {agent}"),
            ]);
        }

        if !result.instructions_skipped.is_empty() {
            table_rows.push(vec![
                tui::label_native("native"),
                "AGENTS.md".bold().to_string(),
                tui::label_native(&result.instructions_skipped.join(", ")),
            ]);
        }

        let rows = tui::table(&table_rows);
        print!("{}", tui::section("Instructions", &rows));
    }

    // Cleanup
    if has_cleanup {
        let rows: Vec<String> = result
            .cleaned
            .iter()
            .map(|path| {
                format!(
                    "{}  {} {}",
                    tui::label_removed("removed"),
                    path.display(),
                    "(broken symlink)".dimmed()
                )
            })
            .collect();
        print!("{}", tui::section("Cleanup", &rows));
    }

    // Warnings
    if has_warnings {
        let rows: Vec<String> = result
            .warnings
            .iter()
            .map(|w| tui::label_warning(&format!("⚠ {w}")))
            .collect();
        print!("{}", tui::section("Warnings", &rows));
    }

    // Footer
    if !has_skills && !has_instructions && !has_cleanup {
        print!("{}", tui::footer_no_changes());
    } else {
        print!("{}", tui::footer_done());
    }
}

// ── status ──

fn run_status(global: bool) -> i32 {
    let base_dir = match resolve_base_dir(global) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{} {e}", "error:".red().bold());
            return 1;
        }
    };

    let config_path = base_dir.join(".agents/hana.toml");
    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} {e}", "error:".red().bold());
            eprintln!(
                "  run {} to create the config first.",
                "hana init".bold()
            );
            return 1;
        }
    };

    let result = status::run(&config, &base_dir, global);
    print!("{}", tui::header("status", false));
    print!("{}", format_status(&result));
    0
}

fn format_status(result: &status::StatusOk) -> String {
    use status::{InstructionState, SkillState};

    let mut out = String::new();

    // Skills
    if result.skills.is_empty() {
        let rows = vec![tui::label_native("(none)")];
        out.push_str(&tui::section("Skills", &rows));
    } else {
        let mut table_rows: Vec<Vec<String>> = Vec::new();
        for skill in &result.skills {
            let mut row = vec![skill.name.bold().to_string()];
            for (agent, state) in &skill.agents {
                row.push(match state {
                    SkillState::Synced => tui::badge_ok(agent),
                    SkillState::RealDir => tui::badge_warn(&format!("{agent} (real dir)")),
                    SkillState::BrokenSymlink => tui::badge_broken(&format!("{agent} (broken)")),
                    SkillState::Missing => tui::badge_err(agent),
                    SkillState::WrongTarget => {
                        tui::badge_warn(&format!("{agent} (wrong target)"))
                    }
                });
            }
            table_rows.push(row);
        }
        let rows = tui::table(&table_rows);
        out.push_str(&tui::section("Skills", &rows));
    }

    // Instructions
    {
        let mut table_rows: Vec<Vec<String>> = Vec::new();

        // Source row
        if result.instructions.source_exists {
            table_rows.push(vec![
                result.instructions.source.bold().to_string(),
                tui::badge_ok("source"),
            ]);
        } else {
            table_rows.push(vec![
                result.instructions.source.bold().to_string(),
                tui::badge_err("missing"),
            ]);
        }

        // Agent rows
        for (agent, state) in &result.instructions.agents {
            table_rows.push(vec![
                agent.to_string(),
                match state {
                    InstructionState::Synced => tui::badge_ok("symlinked"),
                    InstructionState::DirectRead => tui::badge_info("native"),
                    InstructionState::RealFile => tui::badge_warn("real file (conflict)"),
                    InstructionState::Missing => tui::badge_err("missing"),
                    InstructionState::Disabled => tui::badge_skip("disabled"),
                },
            ]);
        }

        let rows = tui::table(&table_rows);
        out.push_str(&tui::section("Instructions", &rows));
    }

    out
}
