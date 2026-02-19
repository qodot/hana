use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{AgentName, Config, TargetFeature};
use crate::helper::broadcast_target_symlink::{broadcast_target_symlink, link_one, LinkOutcome};
use crate::helper::collect_source_skills::collect_source_skills;
use crate::helper::collect_target_skills::collect_target_skills;
use crate::helper::move_target_skills::move_target_skills;
use crate::helper::resolve_target_destinations::resolve_target_destinations;

// --- Options ---

#[derive(Debug, Default)]
pub struct SyncOptions {
    pub dry_run: bool,
    pub force: bool,
    pub global: bool,
}

// --- Ok ---

#[derive(Debug)]
pub struct SyncOk {
    pub skills_linked: Vec<(String, String)>,
    pub skills_collected: Vec<(String, String)>,
    pub instructions_collected: Option<(String, String)>,
    pub instructions_linked: Vec<String>,
    pub instructions_skipped: Vec<String>,
    pub cleaned: Vec<PathBuf>,
    pub warnings: Vec<SyncWarning>,
}

// --- Warning ---

#[derive(Debug)]
pub enum SyncWarning {
    /// Skill name conflict: same name found in multiple agents
    SkillConflict { name: String, agents: Vec<String> },
    /// Source already has a skill with the same name (use --force to overwrite)
    SourceSkillConflict { skill: String, agent: String },
    /// Existing file/directory conflict (--force required)
    FileConflict { skill: String, agent: String },
    /// Instruction file conflict (--force required)
    InstructionConflict { file: String },
    /// Filesystem operation failed
    IoFailed { operation: String, detail: String },
}

impl std::fmt::Display for SyncWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SkillConflict { name, agents } => {
                write!(
                    f,
                    "skill name conflict: '{name}' found in {}",
                    agents.join(", ")
                )
            }
            Self::SourceSkillConflict { skill, agent } => {
                write!(
                    f,
                    "skipped: {skill} ({agent}) — source already has a skill with the same name. Use --force to overwrite."
                )
            }
            Self::FileConflict { skill, agent } => {
                write!(
                    f,
                    "conflict: {skill} ({agent}) has a real file/directory. Use --force to overwrite."
                )
            }
            Self::InstructionConflict { file } => {
                write!(
                    f,
                    "{file} already exists (not a symlink). Use --force to overwrite."
                )
            }
            Self::IoFailed { operation, detail } => {
                write!(f, "{operation}: {detail}")
            }
        }
    }
}

// --- pub fn run ---

pub fn run(config: &Config, base_dir: &Path, opts: &SyncOptions) -> SyncOk {
    let skills = sync_skills(config, base_dir, opts);
    let instructions = sync_instructions(config, base_dir, opts);

    SyncOk {
        skills_linked: skills.linked,
        skills_collected: skills.collected,
        instructions_collected: instructions.collected,
        instructions_linked: instructions.linked,
        instructions_skipped: instructions.skipped,
        cleaned: skills.cleaned,
        warnings: skills
            .warnings
            .into_iter()
            .chain(instructions.warnings)
            .collect(),
    }
}

// --- Internal result types ---

#[derive(Default)]
struct SkillsSyncResult {
    linked: Vec<(String, String)>,
    collected: Vec<(String, String)>,
    cleaned: Vec<PathBuf>,
    warnings: Vec<SyncWarning>,
}

#[derive(Default)]
struct InstructionsSyncResult {
    collected: Option<(String, String)>,
    linked: Vec<String>,
    skipped: Vec<String>,
    warnings: Vec<SyncWarning>,
}

// --- Skills sync ---

fn sync_skills(config: &Config, base_dir: &Path, opts: &SyncOptions) -> SkillsSyncResult {
    let source_dir = config.resolve_source_skills_path(base_dir, opts.global);

    if !source_dir.exists() && !opts.dry_run {
        if let Err(e) = fs::create_dir_all(&source_dir) {
            return SkillsSyncResult {
                warnings: vec![SyncWarning::IoFailed {
                    operation: format!("create source directory ({})", source_dir.display()),
                    detail: e.to_string(),
                }],
                ..Default::default()
            };
        }
    }

    // Phase 1: Collect skills from agent paths into source
    let collected_by_agent = collect_target_skills(config, base_dir, opts.global);
    let move_result =
        move_target_skills(&collected_by_agent, &source_dir, opts.force, opts.dry_run);
    let (tasks, move_warnings) = match move_result {
        Ok(ok) => (ok.tasks, vec![]),
        Err(err) => (err.tasks, err.warnings),
    };
    let collected: Vec<_> = tasks
        .iter()
        .map(|t| (t.skill.clone(), t.agent.as_str().to_string()))
        .collect();

    // Phase 2: Broadcast source skills to agent paths (create symlinks)
    let source_skills = match collect_source_skills(&source_dir) {
        Ok(skills) => skills,
        Err(warning) => {
            return SkillsSyncResult {
                collected,
                warnings: move_warnings.into_iter().chain(Some(warning)).collect(),
                ..Default::default()
            };
        }
    };

    // In dry-run, actual moves don't happen — include pending collected skills for broadcast calculation
    let skills: Vec<String> = source_skills
        .into_iter()
        .chain(if opts.dry_run {
            collected.iter().map(|(name, _)| name.clone()).collect()
        } else {
            vec![]
        })
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect();

    let enabled_targets =
        resolve_target_destinations(config, base_dir, opts.global, TargetFeature::Skills);
    let collected_set: HashSet<(&str, &str)> = collected
        .iter()
        .map(|(name, agent)| (name.as_str(), agent.as_str()))
        .collect();
    let (linked, broadcast_warnings) =
        broadcast_skills(&source_dir, &skills, &enabled_targets, &collected_set, opts);

    // Phase 3: Clean up broken symlinks
    let cleaned = clean_broken_symlinks(&enabled_targets, opts.dry_run);

    SkillsSyncResult {
        linked,
        collected,
        cleaned,
        warnings: move_warnings
            .into_iter()
            .chain(broadcast_warnings)
            .collect(),
    }
}

fn broadcast_skills(
    source_dir: &Path,
    skills: &[String],
    targets: &HashMap<AgentName, PathBuf>,
    collected: &HashSet<(&str, &str)>,
    opts: &SyncOptions,
) -> (Vec<(String, String)>, Vec<SyncWarning>) {
    let mut linked = Vec::new();
    let mut warnings = Vec::new();

    for skill in skills {
        let source = source_dir.join(skill);
        let (ok_linked, conflicts, failed) =
            match broadcast_target_symlink(&source, targets, opts.dry_run, opts.force) {
                Ok(ok) => (ok.linked, vec![], vec![]),
                Err(err) => (err.linked, err.conflicts, err.failed),
            };

        linked.extend(
            ok_linked
                .iter()
                .map(|a| (skill.clone(), a.as_str().to_string())),
        );
        for a in &conflicts {
            if collected.contains(&(skill.as_str(), a.as_str())) {
                linked.push((skill.clone(), a.as_str().to_string()));
            } else {
                warnings.push(SyncWarning::FileConflict {
                    skill: skill.clone(),
                    agent: a.as_str().to_string(),
                });
            }
        }
        warnings.extend(failed.iter().map(|(a, d)| SyncWarning::IoFailed {
            operation: format!("create symlink ({skill}, {})", a.as_str()),
            detail: d.clone(),
        }));
    }

    (linked, warnings)
}

fn clean_broken_symlinks(targets: &HashMap<AgentName, PathBuf>, dry_run: bool) -> Vec<PathBuf> {
    let broken: Vec<PathBuf> = targets
        .values()
        .filter(|dir| dir.exists())
        .flat_map(|dir| {
            fs::read_dir(dir)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
        })
        .map(|e| e.path())
        .filter(|p| p.is_symlink() && !p.exists())
        .collect();

    if !dry_run {
        for path in &broken {
            let _ = fs::remove_file(path);
        }
    }

    broken
}

// --- Instructions sync ---

fn sync_instructions(
    config: &Config,
    base_dir: &Path,
    opts: &SyncOptions,
) -> InstructionsSyncResult {
    let source_path = config.resolve_source_instruction_path(base_dir, opts.global);

    // If source doesn't exist, try collecting from agent-specific instruction files
    let collected = if !source_path.exists() {
        match collect_instruction(config, base_dir, &source_path, opts) {
            Some(collected) => Some(collected),
            None => return InstructionsSyncResult::default(),
        }
    } else {
        None
    };

    let dest_map =
        resolve_target_destinations(config, base_dir, opts.global, TargetFeature::Instructions);

    // Agents not in dest_map but enabled → they read the source path directly (skipped)
    let skipped: Vec<String> = config
        .enabled_targets(TargetFeature::Instructions)
        .filter(|agent| !dest_map.contains_key(agent))
        .map(|agent| agent.as_str().to_string())
        .collect();

    // Skip the collected agent — already symlinked (in dry-run, file hasn't moved so skip to avoid false conflict)
    let collected_agent: Option<&str> = collected.as_ref().map(|(_, agent)| agent.as_str());

    let mut linked = Vec::new();
    let mut warnings = Vec::new();

    for (agent, dest_path) in &dest_map {
        if collected_agent == Some(agent.as_str()) {
            continue;
        }

        let display_name = config
            .target_instruction_path(agent.as_str(), opts.global)
            .unwrap_or(agent.as_str());

        match link_one(&source_path, dest_path, opts.dry_run, opts.force) {
            LinkOutcome::Created => linked.push(agent.as_str().to_string()),
            LinkOutcome::AlreadyValid => {}
            LinkOutcome::Conflict => {
                warnings.push(SyncWarning::InstructionConflict {
                    file: display_name.to_string(),
                });
            }
            LinkOutcome::Failed(detail) => {
                warnings.push(SyncWarning::IoFailed {
                    operation: format!("instruction symlink ({})", display_name),
                    detail,
                });
            }
        }
    }

    InstructionsSyncResult {
        collected,
        linked,
        skipped,
        warnings,
    }
}

/// When the source instruction file is missing, find an agent-specific instruction file
/// (e.g. CLAUDE.md) that is a real file (not a symlink), move it to the source path,
/// and create a symlink in its place.
fn collect_instruction(
    config: &Config,
    base_dir: &Path,
    source_path: &Path,
    opts: &SyncOptions,
) -> Option<(String, String)> {
    let dest_map =
        resolve_target_destinations(config, base_dir, opts.global, TargetFeature::Instructions);

    // Find the first agent with a real instruction file (not a symlink)
    let candidate = AgentName::iter()
        .filter(|agent| dest_map.contains_key(agent))
        .find_map(|agent| {
            let path = dest_map.get(&agent)?;
            if path.exists() && !path.is_symlink() && path.is_file() {
                Some((agent, path.clone()))
            } else {
                None
            }
        });

    let (agent, agent_path) = candidate?;

    if !opts.dry_run {
        if let Err(e) = fs::rename(&agent_path, source_path) {
            eprintln!(
                "  ⚠ failed to collect instruction ({} → {}): {e}",
                agent_path.display(),
                source_path.display()
            );
            return None;
        }
        if let Err(e) = std::os::unix::fs::symlink(source_path, &agent_path) {
            eprintln!(
                "  ⚠ failed to create symlink ({}): {e}",
                agent_path.display()
            );
        }
    }

    let display_name = config
        .target_instruction_path(agent.as_str(), opts.global)
        .unwrap_or(agent.as_str());

    Some((display_name.to_string(), agent.as_str().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    fn setup_source(tmp: &Path) {
        let skills = tmp.join(".agents/skills");
        fs::create_dir_all(skills.join("my-skill")).unwrap();
        fs::write(skills.join("my-skill/SKILL.md"), "# My Skill").unwrap();
        fs::write(tmp.join("AGENTS.md"), "# Instructions").unwrap();
    }

    #[test]
    fn test_sync_skills_and_instructions() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let result = run(&Config::default(), tmp.path(), &SyncOptions::default());

        assert!(tmp.path().join(".claude/skills/my-skill").is_symlink());
        assert!(tmp.path().join(".pi/skills/my-skill").is_symlink());
        assert!(tmp.path().join(".opencode/skills/my-skill").is_symlink());
        assert!(!tmp.path().join(".agents/skills/my-skill").is_symlink());
        assert!(result.skills_linked.len() >= 3);
        assert!(tmp.path().join("CLAUDE.md").is_symlink());
        assert!(result.instructions_linked.contains(&"claude".to_string()));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_sync_idempotent() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());
        let config = Config::default();

        run(&config, tmp.path(), &SyncOptions::default());
        let result = run(&config, tmp.path(), &SyncOptions::default());

        assert!(result.skills_linked.is_empty());
        assert!(result.instructions_linked.is_empty());
    }

    #[test]
    fn test_sync_collects_and_broadcasts() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let pi_new = tmp.path().join(".pi/skills/new-skill");
        fs::create_dir_all(&pi_new).unwrap();
        fs::write(pi_new.join("SKILL.md"), "# New").unwrap();

        let result = run(&Config::default(), tmp.path(), &SyncOptions::default());

        assert!(tmp.path().join(".agents/skills/new-skill").is_dir());
        assert!(tmp.path().join(".pi/skills/new-skill").is_symlink());
        assert!(tmp.path().join(".claude/skills/new-skill").is_symlink());
        assert!(tmp.path().join(".opencode/skills/new-skill").is_symlink());
        assert!(!result.skills_collected.is_empty());
    }

    #[test]
    fn test_sync_global() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".agents/skills")).unwrap();
        fs::write(tmp.path().join("AGENTS.md"), "# Global").unwrap();

        let mut config = Config::default();
        config.source.skills_path_global = ".agents/skills".to_string();
        config.source.instruction_path_global = "AGENTS.md".to_string();

        let opts = SyncOptions { global: true, ..Default::default() };
        run(&config, tmp.path(), &opts);

        assert!(tmp.path().join(".claude/CLAUDE.md").is_symlink());
        assert!(tmp.path().join(".codex/AGENTS.md").is_symlink());
        assert!(tmp.path().join(".config/opencode/AGENTS.md").is_symlink());
        assert!(tmp.path().join(".pi/agent/AGENTS.md").is_symlink());
    }

    #[test]
    fn test_sync_cleans_broken_symlinks() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let claude_dir = tmp.path().join(".claude/skills");
        fs::create_dir_all(&claude_dir).unwrap();
        symlink("/nonexistent/deleted-skill", claude_dir.join("old-skill")).unwrap();

        let result = run(&Config::default(), tmp.path(), &SyncOptions::default());

        assert!(!claude_dir.join("old-skill").is_symlink());
        assert!(!result.cleaned.is_empty());
    }

    #[test]
    fn test_sync_dry_run() {
        let tmp = TempDir::new().unwrap();
        setup_source(tmp.path());

        let opts = SyncOptions { dry_run: true, ..Default::default() };
        let result = run(&Config::default(), tmp.path(), &opts);

        assert!(!result.skills_linked.is_empty());
        assert!(!tmp.path().join(".claude/skills/my-skill").exists());
        assert!(!tmp.path().join("CLAUDE.md").exists());
    }

    #[test]
    fn test_sync_dry_run_collect_no_false_conflict() {
        let tmp = TempDir::new().unwrap();
        let pi_skill = tmp.path().join(".pi/skills/new-skill");
        fs::create_dir_all(&pi_skill).unwrap();
        fs::write(pi_skill.join("SKILL.md"), "# New").unwrap();

        let opts = SyncOptions { dry_run: true, ..Default::default() };
        let result = run(&Config::default(), tmp.path(), &opts);

        assert!(result.skills_collected.iter().any(|(n, a)| n == "new-skill" && a == "pi"));
        assert!(result.skills_linked.iter().any(|(n, a)| n == "new-skill" && a == "pi"));
        assert!(!result.warnings.iter().any(|w| matches!(w, SyncWarning::FileConflict { agent, .. } if agent == "pi")));
    }

    #[test]
    fn test_sync_collects_instruction_from_claude_md() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("CLAUDE.md"), "# Claude Instructions").unwrap();

        let result = run(&Config::default(), tmp.path(), &SyncOptions::default());

        assert!(tmp.path().join("AGENTS.md").is_file());
        assert!(!tmp.path().join("AGENTS.md").is_symlink());
        assert_eq!(
            fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap(),
            "# Claude Instructions"
        );
        assert!(tmp.path().join("CLAUDE.md").is_symlink());
        assert_eq!(
            fs::read_link(tmp.path().join("CLAUDE.md")).unwrap(),
            tmp.path().join("AGENTS.md")
        );
        assert!(result.instructions_collected.is_some());
        let (file, agent) = result.instructions_collected.unwrap();
        assert_eq!(file, "CLAUDE.md");
        assert_eq!(agent, "claude");
    }

    #[test]
    fn test_sync_collects_instruction_dry_run() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("CLAUDE.md"), "# Claude Instructions").unwrap();

        let opts = SyncOptions { dry_run: true, ..Default::default() };
        let result = run(&Config::default(), tmp.path(), &opts);

        assert!(!tmp.path().join("AGENTS.md").exists());
        assert!(tmp.path().join("CLAUDE.md").is_file());
        assert!(!tmp.path().join("CLAUDE.md").is_symlink());
        assert!(result.instructions_collected.is_some());
        assert!(!result.warnings.iter().any(|w| matches!(
            w,
            SyncWarning::InstructionConflict { file } if file == "CLAUDE.md"
        )));
    }

    #[test]
    fn test_sync_no_collect_when_agents_md_exists() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("AGENTS.md"), "# Source").unwrap();
        fs::write(tmp.path().join("CLAUDE.md"), "# Claude").unwrap();

        let opts = SyncOptions { force: true, ..Default::default() };
        let result = run(&Config::default(), tmp.path(), &opts);

        assert!(result.instructions_collected.is_none());
        assert_eq!(
            fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap(),
            "# Source"
        );
    }

    #[test]
    fn test_sync_no_instruction_when_nothing_exists() {
        let tmp = TempDir::new().unwrap();

        let result = run(&Config::default(), tmp.path(), &SyncOptions::default());

        assert!(result.instructions_collected.is_none());
        assert!(result.instructions_linked.is_empty());
    }
}
