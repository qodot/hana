mod config;
mod init;
mod status;
mod sync;

use std::path::PathBuf;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let result = match args.get(1).map(|s| s.as_str()) {
        Some("init") => init::run(&args[2..]),
        Some("sync") => cmd_sync(&args[2..]),
        Some("status") => cmd_status(&args[2..]),
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

fn cmd_sync(args: &[String]) -> Result<(), i32> {
    let is_global = args.iter().any(|a| a == "--global");
    let dry_run = args.iter().any(|a| a == "--dry-run");

    let base_dir = if is_global {
        dirs::home_dir().ok_or_else(|| {
            eprintln!("🌸 홈 디렉토리를 찾을 수 없습니다.");
            1
        })?
    } else {
        PathBuf::from(".")
    };

    let config_path = if is_global {
        base_dir.join(".agents/hana.toml")
    } else {
        base_dir.join(".agents/hana.toml")
    };

    let config = config::Config::load(&config_path).map_err(|e| {
        eprintln!("🌸 {e}");
        eprintln!("   hana init 으로 설정 파일을 먼저 생성하세요.");
        1
    })?;

    if dry_run {
        println!("🌸 hana sync (dry-run)\n");
    } else {
        println!("🌸 hana sync\n");
    }

    let result = sync::execute(&config, &base_dir, dry_run);

    // 스킬 수집
    for (name, agent) in &result.skills_collected {
        println!("  🆕 {name} ({agent}에서 수집)");
    }

    // 스킬 심링크
    if !result.skills_linked.is_empty() {
        println!("스킬 동기화:");
        let mut by_skill: std::collections::HashMap<&str, Vec<&str>> =
            std::collections::HashMap::new();
        for (skill, agent) in &result.skills_linked {
            by_skill.entry(skill).or_default().push(agent);
        }
        for (skill, agents) in &by_skill {
            println!("  ✅ {skill} → {}", agents.join(", "));
        }
    }

    // 지침 동기화
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

    // 정리
    if !result.cleaned.is_empty() {
        println!("정리:");
        for path in &result.cleaned {
            println!("  🗑️  {}", path.display());
        }
    }

    // 에러
    for err in &result.errors {
        eprintln!("  ⚠️  {err}");
    }

    if result.skills_linked.is_empty()
        && result.skills_collected.is_empty()
        && result.instructions_linked.is_empty()
        && result.cleaned.is_empty()
    {
        println!("변경 없음. 모두 동기화 상태입니다.");
    }

    println!("\n완료!");
    Ok(())
}

fn cmd_status(args: &[String]) -> Result<(), i32> {
    let is_global = args.iter().any(|a| a == "--global");

    let base_dir = if is_global {
        dirs::home_dir().ok_or_else(|| {
            eprintln!("🌸 홈 디렉토리를 찾을 수 없습니다.");
            1
        })?
    } else {
        PathBuf::from(".")
    };

    let config_path = base_dir.join(".agents/hana.toml");

    let config = config::Config::load(&config_path).map_err(|e| {
        eprintln!("🌸 {e}");
        eprintln!("   hana init 으로 설정 파일을 먼저 생성하세요.");
        1
    })?;

    let result = status::execute(&config, &base_dir);
    print!("{}", status::format_result(&result));
    Ok(())
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
  --dry-run 실제 변경 없이 미리보기
  -h, --help  도움말"
    );
}
