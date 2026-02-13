use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_CONFIG: &str = r#"# hana - AI 코딩 에이전트 동기화 설정
# https://github.com/qodot/hana

[skills]
source = ".agents/skills"

[instructions]
source = "AGENTS.md"

[targets.claude]
skills = true
instructions = true

[targets.codex]
skills = true
instructions = true

[targets.pi]
skills = true
instructions = true

[targets.opencode]
skills = true
instructions = true
"#;

pub struct InitOptions {
    pub global: bool,
    pub force: bool,
    pub dry_run: bool,
}

impl InitOptions {
    pub fn from_args(args: &[String]) -> Self {
        Self {
            global: args.iter().any(|a| a == "--global"),
            force: args.iter().any(|a| a == "--force"),
            dry_run: args.iter().any(|a| a == "--dry-run"),
        }
    }
}

/// init 실행. base_dir을 받아서 테스트 가능하게 함.
pub fn execute(opts: &InitOptions, base_dir: &Path) -> Result<String, String> {
    if opts.dry_run {
        let path = if opts.global {
            "~/.agents/hana.toml"
        } else {
            ".agents/hana.toml"
        };
        return Ok(format!("🌸 {path} 에 생성될 내용:\n\n{DEFAULT_CONFIG}"));
    }

    let config_path = if opts.global {
        base_dir.join(".agents").join("hana.toml")
    } else {
        base_dir.join(".agents").join("hana.toml")
    };

    if config_path.exists() && !opts.force {
        return Err(format!(
            "🌸 이미 존재합니다: {}\n   덮어쓰려면 --force 옵션을 사용하세요.",
            config_path.display()
        ));
    }

    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("🌸 디렉토리 생성 실패: {e}"))?;
        }
    }

    fs::write(&config_path, DEFAULT_CONFIG)
        .map_err(|e| format!("🌸 파일 생성 실패: {e}"))?;

    Ok(format!("🌸 생성 완료: {}", config_path.display()))
}

pub fn run(args: &[String]) -> Result<(), i32> {
    let opts = InitOptions::from_args(args);

    let base_dir = if opts.global {
        dirs::home_dir().ok_or_else(|| {
            eprintln!("🌸 홈 디렉토리를 찾을 수 없습니다.");
            1
        })?
    } else {
        PathBuf::from(".")
    };

    match execute(&opts, &base_dir) {
        Ok(msg) => {
            print!("{msg}");
            if !msg.ends_with('\n') {
                println!();
            }
            Ok(())
        }
        Err(msg) => {
            eprintln!("{msg}");
            Err(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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
        assert!(result.is_ok());

        let config = tmp.path().join(".agents").join("hana.toml");
        assert!(config.exists());
        assert_eq!(fs::read_to_string(&config).unwrap(), DEFAULT_CONFIG);
    }

    #[test]
    fn test_init_fails_if_exists() {
        let tmp = TempDir::new().unwrap();
        let agents_dir = tmp.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(agents_dir.join("hana.toml"), "existing").unwrap();

        let result = execute(&opts(false, false, false), tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("이미 존재합니다"));
    }

    #[test]
    fn test_init_force_overwrites() {
        let tmp = TempDir::new().unwrap();
        let agents_dir = tmp.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(agents_dir.join("hana.toml"), "old content").unwrap();

        let result = execute(&opts(false, true, false), tmp.path());
        assert!(result.is_ok());

        let content = fs::read_to_string(agents_dir.join("hana.toml")).unwrap();
        assert_eq!(content, DEFAULT_CONFIG);
    }

    #[test]
    fn test_init_dry_run() {
        let tmp = TempDir::new().unwrap();
        let result = execute(&opts(false, false, true), tmp.path());
        assert!(result.is_ok());

        let msg = result.unwrap();
        assert!(msg.contains(".agents/hana.toml"));
        assert!(msg.contains(DEFAULT_CONFIG));

        // 파일이 생성되지 않아야 함
        assert!(!tmp.path().join(".agents").join("hana.toml").exists());
    }

    #[test]
    fn test_init_global_uses_base_dir() {
        let tmp = TempDir::new().unwrap();
        let result = execute(&opts(true, false, false), tmp.path());
        assert!(result.is_ok());

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
