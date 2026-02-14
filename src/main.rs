mod config;
mod init;
mod status;
mod sync;

use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let result = match args.get(1).map(|s| s.as_str()) {
        Some("init") => init::run(&args[2..]),
        Some("sync") => sync::run(&args[2..]),
        Some("status") => status::run(&args[2..]),
        Some("--help" | "-h") | None => {
            print_help();
            Ok(())
        }
        Some(cmd) => {
            eprintln!("🌸 알 수 없는 명령어: {cmd}");
            eprintln!("   hana --help 로 사용법을 확인하세요.");
            Err(1)
        }
    };

    if let Err(code) = result {
        process::exit(code);
    }
}

fn print_help() {
    println!(
        "🌸 hana - AI 코딩 에이전트 스킬/지침 동기화

사용법:
  hana <명령어> [옵션]

명령어:
  init      설정 파일 생성 (.agents/hana.toml)
  sync      스킬과 지침 동기화
  status    동기화 상태 확인

옵션:
  --global  글로벌 설정 (~/.agents/hana.toml) 대상
  --force   기존 파일 덮어쓰기
  --dry-run 실제 변경 없이 미리보기
  -h, --help  도움말"
    );
}
