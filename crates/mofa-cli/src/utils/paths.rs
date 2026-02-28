//! Path resolution utilities

use crate::CliError;
use std::path::{Path, PathBuf};

/// Get the current working directory
pub fn current_dir() -> Result<PathBuf, CliError> {
    std::env::current_dir().map_err(|e| CliError::Io(e))
}

/// Resolve a path relative to the current directory
pub fn resolve_path<P: AsRef<Path>>(path: P) -> Result<PathBuf, CliError> {
    let path = path.as_ref();
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(current_dir()?.join(path))
    }
}

/// Find a file by walking up the directory tree
/// Returns the path to the file if found
pub fn find_file_upward<P: AsRef<Path>>(filename: P) -> Option<PathBuf> {
    let filename = filename.as_ref();
    let mut current = current_dir().ok()?;

    loop {
        let target = current.join(filename);
        if target.exists() {
            return Some(target);
        }

        // Move to parent directory
        if !current.pop() {
            // Reached the root
            return None;
        }
    }
}

/// Find the project root by looking for common project markers
/// Checks for: Cargo.toml, package.json, .git
pub fn find_project_root() -> Option<PathBuf> {
    let markers = ["Cargo.toml", "package.json", ".git"];

    for marker in markers {
        if let Some(path) = find_file_upward(Path::new(marker))
            && let Some(parent) = path.parent()
        {
            return Some(parent.to_path_buf());
        }
    }

    None
}

/// Get the MoFA config directory
/// Platform-specific:
/// - macOS/Linux: ~/.config/mofa
/// - Windows: %APPDATA%\mofa
pub fn mofa_config_dir() -> Result<PathBuf, CliError> {
    let config_dir = dirs_next::config_dir()
        .ok_or_else(|| CliError::Other("Failed to determine config directory".to_string()))?;

    Ok(config_dir.join("mofa"))
}

/// Get the MoFA data directory
/// Platform-specific:
/// - macOS: ~/Library/Application Support/mofa
/// - Linux: ~/.local/share/mofa
/// - Windows: %LOCALAPPDATA%\mofa
pub fn mofa_data_dir() -> Result<PathBuf, CliError> {
    let data_dir = dirs_next::data_local_dir()
        .ok_or_else(|| CliError::Other("Failed to determine data directory".to_string()))?;

    Ok(data_dir.join("mofa"))
}

/// Get the MoFA cache directory
pub fn mofa_cache_dir() -> Result<PathBuf, CliError> {
    let cache_dir = dirs_next::cache_dir()
        .ok_or_else(|| CliError::Other("Failed to determine cache directory".to_string()))?;

    Ok(cache_dir.join("mofa"))
}

/// Ensure a directory exists, creating it if necessary
pub fn ensure_dir<P: AsRef<Path>>(path: P) -> Result<PathBuf, CliError> {
    let path = path.as_ref();
    std::fs::create_dir_all(path)
        .map_err(|e| CliError::Other(format!("Failed to create directory {}: {}", path.display(), e)))?;
    Ok(path.to_path_buf())
}

/// Create the MoFA config directory if it doesn't exist
pub fn ensure_mofa_config_dir() -> Result<PathBuf, CliError> {
    ensure_dir(&mofa_config_dir()?)
}

/// Create the MoFA data directory if it doesn't exist
pub fn ensure_mofa_data_dir() -> Result<PathBuf, CliError> {
    ensure_dir(&mofa_data_dir()?)
}

/// Normalize a path for display
pub fn normalize_path<P: AsRef<Path>>(path: P) -> String {
    let path = path.as_ref();
    if let Ok(cwd) = std::env::current_dir()
        && let Ok(rel) = path.strip_prefix(&cwd)
    {
        return rel.display().to_string();
    }
    path.display().to_string()
}

/// Get the log file path for a given agent.
///
/// Logs are stored under `<data_dir>/logs/<agent_id>.log`.
pub fn agent_log_path(data_dir: &Path, agent_id: &str) -> PathBuf {
    data_dir.join("logs").join(format!("{}.log", agent_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_path() {
        let result = resolve_path("Cargo.toml").unwrap();
        assert!(result.ends_with("Cargo.toml"));
    }

    #[test]
    fn test_find_project_root() {
        // Should find the project root since we're in the mofa workspace
        let root = find_project_root();
        assert!(root.is_some());
        let root = root.unwrap();
        assert!(root.join("Cargo.toml").exists() || root.join("crates").exists());
    }

    #[test]
    fn test_mofa_dirs() {
        let config_dir = mofa_config_dir();
        assert!(config_dir.is_ok());
        assert!(config_dir.unwrap().ends_with("mofa"));
    }
}
