use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::{AgentName, Config, TargetFeature};

/// Build a map of target destinations for a given feature (Skills or Instructions).
/// Returns: agent -> destination_path
pub fn resolve_target_destinations(
    config: &Config,
    base_dir: &Path,
    global: bool,
    feature: TargetFeature,
) -> HashMap<AgentName, PathBuf> {
    let source_path = match feature {
        TargetFeature::Skills => config.resolve_source_skills_path(base_dir, global),
        TargetFeature::Instructions => config.resolve_source_instruction_path(base_dir, global),
    };

    config
        .enabled_targets(feature)
        .filter_map(|agent| {
            let dest_path = match feature {
                TargetFeature::Skills => {
                    config.resolve_target_skills_path(agent.as_str(), base_dir, global)?
                }
                TargetFeature::Instructions => {
                    config.resolve_target_instruction_path(agent.as_str(), base_dir, global)?
                }
            };
            if dest_path == source_path {
                return None;
            }

            Some((agent, dest_path))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tempfile::TempDir;

    #[test]
    fn test_resolve_target_destinations_filters_disabled_and_source() {
        let tmp = TempDir::new().unwrap();
        let mut config = Config::default();
        config.targets.get_mut("pi").unwrap().skills = false;

        let destinations =
            resolve_target_destinations(&config, tmp.path(), false, TargetFeature::Skills);

        let agents: Vec<AgentName> = destinations.keys().copied().collect();
        assert!(agents.contains(&AgentName::Claude));
        assert!(agents.contains(&AgentName::Opencode));
        assert!(!agents.contains(&AgentName::Pi));
        assert!(!agents.contains(&AgentName::Codex)); // same path as source
    }

    #[test]
    fn test_resolve_target_destinations_uses_global_paths() {
        let tmp = TempDir::new().unwrap();
        let mut config = Config::default();
        // Override source to match target (in tests, ~ resolves to home, not tmp)
        config.source.skills_path_global = ".agents/skills".to_string();

        let destinations =
            resolve_target_destinations(&config, tmp.path(), true, TargetFeature::Skills);

        // Pi uses .pi/agent/skills (different from source), so included
        assert!(destinations.contains_key(&AgentName::Pi));
        assert_eq!(
            destinations.get(&AgentName::Pi).unwrap(),
            &tmp.path().join(".pi/agent/skills")
        );
        // Codex uses .agents/skills (same as source), so excluded
        assert!(!destinations.contains_key(&AgentName::Codex));
        assert_eq!(
            destinations.get(&AgentName::Opencode).unwrap(),
            &tmp.path().join(".config/opencode/skills")
        );
    }

    #[test]
    fn test_resolve_target_destinations_respects_custom_source_exclusion() {
        let tmp = TempDir::new().unwrap();
        let mut config = Config::default();
        config.source.skills_path = ".opencode/skills".to_string();

        let destinations =
            resolve_target_destinations(&config, tmp.path(), false, TargetFeature::Skills);

        let agents: Vec<AgentName> = destinations.keys().copied().collect();
        assert!(agents.contains(&AgentName::Claude));
        assert!(!agents.contains(&AgentName::Opencode)); // same path as source
    }

    #[test]
    fn test_resolve_target_destinations_instructions() {
        let tmp = TempDir::new().unwrap();
        let config = Config::default();

        let destinations =
            resolve_target_destinations(&config, tmp.path(), false, TargetFeature::Instructions);

        // claude: CLAUDE.md != AGENTS.md → included
        assert!(destinations.contains_key(&AgentName::Claude));
        // codex: AGENTS.md == AGENTS.md → excluded (same as source)
        assert!(!destinations.contains_key(&AgentName::Codex));
        // pi: AGENTS.md == AGENTS.md → excluded
        assert!(!destinations.contains_key(&AgentName::Pi));
        // opencode: AGENTS.md == AGENTS.md → excluded
        assert!(!destinations.contains_key(&AgentName::Opencode));
    }

    #[test]
    fn test_resolve_target_destinations_instructions_global() {
        let tmp = TempDir::new().unwrap();
        let config = Config::default();

        let destinations =
            resolve_target_destinations(&config, tmp.path(), true, TargetFeature::Instructions);

        // In global mode, each agent has a unique path — all included
        assert!(destinations.contains_key(&AgentName::Claude));
        assert!(destinations.contains_key(&AgentName::Codex));
        assert!(destinations.contains_key(&AgentName::Pi));
        assert!(destinations.contains_key(&AgentName::Opencode));
    }
}
