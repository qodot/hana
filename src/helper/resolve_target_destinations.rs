use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::{AgentName, Config, TargetFeature};

/// 설정(`target.*`)을 기준으로 feature(Skills 또는 Instructions)를 전파할 대상 경로 맵을 만든다.
/// 반환: agent -> destination_path
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
        assert!(!agents.contains(&AgentName::Codex)); // source와 동일 경로이므로 제외
    }

    #[test]
    fn test_resolve_target_destinations_uses_global_paths() {
        let tmp = TempDir::new().unwrap();
        let config = Config::default();

        let destinations =
            resolve_target_destinations(&config, tmp.path(), true, TargetFeature::Skills);

        assert_eq!(
            destinations.get(&AgentName::Pi).unwrap(),
            &tmp.path().join(".pi/agent/skills")
        );
        assert_eq!(
            destinations.get(&AgentName::Opencode).unwrap(),
            &tmp.path().join(".config/opencode/skills")
        );
    }

    #[test]
    fn test_resolve_target_destinations_respects_custom_source_exclusion() {
        let tmp = TempDir::new().unwrap();
        let mut config = Config::default();
        config.source.skills_path = ".pi/skills".to_string();

        let destinations =
            resolve_target_destinations(&config, tmp.path(), false, TargetFeature::Skills);

        let agents: Vec<AgentName> = destinations.keys().copied().collect();
        assert!(agents.contains(&AgentName::Claude));
        assert!(!agents.contains(&AgentName::Pi)); // source와 동일 경로이므로 제외
    }

    #[test]
    fn test_resolve_target_destinations_instructions() {
        let tmp = TempDir::new().unwrap();
        let config = Config::default();

        let destinations =
            resolve_target_destinations(&config, tmp.path(), false, TargetFeature::Instructions);

        // claude: CLAUDE.md != AGENTS.md → 포함
        assert!(destinations.contains_key(&AgentName::Claude));
        // codex: AGENTS.md == AGENTS.md → 제외 (source와 동일)
        assert!(!destinations.contains_key(&AgentName::Codex));
        // pi: AGENTS.md == AGENTS.md → 제외
        assert!(!destinations.contains_key(&AgentName::Pi));
        // opencode: AGENTS.md == AGENTS.md → 제외
        assert!(!destinations.contains_key(&AgentName::Opencode));
    }

    #[test]
    fn test_resolve_target_destinations_instructions_global() {
        let tmp = TempDir::new().unwrap();
        let config = Config::default();

        let destinations =
            resolve_target_destinations(&config, tmp.path(), true, TargetFeature::Instructions);

        // global에서는 각 에이전트마다 다른 경로를 사용하므로 모두 포함
        assert!(destinations.contains_key(&AgentName::Claude));
        assert!(destinations.contains_key(&AgentName::Codex));
        assert!(destinations.contains_key(&AgentName::Pi));
        assert!(destinations.contains_key(&AgentName::Opencode));
    }
}
