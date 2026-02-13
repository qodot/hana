use std::fs;
use std::path::PathBuf;
use std::process;

const DEFAULT_CONFIG: &str = r#"# hana - AI 코딩 에이전트 동기화 설정
# https://github.com/qodot/hana

[skills]
source = ".agents/skills"

[instructions]
source = "AGENTS.md"

[targets.claude]
skills = true
instructions = true

[targets.codex]
skills = true
instructions = true

[targets.pi]
skills = true
instructions = true

[targets.opencode]
skills = true
instructions = true
"#;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("init") => cmd_init(&args[2..]),
        Some("--help" | "-h") | None => print_help(),
        Some(cmd) => {
            eprintln!("🌸 알 수 없는 명령어: {cmd}");
            eprintln!("   hana --help 로 사용법을 확인하세요.");
            process::exit(1);
        }
    }
}

fn cmd_init(args: &[String]) {
    let is_global = args.iter().any(|a| a == "--global");
    let is_dry_run = args.iter().any(|a| a == "--dry-run");

    if is_dry_run {
        let path = if is_global { "~/.agents/hana.toml" } else { ".agents/hana.toml" };
        println!("🌸 {path} 에 생성될 내용:\n");
        print!("{DEFAULT_CONFIG}");
        return;
    }

    let config_path = if is_global {
        let home = dirs::home_dir().unwrap_or_else(|| {
            eprintln!("🌸 홈 디렉토리를 찾을 수 없습니다.");
            process::exit(1);
        });
        home.join(".agents").join("hana.toml")
    } else {
        PathBuf::from(".agents").join("hana.toml")
    };

    if config_path.exists() {
        eprintln!("🌸 이미 존재합니다: {}", config_path.display());
        eprintln!("   덮어쓰려면 --force 옵션을 사용하세요.");
        if !args.iter().any(|a| a == "--force") {
            process::exit(1);
        }
    }

    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).unwrap_or_else(|e| {
                eprintln!("🌸 디렉토리 생성 실패: {e}");
                process::exit(1);
            });
        }
    }

    fs::write(&config_path, DEFAULT_CONFIG).unwrap_or_else(|e| {
        eprintln!("🌸 파일 생성 실패: {e}");
        process::exit(1);
    });

    println!("🌸 생성 완료: {}", config_path.display());
}

fn print_help() {
    println!(
        "🌸 hana - AI 코딩 에이전트 스킬/지침 동기화

사용법:
  hana <명령어> [옵션]

명령어:
  init      설정 파일 생성 (.agents/hana.toml)
  sync      스킬과 지침 동기화 (미구현)
  status    동기화 상태 확인 (미구현)

옵션:
  --global  글로벌 설정 (~/.agents/hana.toml) 대상
  --force   기존 파일 덮어쓰기
  -h, --help  도움말"
    );
}
