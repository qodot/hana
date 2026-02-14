mod agents;
mod config;
mod error;
mod init;
mod status;
mod sync;

use clap::{Parser, Subcommand};

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

    let result = match cli.command {
        Commands::Init {
            global,
            force,
            dry_run,
        } => init::run(&init::InitOptions {
            global,
            force,
            dry_run,
        }),

        Commands::Sync {
            global,
            force,
            dry_run,
        } => sync::run(&sync::SyncOptions {
            global,
            force,
            dry_run,
        }),

        Commands::Status { global } => status::run(global),
    };

    if let Err(code) = result {
        std::process::exit(code);
    }
}
