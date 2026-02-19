use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum InitOk {
    /// 설정 파일 생성 완료
    Created { path: PathBuf },
    /// dry-run: 내용만 출력
    DryRun { path: String, content: String },
}

#[derive(Debug)]
pub enum InitError {
    /// 설정 파일이 이미 존재 (--force 없이)
    AlreadyExists { path: PathBuf },
    /// 디렉토리 생성 실패
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    /// 파일 쓰기 실패
    WriteFile {
        path: PathBuf,
        source: std::io::Error,
    },
    /// 홈 디렉토리를 찾을 수 없음
    NoHomeDir,
}

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyExists { path } => {
                write!(
                    f,
                    "이미 존재합니다: {}\n   덮어쓰려면 --force 옵션을 사용하세요.",
                    path.display()
                )
            }
            Self::CreateDir { path, source } => {
                write!(f, "디렉토리 생성 실패 ({}): {source}", path.display())
            }
            Self::WriteFile { path, source } => {
                write!(f, "파일 생성 실패 ({}): {source}", path.display())
            }
            Self::NoHomeDir => write!(f, "홈 디렉토리를 찾을 수 없습니다."),
        }
    }
}

pub const PROJECT_CONFIG: &str = r#"# hana - AI 코딩 에이전트 동기화 설정
# https://github.com/qodot/hana

[source]
skills_path = ".agents/skills"
skills_path_global = "~/.agents/skills"
instruction_path = "AGENTS.md"
instruction_path_global = "~/.agents/AGENTS.md"

[target.claude]
skills = true
instructions = true
skills_path = ".claude/skills"
skills_path_global = ".claude/skills"
instruction_path = "CLAUDE.md"
instruction_path_global = ".claude/CLAUDE.md"

[target.codex]
skills = true
instructions = true
skills_path = ".agents/skills"
skills_path_global = ".agents/skills"
instruction_path = "AGENTS.md"
instruction_path_global = ".codex/AGENTS.md"

[target.pi]
skills = true
instructions = true
skills_path = ".pi/skills"
skills_path_global = ".pi/agent/skills"
instruction_path = "AGENTS.md"
instruction_path_global = ".pi/agent/AGENTS.md"

[target.opencode]
skills = true
instructions = true
skills_path = ".opencode/skills"
skills_path_global = ".config/opencode/skills"
instruction_path = "AGENTS.md"
instruction_path_global = ".config/opencode/AGENTS.md"
"#;

pub const GLOBAL_CONFIG: &str = r#"# hana - AI 코딩 에이전트 글로벌 동기화 설정
# https://github.com/qodot/hana

[source]
skills_path = ".agents/skills"
skills_path_global = "~/.agents/skills"
instruction_path = "AGENTS.md"
instruction_path_global = "~/.agents/AGENTS.md"

[target.claude]
skills = true
instructions = true
skills_path = ".claude/skills"
skills_path_global = ".claude/skills"
instruction_path = "CLAUDE.md"
instruction_path_global = ".claude/CLAUDE.md"

[target.codex]
skills = true
instructions = true
skills_path = ".agents/skills"
skills_path_global = ".agents/skills"
instruction_path = "AGENTS.md"
instruction_path_global = ".codex/AGENTS.md"

[target.pi]
skills = true
instructions = true
skills_path = ".pi/skills"
skills_path_global = ".pi/agent/skills"
instruction_path = "AGENTS.md"
instruction_path_global = ".pi/agent/AGENTS.md"

[target.opencode]
skills = true
instructions = true
skills_path = ".opencode/skills"
skills_path_global = ".config/opencode/skills"
instruction_path = "AGENTS.md"
instruction_path_global = ".config/opencode/AGENTS.md"
"#;

pub struct InitOptions {
    pub global: bool,
    pub force: bool,
    pub dry_run: bool,
}

fn config_template(global: bool) -> &'static str {
    if global {
        GLOBAL_CONFIG
    } else {
        PROJECT_CONFIG
    }
}

pub fn execute(opts: &InitOptions, base_dir: &Path) -> Result<InitOk, InitError> {
    let template = config_template(opts.global);

    if opts.dry_run {
        let path = if opts.global {
            "~/.agents/hana.toml"
        } else {
            ".agents/hana.toml"
        };
        return Ok(InitOk::DryRun {
            path: path.to_string(),
            content: template.to_string(),
        });
    }

    let config_path = base_dir.join(".agents").join("hana.toml");

    if config_path.exists() && !opts.force {
        return Err(InitError::AlreadyExists { path: config_path });
    }

    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| InitError::CreateDir {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
    }

    fs::write(&config_path, template).map_err(|e| InitError::WriteFile {
        path: config_path.clone(),
        source: e,
    })?;

    Ok(InitOk::Created { path: config_path })
}

pub fn run(opts: &InitOptions) -> Result<InitOk, InitError> {
    let base_dir = if opts.global {
        dirs::home_dir().ok_or(InitError::NoHomeDir)?
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };

    execute(opts, &base_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn opts(global: bool, force: bool, dry_run: bool) -> InitOptions {
        InitOptions {
            global,
            force,
            dry_run,
        }
    }

    #[test]
    fn test_init_creates_config() {
        let tmp = TempDir::new().unwrap();
        let result = execute(&opts(false, false, false), tmp.path());
        assert!(matches!(result, Ok(InitOk::Created { .. })));

        let config = tmp.path().join(".agents").join("hana.toml");
        assert!(config.exists());
        assert_eq!(fs::read_to_string(&config).unwrap(), PROJECT_CONFIG);
    }

    #[test]
    fn test_init_fails_if_exists() {
        let tmp = TempDir::new().unwrap();
        let agents_dir = tmp.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(agents_dir.join("hana.toml"), "existing").unwrap();

        let result = execute(&opts(false, false, false), tmp.path());
        assert!(matches!(result, Err(InitError::AlreadyExists { .. })));
    }

    #[test]
    fn test_init_force_overwrites() {
        let tmp = TempDir::new().unwrap();
        let agents_dir = tmp.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(agents_dir.join("hana.toml"), "old content").unwrap();

        let result = execute(&opts(false, true, false), tmp.path());
        assert!(matches!(result, Ok(InitOk::Created { .. })));

        let content = fs::read_to_string(agents_dir.join("hana.toml")).unwrap();
        assert_eq!(content, PROJECT_CONFIG);
    }

    #[test]
    fn test_init_dry_run() {
        let tmp = TempDir::new().unwrap();
        let result = execute(&opts(false, false, true), tmp.path());
        assert!(matches!(result, Ok(InitOk::DryRun { .. })));

        if let Ok(InitOk::DryRun { path, content }) = result {
            assert!(path.contains("hana.toml"));
            assert_eq!(content, PROJECT_CONFIG);
        }

        assert!(!tmp.path().join(".agents").join("hana.toml").exists());
    }

    #[test]
    fn test_init_global_uses_global_template() {
        let tmp = TempDir::new().unwrap();
        let result = execute(&opts(true, false, false), tmp.path());
        assert!(matches!(result, Ok(InitOk::Created { .. })));

        let content = fs::read_to_string(tmp.path().join(".agents/hana.toml")).unwrap();
        assert!(content.contains("~/.agents/skills"));
        assert!(content.contains("~/.agents/AGENTS.md"));
        assert_eq!(content, GLOBAL_CONFIG);
    }

    #[test]
    fn test_init_dry_run_global_shows_global_template() {
        let tmp = TempDir::new().unwrap();
        let result = execute(&opts(true, false, true), tmp.path());
        if let Ok(InitOk::DryRun { content, .. }) = result {
            assert!(content.contains("~/.agents/skills"));
        } else {
            panic!("expected DryRun");
        }
    }

    #[test]
    fn test_init_global_uses_base_dir() {
        let tmp = TempDir::new().unwrap();
        let result = execute(&opts(true, false, false), tmp.path());
        assert!(matches!(result, Ok(InitOk::Created { .. })));

        let config = tmp.path().join(".agents").join("hana.toml");
        assert!(config.exists());
    }

    #[test]
    fn test_init_creates_agents_directory() {
        let tmp = TempDir::new().unwrap();
        assert!(!tmp.path().join(".agents").exists());

        execute(&opts(false, false, false), tmp.path()).unwrap();
        assert!(tmp.path().join(".agents").exists());
    }
}
