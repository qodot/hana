/// 에이전트별 스킬 경로를 수집한다.
/// 반환: (agent, path) 목록. 소스와 동일한 경로는 제외.
pub fn collect_skills(global: bool, source: &str) -> Vec<(&'static str, &'static str)> {
    let paths: &[(&str, &str)] = if global {
        &[
            ("claude", ".claude/skills"),
            ("codex", ".agents/skills"),
            ("pi", ".pi/agent/skills"),
            ("opencode", ".config/opencode/skills"),
        ]
    } else {
        &[
            ("claude", ".claude/skills"),
            ("codex", ".agents/skills"),
            ("pi", ".pi/skills"),
            ("opencode", ".opencode/skills"),
        ]
    };

    paths
        .iter()
        .filter(|(_, path)| *path != source)
        .copied()
        .collect()
}

/// 에이전트별 지침 파일 경로를 수집한다.
/// 반환: (agent, path) 목록. None인 에이전트는 소스를 직접 읽으므로 포함하지 않는다.
pub fn collect_instructions(global: bool) -> Vec<(&'static str, Option<&'static str>)> {
    if global {
        // 글로벌: 모든 에이전트에 심링크 필요
        vec![
            ("claude", Some(".claude/CLAUDE.md")),
            ("codex", Some(".codex/AGENTS.md")),
            ("pi", Some(".pi/agent/AGENTS.md")),
            ("opencode", Some(".config/opencode/AGENTS.md")),
        ]
    } else {
        // 프로젝트: claude만 심링크, 나머지는 AGENTS.md 직접 읽음
        vec![
            ("claude", Some("CLAUDE.md")),
            ("codex", None),
            ("pi", None),
            ("opencode", None),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_skills_excludes_source() {
        let skills = collect_skills(false, ".agents/skills");
        let agents: Vec<&str> = skills.iter().map(|(a, _)| *a).collect();
        assert!(agents.contains(&"claude"));
        assert!(agents.contains(&"pi"));
        assert!(agents.contains(&"opencode"));
        assert!(!agents.contains(&"codex")); // 소스와 동일
    }

    #[test]
    fn test_collect_skills_global_paths() {
        let skills = collect_skills(true, ".agents/skills");
        let pi = skills.iter().find(|(a, _)| *a == "pi").unwrap();
        assert_eq!(pi.1, ".pi/agent/skills");
        let oc = skills.iter().find(|(a, _)| *a == "opencode").unwrap();
        assert_eq!(oc.1, ".config/opencode/skills");
    }

    #[test]
    fn test_collect_instructions_project() {
        let instructions = collect_instructions(false);
        let claude = instructions.iter().find(|(a, _)| *a == "claude").unwrap();
        assert_eq!(claude.1, Some("CLAUDE.md"));
        let codex = instructions.iter().find(|(a, _)| *a == "codex").unwrap();
        assert_eq!(codex.1, None); // 직접 읽음
    }

    #[test]
    fn test_collect_instructions_global() {
        let instructions = collect_instructions(true);
        // 글로벌에서는 모두 Some
        for (_, path) in &instructions {
            assert!(path.is_some());
        }
        let claude = instructions.iter().find(|(a, _)| *a == "claude").unwrap();
        assert_eq!(claude.1, Some(".claude/CLAUDE.md"));
        let pi = instructions.iter().find(|(a, _)| *a == "pi").unwrap();
        assert_eq!(pi.1, Some(".pi/agent/AGENTS.md"));
    }

    #[test]
    fn test_all_agents_covered() {
        let skills = collect_skills(false, "");
        assert_eq!(skills.len(), 4);
        let instructions = collect_instructions(false);
        assert_eq!(instructions.len(), 4);
    }
}
