use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct BroadcastOk {
    pub linked: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct BroadcastErr {
    pub linked: Vec<PathBuf>,
    pub conflicts: Vec<PathBuf>,
    pub failed: Vec<(PathBuf, String)>,
}

/// source 하나를 여러 dest에 심링크로 전파한다.
pub fn broadcast_symlink(
    source: &Path,
    dests: &[PathBuf],
    dry_run: bool,
    force: bool,
) -> Result<BroadcastOk, BroadcastErr> {
    let mut linked = Vec::new();
    let mut conflicts = Vec::new();
    let mut failed = Vec::new();

    for dest in dests {
        match link_one(source, dest, dry_run, force) {
            LinkOutcome::Created => linked.push(dest.clone()),
            LinkOutcome::AlreadyValid => {}
            LinkOutcome::Conflict => conflicts.push(dest.clone()),
            LinkOutcome::Failed(detail) => failed.push((dest.clone(), detail)),
        }
    }

    if conflicts.is_empty() && failed.is_empty() {
        Ok(BroadcastOk { linked })
    } else {
        Err(BroadcastErr {
            linked,
            conflicts,
            failed,
        })
    }
}

enum LinkOutcome {
    Created,
    AlreadyValid,
    Conflict,
    Failed(String),
}

fn link_one(source: &Path, dest: &Path, dry_run: bool, force: bool) -> LinkOutcome {
    // 이미 올바른 심링크면 스킵
    if dest.is_symlink() {
        if let Ok(target) = fs::read_link(dest) {
            if target == source {
                return LinkOutcome::AlreadyValid;
            }
        }
    }

    // 실제 디렉토리/파일이 존재하면
    if dest.exists() && !dest.is_symlink() {
        if force {
            if !dry_run {
                if dest.is_dir() {
                    let _ = fs::remove_dir_all(dest);
                } else {
                    let _ = fs::remove_file(dest);
                }
            }
        } else {
            return LinkOutcome::Conflict;
        }
    }

    if !dry_run {
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        // 잘못된 심링크 제거
        if dest.is_symlink() {
            let _ = fs::remove_file(dest);
        }
        if let Err(e) = std::os::unix::fs::symlink(source, dest) {
            return LinkOutcome::Failed(e.to_string());
        }
    }

    LinkOutcome::Created
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_creates_symlinks() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        fs::create_dir_all(&source).unwrap();

        let dests = vec![
            tmp.path().join("agent1/skill-a"),
            tmp.path().join("agent2/skill-a"),
        ];
        for d in &dests {
            fs::create_dir_all(d.parent().unwrap()).unwrap();
        }

        let result = broadcast_symlink(&source, &dests, false, false).unwrap();

        assert_eq!(result.linked.len(), 2);
        assert_eq!(fs::read_link(&dests[0]).unwrap(), source);
        assert_eq!(fs::read_link(&dests[1]).unwrap(), source);
    }

    #[test]
    fn test_skips_already_valid() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        fs::create_dir_all(&source).unwrap();

        let dest = tmp.path().join("agent1/skill-a");
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&source, &dest).unwrap();

        let result = broadcast_symlink(&source, &[dest], false, false).unwrap();

        assert!(result.linked.is_empty());
    }

    #[test]
    fn test_conflict_without_force() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        fs::create_dir_all(&source).unwrap();

        let dest = tmp.path().join("agent1/skill-a");
        fs::create_dir_all(&dest).unwrap();

        let err = broadcast_symlink(&source, &[dest.clone()], false, false).unwrap_err();

        assert!(err.linked.is_empty());
        assert_eq!(err.conflicts, vec![dest.clone()]);
        assert!(dest.is_dir());
    }

    #[test]
    fn test_force_replaces_existing_dir() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        fs::create_dir_all(&source).unwrap();

        let dest = tmp.path().join("agent1/skill-a");
        fs::create_dir_all(&dest).unwrap();

        let result = broadcast_symlink(&source, &[dest.clone()], false, true).unwrap();

        assert_eq!(result.linked, vec![dest.clone()]);
        assert!(dest.is_symlink());
        assert_eq!(fs::read_link(&dest).unwrap(), source);
    }

    #[test]
    fn test_force_replaces_existing_file() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        fs::create_dir_all(&source).unwrap();

        let dest = tmp.path().join("agent1/skill-a");
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::write(&dest, "existing").unwrap();

        let result = broadcast_symlink(&source, &[dest.clone()], false, true).unwrap();

        assert_eq!(result.linked, vec![dest.clone()]);
        assert!(dest.is_symlink());
    }

    #[test]
    fn test_dry_run_no_fs_changes() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        fs::create_dir_all(&source).unwrap();

        let dest = tmp.path().join("agent1/skill-a");
        fs::create_dir_all(dest.parent().unwrap()).unwrap();

        let result = broadcast_symlink(&source, &[dest.clone()], true, false).unwrap();

        assert_eq!(result.linked, vec![dest.clone()]);
        assert!(!dest.exists());
    }

    #[test]
    fn test_replaces_wrong_symlink() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        let wrong = tmp.path().join("source/skill-b");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&wrong).unwrap();

        let dest = tmp.path().join("agent1/skill-a");
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&wrong, &dest).unwrap();

        let result = broadcast_symlink(&source, &[dest.clone()], false, false).unwrap();

        assert_eq!(result.linked, vec![dest.clone()]);
        assert_eq!(fs::read_link(&dest).unwrap(), source);
    }

    #[test]
    fn test_creates_parent_directories() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        fs::create_dir_all(&source).unwrap();

        let dest = tmp.path().join("deep/nested/agent/skill-a");

        let result = broadcast_symlink(&source, &[dest.clone()], false, false).unwrap();

        assert_eq!(result.linked, vec![dest.clone()]);
        assert!(dest.is_symlink());
    }

    #[test]
    fn test_partial_success_returns_err() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        fs::create_dir_all(&source).unwrap();

        let good = tmp.path().join("agent1/skill-a");
        fs::create_dir_all(good.parent().unwrap()).unwrap();

        let conflict = tmp.path().join("agent2/skill-a");
        fs::create_dir_all(&conflict).unwrap();

        let err = broadcast_symlink(&source, &[good.clone(), conflict.clone()], false, false)
            .unwrap_err();

        assert_eq!(err.linked, vec![good]);
        assert_eq!(err.conflicts, vec![conflict]);
    }
}
