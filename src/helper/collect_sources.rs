use std::fs;
use std::path::Path;

use crate::sync::SyncWarning;

pub fn collect_sources(source_dir: &Path) -> Result<Vec<String>, SyncWarning> {
    if !source_dir.exists() {
        return Ok(vec![]);
    }
    match fs::read_dir(source_dir) {
        Ok(entries) => Ok(entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect()),
        Err(e) => Err(SyncWarning::IoFailed {
            operation: format!("소스 스킬 목록 읽기 ({})", source_dir.display()),
            detail: e.to_string(),
        }),
    }
}
