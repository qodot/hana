use std::path::PathBuf;

// === Init ===

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
    CreateDir { path: PathBuf, source: std::io::Error },
    /// 파일 쓰기 실패
    WriteFile { path: PathBuf, source: std::io::Error },
    /// 홈 디렉토리를 찾을 수 없음
    NoHomeDir,
}

// === Config ===

#[derive(Debug)]
pub enum ConfigError {
    /// 설정 파일을 읽을 수 없음
    ReadFile { path: PathBuf, source: std::io::Error },
    /// TOML 파싱 실패
    Parse { message: String },
}

// === Sync ===

#[derive(Debug)]
pub struct SyncOk {
    pub skills_linked: Vec<(String, String)>,
    pub skills_collected: Vec<(String, String)>,
    pub instructions_linked: Vec<String>,
    pub instructions_skipped: Vec<String>,
    pub cleaned: Vec<PathBuf>,
    pub warnings: Vec<SyncWarning>,
}

#[derive(Debug)]
pub enum SyncWarning {
    /// 스킬 이름 충돌: 여러 에이전트에서 동일 이름 발견
    SkillConflict { name: String, agents: Vec<String> },
    /// 기존 파일/디렉토리 충돌 (--force 필요)
    FileConflict { skill: String, agent: String },
    /// 지침 파일 충돌 (--force 필요)
    InstructionConflict { file: String },
    /// 파일시스템 작업 실패
    IoFailed { operation: String, detail: String },
}

#[derive(Debug)]
pub enum SyncError {
    /// 설정 파일 로딩 실패
    Config(ConfigError),
    /// 홈 디렉토리를 찾을 수 없음
    NoHomeDir,
}

// === Status ===

#[derive(Debug, Clone, PartialEq)]
pub enum SkillState {
    Synced,
    RealDir,
    BrokenSymlink,
    Missing,
    WrongTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InstructionState {
    Synced,
    DirectRead,
    RealFile,
    Missing,
    Disabled,
}

#[derive(Debug)]
pub struct StatusOk {
    pub skills: Vec<SkillStatusEntry>,
    pub instructions: InstructionStatusEntry,
}

#[derive(Debug)]
pub struct SkillStatusEntry {
    pub name: String,
    pub agents: Vec<(String, SkillState)>,
}

#[derive(Debug)]
pub struct InstructionStatusEntry {
    pub source: String,
    pub source_exists: bool,
    pub agents: Vec<(String, InstructionState)>,
}

#[derive(Debug)]
pub enum StatusError {
    /// 설정 파일 로딩 실패
    Config(ConfigError),
    /// 홈 디렉토리를 찾을 수 없음
    NoHomeDir,
}

// === Display 구현 ===

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyExists { path } => {
                write!(f, "이미 존재합니다: {}\n   덮어쓰려면 --force 옵션을 사용하세요.", path.display())
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

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadFile { path, source } => {
                write!(f, "설정 파일을 읽을 수 없습니다 ({}): {source}", path.display())
            }
            Self::Parse { message } => write!(f, "TOML 파싱 실패: {message}"),
        }
    }
}

impl std::fmt::Display for SyncWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SkillConflict { name, agents } => {
                write!(f, "스킬 이름 충돌: '{name}' — {}", agents.join(", "))
            }
            Self::FileConflict { skill, agent } => {
                write!(f, "충돌: {skill} ({agent}) 에 실제 파일/디렉토리 존재. --force로 덮어쓰세요.")
            }
            Self::InstructionConflict { file } => {
                write!(f, "{file} 가 이미 존재합니다 (심링크가 아님). --force로 덮어쓰세요.")
            }
            Self::IoFailed { operation, detail } => {
                write!(f, "{operation}: {detail}")
            }
        }
    }
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(e) => write!(f, "{e}"),
            Self::NoHomeDir => write!(f, "홈 디렉토리를 찾을 수 없습니다."),
        }
    }
}

impl std::fmt::Display for StatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(e) => write!(f, "{e}"),
            Self::NoHomeDir => write!(f, "홈 디렉토리를 찾을 수 없습니다."),
        }
    }
}

impl From<ConfigError> for SyncError {
    fn from(e: ConfigError) -> Self {
        Self::Config(e)
    }
}

impl From<ConfigError> for StatusError {
    fn from(e: ConfigError) -> Self {
        Self::Config(e)
    }
}
