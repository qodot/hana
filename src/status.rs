use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;

// --- Ok ---

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

// --- pub fn run ---

pub fn run(config: &Config, base_dir: &Path, global: bool) -> StatusOk {
    let source_dir = config.resolve_source_skills_path(base_dir, global);

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
                return None;
            }
            Some((name.to_string(), target_dir))
        })
        .collect();

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

    // Instruction status
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

        let config = default_config();
        crate::sync::run(&config, tmp.path(), &crate::sync::SyncOptions::default());

        let result = run(&config, tmp.path(), false);

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

        let config = default_config();
        let result = run(&config, tmp.path(), false);

        assert_eq!(result.skills.len(), 1);
        for (agent, state) in &result.skills[0].agents {
            assert_eq!(*state, SkillState::Missing, "agent: {agent}");
        }
    }

    #[test]
    fn test_status_real_dir_detected() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        fs::create_dir_all(tmp.path().join(".claude/skills/my-skill")).unwrap();

        let config = default_config();
        let result = run(&config, tmp.path(), false);

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
        let result = run(&config, tmp.path(), false);

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
        crate::sync::run(&config, tmp.path(), &crate::sync::SyncOptions::default());

        let result = run(&config, tmp.path(), false);

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
        let result = run(&config, tmp.path(), false);

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
        let result = run(&config, tmp.path(), false);

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

        let result = run(&config, tmp.path(), false);

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
        let result = run(&config, tmp.path(), false);

        assert!(result.skills.is_empty());
        assert!(!result.instructions.source_exists);
    }

    #[test]
    fn test_status_data_structure() {
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

        assert_eq!(result.skills[0].agents[0].1, SkillState::Synced);
        assert_eq!(result.skills[0].agents[1].1, SkillState::Missing);
        assert!(result.instructions.source_exists);
        assert_eq!(result.instructions.agents[0].1, InstructionState::Synced);
        assert_eq!(result.instructions.agents[1].1, InstructionState::DirectRead);
    }
}
