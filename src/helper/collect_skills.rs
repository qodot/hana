use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{AgentName, Config};
use crate::helper::build_destinations::build_destinations;

pub fn collect_skills(
    config: &Config,
    base_dir: &Path,
    global: bool,
) -> HashMap<AgentName, Vec<(String, PathBuf)>> {
    build_destinations(config, base_dir, global)
        .into_iter()
        .map(|(agent, agent_dir)| {
            let skills = fs::read_dir(&agent_dir)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .map(|entry| {
                    (
                        entry.file_name().to_string_lossy().to_string(),
                        entry.path(),
                    )
                })
                .filter(|(_, path)| path.is_dir() && !path.is_symlink())
                .collect::<Vec<_>>();
            (agent, skills)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    #[test]
    fn test_collect_skills_filters_non_dirs_symlinks_and_disabled_targets() {
        let tmp = TempDir::new().unwrap();
        let mut config = Config::default();
        config.targets.get_mut("pi").unwrap().skills = false;

        let claude_dir = tmp.path().join(".claude/skills");
        fs::create_dir_all(claude_dir.join("real-skill")).unwrap();
        fs::write(claude_dir.join("real-skill/SKILL.md"), "# Real").unwrap();
        fs::write(claude_dir.join("file-skill"), "not a dir").unwrap();
        symlink(
            claude_dir.join("real-skill"),
            claude_dir.join("link-to-real-skill"),
        )
        .unwrap();

        fs::create_dir_all(tmp.path().join(".pi/skills/pi-skill")).unwrap();

        let result = collect_skills(&config, tmp.path(), false);

        assert!(!result.contains_key(&AgentName::Codex)); // source와 동일한 경로는 제외
        assert!(!result.contains_key(&AgentName::Pi)); // 대상 비활성화
        let claude_skills = result.get(&AgentName::Claude).unwrap();
        assert_eq!(claude_skills.len(), 1);
        assert_eq!(claude_skills[0].0, "real-skill");
    }

    #[test]
    fn test_collect_skills_uses_global_pi_path() {
        let tmp = TempDir::new().unwrap();
        let config = Config::default();

        fs::create_dir_all(tmp.path().join(".pi/agent/skills/global-skill")).unwrap();
        fs::create_dir_all(tmp.path().join(".pi/skills/project-skill")).unwrap();

        let result = collect_skills(&config, tmp.path(), true);
        let pi_skills = result.get(&AgentName::Pi).unwrap();
        let names: Vec<&str> = pi_skills.iter().map(|(name, _)| name.as_str()).collect();

        assert!(names.contains(&"global-skill"));
        assert!(!names.contains(&"project-skill"));
    }

    #[test]
    fn test_collect_skills_respects_custom_source_exclusion() {
        let tmp = TempDir::new().unwrap();
        let mut config = Config::default();
        config.source.skills_path = ".pi/skills".to_string();

        fs::create_dir_all(tmp.path().join(".pi/skills/pi-source-skill")).unwrap();
        fs::create_dir_all(tmp.path().join(".claude/skills/claude-skill")).unwrap();

        let result = collect_skills(&config, tmp.path(), false);

        assert!(!result.contains_key(&AgentName::Pi)); // source와 동일 경로는 제외
        let claude_skills = result.get(&AgentName::Claude).unwrap();
        assert_eq!(claude_skills.len(), 1);
        assert_eq!(claude_skills[0].0, "claude-skill");
    }
}
