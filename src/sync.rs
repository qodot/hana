use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::agents;
use crate::config::Config;

pub fn run(args: &[String]) -> Result<(), i32> {
    let is_global = args.iter().any(|a| a == "--global");
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let force = args.iter().any(|a| a == "--force");

    let base_dir = if is_global {
        dirs::home_dir().ok_or_else(|| {
            eprintln!("🌸 홈 디렉토리를 찾을 수 없습니다.");
            1
        })?
    } else {
        PathBuf::from(".")
    };

    let config_path = base_dir.join(".agents/hana.toml");

    let config = Config::load(&config_path).map_err(|e| {
        eprintln!("🌸 {e}");
        eprintln!("   hana init 으로 설정 파일을 먼저 생성하세요.");
        1
    })?;

    if dry_run {
        println!("🌸 hana sync (dry-run)\n");
    } else {
        println!("🌸 hana sync\n");
    }

    let opts = SyncOptions {
        dry_run,
        force,
        global: is_global,
    };
    let result = execute(&config, &base_dir, &opts);

    // 스킬 수집
    for (name, agent) in &result.skills_collected {
        println!("  🆕 {name} ({agent}에서 수집)");
    }

    // 스킬 심링크
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

// 경로 매핑은 agents 모듈에서 관리


#[derive(Debug, Default)]
pub struct SyncOptions {
    pub dry_run: bool,
    pub force: bool,
    pub global: bool,
}

#[derive(Debug, Default)]
pub struct SyncResult {
    pub skills_linked: Vec<(String, String)>,   // (skill_name, agent)
    pub skills_collected: Vec<(String, String)>, // (skill_name, from_agent)
    pub instructions_linked: Vec<String>,        // agent names
    pub instructions_skipped: Vec<String>,       // agent names (직접 읽음)
    pub cleaned: Vec<PathBuf>,                   // 제거된 깨진 심링크
    pub errors: Vec<String>,
}

pub fn execute(config: &Config, base_dir: &Path, opts: &SyncOptions) -> SyncResult {
    let mut result = SyncResult::default();

    // 1. 스킬 동기화
    sync_skills(config, base_dir, opts, &mut result);

    // 2. 지침 동기화
    sync_instructions(config, base_dir, opts, &mut result);

    result
}

fn sync_skills(config: &Config, base_dir: &Path, opts: &SyncOptions, result: &mut SyncResult) {
    let source_dir = base_dir.join(&config.skills_source);

    if !source_dir.exists() {
        return;
    }

    // 1단계: 역방향 수집 (모든 에이전트에서)
    // 동일한 이름이 여러 에이전트에 있으면 충돌 감지
    let skill_targets = agents::collect_skills(opts.global, &config.skills_source);
    let mut new_skills: HashMap<String, Vec<(String, PathBuf)>> = HashMap::new(); // name → [(agent, path)]

    for &(agent, agent_skill_dir) in &skill_targets {
        if !config.targets.get(agent).map(|t| t.skills).unwrap_or(false) {
            continue;
        }
        let agent_dir = base_dir.join(agent_skill_dir);
        if !agent_dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&agent_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if path.is_dir() && !path.is_symlink() && !source_dir.join(&name).exists() {
                    new_skills
                        .entry(name)
                        .or_default()
                        .push((agent.to_string(), path));
                }
            }
        }
    }

    // 충돌 감지: 같은 이름이 여러 에이전트에서 발견
    for (name, sources) in &new_skills {
        if sources.len() > 1 {
            let agents: Vec<&str> = sources.iter().map(|(a, _)| a.as_str()).collect();
            result.errors.push(format!(
                "스킬 이름 충돌: '{name}' 이(가) 여러 에이전트에서 발견됨 ({}). 수동으로 해결하세요.",
                agents.join(", ")
            ));
            continue;
        }

        let (agent, path) = &sources[0];
        let dest = source_dir.join(name);

        if !opts.dry_run {
            if let Err(e) = fs::rename(path, &dest) {
                result
                    .errors
                    .push(format!("스킬 수집 실패 ({name}, {agent}): {e}"));
                continue;
            }
            if let Err(e) = std::os::unix::fs::symlink(&dest, path) {
                result
                    .errors
                    .push(format!("심링크 생성 실패 ({name}, {agent}): {e}"));
                continue;
            }
        }
        result
            .skills_collected
            .push((name.clone(), agent.to_string()));
    }

    // 2단계: 소스에서 스킬 목록 재수집 (수집 후 업데이트된 목록)
    let skills: Vec<String> = match fs::read_dir(&source_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect(),
        Err(_) => return,
    };

    // 3단계: 정방향 동기화 + 정리
    for &(agent, agent_skill_dir) in &skill_targets {
        if !config.targets.get(agent).map(|t| t.skills).unwrap_or(false) {
            continue;
        }
        let agent_dir = base_dir.join(agent_skill_dir);

        for skill in &skills {
            let link_path = agent_dir.join(skill);
            let target_path = source_dir.join(skill);

            // 이미 올바른 심링크면 스킵
            if link_path.is_symlink() {
                if let Ok(link_target) = fs::read_link(&link_path) {
                    if link_target == target_path {
                        continue;
                    }
                }
            }

            // 실제 디렉토리/파일이 존재하면
            if link_path.exists() && !link_path.is_symlink() {
                if opts.force {
                    if !opts.dry_run {
                        if link_path.is_dir() {
                            let _ = fs::remove_dir_all(&link_path);
                        } else {
                            let _ = fs::remove_file(&link_path);
                        }
                    }
                } else {
                    result.errors.push(format!(
                        "충돌: {skill} ({agent}) 에 실제 파일/디렉토리가 존재합니다. --force로 덮어쓰세요."
                    ));
                    continue;
                }
            }

            if !opts.dry_run {
                if let Some(parent) = link_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                // 잘못된 심링크 제거
                if link_path.is_symlink() {
                    let _ = fs::remove_file(&link_path);
                }
                if let Err(e) = std::os::unix::fs::symlink(&target_path, &link_path) {
                    result
                        .errors
                        .push(format!("심링크 생성 실패 ({skill}, {agent}): {e}"));
                    continue;
                }
            }
            result.skills_linked.push((skill.clone(), agent.to_string()));
        }

        // 정리: 깨진 심링크 제거
        if agent_dir.exists() {
            if let Ok(entries) = fs::read_dir(&agent_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_symlink() && !path.exists() {
                        if !opts.dry_run {
                            let _ = fs::remove_file(&path);
                        }
                        result.cleaned.push(path);
                    }
                }
            }
        }
    }
}

fn sync_instructions(
    config: &Config,
    base_dir: &Path,
    opts: &SyncOptions,
    result: &mut SyncResult,
) {
    let source_path = base_dir.join(&config.instructions_source);
    if !source_path.exists() {
        return;
    }

    for (agent, maybe_path) in agents::collect_instructions(opts.global) {
        if !config
            .targets
            .get(agent)
            .map(|t| t.instructions)
            .unwrap_or(false)
        {
            continue;
        }
        match maybe_path {
            Some(rel_path) => {
                let link_path = base_dir.join(rel_path);

                // 소스와 동일 경로면 스킵
                if link_path == source_path {
                    result.instructions_skipped.push(agent.to_string());
                    continue;
                }

                sync_instruction_link(
                    &source_path,
                    &link_path,
                    rel_path,
                    agent,
                    opts,
                    result,
                );
            }
            None => {
                result.instructions_skipped.push(agent.to_string());
            }
        }
    }
}

fn sync_instruction_link(
    source_path: &Path,
    link_path: &Path,
    display_name: &str,
    agent: &str,
    opts: &SyncOptions,
    result: &mut SyncResult,
) {
    // 이미 올바른 심링크면 스킵
    if link_path.is_symlink() {
        if let Ok(target) = fs::read_link(link_path) {
            if target == source_path {
                return;
            }
        }
    }

    // 실제 파일 충돌
    if link_path.exists() && !link_path.is_symlink() {
        if opts.force {
            if !opts.dry_run {
                let _ = fs::remove_file(link_path);
            }
        } else {
            result.errors.push(format!(
                "{display_name} 가 이미 존재합니다 (심링크가 아님). --force로 덮어쓰세요."
            ));
            return;
        }
    }

    if !opts.dry_run {
        // 부모 디렉토리 생성
        if let Some(parent) = link_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if link_path.is_symlink() {
            let _ = fs::remove_file(link_path);
        }
        if let Err(e) = std::os::unix::fs::symlink(source_path, link_path) {
            result
                .errors
                .push(format!("지침 심링크 실패 ({display_name}): {e}"));
            return;
        }
    }

    result.instructions_linked.push(agent.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    fn default_config() -> Config {
        Config::default()
    }

    fn default_opts() -> SyncOptions {
        SyncOptions {
            dry_run: false,
            force: false,
            global: false,
        }
    }

    fn setup_source(tmp: &Path) {
        let skills = tmp.join(".agents/skills");
        fs::create_dir_all(skills.join("my-skill")).unwrap();
        fs::write(skills.join("my-skill/SKILL.md"), "# My Skill").unwrap();
        fs::write(tmp.join("AGENTS.md"), "# Instructions").unwrap();
    }

    // === 스킬 정방향 동기화 ===

    #[test]
    fn test_sync_creates_skill_symlinks() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let result = execute(&default_config(), tmp.path(), &default_opts());

        assert!(tmp.path().join(".claude/skills/my-skill").is_symlink());
        assert!(tmp.path().join(".pi/skills/my-skill").is_symlink());
        assert!(tmp.path().join(".opencode/skills/my-skill").is_symlink());
        assert!(!tmp.path().join(".agents/skills/my-skill").is_symlink());
        assert!(result.skills_linked.len() >= 3);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_sync_skips_codex_same_source() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let result = execute(&default_config(), tmp.path(), &default_opts());

        let codex_links: Vec<_> = result
            .skills_linked
            .iter()
            .filter(|(_, a)| a == "codex")
            .collect();
        assert!(codex_links.is_empty());
    }

    #[test]
    fn test_sync_skips_existing_correct_symlink() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());
        let config = default_config();

        execute(&config, tmp.path(), &default_opts());
        let result = execute(&config, tmp.path(), &default_opts());
        assert!(result.skills_linked.is_empty());
    }

    #[test]
    fn test_sync_skips_disabled_target() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());
        let mut config = default_config();
        config.targets.get_mut("claude").unwrap().skills = false;

        let result = execute(&config, tmp.path(), &default_opts());

        assert!(!tmp.path().join(".claude/skills/my-skill").exists());
        let claude_links: Vec<_> = result
            .skills_linked
            .iter()
            .filter(|(_, a)| a == "claude")
            .collect();
        assert!(claude_links.is_empty());
    }

    // === 스킬 역방향 수집 ===

    #[test]
    fn test_sync_collects_new_skill_from_agent() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let pi_new = tmp.path().join(".pi/skills/new-skill");
        fs::create_dir_all(&pi_new).unwrap();
        fs::write(pi_new.join("SKILL.md"), "# New").unwrap();

        let result = execute(&default_config(), tmp.path(), &default_opts());

        // 소스로 이동
        assert!(tmp.path().join(".agents/skills/new-skill").is_dir());
        assert!(!tmp.path().join(".agents/skills/new-skill").is_symlink());
        // 원래 위치는 심링크
        assert!(tmp.path().join(".pi/skills/new-skill").is_symlink());
        assert!(!result.skills_collected.is_empty());
    }

    #[test]
    fn test_sync_collected_skill_synced_to_all_agents() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        // pi에서만 새 스킬 생성
        let pi_new = tmp.path().join(".pi/skills/new-skill");
        fs::create_dir_all(&pi_new).unwrap();
        fs::write(pi_new.join("SKILL.md"), "# New").unwrap();

        execute(&default_config(), tmp.path(), &default_opts());

        // 수집 후 다른 에이전트에도 심링크 생성되었는지
        assert!(tmp.path().join(".claude/skills/new-skill").is_symlink());
        assert!(tmp.path().join(".opencode/skills/new-skill").is_symlink());
        assert!(tmp.path().join(".pi/skills/new-skill").is_symlink());
    }

    #[test]
    fn test_sync_does_not_collect_existing_source_skill() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let pi_existing = tmp.path().join(".pi/skills/my-skill");
        fs::create_dir_all(&pi_existing).unwrap();

        let result = execute(&default_config(), tmp.path(), &default_opts());

        let collected: Vec<_> = result
            .skills_collected
            .iter()
            .filter(|(name, _)| name == "my-skill")
            .collect();
        assert!(collected.is_empty());
    }

    // === 스킬 이름 충돌 ===

    #[test]
    fn test_sync_detects_skill_name_conflict() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        // 같은 이름의 실제 디렉토리를 pi, claude 양쪽에 생성
        fs::create_dir_all(tmp.path().join(".pi/skills/conflict-skill")).unwrap();
        fs::create_dir_all(tmp.path().join(".claude/skills/conflict-skill")).unwrap();

        let result = execute(&default_config(), tmp.path(), &default_opts());

        // 충돌 에러 발생
        let conflict_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|e| e.contains("충돌") && e.contains("conflict-skill"))
            .collect();
        assert!(!conflict_errors.is_empty());

        // 소스로 이동되지 않아야 함
        assert!(!tmp.path().join(".agents/skills/conflict-skill").exists());
    }

    // === 기존 파일 충돌 + --force ===

    #[test]
    fn test_sync_errors_on_existing_real_skill_dir() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        // claude에 소스와 같은 이름의 실제 디렉토리 (이미 소스에도 있음)
        fs::create_dir_all(tmp.path().join(".claude/skills/my-skill")).unwrap();

        let result = execute(&default_config(), tmp.path(), &default_opts());

        // 에러 발생
        let conflict_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|e| e.contains("충돌") && e.contains("my-skill"))
            .collect();
        assert!(!conflict_errors.is_empty());
    }

    #[test]
    fn test_sync_force_overwrites_existing_skill_dir() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        fs::create_dir_all(tmp.path().join(".claude/skills/my-skill")).unwrap();
        fs::write(
            tmp.path().join(".claude/skills/my-skill/old.txt"),
            "old",
        )
        .unwrap();

        let opts = SyncOptions {
            force: true,
            ..default_opts()
        };
        let result = execute(&default_config(), tmp.path(), &opts);

        // 심링크로 대체됨
        assert!(tmp.path().join(".claude/skills/my-skill").is_symlink());
        assert!(result.errors.is_empty());
    }

    // === 지침 동기화 ===

    #[test]
    fn test_sync_creates_instruction_symlink() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let result = execute(&default_config(), tmp.path(), &default_opts());

        assert!(tmp.path().join("CLAUDE.md").is_symlink());
        assert!(result.instructions_linked.contains(&"claude".to_string()));
        assert!(result.instructions_skipped.contains(&"codex".to_string()));
        assert!(result.instructions_skipped.contains(&"pi".to_string()));
        assert!(result
            .instructions_skipped
            .contains(&"opencode".to_string()));
    }

    #[test]
    fn test_sync_skips_instruction_when_disabled() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());
        let mut config = default_config();
        config.targets.get_mut("claude").unwrap().instructions = false;

        let result = execute(&config, tmp.path(), &default_opts());

        assert!(!tmp.path().join("CLAUDE.md").exists());
        assert!(!result.instructions_linked.contains(&"claude".to_string()));
    }

    #[test]
    fn test_sync_errors_on_existing_real_instruction_file() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());
        fs::write(tmp.path().join("CLAUDE.md"), "# Real file").unwrap();

        let result = execute(&default_config(), tmp.path(), &default_opts());

        assert!(!result.errors.is_empty());
        assert!(result.errors[0].contains("CLAUDE.md"));
    }

    #[test]
    fn test_sync_force_overwrites_existing_instruction_file() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());
        fs::write(tmp.path().join("CLAUDE.md"), "# Real file").unwrap();

        let opts = SyncOptions {
            force: true,
            ..default_opts()
        };
        let result = execute(&default_config(), tmp.path(), &opts);

        assert!(tmp.path().join("CLAUDE.md").is_symlink());
        assert!(result.errors.is_empty());
    }

    // === 글로벌 지침 동기화 ===

    #[test]
    fn test_sync_global_instructions() {
        let tmp = TempDir::new().unwrap();
        // 글로벌 소스 생성
        fs::create_dir_all(tmp.path().join(".agents/skills")).unwrap();
        fs::write(tmp.path().join("AGENTS.md"), "# Global Instructions").unwrap();

        let config = default_config();
        let opts = SyncOptions {
            global: true,
            ..default_opts()
        };
        let _result = execute(&config, tmp.path(), &opts);

        // 글로벌에서는 모든 에이전트에 심링크 (claude, codex, opencode, pi)
        assert!(tmp.path().join(".claude/CLAUDE.md").is_symlink());
        assert!(tmp.path().join(".codex/AGENTS.md").is_symlink());
        assert!(tmp.path().join(".config/opencode/AGENTS.md").is_symlink());
        assert!(tmp.path().join(".pi/agent/AGENTS.md").is_symlink());
    }

    // === 깨진 심링크 정리 ===

    #[test]
    fn test_sync_cleans_broken_symlinks() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let claude_dir = tmp.path().join(".claude/skills");
        fs::create_dir_all(&claude_dir).unwrap();
        symlink("/nonexistent/deleted-skill", claude_dir.join("old-skill")).unwrap();

        let result = execute(&default_config(), tmp.path(), &default_opts());

        assert!(!claude_dir.join("old-skill").exists());
        assert!(!claude_dir.join("old-skill").is_symlink());
        assert!(!result.cleaned.is_empty());
    }

    // === dry-run ===

    #[test]
    fn test_sync_dry_run_no_changes() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let opts = SyncOptions {
            dry_run: true,
            ..default_opts()
        };
        let result = execute(&default_config(), tmp.path(), &opts);

        assert!(!result.skills_linked.is_empty());
        assert!(!tmp.path().join(".claude/skills/my-skill").exists());
        assert!(!tmp.path().join("CLAUDE.md").exists());
    }

    // === 소스 없을 때 ===

    #[test]
    fn test_sync_no_source_dir() {
        let tmp = TempDir::new().unwrap();

        let result = execute(&default_config(), tmp.path(), &default_opts());

        assert!(result.skills_linked.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_sync_no_instructions_source() {
        let tmp = TempDir::new().unwrap();

        let result = execute(&default_config(), tmp.path(), &default_opts());

        assert!(result.instructions_linked.is_empty());
    }
}
