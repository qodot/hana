/// Collect skill paths for each agent.
/// Returns: (agent, path) list. Excludes paths identical to source.
pub fn collect_skills(global: bool, source: &str) -> Vec<(&'static str, &'static str)> {
    let paths: &[(&str, &str)] = if global {
        &[
            ("claude", ".claude/skills"),
            ("codex", ".agents/skills"),
            ("pi", ".agents/skills"),
            ("opencode", ".config/opencode/skills"),
        ]
    } else {
        &[
            ("claude", ".claude/skills"),
            ("codex", ".agents/skills"),
            ("pi", ".agents/skills"),
            ("opencode", ".opencode/skills"),
        ]
    };

    paths
        .iter()
        .filter(|(_, path)| *path != source)
        .copied()
        .collect()
}

/// Collect instruction file paths for each agent.
/// Returns: (agent, path) list. None means the agent reads the source directly.
pub fn collect_instructions(global: bool) -> Vec<(&'static str, Option<&'static str>)> {
    if global {
        // Global: all agents need symlinks
        vec![
            ("claude", Some(".claude/CLAUDE.md")),
            ("codex", Some(".codex/AGENTS.md")),
            ("pi", Some(".pi/agent/AGENTS.md")),
            ("opencode", Some(".config/opencode/AGENTS.md")),
        ]
    } else {
        // Project: only claude needs a symlink, others read AGENTS.md directly
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
        assert!(agents.contains(&"opencode"));
        // codex and pi use the same path as source, so they are excluded
        assert!(!agents.contains(&"codex"));
        assert!(!agents.contains(&"pi"));
    }

    #[test]
    fn test_collect_skills_global_paths() {
        let skills = collect_skills(true, ".agents/skills");
        // pi uses .agents/skills (same as source), so excluded
        assert!(!skills.iter().any(|(a, _)| *a == "pi"));
        let oc = skills.iter().find(|(a, _)| *a == "opencode").unwrap();
        assert_eq!(oc.1, ".config/opencode/skills");
    }

    #[test]
    fn test_collect_instructions_project() {
        let instructions = collect_instructions(false);
        let claude = instructions.iter().find(|(a, _)| *a == "claude").unwrap();
        assert_eq!(claude.1, Some("CLAUDE.md"));
        let codex = instructions.iter().find(|(a, _)| *a == "codex").unwrap();
        assert_eq!(codex.1, None);
    }

    #[test]
    fn test_collect_instructions_global() {
        let instructions = collect_instructions(true);
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
        // With empty source, all 4 agents are included
        let skills = collect_skills(false, "");
        assert_eq!(skills.len(), 4);
        let instructions = collect_instructions(false);
        assert_eq!(instructions.len(), 4);
    }
}
