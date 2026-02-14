use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;

pub fn run(args: &[String]) -> Result<(), i32> {
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

    let result = execute(&config, &base_dir, dry_run);

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

/// 에이전트별 스킬 경로 매핑 (프로젝트 레벨)
fn skill_path(agent: &str) -> Option<&'static str> {
    match agent {
        "claude" => Some(".claude/skills"),
        "codex" => Some(".agents/skills"), // 소스와 동일
        "pi" => Some(".pi/skills"),
        "opencode" => Some(".opencode/skills"),
        _ => None,
    }
}

/// 에이전트별 지침 파일명 매핑 (프로젝트 레벨)
/// None이면 AGENTS.md를 직접 읽으므로 심링크 불필요
fn instruction_file(agent: &str) -> Option<&'static str> {
    match agent {
        "claude" => Some("CLAUDE.md"),
        // codex, pi, opencode는 AGENTS.md 직접 읽음
        _ => None,
    }
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

pub fn execute(config: &Config, base_dir: &Path, dry_run: bool) -> SyncResult {
    let mut result = SyncResult::default();

    // 1. 스킬 동기화
    sync_skills(config, base_dir, dry_run, &mut result);

    // 2. 지침 동기화
    sync_instructions(config, base_dir, dry_run, &mut result);

    result
}

fn sync_skills(config: &Config, base_dir: &Path, dry_run: bool, result: &mut SyncResult) {
    let source_dir = base_dir.join(&config.skills_source);

    // 소스 디렉토리가 없으면 스킵
    if !source_dir.exists() {
        return;
    }

    // 소스에서 스킬 목록 수집
    let skills: Vec<String> = match fs::read_dir(&source_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect(),
        Err(_) => return,
    };

    for (agent, target_config) in &config.targets {
        if !target_config.skills {
            continue;
        }

        let agent_skill_dir = match skill_path(agent) {
            Some(p) => p,
            None => continue,
        };

        // 소스와 동일 경로면 스킵
        if agent_skill_dir == config.skills_source {
            continue;
        }

        let agent_dir = base_dir.join(agent_skill_dir);

        // 역방향 수집: 에이전트 경로에서 실제 디렉토리(심링크 아닌) 찾기
        if agent_dir.exists() {
            if let Ok(entries) = fs::read_dir(&agent_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();

                    // 실제 디렉토리이고 소스에 없는 경우 → 수집
                    if path.is_dir() && !path.is_symlink() && !source_dir.join(&name).exists() {
                        let dest = source_dir.join(&name);
                        if !dry_run {
                            if let Err(e) = fs::rename(&path, &dest) {
                                result.errors.push(format!(
                                    "스킬 수집 실패 ({name}, {agent}): {e}"
                                ));
                                continue;
                            }
                            // 이동 후 심링크 생성
                            if let Err(e) = std::os::unix::fs::symlink(&dest, &path) {
                                result.errors.push(format!(
                                    "심링크 생성 실패 ({name}, {agent}): {e}"
                                ));
                                continue;
                            }
                        }
                        result.skills_collected.push((name, agent.clone()));
                    }
                }
            }
        }

        // 정방향 동기화: 소스 스킬을 에이전트 경로에 심링크
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

            // 실제 디렉토리면 스킵 (충돌)
            if link_path.exists() && !link_path.is_symlink() {
                continue;
            }

            if !dry_run {
                // 부모 디렉토리 생성
                if let Some(parent) = link_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }

                // 깨진 심링크 제거
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

            result.skills_linked.push((skill.clone(), agent.clone()));
        }

        // 정리: 깨진 심링크 제거
        if agent_dir.exists() {
            if let Ok(entries) = fs::read_dir(&agent_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_symlink() && !path.exists() {
                        if !dry_run {
                            let _ = fs::remove_file(&path);
                        }
                        result.cleaned.push(path);
                    }
                }
            }
        }
    }
}

fn sync_instructions(config: &Config, base_dir: &Path, dry_run: bool, result: &mut SyncResult) {
    let source_path = base_dir.join(&config.instructions_source);

    if !source_path.exists() {
        return;
    }

    for (agent, target_config) in &config.targets {
        if !target_config.instructions {
            continue;
        }

        match instruction_file(agent) {
            Some(filename) => {
                let link_path = base_dir.join(filename);

                // 이미 올바른 심링크면 스킵
                if link_path.is_symlink() {
                    if let Ok(target) = fs::read_link(&link_path) {
                        if target == source_path {
                            continue;
                        }
                    }
                }

                // 실제 파일이 존재하면 스킵 (충돌)
                if link_path.exists() && !link_path.is_symlink() {
                    result.errors.push(format!(
                        "{filename} 가 이미 존재합니다 (심링크가 아님). --force로 덮어쓰세요."
                    ));
                    continue;
                }

                if !dry_run {
                    if link_path.is_symlink() {
                        let _ = fs::remove_file(&link_path);
                    }
                    if let Err(e) = std::os::unix::fs::symlink(&source_path, &link_path) {
                        result
                            .errors
                            .push(format!("지침 심링크 실패 ({filename}): {e}"));
                        continue;
                    }
                }

                result.instructions_linked.push(agent.clone());
            }
            None => {
                // AGENTS.md 직접 읽는 에이전트
                result.instructions_skipped.push(agent.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    fn default_config() -> Config {
        Config::default()
    }

    fn setup_source(tmp: &Path) {
        let skills = tmp.join(".agents/skills");
        fs::create_dir_all(skills.join("my-skill")).unwrap();
        fs::write(skills.join("my-skill/SKILL.md"), "# My Skill").unwrap();
        fs::write(tmp.join("AGENTS.md"), "# Instructions").unwrap();
    }

    // --- 스킬 정방향 동기화 ---

    #[test]
    fn test_sync_creates_skill_symlinks() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let config = default_config();
        let result = execute(&config, tmp.path(), false);

        // claude, pi, opencode에 심링크 생성 (codex는 소스와 동일하므로 스킵)
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

        let config = default_config();
        let result = execute(&config, tmp.path(), false);

        // codex용 심링크가 생성되지 않아야 함
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

        // 먼저 한번 sync
        execute(&config, tmp.path(), false);

        // 두번째 sync — 이미 올바른 심링크이므로 스킵
        let result = execute(&config, tmp.path(), false);
        assert!(result.skills_linked.is_empty());
    }

    #[test]
    fn test_sync_skips_disabled_target() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let mut config = default_config();
        config.targets.get_mut("claude").unwrap().skills = false;

        let result = execute(&config, tmp.path(), false);

        assert!(!tmp.path().join(".claude/skills/my-skill").exists());
        let claude_links: Vec<_> = result
            .skills_linked
            .iter()
            .filter(|(_, a)| a == "claude")
            .collect();
        assert!(claude_links.is_empty());
    }

    // --- 스킬 역방향 수집 ---

    #[test]
    fn test_sync_collects_new_skill_from_agent() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        // pi에 새 스킬 생성 (실제 디렉토리)
        let pi_new = tmp.path().join(".pi/skills/new-skill");
        fs::create_dir_all(&pi_new).unwrap();
        fs::write(pi_new.join("SKILL.md"), "# New").unwrap();

        let config = default_config();
        let result = execute(&config, tmp.path(), false);

        // 소스로 이동되었는지
        assert!(tmp.path().join(".agents/skills/new-skill").is_dir());
        assert!(!tmp.path().join(".agents/skills/new-skill").is_symlink());

        // 원래 위치는 심링크로 변경
        assert!(tmp.path().join(".pi/skills/new-skill").is_symlink());

        assert!(!result.skills_collected.is_empty());
    }

    #[test]
    fn test_sync_does_not_collect_existing_source_skill() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        // pi에 소스와 같은 이름의 실제 디렉토리
        let pi_existing = tmp.path().join(".pi/skills/my-skill");
        fs::create_dir_all(&pi_existing).unwrap();

        let config = default_config();
        let result = execute(&config, tmp.path(), false);

        // 이미 소스에 있으므로 수집하지 않음
        let collected: Vec<_> = result
            .skills_collected
            .iter()
            .filter(|(name, _)| name == "my-skill")
            .collect();
        assert!(collected.is_empty());
    }

    // --- 지침 동기화 ---

    #[test]
    fn test_sync_creates_instruction_symlink() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let config = default_config();
        let result = execute(&config, tmp.path(), false);

        // CLAUDE.md만 심링크
        assert!(tmp.path().join("CLAUDE.md").is_symlink());
        assert!(result.instructions_linked.contains(&"claude".to_string()));

        // codex, pi, opencode는 스킵 (AGENTS.md 직접 읽음)
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

        let result = execute(&config, tmp.path(), false);

        assert!(!tmp.path().join("CLAUDE.md").exists());
        assert!(!result.instructions_linked.contains(&"claude".to_string()));
    }

    #[test]
    fn test_sync_errors_on_existing_real_instruction_file() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        // 실제 CLAUDE.md 파일이 이미 존재
        fs::write(tmp.path().join("CLAUDE.md"), "# Real file").unwrap();

        let config = default_config();
        let result = execute(&config, tmp.path(), false);

        assert!(!result.errors.is_empty());
        assert!(result.errors[0].contains("CLAUDE.md"));
    }

    // --- 깨진 심링크 정리 ---

    #[test]
    fn test_sync_cleans_broken_symlinks() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        // 깨진 심링크 생성
        let claude_dir = tmp.path().join(".claude/skills");
        fs::create_dir_all(&claude_dir).unwrap();
        symlink("/nonexistent/deleted-skill", claude_dir.join("old-skill")).unwrap();

        let config = default_config();
        let result = execute(&config, tmp.path(), false);

        assert!(!claude_dir.join("old-skill").exists());
        assert!(!claude_dir.join("old-skill").is_symlink());
        assert!(!result.cleaned.is_empty());
    }

    // --- dry-run ---

    #[test]
    fn test_sync_dry_run_no_changes() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let config = default_config();
        let result = execute(&config, tmp.path(), true);

        // 결과는 있지만 실제 변경 없음
        assert!(!result.skills_linked.is_empty());
        assert!(!tmp.path().join(".claude/skills/my-skill").exists());
        assert!(!tmp.path().join("CLAUDE.md").exists());
    }

    // --- 소스 없을 때 ---

    #[test]
    fn test_sync_no_source_dir() {
        let tmp = TempDir::new().unwrap();
        // 소스 디렉토리 없이

        let config = default_config();
        let result = execute(&config, tmp.path(), false);

        assert!(result.skills_linked.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_sync_no_instructions_source() {
        let tmp = TempDir::new().unwrap();
        // AGENTS.md 없이

        let config = default_config();
        let result = execute(&config, tmp.path(), false);

        assert!(result.instructions_linked.is_empty());
    }
}
