use super::*;
use serde_json::json;
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

    fn is_path_allowed(&self, path: &str) -> bool {
        if self.allowed_paths.is_empty() {
            return false; // Default deny if no paths specified
        }
        let path = match std::path::Path::new(path).canonicalize() {
            Ok(p) => p,
            Err(_) => return false, // Deny if path cannot be resolved
        };
        self.allowed_paths.iter().any(|allowed| {
            let allowed_path = match std::path::Path::new(allowed).canonicalize() {
                Ok(p) => p,
                Err(_) => return false,
            };
            path.starts_with(allowed_path)
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
