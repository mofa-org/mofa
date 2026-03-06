use super::*;
use serde_json::json;
use std::path::{Path, PathBuf};
use tokio::fs;

/// 文件系统工具 - 读写文件、列出目录
/// File system utilities - Read/write files, list directories
pub struct FileSystemTool {
    definition: ToolDefinition,
    allowed_paths: Vec<String>,
}

impl FileSystemTool {
    pub fn new(allowed_paths: Vec<String>) -> Self {
        Self {
            definition: ToolDefinition {
                name: "filesystem".to_string(),
                description: "File system operations: read files, write files, list directories, check if file exists.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["read", "write", "list", "exists", "delete", "mkdir"],
                            "description": "File operation to perform"
                        },
                        "path": {
                            "type": "string",
                            "description": "File or directory path"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write (for write operation)"
                        }
                    },
                    "required": ["operation", "path"]
                }),
                requires_confirmation: true,
            },
            allowed_paths,
        }
    }

    /// Create with default allowed paths (temporary directory and current directory)
    pub fn new_with_defaults() -> PluginResult<Self> {
        Ok(Self::new(vec![
            std::env::temp_dir().to_string_lossy().to_string(),
            std::env::current_dir()?.to_string_lossy().to_string(),
        ]))
    }

    fn resolve_path(path: &Path) -> Option<PathBuf> {
        if let Ok(canonical) = path.canonicalize() {
            return Some(canonical);
        }

        let mut remaining: Vec<std::ffi::OsString> = Vec::new();
        let mut current = path.to_path_buf();
        loop {
            if let Some(parent) = current.parent() {
                let name = current
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new(""));

                // Reject `..` in unresolved tail to prevent traversal escape.
                if name == ".." {
                    return None;
                }

                remaining.push(name.to_os_string());
                current = parent.to_path_buf();
                if let Ok(canonical_parent) = current.canonicalize() {
                    let mut resolved = canonical_parent;
                    for component in remaining.into_iter().rev() {
                        resolved.push(component);
                    }
                    return Some(resolved);
                }
            } else {
                return None;
            }
        }
    }

    fn is_path_allowed(&self, path: &str) -> bool {
        if self.allowed_paths.is_empty() {
            return false;
        }

        let raw = Path::new(path);

        let resolved = match Self::resolve_path(raw) {
            Some(p) => p,
            None => return false,
        };

        if raw.is_symlink() {
            match raw.canonicalize() {
                Ok(canonical_target) => {
                    if !self.path_under_allowed_root(&canonical_target) {
                        return false;
                    }
                }
                Err(_) => return false,
            }
        }

        self.path_under_allowed_root(&resolved)
    }

    fn path_under_allowed_root(&self, resolved: &Path) -> bool {
        self.allowed_paths.iter().any(|allowed| {
            let allowed_path = match Path::new(allowed).canonicalize() {
                Ok(p) => p,
                Err(_) => return false,
            };
            resolved.starts_with(&allowed_path)
        })
    }
}

#[async_trait::async_trait]
impl ToolExecutor for FileSystemTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, arguments: serde_json::Value) -> PluginResult<serde_json::Value> {
        let operation = arguments["operation"]
            .as_str()
            .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Operation is required".to_string()))?;
        let path = arguments["path"]
            .as_str()
            .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Path is required".to_string()))?;

        if !self.is_path_allowed(path) {
            return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(format!(
                "Access denied: path '{}' is not in allowed paths",
                path
            )));
        }

        match operation {
            "read" => {
                let content = fs::read_to_string(path).await?;
                let truncated = if content.len() > 10000 {
                    format!(
                        "{}... [truncated, total {} bytes]",
                        &content[..10000],
                        content.len()
                    )
                } else {
                    content
                };
                Ok(json!({
                    "success": true,
                    "content": truncated
                }))
            }
            "write" => {
                let content = arguments["content"]
                    .as_str()
                    .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Content is required for write operation".to_string()))?;
                fs::write(path, content).await?;
                Ok(json!({
                    "success": true,
                    "message": format!("Written {} bytes to {}", content.len(), path)
                }))
            }
            "list" => {
                let mut entries = Vec::new();
                let mut dir = fs::read_dir(path).await?;
                while let Some(entry) = dir.next_entry().await? {
                    let metadata = entry.metadata().await?;
                    entries.push(json!({
                        "name": entry.file_name().to_string_lossy(),
                        "is_dir": metadata.is_dir(),
                        "is_file": metadata.is_file(),
                        "size": metadata.len()
                    }));
                }
                Ok(json!({
                    "success": true,
                    "entries": entries
                }))
            }
            "exists" => {
                let exists = fs::try_exists(path).await?;
                Ok(json!({
                    "success": true,
                    "exists": exists
                }))
            }
            "delete" => {
                let metadata = fs::metadata(path).await?;
                if metadata.is_dir() {
                    fs::remove_dir_all(path).await?;
                } else {
                    fs::remove_file(path).await?;
                }
                Ok(json!({
                    "success": true,
                    "message": format!("Deleted {}", path)
                }))
            }
            "mkdir" => {
                fs::create_dir_all(path).await?;
                Ok(json!({
                    "success": true,
                    "message": format!("Created directory {}", path)
                }))
            }
            _ => Err(mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Unknown operation: {}", operation))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs as stdfs;
    use tempfile::TempDir;

    /// tool whose allowed root is a temp directory.
    fn tool_in(root: &std::path::Path) -> FileSystemTool {
        FileSystemTool::new(vec![root.to_string_lossy().to_string()])
    }

    #[test]
    fn write_to_new_file_inside_allowed_root_is_permitted() {
        let tmp = TempDir::new().unwrap();
        let tool = tool_in(tmp.path());

        let new_file = tmp.path().join("does_not_exist.txt");
        assert!(
            tool.is_path_allowed(&new_file.to_string_lossy()),
            "Writing a new file inside an allowed root should be permitted"
        );
    }

    // symlink escaping allowed root must be denied
    #[cfg(unix)]
    #[test]
    fn symlink_inside_root_pointing_outside_is_denied() {
        let allowed = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let secret = outside.path().join("secret.txt");
        stdfs::write(&secret, "sensitive data").unwrap();

        let link = allowed.path().join("link");
        std::os::unix::fs::symlink(&secret, &link).unwrap();

        let tool = tool_in(allowed.path());
        assert!(
            !tool.is_path_allowed(&link.to_string_lossy()),
            "Symlink whose target is outside allowed root must be denied"
        );
    }

    // end to end write and symlink delete
    #[tokio::test]
    async fn execute_write_new_file_succeeds() {
        let tmp = TempDir::new().unwrap();
        let tool = tool_in(tmp.path());
        let new_file = tmp.path().join("new.txt");

        let result = tool
            .execute(json!({
                "operation": "write",
                "path": new_file.to_string_lossy(),
                "content": "hello world"
            }))
            .await;

        assert!(result.is_ok(), "Write to new file should succeed: {:?}", result.err());
        assert_eq!(stdfs::read_to_string(&new_file).unwrap(), "hello world");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn execute_delete_symlink_escaping_root_is_denied() {
        let allowed = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let target = outside.path().join("important.txt");
        stdfs::write(&target, "do not delete").unwrap();

        let link = allowed.path().join("escape_link");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let tool = tool_in(allowed.path());
        let result = tool
            .execute(json!({
                "operation": "delete",
                "path": link.to_string_lossy(),
            }))
            .await;

        assert!(result.is_err(), "Delete via escaping symlink must be denied");
        assert!(target.exists(), "Target file outside root must not be deleted");
    }

    #[test]
    fn dotdot_traversal_escape_is_denied() {
        let tmp = TempDir::new().unwrap();
        let tool = tool_in(tmp.path());

        let escape = tmp.path().join("..").join("outside").join("file.txt");
        assert!(
            !tool.is_path_allowed(&escape.to_string_lossy()),
            "Path with .. escaping allowed root must be denied"
        );
    }
}
