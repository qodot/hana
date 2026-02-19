use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::AgentName;
use crate::error::SyncWarning;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillLinkTask {
    pub skill: String,
    pub agent: AgentName,
    pub target_path: PathBuf,
    pub link_path: PathBuf,
}

#[derive(Debug, Default)]
pub struct MoveOk {
    pub tasks: Vec<SkillLinkTask>,
}

#[derive(Debug, Default)]
pub struct MoveErr {
    pub tasks: Vec<SkillLinkTask>,
    pub warnings: Vec<SyncWarning>,
}

pub fn mv_skills(
    collected_by_agent: &HashMap<AgentName, Vec<(String, PathBuf)>>,
    source_dir: &Path,
    force: bool,
    dry_run: bool,
) -> Result<MoveOk, MoveErr> {
    let skill_names: Vec<String> = collected_by_agent
        .values()
        .flat_map(|skills| skills.iter().map(|(name, _)| name.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let mut tasks = Vec::new();
    let mut warnings = Vec::new();

    for name in &skill_names {
        let sources: Vec<(AgentName, PathBuf)> = collected_by_agent
            .iter()
            .flat_map(|(agent, skills)| {
                skills
                    .iter()
                    .filter(|(skill_name, _)| skill_name == name)
                    .map(|(_, path)| (*agent, path.clone()))
            })
            .collect();

        if sources.len() > 1 {
            let agent_names: Vec<String> = sources
                .iter()
                .map(|(a, _)| a.as_str().to_string())
                .collect();
            warnings.push(SyncWarning::SkillConflict {
                name: name.clone(),
                agents: agent_names,
            });
            continue;
        }

        if sources.is_empty() {
            continue;
        }

        let (agent, path) = &sources[0];
        let dest = source_dir.join(name.as_str());

        if dest.exists() {
            if force {
                if !dry_run {
                    let remove_result = if dest.is_symlink() {
                        fs::remove_file(&dest)
                    } else if dest.is_dir() {
                        fs::remove_dir_all(&dest)
                    } else {
                        fs::remove_file(&dest)
                    };
                    if let Err(e) = remove_result {
                        warnings.push(SyncWarning::IoFailed {
                            operation: format!("기존 소스 스킬 제거 ({name})"),
                            detail: e.to_string(),
                        });
                        continue;
                    }
                }
            } else {
                warnings.push(SyncWarning::SourceSkillConflict {
                    skill: name.clone(),
                    agent: agent.as_str().to_string(),
                });
                continue;
            }
        }

        if !dry_run {
            if let Err(e) = fs::rename(path, &dest) {
                warnings.push(SyncWarning::IoFailed {
                    operation: format!("스킬 수집 ({name}, {agent})"),
                    detail: e.to_string(),
                });
                continue;
            }
        }

        tasks.push(SkillLinkTask {
            skill: name.clone(),
            agent: *agent,
            target_path: dest,
            link_path: path.clone(),
        });
    }

    if warnings.is_empty() {
        Ok(MoveOk { tasks })
    } else {
        Err(MoveErr { tasks, warnings })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_mv_skills_moves_and_returns_task() {
        let tmp = TempDir::new().unwrap();
        let source_dir = tmp.path().join(".agents/skills");
        fs::create_dir_all(&source_dir).unwrap();

        let pi_skill = tmp.path().join(".pi/skills/new-skill");
        fs::create_dir_all(&pi_skill).unwrap();
        fs::write(pi_skill.join("SKILL.md"), "# New").unwrap();

        let collected_by_agent = HashMap::from([(
            AgentName::Pi,
            vec![("new-skill".to_string(), pi_skill.clone())],
        )]);

        let move_result = mv_skills(&collected_by_agent, &source_dir, false, false).unwrap();
        let tasks = move_result.tasks;

        assert!(source_dir.join("new-skill").is_dir());
        assert!(!pi_skill.exists());
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].skill, "new-skill");
        assert_eq!(tasks[0].agent, AgentName::Pi);
        assert_eq!(tasks[0].target_path, source_dir.join("new-skill"));
        assert_eq!(tasks[0].link_path, pi_skill);
    }

    #[test]
    fn test_mv_skills_warns_on_existing_source_without_force() {
        let tmp = TempDir::new().unwrap();
        let source_dir = tmp.path().join(".agents/skills");
        fs::create_dir_all(source_dir.join("my-skill")).unwrap();
        fs::write(source_dir.join("my-skill/SKILL.md"), "# Source").unwrap();

        let pi_skill = tmp.path().join(".pi/skills/my-skill");
        fs::create_dir_all(&pi_skill).unwrap();
        fs::write(pi_skill.join("SKILL.md"), "# Pi").unwrap();

        let collected_by_agent = HashMap::from([(
            AgentName::Pi,
            vec![("my-skill".to_string(), pi_skill.clone())],
        )]);

        let move_result = mv_skills(&collected_by_agent, &source_dir, false, false).unwrap_err();
        let tasks = move_result.tasks;

        assert!(tasks.is_empty());
        assert!(move_result.warnings.iter().any(|w| matches!(
            w,
            SyncWarning::SourceSkillConflict { skill, agent }
                if skill == "my-skill" && agent == "pi"
        )));
        assert!(pi_skill.is_dir());
        assert!(!pi_skill.is_symlink());
        assert_eq!(
            fs::read_to_string(source_dir.join("my-skill/SKILL.md")).unwrap(),
            "# Source"
        );
    }

    #[test]
    fn test_mv_skills_overwrites_existing_source_with_force() {
        let tmp = TempDir::new().unwrap();
        let source_dir = tmp.path().join(".agents/skills");
        fs::create_dir_all(source_dir.join("my-skill")).unwrap();
        fs::write(source_dir.join("my-skill/SKILL.md"), "# Source").unwrap();

        let pi_skill = tmp.path().join(".pi/skills/my-skill");
        fs::create_dir_all(&pi_skill).unwrap();
        fs::write(pi_skill.join("SKILL.md"), "# Pi").unwrap();

        let collected_by_agent = HashMap::from([(
            AgentName::Pi,
            vec![("my-skill".to_string(), pi_skill.clone())],
        )]);

        let move_result = mv_skills(&collected_by_agent, &source_dir, true, false).unwrap();
        let tasks = move_result.tasks;

        assert_eq!(tasks.len(), 1);
        assert_eq!(
            fs::read_to_string(source_dir.join("my-skill/SKILL.md")).unwrap(),
            "# Pi"
        );
        assert!(!pi_skill.exists());
    }

    #[test]
    fn test_mv_skills_warns_on_conflict_between_agents() {
        let tmp = TempDir::new().unwrap();
        let source_dir = tmp.path().join(".agents/skills");
        fs::create_dir_all(&source_dir).unwrap();

        let pi_skill = tmp.path().join(".pi/skills/dup-skill");
        fs::create_dir_all(&pi_skill).unwrap();
        let claude_skill = tmp.path().join(".claude/skills/dup-skill");
        fs::create_dir_all(&claude_skill).unwrap();

        let collected_by_agent = HashMap::from([
            (
                AgentName::Pi,
                vec![("dup-skill".to_string(), pi_skill.clone())],
            ),
            (
                AgentName::Claude,
                vec![("dup-skill".to_string(), claude_skill.clone())],
            ),
        ]);

        let move_result = mv_skills(&collected_by_agent, &source_dir, false, false).unwrap_err();
        let tasks = move_result.tasks;

        assert!(tasks.is_empty());
        assert!(move_result.warnings.iter().any(|w| matches!(
            w,
            SyncWarning::SkillConflict { name, agents }
                if name == "dup-skill"
                    && agents.contains(&"pi".to_string())
                    && agents.contains(&"claude".to_string())
        )));
        assert!(!source_dir.join("dup-skill").exists());
        assert!(pi_skill.is_dir());
        assert!(claude_skill.is_dir());
    }

    #[test]
    fn test_mv_skills_dry_run_returns_tasks_without_fs_changes() {
        let tmp = TempDir::new().unwrap();
        let source_dir = tmp.path().join(".agents/skills");
        fs::create_dir_all(&source_dir).unwrap();

        let pi_skill = tmp.path().join(".pi/skills/new-skill");
        fs::create_dir_all(&pi_skill).unwrap();
        fs::write(pi_skill.join("SKILL.md"), "# New").unwrap();

        let collected_by_agent = HashMap::from([(
            AgentName::Pi,
            vec![("new-skill".to_string(), pi_skill.clone())],
        )]);

        let move_result = mv_skills(&collected_by_agent, &source_dir, false, true).unwrap();
        let tasks = move_result.tasks;

        assert_eq!(tasks.len(), 1);
        assert!(!source_dir.join("new-skill").exists());
        assert!(pi_skill.is_dir());
        assert!(!pi_skill.is_symlink());
        assert_eq!(tasks[0].skill, "new-skill");
        assert_eq!(tasks[0].agent, AgentName::Pi);
        assert_eq!(tasks[0].target_path, source_dir.join("new-skill"));
        assert_eq!(tasks[0].link_path, pi_skill);
    }
}
