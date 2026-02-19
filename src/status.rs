use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{Config, ConfigError};

#[derive(Debug, Clone, PartialEq)]
pub enum SkillState {
    Synced,
    RealDir,
    BrokenSymlink,
    Missing,
    WrongTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InstructionState {
    Synced,
    DirectRead,
    RealFile,
    Missing,
    Disabled,
}

#[derive(Debug)]
pub struct StatusOk {
    pub skills: Vec<SkillStatusEntry>,
    pub instructions: InstructionStatusEntry,
}

#[derive(Debug)]
pub struct SkillStatusEntry {
    pub name: String,
    pub agents: Vec<(String, SkillState)>,
}

#[derive(Debug)]
pub struct InstructionStatusEntry {
    pub source: String,
    pub source_exists: bool,
    pub agents: Vec<(String, InstructionState)>,
}

#[derive(Debug)]
pub enum StatusError {
    /// 설정 파일 로딩 실패
    Config(ConfigError),
    /// 홈 디렉토리를 찾을 수 없음
    NoHomeDir,
}

impl std::fmt::Display for StatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(e) => write!(f, "{e}"),
            Self::NoHomeDir => write!(f, "홈 디렉토리를 찾을 수 없습니다."),
        }
    }
}

impl From<ConfigError> for StatusError {
    fn from(e: ConfigError) -> Self {
        Self::Config(e)
    }
}

pub fn run(is_global: bool) -> Result<StatusOk, StatusError> {
    let base_dir = if is_global {
        dirs::home_dir().ok_or(StatusError::NoHomeDir)?
    } else {
        PathBuf::from(".")
    };

    let config_path = base_dir.join(".agents/hana.toml");
    let config = Config::load(&config_path)?;

    Ok(execute(&config, &base_dir, is_global))
}

pub fn format_result(result: &StatusOk) -> String {
    let mut out = String::from("🌸 hana status\n");

    // 스킬
    if result.skills.is_empty() {
        out.push_str("\n스킬: (없음)\n");
    } else {
        out.push_str("\n스킬:\n");
        for skill in &result.skills {
            let states: Vec<String> = skill
                .agents
                .iter()
                .map(|(agent, state)| match state {
                    SkillState::Synced => format!("✅ {agent}"),
                    SkillState::RealDir => format!("⚠️ {agent}(실제)"),
                    SkillState::BrokenSymlink => format!("💔 {agent}(깨짐)"),
                    SkillState::Missing => format!("❌ {agent}"),
                    SkillState::WrongTarget => format!("⚠️ {agent}(다른 타겟)"),
                })
                .collect();
            out.push_str(&format!("  {}  {}\n", skill.name, states.join(" ")));
        }
    }

    // 지침
    out.push_str("\n지침:\n");
    if result.instructions.source_exists {
        out.push_str(&format!("  {}  ✅ 소스\n", result.instructions.source));
    } else {
        out.push_str(&format!("  {}  ❌ 소스 없음\n", result.instructions.source));
    }
    for (agent, state) in &result.instructions.agents {
        match state {
            InstructionState::Synced => {
                out.push_str(&format!("  {agent}  ✅ 심링크\n"));
            }
            InstructionState::DirectRead => {
                out.push_str(&format!("  {agent}  ℹ️  직접 읽음\n"));
            }
            InstructionState::RealFile => {
                out.push_str(&format!("  {agent}  ⚠️ 실제 파일 (충돌)\n"));
            }
            InstructionState::Missing => {
                out.push_str(&format!("  {agent}  ❌ 없음\n"));
            }
            InstructionState::Disabled => {
                out.push_str(&format!("  {agent}  ⏭️  비활성화\n"));
            }
        }
    }

    out
}

pub fn execute(config: &Config, base_dir: &Path, global: bool) -> StatusOk {
    let source_dir = config.resolve_source_skills_path(base_dir, global);

    // 소스 스킬 목록
    let mut skill_names: Vec<String> = if source_dir.exists() {
        fs::read_dir(&source_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect()
    } else {
        vec![]
    };
    skill_names.sort();

    let skill_targets: Vec<(String, PathBuf)> = Config::agent_names()
        .filter_map(|agent| {
            let name = agent.as_str();
            let target_dir = config.resolve_target_skills_path(name, base_dir, global)?;
            if target_dir == source_dir {
                return None; // 소스와 동일 경로는 상태 대상에서 제외
            }
            Some((name.to_string(), target_dir))
        })
        .collect();

    // 스킬 상태
    let skills = skill_names
        .iter()
        .map(|name| {
            let expected_target = source_dir.join(name);
            let agent_states: Vec<(String, SkillState)> = skill_targets
                .iter()
                .filter_map(|(agent, agent_dir)| {
                    let target_config = config.targets.get(agent)?;
                    if !target_config.skills {
                        return Some((agent.clone(), SkillState::Missing));
                    }
                    let link_path = agent_dir.join(name);
                    let state = check_skill_state(&link_path, &expected_target);
                    Some((agent.clone(), state))
                })
                .collect();
            SkillStatusEntry {
                name: name.clone(),
                agents: agent_states,
            }
        })
        .collect();

    // 지침 상태
    let source_path = config.resolve_source_instruction_path(base_dir, global);
    let source_exists = source_path.exists();

    let instruction_agents = Config::agent_names()
        .map(|agent| {
            let name = agent.as_str();
            let disabled = config
                .targets
                .get(name)
                .map(|t| !t.instructions)
                .unwrap_or(true);

            if disabled {
                return (name.to_string(), InstructionState::Disabled);
            }

            let Some(link_path) = config.resolve_target_instruction_path(name, base_dir, global)
            else {
                return (name.to_string(), InstructionState::Missing);
            };

            // 소스와 동일 경로면 직접 읽음
            if link_path == source_path {
                return (name.to_string(), InstructionState::DirectRead);
            }

            if link_path.is_symlink() {
                if let Ok(target) = fs::read_link(&link_path) {
                    if target == source_path {
                        (name.to_string(), InstructionState::Synced)
                    } else {
                        (name.to_string(), InstructionState::Missing)
                    }
                } else {
                    (name.to_string(), InstructionState::Missing)
                }
            } else if link_path.exists() {
                (name.to_string(), InstructionState::RealFile)
            } else {
                (name.to_string(), InstructionState::Missing)
            }
        })
        .collect();

    StatusOk {
        skills,
        instructions: InstructionStatusEntry {
            source: config.source_instruction_path(global).to_string(),
            source_exists,
            agents: instruction_agents,
        },
    }
}

fn check_skill_state(link_path: &Path, expected_target: &Path) -> SkillState {
    if link_path.is_symlink() {
        if !link_path.exists() {
            SkillState::BrokenSymlink
        } else if let Ok(target) = fs::read_link(link_path) {
            if target == expected_target {
                SkillState::Synced
            } else {
                SkillState::WrongTarget
            }
        } else {
            SkillState::BrokenSymlink
        }
    } else if link_path.is_dir() {
        SkillState::RealDir
    } else {
        SkillState::Missing
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
        fs::write(skills.join("my-skill/SKILL.md"), "# Skill").unwrap();
        fs::write(tmp.join("AGENTS.md"), "# Instructions").unwrap();
    }

    #[test]
    fn test_status_all_synced() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        // sync 실행
        let config = default_config();
        crate::sync::execute(&config, tmp.path(), &crate::sync::SyncOptions::default());

        let result = execute(&config, tmp.path(), false);

        assert_eq!(result.skills.len(), 1);
        assert_eq!(result.skills[0].name, "my-skill");
        for (_, state) in &result.skills[0].agents {
            assert_eq!(*state, SkillState::Synced);
        }
    }

    #[test]
    fn test_status_missing_symlinks() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        // sync 안 함 → 심링크 없음
        let config = default_config();
        let result = execute(&config, tmp.path(), false);

        assert_eq!(result.skills.len(), 1);
        for (agent, state) in &result.skills[0].agents {
            assert_eq!(*state, SkillState::Missing, "agent: {agent}");
        }
    }

    #[test]
    fn test_status_real_dir_detected() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        // claude에 실제 디렉토리 생성
        fs::create_dir_all(tmp.path().join(".claude/skills/my-skill")).unwrap();

        let config = default_config();
        let result = execute(&config, tmp.path(), false);

        let claude_state = result.skills[0]
            .agents
            .iter()
            .find(|(a, _)| a == "claude")
            .map(|(_, s)| s)
            .unwrap();
        assert_eq!(*claude_state, SkillState::RealDir);
    }

    #[test]
    fn test_status_broken_symlink() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let claude_dir = tmp.path().join(".claude/skills");
        fs::create_dir_all(&claude_dir).unwrap();
        symlink("/nonexistent", claude_dir.join("my-skill")).unwrap();

        let config = default_config();
        let result = execute(&config, tmp.path(), false);

        let claude_state = result.skills[0]
            .agents
            .iter()
            .find(|(a, _)| a == "claude")
            .map(|(_, s)| s)
            .unwrap();
        assert_eq!(*claude_state, SkillState::BrokenSymlink);
    }

    #[test]
    fn test_status_instruction_synced() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let config = default_config();
        crate::sync::execute(&config, tmp.path(), &crate::sync::SyncOptions::default());

        let result = execute(&config, tmp.path(), false);

        let claude = result
            .instructions
            .agents
            .iter()
            .find(|(a, _)| a == "claude")
            .unwrap();
        assert_eq!(claude.1, InstructionState::Synced);

        let codex = result
            .instructions
            .agents
            .iter()
            .find(|(a, _)| a == "codex")
            .unwrap();
        assert_eq!(codex.1, InstructionState::DirectRead);
    }

    #[test]
    fn test_status_instruction_missing() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let config = default_config();
        let result = execute(&config, tmp.path(), false);

        let claude = result
            .instructions
            .agents
            .iter()
            .find(|(a, _)| a == "claude")
            .unwrap();
        assert_eq!(claude.1, InstructionState::Missing);
    }

    #[test]
    fn test_status_instruction_real_file_conflict() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());
        fs::write(tmp.path().join("CLAUDE.md"), "real file").unwrap();

        let config = default_config();
        let result = execute(&config, tmp.path(), false);

        let claude = result
            .instructions
            .agents
            .iter()
            .find(|(a, _)| a == "claude")
            .unwrap();
        assert_eq!(claude.1, InstructionState::RealFile);
    }

    #[test]
    fn test_status_instruction_disabled() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let mut config = default_config();
        config.targets.get_mut("claude").unwrap().instructions = false;

        let result = execute(&config, tmp.path(), false);

        let claude = result
            .instructions
            .agents
            .iter()
            .find(|(a, _)| a == "claude")
            .unwrap();
        assert_eq!(claude.1, InstructionState::Disabled);
    }

    #[test]
    fn test_status_no_source() {
        let tmp = TempDir::new().unwrap();

        let config = default_config();
        let result = execute(&config, tmp.path(), false);

        assert!(result.skills.is_empty());
        assert!(!result.instructions.source_exists);
    }

    #[test]
    fn test_format_result_output() {
        let result = StatusOk {
            skills: vec![SkillStatusEntry {
                name: "my-skill".to_string(),
                agents: vec![
                    ("claude".to_string(), SkillState::Synced),
                    ("pi".to_string(), SkillState::Missing),
                ],
            }],
            instructions: InstructionStatusEntry {
                source: "AGENTS.md".to_string(),
                source_exists: true,
                agents: vec![
                    ("claude".to_string(), InstructionState::Synced),
                    ("codex".to_string(), InstructionState::DirectRead),
                ],
            },
        };

        let output = format_result(&result);
        assert!(output.contains("✅ claude"));
        assert!(output.contains("❌ pi"));
        assert!(output.contains("직접 읽음"));
    }
}
