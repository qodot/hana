use std::path::{Path, PathBuf};

/// Compute the relative path from `base` directory to `target`.
/// Both paths should be absolute for correct results.
pub fn relative_path(base: &Path, target: &Path) -> PathBuf {
    let base_components: Vec<_> = base.components().collect();
    let target_components: Vec<_> = target.components().collect();

    let common_len = base_components
        .iter()
        .zip(target_components.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let mut result = PathBuf::new();
    for _ in common_len..base_components.len() {
        result.push("..");
    }
    for comp in &target_components[common_len..] {
        result.push(comp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sibling_directories() {
        assert_eq!(
            relative_path(Path::new("/a/b/c"), Path::new("/a/b/d")),
            PathBuf::from("../d")
        );
    }

    #[test]
    fn test_child_path() {
        assert_eq!(
            relative_path(Path::new("/a/b"), Path::new("/a/b/c/d")),
            PathBuf::from("c/d")
        );
    }

    #[test]
    fn test_parent_path() {
        assert_eq!(
            relative_path(Path::new("/a/b/c/d"), Path::new("/a/b")),
            PathBuf::from("../..")
        );
    }

    #[test]
    fn test_same_path() {
        assert_eq!(
            relative_path(Path::new("/a/b"), Path::new("/a/b")),
            PathBuf::from("")
        );
    }

    #[test]
    fn test_distant_paths() {
        assert_eq!(
            relative_path(Path::new("/a/b/c"), Path::new("/x/y/z")),
            PathBuf::from("../../../x/y/z")
        );
    }

    #[test]
    fn test_typical_symlink_case() {
        // .claude/skills -> ../../.agents/skills/my-skill
        assert_eq!(
            relative_path(
                Path::new("/project/.claude/skills"),
                Path::new("/project/.agents/skills/my-skill")
            ),
            PathBuf::from("../../.agents/skills/my-skill")
        );
    }

    #[test]
    fn test_instruction_same_dir() {
        // CLAUDE.md -> AGENTS.md (same directory)
        assert_eq!(
            relative_path(
                Path::new("/project"),
                Path::new("/project/AGENTS.md")
            ),
            PathBuf::from("AGENTS.md")
        );
    }
}
