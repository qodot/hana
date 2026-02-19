use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{AgentName, Config, TargetFeature};
use crate::config::ConfigError;
use crate::helper::broadcast_target_symlink::broadcast_target_symlink;
use crate::helper::collect_source_skills::collect_source_skills;
use crate::helper::collect_target_skills::collect_target_skills;
use crate::helper::move_target_skills::move_target_skills;
use crate::helper::resolve_target_destinations::resolve_target_destinations;

#[derive(Debug)]
pub enum SyncWarning {
    /// 스킬 이름 충돌: 여러 에이전트에서 동일 이름 발견
    SkillConflict { name: String, agents: Vec<String> },
    /// 소스에 동일 이름 스킬이 이미 존재 (--force로 덮어쓰기 가능)
    SourceSkillConflict { skill: String, agent: String },
    /// 기존 파일/디렉토리 충돌 (--force 필요)
    FileConflict { skill: String, agent: String },
    /// 지침 파일 충돌 (--force 필요)
    InstructionConflict { file: String },
    /// 파일시스템 작업 실패
    IoFailed { operation: String, detail: String },
}

#[derive(Debug)]
pub enum SyncError {
    /// 설정 파일 로딩 실패
    Config(ConfigError),
    /// 홈 디렉토리를 찾을 수 없음
    NoHomeDir,
}

impl std::fmt::Display for SyncWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SkillConflict { name, agents } => {
                write!(f, "스킬 이름 충돌: '{name}' — {}", agents.join(", "))
            }
            Self::SourceSkillConflict { skill, agent } => {
                write!(
                    f,
                    "수집 건너뜀: {skill} ({agent}) — 소스에 동일 이름 스킬이 이미 존재합니다. --force로 덮어쓸 수 있습니다."
                )
            }
            Self::FileConflict { skill, agent } => {
                write!(
                    f,
                    "충돌: {skill} ({agent}) 에 실제 파일/디렉토리 존재. --force로 덮어쓰세요."
                )
            }
            Self::InstructionConflict { file } => {
                write!(
                    f,
                    "{file} 가 이미 존재합니다 (심링크가 아님). --force로 덮어쓰세요."
                )
            }
            Self::IoFailed { operation, detail } => {
                write!(f, "{operation}: {detail}")
            }
        }
    }
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(e) => write!(f, "{e}"),
            Self::NoHomeDir => write!(f, "홈 디렉토리를 찾을 수 없습니다."),
        }
    }
}

impl From<ConfigError> for SyncError {
    fn from(e: ConfigError) -> Self {
        Self::Config(e)
    }
}

#[derive(Debug)]
pub struct SyncOk {
    pub skills_linked: Vec<(String, String)>,
    pub skills_collected: Vec<(String, String)>,
    pub instructions_linked: Vec<String>,
    pub instructions_skipped: Vec<String>,
    pub cleaned: Vec<PathBuf>,
    pub warnings: Vec<SyncWarning>,
}

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

#[derive(Debug, Default)]
pub struct SyncOptions {
    pub dry_run: bool,
    pub force: bool,
    pub global: bool,
}

pub fn execute(config: &Config, base_dir: &Path, opts: &SyncOptions) -> SyncOk {
    let skills = sync_skills(config, base_dir, opts);
    let instructions = sync_instructions(config, base_dir, opts);

    SyncOk {
        skills_linked: skills.linked,
        skills_collected: skills.collected,
        instructions_linked: instructions.linked,
        instructions_skipped: instructions.skipped,
        cleaned: skills.cleaned,
        warnings: skills
            .warnings
            .into_iter()
            .chain(instructions.warnings)
            .collect(),
    }
}

// --- Internal result types ---

#[derive(Default)]
struct SkillsSyncResult {
    linked: Vec<(String, String)>,
    collected: Vec<(String, String)>,
    cleaned: Vec<PathBuf>,
    warnings: Vec<SyncWarning>,
}

#[derive(Default)]
struct InstructionsSyncResult {
    linked: Vec<String>,
    skipped: Vec<String>,
    warnings: Vec<SyncWarning>,
}

enum InstructionLinkOutcome {
    AlreadyValid,
    Linked,
    Warning(SyncWarning),
}

// --- Skills sync ---

fn sync_skills(config: &Config, base_dir: &Path, opts: &SyncOptions) -> SkillsSyncResult {
    let source_dir = config.resolve_source_skills_path(base_dir, opts.global);

    if !source_dir.exists() && !opts.dry_run {
        if let Err(e) = fs::create_dir_all(&source_dir) {
            return SkillsSyncResult {
                warnings: vec![SyncWarning::IoFailed {
                    operation: format!("소스 디렉토리 생성 ({})", source_dir.display()),
                    detail: e.to_string(),
                }],
                ..Default::default()
            };
        }
    }

    // 1단계: source에 모든 스킬 수집
    let collected_by_agent = collect_target_skills(config, base_dir, opts.global);
    let move_result =
        move_target_skills(&collected_by_agent, &source_dir, opts.force, opts.dry_run);
    let (tasks, move_warnings) = match move_result {
        Ok(ok) => (ok.tasks, vec![]),
        Err(err) => (err.tasks, err.warnings),
    };
    let collected: Vec<_> = tasks
        .iter()
        .map(|t| (t.skill.clone(), t.agent.as_str().to_string()))
        .collect();

    // 2단계: target에 동기화 (심링크 생성)
    let source_skills = match collect_source_skills(&source_dir) {
        Ok(skills) => skills,
        Err(warning) => {
            return SkillsSyncResult {
                collected,
                warnings: move_warnings.into_iter().chain(Some(warning)).collect(),
                ..Default::default()
            };
        }
    };

    // dry-run에서는 실제 이동이 없으므로, 이번 실행에서 수집 예정인 스킬을 추가로 포함해 동기화 대상을 계산한다.
    let skills: Vec<String> = source_skills
        .into_iter()
        .chain(if opts.dry_run {
            collected.iter().map(|(name, _)| name.clone()).collect()
        } else {
            vec![]
        })
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect();

    let enabled_targets = resolve_target_destinations(config, base_dir, opts.global);
    let (linked, broadcast_warnings) =
        broadcast_skills(&source_dir, &skills, &enabled_targets, opts);

    // 3단계: 깨진 심링크 정리
    let cleaned = clean_broken_symlinks(&enabled_targets, opts.dry_run);

    SkillsSyncResult {
        linked,
        collected,
        cleaned,
        warnings: move_warnings
            .into_iter()
            .chain(broadcast_warnings)
            .collect(),
    }
}

fn broadcast_skills(
    source_dir: &Path,
    skills: &[String],
    targets: &HashMap<AgentName, PathBuf>,
    opts: &SyncOptions,
) -> (Vec<(String, String)>, Vec<SyncWarning>) {
    let mut linked = Vec::new();
    let mut warnings = Vec::new();

    for skill in skills {
        let source = source_dir.join(skill);
        let (ok_linked, conflicts, failed) =
            match broadcast_target_symlink(&source, targets, opts.dry_run, opts.force) {
                Ok(ok) => (ok.linked, vec![], vec![]),
                Err(err) => (err.linked, err.conflicts, err.failed),
            };

        linked.extend(
            ok_linked
                .iter()
                .map(|a| (skill.clone(), a.as_str().to_string())),
        );
        warnings.extend(conflicts.iter().map(|a| SyncWarning::FileConflict {
            skill: skill.clone(),
            agent: a.as_str().to_string(),
        }));
        warnings.extend(failed.iter().map(|(a, d)| SyncWarning::IoFailed {
            operation: format!("심링크 생성 ({skill}, {})", a.as_str()),
            detail: d.clone(),
        }));
    }

    (linked, warnings)
}

fn clean_broken_symlinks(targets: &HashMap<AgentName, PathBuf>, dry_run: bool) -> Vec<PathBuf> {
    let broken: Vec<PathBuf> = targets
        .values()
        .filter(|dir| dir.exists())
        .flat_map(|dir| {
            fs::read_dir(dir)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
        })
        .map(|e| e.path())
        .filter(|p| p.is_symlink() && !p.exists())
        .collect();

    if !dry_run {
        for path in &broken {
            let _ = fs::remove_file(path);
        }
    }

    broken
}

// --- Instructions sync ---

fn sync_instructions(
    config: &Config,
    base_dir: &Path,
    opts: &SyncOptions,
) -> InstructionsSyncResult {
    let source_path = config.resolve_source_instruction_path(base_dir, opts.global);
    if !source_path.exists() {
        return InstructionsSyncResult::default();
    }

    let mut linked = Vec::new();
    let mut skipped = Vec::new();
    let mut warnings = Vec::new();

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

        if link_path == source_path {
            skipped.push(agent_name.to_string());
            continue;
        }

        match link_instruction(&source_path, &link_path, display_name, opts) {
            InstructionLinkOutcome::Linked => linked.push(agent_name.to_string()),
            InstructionLinkOutcome::AlreadyValid => {}
            InstructionLinkOutcome::Warning(w) => warnings.push(w),
        }
    }

    InstructionsSyncResult {
        linked,
        skipped,
        warnings,
    }
}

fn link_instruction(
    source_path: &Path,
    link_path: &Path,
    display_name: &str,
    opts: &SyncOptions,
) -> InstructionLinkOutcome {
    // 이미 올바른 심링크면 스킵
    if link_path.is_symlink() {
        if let Ok(target) = fs::read_link(link_path) {
            if target == source_path {
                return InstructionLinkOutcome::AlreadyValid;
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
            return InstructionLinkOutcome::Warning(SyncWarning::InstructionConflict {
                file: display_name.to_string(),
            });
        }
    }

    if !opts.dry_run {
        if let Some(parent) = link_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if link_path.is_symlink() {
            let _ = fs::remove_file(link_path);
        }
        if let Err(e) = std::os::unix::fs::symlink(source_path, link_path) {
            return InstructionLinkOutcome::Warning(SyncWarning::IoFailed {
                operation: format!("지침 심링크 ({display_name})"),
                detail: e.to_string(),
            });
        }
    }

    InstructionLinkOutcome::Linked
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
        fs::create_dir_all(tmp.path().join(".agents/skills")).unwrap();
        fs::write(tmp.path().join("AGENTS.md"), "# Global Instructions").unwrap();

        let mut config = default_config();
        config.source.skills_path_global = ".agents/skills".to_string();
        config.source.instruction_path_global = "AGENTS.md".to_string();

        let opts = SyncOptions {
            global: true,
            ..default_opts()
        };
        let _result = execute(&config, tmp.path(), &opts);

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
