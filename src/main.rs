mod agents;
mod config;
mod error;
mod helper;
mod init;
mod status;
mod sync;

use std::collections::HashMap;

use clap::{Parser, Subcommand};

use error::{InitOk, SyncOk};

#[derive(Parser)]
#[command(name = "hana", version, about = "🌸 AI 코딩 에이전트 스킬/지침 동기화")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 설정 파일 생성 (.agents/hana.toml)
    Init {
        /// 글로벌 설정 (~/.agents/hana.toml) 대상
        #[arg(long)]
        global: bool,

        /// 기존 파일 덮어쓰기
        #[arg(long)]
        force: bool,

        /// 실제 변경 없이 미리보기
        #[arg(long)]
        dry_run: bool,
    },

    /// 스킬과 지침 동기화
    Sync {
        /// 글로벌 설정 (~/.agents/hana.toml) 대상
        #[arg(long)]
        global: bool,

        /// 기존 파일 덮어쓰기
        #[arg(long)]
        force: bool,

        /// 실제 변경 없이 미리보기
        #[arg(long)]
        dry_run: bool,
    },

    /// 동기화 상태 확인
    Status {
        /// 글로벌 설정 (~/.agents/hana.toml) 대상
        #[arg(long)]
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

fn run_init(opts: init::InitOptions) -> i32 {
    match init::run(&opts) {
        Ok(InitOk::Created { path }) => {
            println!("🌸 생성 완료: {}", path.display());
            0
        }
        Ok(InitOk::DryRun { path, content }) => {
            println!("🌸 {path} 에 생성될 내용:\n");
            print!("{content}");
            0
        }
        Err(e) => {
            eprintln!("🌸 {e}");
            1
        }
    }
}

fn run_sync(opts: sync::SyncOptions) -> i32 {
    let result = match sync::run(&opts) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("🌸 {e}");
            if matches!(e, error::SyncError::Config(_)) {
                eprintln!("   hana init 으로 설정 파일을 먼저 생성하세요.");
            }
            return 1;
        }
    };

    if opts.dry_run {
        println!("🌸 hana sync (dry-run)\n");
    } else {
        println!("🌸 hana sync\n");
    }

    print_sync_result(&result, &opts);
    0
}

fn print_sync_result(result: &SyncOk, _opts: &sync::SyncOptions) {
    for (name, agent) in &result.skills_collected {
        println!("  🆕 {name} ({agent}에서 수집)");
    }

    if !result.skills_linked.is_empty() {
        println!("스킬 동기화:");
        let mut by_skill: HashMap<&str, Vec<&str>> = HashMap::new();
        for (skill, agent) in &result.skills_linked {
            by_skill.entry(skill).or_default().push(agent);
        }
        for (skill, agents) in &by_skill {
            println!("  ✅ {skill} → {}", agents.join(", "));
        }
    }

    if !result.instructions_linked.is_empty() || !result.instructions_skipped.is_empty() {
        println!("지침 동기화:");
        for agent in &result.instructions_linked {
            println!("  ✅ {agent}");
        }
        if !result.instructions_skipped.is_empty() {
            println!(
                "  ℹ️  AGENTS.md ({} 직접 사용)",
                result.instructions_skipped.join(", ")
            );
        }
    }

    if !result.cleaned.is_empty() {
        println!("정리:");
        for path in &result.cleaned {
            println!("  🗑️  {}", path.display());
        }
    }

    for warn in &result.warnings {
        eprintln!("  ⚠️  {warn}");
    }

    if result.skills_linked.is_empty()
        && result.skills_collected.is_empty()
        && result.instructions_linked.is_empty()
        && result.cleaned.is_empty()
    {
        println!("변경 없음. 모두 동기화 상태입니다.");
    }

    println!("\n완료!");
}

fn run_status(global: bool) -> i32 {
    let result = match status::run(global) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("🌸 {e}");
            if matches!(e, error::StatusError::Config(_)) {
                eprintln!("   hana init 으로 설정 파일을 먼저 생성하세요.");
            }
            return 1;
        }
    };

    print!("{}", status::format_result(&result));
    0
}
