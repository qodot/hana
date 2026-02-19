use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{Config, TargetFeature};
use crate::error::{SyncError, SyncOk, SyncWarning};
use crate::helper::broadcast_symlink::broadcast_symlink;
use crate::helper::build_destinations::build_destinations;
use crate::helper::collect_skills::collect_skills as collect_target_skills;
use crate::helper::collect_sources::collect_sources;
use crate::helper::mv_skills::mv_skills as move_collected_skills;

pub fn run(opts: &SyncOptions) -> Result<SyncOk, SyncError> {
    let base_dir = if opts.global {
        dirs::home_dir().ok_or(SyncError::NoHomeDir)?
    } else {
        PathBuf::from(".")
    };

    let config_path = base_dir.join(".agents/hana.toml");
    let config = Config::load(&config_path)?;

    Ok(execute(&config, &base_dir, opts))
}

// 경로 매핑은 config(hana.toml)에서 관리

#[derive(Debug, Default)]
pub struct SyncOptions {
    pub dry_run: bool,
    pub force: bool,
    pub global: bool,
}

pub fn execute(config: &Config, base_dir: &Path, opts: &SyncOptions) -> SyncOk {
    let mut result = SyncOk {
        skills_linked: vec![],
        skills_collected: vec![],
        instructions_linked: vec![],
        instructions_skipped: vec![],
        cleaned: vec![],
        warnings: vec![],
    };

    // 1. 스킬 동기화
    sync_skills(config, base_dir, opts, &mut result);

    // 2. 지침 동기화
    sync_instructions(config, base_dir, opts, &mut result);

    result
}

fn sync_skills(config: &Config, base_dir: &Path, opts: &SyncOptions, result: &mut SyncOk) {
    let source_dir = config.resolve_source_skills_path(base_dir, opts.global);

    if !source_dir.exists() {
        if !opts.dry_run {
            if let Err(e) = fs::create_dir_all(&source_dir) {
                result.warnings.push(SyncWarning::IoFailed {
                    operation: format!("소스 디렉토리 생성 ({})", source_dir.display()),
                    detail: e.to_string(),
                });
                return;
            }
        }
    }

    // 1단계: 수집 (모든 에이전트에서)
    // 동일한 이름이 여러 에이전트에 있으면 충돌 감지
    let collected_by_agent = collect_target_skills(config, base_dir, opts.global);
    let move_result =
        move_collected_skills(&collected_by_agent, &source_dir, opts.force, opts.dry_run);
    let tasks = match move_result {
        Ok(ok) => ok.tasks,
        Err(err) => {
            result.warnings.extend(err.warnings);
            err.tasks
        }
    };

    result.skills_collected.extend(
        tasks
            .iter()
            .map(|t| (t.skill.clone(), t.agent.as_str().to_string())),
    );

    // 2단계: 소스에서 스킬 목록 재조회 (수집 후 업데이트된 목록)
    let source_skills = match collect_sources(&source_dir) {
        Ok(skills) => skills,
        Err(warning) => {
            result.warnings.push(warning);
            return;
        }
    };

    // dry-run에서는 실제 이동이 없으므로, 이번 실행에서 수집 예정인 스킬을 추가로 포함해 동기화 대상을 계산한다.
    let skills: Vec<String> = source_skills
        .into_iter()
        .chain(if opts.dry_run {
            result
                .skills_collected
                .iter()
                .map(|(name, _)| name.clone())
                .collect()
        } else {
            vec![]
        })
        .collect::<BTreeSet<String>>() // 중복 제거 및 정렬
        .into_iter()
        .collect();

    // 3단계: 정방향 동기화
    let enabled_targets = build_destinations(config, base_dir, opts.global);

    for skill in &skills {
        let source = source_dir.join(skill);
        let agent_dests: Vec<(&str, PathBuf)> = enabled_targets
            .iter()
            .map(|(agent, dir)| (agent.as_str(), dir.join(skill)))
            .collect();
        let dests: Vec<PathBuf> = agent_dests.iter().map(|(_, d)| d.clone()).collect();

        let (linked, conflicts, failed) =
            match broadcast_symlink(&source, &dests, opts.dry_run, opts.force) {
                Ok(ok) => (ok.linked, vec![], vec![]),
                Err(err) => (err.linked, err.conflicts, err.failed),
            };

        let find_agent = |path: &PathBuf| -> Option<String> {
            agent_dests
                .iter()
                .find(|(_, d)| d == path)
                .map(|(agent, _)| agent.to_string())
        };

        for path in &linked {
            if let Some(agent) = find_agent(path) {
                result.skills_linked.push((skill.clone(), agent));
            }
        }
        for path in &conflicts {
            if let Some(agent) = find_agent(path) {
                result.warnings.push(SyncWarning::FileConflict {
                    skill: skill.clone(),
                    agent,
                });
            }
        }
        for (path, detail) in &failed {
            if let Some(agent) = find_agent(path) {
                result.warnings.push(SyncWarning::IoFailed {
                    operation: format!("심링크 생성 ({skill}, {agent})"),
                    detail: detail.clone(),
                });
            }
        }
    }

    // 4단계: 깨진 심링크 정리
    for (_, agent_dir) in &enabled_targets {
        if agent_dir.exists() {
            if let Ok(entries) = fs::read_dir(agent_dir) {
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

fn sync_instructions(config: &Config, base_dir: &Path, opts: &SyncOptions, result: &mut SyncOk) {
    let source_path = config.resolve_source_instruction_path(base_dir, opts.global);
    if !source_path.exists() {
        return;
    }

    for agent in config.enabled_targets(TargetFeature::Instructions) {
        let agent_name = agent.as_str();

        let Some(link_path) =
            config.resolve_target_instruction_path(agent_name, base_dir, opts.global)
        else {
            continue;
        };
        let display_name = config
            .target_instruction_path(agent_name, opts.global)
            .unwrap_or(agent_name);

        // 소스와 동일 경로면 스킵(직접 읽는 에이전트)
        if link_path == source_path {
            result.instructions_skipped.push(agent_name.to_string());
            continue;
        }

        sync_instruction_link(
            &source_path,
            &link_path,
            display_name,
            agent_name,
            opts,
            result,
        );
    }
}

fn sync_instruction_link(
    source_path: &Path,
    link_path: &Path,
    display_name: &str,
    agent: &str,
    opts: &SyncOptions,
    result: &mut SyncOk,
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
            result.warnings.push(SyncWarning::InstructionConflict {
                file: display_name.to_string(),
            });
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
            result.warnings.push(SyncWarning::IoFailed {
                operation: format!("지침 심링크 ({display_name})"),
                detail: e.to_string(),
            });
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
        assert!(result.warnings.is_empty());
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

    // === 스킬 수집 ===

    #[test]
    fn test_sync_collects_new_skill_and_links_all_agents() {
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
        assert!(result.warnings.iter().any(|w| matches!(
            w,
            SyncWarning::SourceSkillConflict { skill, agent }
                if skill == "my-skill" && agent == "pi"
        )));
    }

    #[test]
    fn test_sync_force_overwrites_existing_source_skill() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let pi_existing = tmp.path().join(".pi/skills/my-skill");
        fs::create_dir_all(&pi_existing).unwrap();
        fs::write(pi_existing.join("SKILL.md"), "# Pi Skill").unwrap();

        let opts = SyncOptions {
            force: true,
            ..default_opts()
        };
        let result = execute(&default_config(), tmp.path(), &opts);

        assert_eq!(
            fs::read_to_string(tmp.path().join(".agents/skills/my-skill/SKILL.md")).unwrap(),
            "# Pi Skill"
        );
        assert!(tmp.path().join(".pi/skills/my-skill").is_symlink());
        assert!(
            result
                .skills_collected
                .iter()
                .any(|(name, agent)| name == "my-skill" && agent == "pi")
        );
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

        // 충돌 경고 발생
        assert!(result.warnings.iter().any(|w| matches!(
            w,
            SyncWarning::SkillConflict { name, .. } if name == "conflict-skill"
        )));

        // 소스로 이동되지 않아야 함
        assert!(!tmp.path().join(".agents/skills/conflict-skill").exists());
    }

    // === 기존 파일 충돌 + --force ===

    #[test]
    fn test_sync_existing_real_skill_dir_requires_force() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        fs::create_dir_all(tmp.path().join(".claude/skills/my-skill")).unwrap();
        fs::write(tmp.path().join(".claude/skills/my-skill/old.txt"), "old").unwrap();

        let result_without_force = execute(&default_config(), tmp.path(), &default_opts());
        assert!(result_without_force.warnings.iter().any(|w| matches!(
            w,
            SyncWarning::FileConflict { skill, .. } if skill == "my-skill"
        )));
        assert!(tmp.path().join(".claude/skills/my-skill").is_dir());

        let opts = SyncOptions {
            force: true,
            ..default_opts()
        };
        let result_with_force = execute(&default_config(), tmp.path(), &opts);

        // 심링크로 대체됨
        assert!(tmp.path().join(".claude/skills/my-skill").is_symlink());
        assert!(result_with_force.warnings.is_empty());
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
        assert!(
            result
                .instructions_skipped
                .contains(&"opencode".to_string())
        );
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
    fn test_sync_existing_real_instruction_file_requires_force() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());
        fs::write(tmp.path().join("CLAUDE.md"), "# Real file").unwrap();

        let result_without_force = execute(&default_config(), tmp.path(), &default_opts());
        assert!(result_without_force.warnings.iter().any(|w| matches!(
            w,
            SyncWarning::InstructionConflict { file } if file == "CLAUDE.md"
        )));
        assert!(tmp.path().join("CLAUDE.md").is_file());

        let opts = SyncOptions {
            force: true,
            ..default_opts()
        };
        let result_with_force = execute(&default_config(), tmp.path(), &opts);

        assert!(tmp.path().join("CLAUDE.md").is_symlink());
        assert!(result_with_force.warnings.is_empty());
    }

    // === 글로벌 지침 동기화 ===

    #[test]
    fn test_sync_global_instructions() {
        let tmp = TempDir::new().unwrap();
        // 글로벌 소스 생성
        fs::create_dir_all(tmp.path().join(".agents/skills")).unwrap();
        fs::write(tmp.path().join("AGENTS.md"), "# Global Instructions").unwrap();

        let mut config = default_config();
        // 테스트에서는 tmp 경로를 글로벌 기준 루트로 사용
        config.source.skills_path_global = ".agents/skills".to_string();
        config.source.instruction_path_global = "AGENTS.md".to_string();

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

    #[test]
    fn test_sync_dry_run_includes_collected_skills_when_source_missing() {
        let tmp = TempDir::new().unwrap();
        let pi_new = tmp.path().join(".pi/skills/new-skill");
        fs::create_dir_all(&pi_new).unwrap();
        fs::write(pi_new.join("SKILL.md"), "# New").unwrap();

        let opts = SyncOptions {
            dry_run: true,
            ..default_opts()
        };
        let result = execute(&default_config(), tmp.path(), &opts);

        assert!(
            result
                .skills_collected
                .iter()
                .any(|(name, agent)| name == "new-skill" && agent == "pi")
        );
        assert!(
            result
                .skills_linked
                .iter()
                .any(|(name, agent)| name == "new-skill" && agent == "claude")
        );
        assert!(
            result
                .skills_linked
                .iter()
                .any(|(name, agent)| name == "new-skill" && agent == "opencode")
        );
        assert!(result.warnings.iter().any(|w| matches!(
            w,
            SyncWarning::FileConflict { skill, agent }
                if skill == "new-skill" && agent == "pi"
        )));
        assert!(!tmp.path().join(".agents/skills").exists());
        assert!(pi_new.is_dir());
    }

    // === 소스 없을 때 ===

    #[test]
    fn test_sync_creates_source_and_collects_from_pi_when_source_missing() {
        let tmp = TempDir::new().unwrap();
        let pi_new = tmp.path().join(".pi/skills/new-skill");
        fs::create_dir_all(&pi_new).unwrap();
        fs::write(pi_new.join("SKILL.md"), "# New").unwrap();

        let result = execute(&default_config(), tmp.path(), &default_opts());

        assert!(tmp.path().join(".agents/skills").exists());
        assert!(tmp.path().join(".agents/skills/new-skill").is_dir());
        assert!(tmp.path().join(".pi/skills/new-skill").is_symlink());
        assert!(result.warnings.is_empty());
        assert!(
            result
                .skills_collected
                .iter()
                .any(|(name, agent)| name == "new-skill" && agent == "pi")
        );
    }

    #[test]
    fn test_sync_no_instructions_source() {
        let tmp = TempDir::new().unwrap();

        let result = execute(&default_config(), tmp.path(), &default_opts());

        assert!(result.instructions_linked.is_empty());
    }
}
