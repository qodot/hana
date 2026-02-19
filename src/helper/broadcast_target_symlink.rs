use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::AgentName;

#[derive(Debug, Default)]
pub struct BroadcastOk {
    pub linked: Vec<AgentName>,
}

#[derive(Debug, Default)]
pub struct BroadcastErr {
    pub linked: Vec<AgentName>,
    pub conflicts: Vec<AgentName>,
    pub failed: Vec<(AgentName, String)>,
}

/// Broadcast a single source as symlinks to multiple target directories.
pub fn broadcast_target_symlink(
    source: &Path,
    dest_dirs: &HashMap<AgentName, PathBuf>,
    dry_run: bool,
    force: bool,
) -> Result<BroadcastOk, BroadcastErr> {
    let mut linked = Vec::new();
    let mut conflicts = Vec::new();
    let mut failed = Vec::new();

    let Some(source_name) = source.file_name() else {
        failed.extend(
            dest_dirs
                .keys()
                .copied()
                .map(|agent| (agent, "cannot determine source name".to_string())),
        );
        failed.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
        return Err(BroadcastErr {
            linked,
            conflicts,
            failed,
        });
    };

    for (agent, dest_dir) in dest_dirs {
        let dest = dest_dir.join(source_name);
        match link_one(source, &dest, dry_run, force) {
            LinkOutcome::Created => linked.push(*agent),
            LinkOutcome::AlreadyValid => {}
            LinkOutcome::Conflict => conflicts.push(*agent),
            LinkOutcome::Failed(detail) => failed.push((*agent, detail)),
        }
    }

    linked.sort_by_key(|a| a.as_str());
    conflicts.sort_by_key(|a| a.as_str());
    failed.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

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

pub enum LinkOutcome {
    Created,
    AlreadyValid,
    Conflict,
    Failed(String),
}

pub fn link_one(source: &Path, dest: &Path, dry_run: bool, force: bool) -> LinkOutcome {
    // Already a valid symlink — skip
    if dest.is_symlink() {
        if let Ok(target) = fs::read_link(dest) {
            if target == source {
                return LinkOutcome::AlreadyValid;
            }
        }
    }

    // Real file/directory exists at dest
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
        // Remove stale symlink
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

        let claude_dir = tmp.path().join("agent1");
        let pi_dir = tmp.path().join("agent2");
        let dests = HashMap::from([
            (AgentName::Claude, claude_dir.clone()),
            (AgentName::Pi, pi_dir.clone()),
        ]);
        for d in dests.values() {
            fs::create_dir_all(d).unwrap();
        }

        let result = broadcast_target_symlink(&source, &dests, false, false).unwrap();

        assert_eq!(result.linked.len(), 2);
        assert!(result.linked.contains(&AgentName::Claude));
        assert!(result.linked.contains(&AgentName::Pi));
        assert_eq!(fs::read_link(claude_dir.join("skill-a")).unwrap(), source);
        assert_eq!(fs::read_link(pi_dir.join("skill-a")).unwrap(), source);
    }

    #[test]
    fn test_skips_already_valid() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        fs::create_dir_all(&source).unwrap();

        let dest_dir = tmp.path().join("agent1");
        fs::create_dir_all(&dest_dir).unwrap();
        std::os::unix::fs::symlink(&source, dest_dir.join("skill-a")).unwrap();

        let dests = HashMap::from([(AgentName::Claude, dest_dir)]);
        let result = broadcast_target_symlink(&source, &dests, false, false).unwrap();

        assert!(result.linked.is_empty());
    }

    #[test]
    fn test_conflict_without_force() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        fs::create_dir_all(&source).unwrap();

        let dest_dir = tmp.path().join("agent1");
        fs::create_dir_all(dest_dir.join("skill-a")).unwrap();

        let dests = HashMap::from([(AgentName::Claude, dest_dir.clone())]);
        let err = broadcast_target_symlink(&source, &dests, false, false).unwrap_err();

        assert!(err.linked.is_empty());
        assert_eq!(err.conflicts, vec![AgentName::Claude]);
        assert!(dest_dir.join("skill-a").is_dir());
    }

    #[test]
    fn test_force_replaces_existing_dir() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        fs::create_dir_all(&source).unwrap();

        let dest_dir = tmp.path().join("agent1");
        fs::create_dir_all(dest_dir.join("skill-a")).unwrap();

        let dests = HashMap::from([(AgentName::Claude, dest_dir.clone())]);
        let result = broadcast_target_symlink(&source, &dests, false, true).unwrap();

        assert_eq!(result.linked, vec![AgentName::Claude]);
        assert!(dest_dir.join("skill-a").is_symlink());
        assert_eq!(fs::read_link(dest_dir.join("skill-a")).unwrap(), source);
    }

    #[test]
    fn test_force_replaces_existing_file() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        fs::create_dir_all(&source).unwrap();

        let dest_dir = tmp.path().join("agent1");
        fs::create_dir_all(&dest_dir).unwrap();
        fs::write(dest_dir.join("skill-a"), "existing").unwrap();

        let dests = HashMap::from([(AgentName::Claude, dest_dir.clone())]);
        let result = broadcast_target_symlink(&source, &dests, false, true).unwrap();

        assert_eq!(result.linked, vec![AgentName::Claude]);
        assert!(dest_dir.join("skill-a").is_symlink());
    }

    #[test]
    fn test_dry_run_no_fs_changes() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        fs::create_dir_all(&source).unwrap();

        let dest_dir = tmp.path().join("agent1");
        fs::create_dir_all(&dest_dir).unwrap();

        let dests = HashMap::from([(AgentName::Claude, dest_dir.clone())]);
        let result = broadcast_target_symlink(&source, &dests, true, false).unwrap();

        assert_eq!(result.linked, vec![AgentName::Claude]);
        assert!(!dest_dir.join("skill-a").exists());
    }

    #[test]
    fn test_replaces_wrong_symlink() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        let wrong = tmp.path().join("source/skill-b");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&wrong).unwrap();

        let dest_dir = tmp.path().join("agent1");
        fs::create_dir_all(&dest_dir).unwrap();
        std::os::unix::fs::symlink(&wrong, dest_dir.join("skill-a")).unwrap();

        let dests = HashMap::from([(AgentName::Claude, dest_dir.clone())]);
        let result = broadcast_target_symlink(&source, &dests, false, false).unwrap();

        assert_eq!(result.linked, vec![AgentName::Claude]);
        assert_eq!(fs::read_link(dest_dir.join("skill-a")).unwrap(), source);
    }

    #[test]
    fn test_creates_parent_directories() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        fs::create_dir_all(&source).unwrap();

        let dest_dir = tmp.path().join("deep/nested/agent");

        let dests = HashMap::from([(AgentName::Claude, dest_dir.clone())]);
        let result = broadcast_target_symlink(&source, &dests, false, false).unwrap();

        assert_eq!(result.linked, vec![AgentName::Claude]);
        assert!(dest_dir.join("skill-a").is_symlink());
    }

    #[test]
    fn test_partial_success_returns_err() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source/skill-a");
        fs::create_dir_all(&source).unwrap();

        let good_dir = tmp.path().join("agent1");
        fs::create_dir_all(&good_dir).unwrap();

        let conflict_dir = tmp.path().join("agent2");
        fs::create_dir_all(conflict_dir.join("skill-a")).unwrap();

        let dests = HashMap::from([
            (AgentName::Claude, good_dir.clone()),
            (AgentName::Pi, conflict_dir.clone()),
        ]);

        let err = broadcast_target_symlink(&source, &dests, false, false).unwrap_err();

        assert_eq!(err.linked, vec![AgentName::Claude]);
        assert_eq!(err.conflicts, vec![AgentName::Pi]);
        assert!(good_dir.join("skill-a").is_symlink());
    }
}
