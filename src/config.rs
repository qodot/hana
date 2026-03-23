use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ConfigError {
    /// Cannot read config file
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    /// TOML parse error
    Parse { message: String },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadFile { path, source } => {
                write!(f, "cannot read config file ({}): {source}", path.display())
            }
            Self::Parse { message } => write!(f, "TOML parse error: {message}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentName {
    Claude,
    Codex,
    Pi,
    Opencode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetFeature {
    Skills,
    Instructions,
}

impl AgentName {
    pub const ALL: [AgentName; 4] = [
        AgentName::Claude,
        AgentName::Codex,
        AgentName::Pi,
        AgentName::Opencode,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            AgentName::Claude => "claude",
            AgentName::Codex => "codex",
            AgentName::Pi => "pi",
            AgentName::Opencode => "opencode",
        }
    }

    pub fn iter() -> impl Iterator<Item = AgentName> {
        Self::ALL.into_iter()
    }
}

impl fmt::Display for AgentName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct SourceConfig {
    pub skills_path: String,
    pub skills_path_global: String,
    pub instruction_path: String,
    pub instruction_path_global: String,
}

impl Default for SourceConfig {
    fn default() -> Self {
        Self {
            skills_path: ".agents/skills".to_string(),
            skills_path_global: "~/.agents/skills".to_string(),
            instruction_path: "AGENTS.md".to_string(),
            instruction_path_global: "~/.agents/AGENTS.md".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TargetConfig {
    pub skills: bool,
    pub instructions: bool,
    pub skills_path: String,
    pub skills_path_global: String,
    pub instruction_path: String,
    pub instruction_path_global: String,
}

impl TargetConfig {
    pub fn default_for(agent: AgentName) -> Self {
        let (skills_path, skills_path_global) = match agent {
            AgentName::Claude => (".claude/skills", ".claude/skills"),
            AgentName::Codex => (".agents/skills", ".agents/skills"),
            AgentName::Pi => (".pi/skills", ".pi/agent/skills"),
            AgentName::Opencode => (".opencode/skills", ".config/opencode/skills"),
        };

        let (instruction_path, instruction_path_global) = match agent {
            AgentName::Claude => ("CLAUDE.md", ".claude/CLAUDE.md"),
            AgentName::Codex => ("AGENTS.md", ".codex/AGENTS.md"),
            AgentName::Pi => ("AGENTS.md", ".pi/agent/AGENTS.md"),
            AgentName::Opencode => ("AGENTS.md", ".config/opencode/AGENTS.md"),
        };

        Self {
            skills: true,
            instructions: true,
            skills_path: skills_path.to_string(),
            skills_path_global: skills_path_global.to_string(),
            instruction_path: instruction_path.to_string(),
            instruction_path_global: instruction_path_global.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub source: SourceConfig,
    pub targets: HashMap<String, TargetConfig>,
}

impl Default for Config {
    fn default() -> Self {
        let mut targets = HashMap::new();
        for agent in AgentName::iter() {
            targets.insert(agent.as_str().to_string(), TargetConfig::default_for(agent));
        }
        Self {
            source: SourceConfig::default(),
            targets,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path).map_err(|e| ConfigError::ReadFile {
            path: path.to_path_buf(),
            source: e,
        })?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Result<Self, ConfigError> {
        let table: toml::Table =
            content
                .parse()
                .map_err(|e: toml::de::Error| ConfigError::Parse {
                    message: e.to_string(),
                })?;

        let source_table = table.get("source");
        let legacy_skills_source = table
            .get("skills")
            .and_then(|v| v.get("source"))
            .and_then(|v| v.as_str());
        let legacy_instruction_source = table
            .get("instructions")
            .and_then(|v| v.get("source"))
            .and_then(|v| v.as_str());

        let source_defaults = SourceConfig::default();
        let source = SourceConfig {
            skills_path: table_get_str(source_table, "skills_path")
                .or_else(|| legacy_skills_source.map(str::to_string))
                .unwrap_or(source_defaults.skills_path),
            skills_path_global: table_get_str(source_table, "skills_path_global")
                .or_else(|| legacy_skills_source.map(str::to_string))
                .unwrap_or(source_defaults.skills_path_global),
            instruction_path: table_get_str(source_table, "instruction_path")
                .or_else(|| legacy_instruction_source.map(str::to_string))
                .unwrap_or(source_defaults.instruction_path),
            instruction_path_global: table_get_str(source_table, "instruction_path_global")
                .or_else(|| legacy_instruction_source.map(str::to_string))
                .unwrap_or(source_defaults.instruction_path_global),
        };

        let mut targets = HashMap::new();
        for agent in AgentName::iter() {
            let name = agent.as_str();
            let target_table = table.get("target").and_then(|v| v.get(name));
            let legacy_target_table = table.get("targets").and_then(|v| v.get(name));

            let default_target = TargetConfig::default_for(agent);
            let target = TargetConfig {
                skills: table_get_bool(target_table, "skills")
                    .or_else(|| table_get_bool(legacy_target_table, "skills"))
                    .unwrap_or(true),
                instructions: table_get_bool(target_table, "instructions")
                    .or_else(|| table_get_bool(legacy_target_table, "instructions"))
                    .unwrap_or(true),
                skills_path: table_get_str(target_table, "skills_path")
                    .or_else(|| table_get_str(legacy_target_table, "skills_path"))
                    .unwrap_or(default_target.skills_path),
                skills_path_global: table_get_str(target_table, "skills_path_global")
                    .or_else(|| table_get_str(legacy_target_table, "skills_path_global"))
                    .unwrap_or(default_target.skills_path_global),
                instruction_path: table_get_str(target_table, "instruction_path")
                    .or_else(|| table_get_str(legacy_target_table, "instruction_path"))
                    .unwrap_or(default_target.instruction_path),
                instruction_path_global: table_get_str(target_table, "instruction_path_global")
                    .or_else(|| table_get_str(legacy_target_table, "instruction_path_global"))
                    .unwrap_or(default_target.instruction_path_global),
            };
            targets.insert(name.to_string(), target);
        }

        Ok(Self { source, targets })
    }

    pub fn agent_names() -> impl Iterator<Item = AgentName> {
        AgentName::iter()
    }

    pub fn enabled_targets(&self, feature: TargetFeature) -> impl Iterator<Item = AgentName> + '_ {
        AgentName::iter().filter(move |agent| {
            self.targets
                .get(agent.as_str())
                .map(|target| match feature {
                    TargetFeature::Skills => target.skills,
                    TargetFeature::Instructions => target.instructions,
                })
                .unwrap_or(false)
        })
    }

    pub fn source_skills_path(&self, global: bool) -> &str {
        if global {
            &self.source.skills_path_global
        } else {
            &self.source.skills_path
        }
    }

    pub fn source_instruction_path(&self, global: bool) -> &str {
        if global {
            &self.source.instruction_path_global
        } else {
            &self.source.instruction_path
        }
    }

    pub fn target_skills_path(&self, agent: &str, global: bool) -> Option<&str> {
        self.targets.get(agent).map(|target| {
            if global {
                target.skills_path_global.as_str()
            } else {
                target.skills_path.as_str()
            }
        })
    }

    pub fn target_instruction_path(&self, agent: &str, global: bool) -> Option<&str> {
        self.targets.get(agent).map(|target| {
            if global {
                target.instruction_path_global.as_str()
            } else {
                target.instruction_path.as_str()
            }
        })
    }

    pub fn resolve_source_skills_path(&self, base_dir: &Path, global: bool) -> PathBuf {
        resolve_path(base_dir, self.source_skills_path(global))
    }

    pub fn resolve_source_instruction_path(&self, base_dir: &Path, global: bool) -> PathBuf {
        resolve_path(base_dir, self.source_instruction_path(global))
    }

    pub fn resolve_target_skills_path(
        &self,
        agent: &str,
        base_dir: &Path,
        global: bool,
    ) -> Option<PathBuf> {
        self.target_skills_path(agent, global)
            .map(|path| resolve_path(base_dir, path))
    }

    pub fn resolve_target_instruction_path(
        &self,
        agent: &str,
        base_dir: &Path,
        global: bool,
    ) -> Option<PathBuf> {
        self.target_instruction_path(agent, global)
            .map(|path| resolve_path(base_dir, path))
    }
}

fn table_get_str(table: Option<&toml::Value>, key: &str) -> Option<String> {
    table
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn table_get_bool(table: Option<&toml::Value>, key: &str) -> Option<bool> {
    table.and_then(|v| v.get(key)).and_then(|v| v.as_bool())
}

fn resolve_path(base_dir: &Path, raw: &str) -> PathBuf {
    if raw == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
        return base_dir.to_path_buf();
    }

    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
        return base_dir.join(rest);
    }

    let path = Path::new(raw);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_enum_iter() {
        let names: Vec<&str> = Config::agent_names().map(|a| a.as_str()).collect();
        assert_eq!(names, vec!["claude", "codex", "pi", "opencode"]);
    }

    #[test]
    fn test_parse_default_config() {
        let config = Config::parse(crate::init::PROJECT_CONFIG).unwrap();
        assert_eq!(config.source.skills_path, ".agents/skills");
        assert_eq!(config.source.instruction_path, "AGENTS.md");
        assert_eq!(config.targets.len(), 4);
        assert_eq!(
            config.target_skills_path("pi", false).unwrap(),
            ".pi/skills"
        );
        assert_eq!(
            config.target_skills_path("pi", true).unwrap(),
            ".pi/agent/skills"
        );
    }

    #[test]
    fn test_parse_partial_disable() {
        let toml = r#"
[source]
skills_path = ".agents/skills"
instruction_path = "AGENTS.md"

[target.codex]
skills = false
instructions = true

[target.pi]
skills = true
instructions = false
"#;
        let config = Config::parse(toml).unwrap();
        assert!(!config.targets["codex"].skills);
        assert!(config.targets["codex"].instructions);
        assert!(config.targets["pi"].skills);
        assert!(!config.targets["pi"].instructions);
    }

    #[test]
    fn test_enabled_targets_by_feature() {
        let toml = r#"
[target.claude]
skills = true
instructions = false

[target.codex]
skills = false
instructions = true

[target.pi]
skills = true
instructions = true

[target.opencode]
skills = false
instructions = false
"#;
        let config = Config::parse(toml).unwrap();

        let skill_agents: Vec<&str> = config
            .enabled_targets(TargetFeature::Skills)
            .map(|a| a.as_str())
            .collect();
        assert_eq!(skill_agents, vec!["claude", "pi"]);

        let instruction_agents: Vec<&str> = config
            .enabled_targets(TargetFeature::Instructions)
            .map(|a| a.as_str())
            .collect();
        assert_eq!(instruction_agents, vec!["codex", "pi"]);
    }

    #[test]
    fn test_parse_missing_targets_uses_defaults() {
        let toml = r#"
[source]
skills_path = "custom/skills"
"#;
        let config = Config::parse(toml).unwrap();
        assert_eq!(config.source.skills_path, "custom/skills");
        assert!(config.targets["claude"].skills);
        assert!(config.targets["pi"].instructions);
        assert_eq!(
            config.target_instruction_path("claude", true).unwrap(),
            ".claude/CLAUDE.md"
        );
    }

    #[test]
    fn test_parse_target_paths() {
        let toml = r#"
[target.pi]
skills_path = "custom/pi/skills"
skills_path_global = "~/global/pi/skills"
instruction_path = "custom/pi/AGENTS.md"
instruction_path_global = "~/global/pi/AGENTS.md"
"#;
        let config = Config::parse(toml).unwrap();
        assert_eq!(
            config.target_skills_path("pi", false).unwrap(),
            "custom/pi/skills"
        );
        assert_eq!(
            config.target_skills_path("pi", true).unwrap(),
            "~/global/pi/skills"
        );
        assert_eq!(
            config.target_instruction_path("pi", false).unwrap(),
            "custom/pi/AGENTS.md"
        );
        assert_eq!(
            config.target_instruction_path("pi", true).unwrap(),
            "~/global/pi/AGENTS.md"
        );
    }

    #[test]
    fn test_parse_legacy_format_compatibility() {
        let toml = r#"
[skills]
source = ".agents/skills"

[instructions]
source = "AGENTS.md"

[targets.pi]
skills = false
instructions = true
"#;
        let config = Config::parse(toml).unwrap();
        assert_eq!(config.source.skills_path, ".agents/skills");
        assert_eq!(config.source.skills_path_global, ".agents/skills");
        assert!(!config.targets["pi"].skills);
    }

    #[test]
    fn test_resolve_path_expands_home() {
        let config = Config::default();
        let base = Path::new("/tmp/project");
        let resolved = config.resolve_source_skills_path(base, true);
        if let Some(home) = dirs::home_dir() {
            assert_eq!(resolved, home.join(".agents/skills"));
        }
    }

    #[test]
    fn test_parse_invalid_toml() {
        let result = Config::parse("not valid [[[toml");
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::Parse { .. } => {}
            other => panic!("expected Parse, got {other:?}"),
        }
    }

    #[test]
    fn test_load_missing_file() {
        let result = Config::load(Path::new("/nonexistent/hana.toml"));
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::ReadFile { .. } => {}
            other => panic!("expected ReadFile, got {other:?}"),
        }
    }
}
