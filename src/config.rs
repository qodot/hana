use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct TargetConfig {
    pub skills: bool,
    pub instructions: bool,
}

impl Default for TargetConfig {
    fn default() -> Self {
        Self {
            skills: true,
            instructions: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub skills_source: String,
    pub instructions_source: String,
    pub targets: HashMap<String, TargetConfig>,
}

impl Default for Config {
    fn default() -> Self {
        let mut targets = HashMap::new();
        for name in ["claude", "codex", "pi", "opencode"] {
            targets.insert(name.to_string(), TargetConfig::default());
        }
        Self {
            skills_source: ".agents/skills".to_string(),
            instructions_source: "AGENTS.md".to_string(),
            targets,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("설정 파일을 읽을 수 없습니다: {e}"))?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Result<Self, String> {
        let table: toml::Table = content
            .parse()
            .map_err(|e| format!("TOML 파싱 실패: {e}"))?;

        let skills_source = table
            .get("skills")
            .and_then(|v| v.get("source"))
            .and_then(|v| v.as_str())
            .unwrap_or(".agents/skills")
            .to_string();

        let instructions_source = table
            .get("instructions")
            .and_then(|v| v.get("source"))
            .and_then(|v| v.as_str())
            .unwrap_or("AGENTS.md")
            .to_string();

        let mut targets = HashMap::new();
        let default_target = TargetConfig::default();

        for name in ["claude", "codex", "pi", "opencode"] {
            let target = table
                .get("targets")
                .and_then(|v| v.get(name))
                .map(|v| TargetConfig {
                    skills: v.get("skills").and_then(|b| b.as_bool()).unwrap_or(true),
                    instructions: v
                        .get("instructions")
                        .and_then(|b| b.as_bool())
                        .unwrap_or(true),
                })
                .unwrap_or_else(|| default_target.clone());
            targets.insert(name.to_string(), target);
        }

        Ok(Self {
            skills_source,
            instructions_source,
            targets,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_default_config() {
        let config = Config::parse(crate::init::DEFAULT_CONFIG).unwrap();
        assert_eq!(config.skills_source, ".agents/skills");
        assert_eq!(config.instructions_source, "AGENTS.md");
        assert_eq!(config.targets.len(), 4);
        assert!(config.targets["claude"].skills);
        assert!(config.targets["claude"].instructions);
    }

    #[test]
    fn test_parse_partial_disable() {
        let toml = r#"
[skills]
source = ".agents/skills"

[instructions]
source = "AGENTS.md"

[targets.claude]
skills = true
instructions = true

[targets.codex]
skills = false
instructions = true

[targets.pi]
skills = true
instructions = false

[targets.opencode]
skills = true
instructions = true
"#;
        let config = Config::parse(toml).unwrap();
        assert!(!config.targets["codex"].skills);
        assert!(config.targets["codex"].instructions);
        assert!(config.targets["pi"].skills);
        assert!(!config.targets["pi"].instructions);
    }

    #[test]
    fn test_parse_missing_targets_uses_defaults() {
        let toml = r#"
[skills]
source = "custom/skills"

[instructions]
source = "AGENTS.md"
"#;
        let config = Config::parse(toml).unwrap();
        assert_eq!(config.skills_source, "custom/skills");
        assert!(config.targets["claude"].skills);
        assert!(config.targets["pi"].instructions);
    }

    #[test]
    fn test_parse_invalid_toml() {
        let result = Config::parse("not valid [[[toml");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_missing_file() {
        let result = Config::load(Path::new("/nonexistent/hana.toml"));
        assert!(result.is_err());
    }
}
