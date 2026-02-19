use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::{AgentName, Config, TargetFeature};

/// 설정(`target.*.skills`)을 기준으로 스킬을 전파할 대상 디렉토리 맵을 만든다.
/// 반환: agent -> destination_dir
pub fn build_destinations(
    config: &Config,
    base_dir: &Path,
    global: bool,
) -> HashMap<AgentName, PathBuf> {
    let source_dir = config.resolve_source_skills_path(base_dir, global);

    config
        .enabled_targets(TargetFeature::Skills)
        .filter_map(|agent| {
            let dest_dir = config.resolve_target_skills_path(agent.as_str(), base_dir, global)?;
            if dest_dir == source_dir {
                return None;
            }

            Some((agent, dest_dir))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tempfile::TempDir;

    #[test]
    fn test_build_destinations_filters_disabled_and_source() {
        let tmp = TempDir::new().unwrap();
        let mut config = Config::default();
        config.targets.get_mut("pi").unwrap().skills = false;

        let destinations = build_destinations(&config, tmp.path(), false);

        let agents: Vec<AgentName> = destinations.keys().copied().collect();
        assert!(agents.contains(&AgentName::Claude));
        assert!(agents.contains(&AgentName::Opencode));
        assert!(!agents.contains(&AgentName::Pi));
        assert!(!agents.contains(&AgentName::Codex)); // source와 동일 경로이므로 제외
    }

    #[test]
    fn test_build_destinations_uses_global_paths() {
        let tmp = TempDir::new().unwrap();
        let config = Config::default();

        let destinations = build_destinations(&config, tmp.path(), true);

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
    fn test_build_destinations_respects_custom_source_exclusion() {
        let tmp = TempDir::new().unwrap();
        let mut config = Config::default();
        config.source.skills_path = ".pi/skills".to_string();

        let destinations = build_destinations(&config, tmp.path(), false);

        let agents: Vec<AgentName> = destinations.keys().copied().collect();
        assert!(agents.contains(&AgentName::Claude));
        assert!(!agents.contains(&AgentName::Pi)); // source와 동일 경로이므로 제외
    }
}
