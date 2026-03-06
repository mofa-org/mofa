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

    /// Determine whether `path` resides within one of the allowed base directories.
    ///
    /// SECURITY: Two-phase resolution strategy:
    ///   1. If the path already exists on disk, `canonicalize()` it directly so
    ///      symlinks and `..` are fully resolved, then verify containment.
    ///   2. If the path does **not** exist (e.g., a new file or directory to be
    ///      created), walk up to the nearest **existing** ancestor, canonicalize
    ///      that ancestor, append the remaining relative tail, and verify:
    ///      (a) the tail contains no `..` components (prevents sandbox escape),
    ///      (b) the resulting logical path starts with an allowed base directory.
    ///
    /// This avoids the deadlock where `canonicalize()` fails on a path that has
    /// not been created yet, which previously caused `mkdir` and `write` for new
    /// paths to be unconditionally denied.
    fn is_path_allowed(&self, path: &str) -> bool {
        use std::path::{Component, Path, PathBuf};

        if self.allowed_paths.is_empty() {
            return false; // Default deny if no paths specified
        }

        let target = Path::new(path);

        // --- Phase 1: path already exists – use strict canonicalize. ---
        if target.exists() {
            let canonical = match target.canonicalize() {
                Ok(p) => p,
                Err(_) => return false,
            };
            return self.starts_with_allowed(&canonical);
        }

        // --- Phase 2: path does not exist – resolve via nearest existing ancestor. ---
        //
        // Walk up from the requested path until we find a component that exists,
        // collecting the "tail" of not-yet-created segments.
        let mut ancestor: &Path = target.as_ref();
        let mut tail_parts: Vec<&std::ffi::OsStr> = Vec::new();

        loop {
            match ancestor.parent() {
                Some(parent) => {
                    // Collect the file-name component that sits on top of `parent`.
                    if let Some(name) = ancestor.file_name() {
                        tail_parts.push(name);
                    } else {
                        // No file_name (e.g., root or prefix) – deny.
                        return false;
                    }
                    ancestor = parent;
                    if ancestor.exists() {
                        break;
                    }
                }
                None => {
                    // Reached filesystem root without finding an existing dir.
                    return false;
                }
            }
        }

        // Canonicalize the existing ancestor (resolves symlinks / `..`).
        let canonical_ancestor = match ancestor.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };

        // Reconstruct the full logical path from canonical ancestor + tail.
        // tail_parts were collected bottom-up, so reverse them.
        let mut resolved: PathBuf = canonical_ancestor;
        for part in tail_parts.iter().rev() {
            resolved.push(part);
        }

        // SECURITY: Ensure no component in the unresolved tail is `..`.
        // An attacker could craft something like `/allowed/../../etc/shadow`; the
        // ancestor resolution above would stop at `/allowed`, but the tail would
        // contain `..` segments that escape the sandbox.
        for component in resolved.components() {
            if matches!(component, Component::ParentDir) {
                return false;
            }
        }

        self.starts_with_allowed(&resolved)
    }

    /// Check whether `canonical` starts with at least one allowed base directory.
    fn starts_with_allowed(&self, canonical: &std::path::Path) -> bool {
        self.allowed_paths.iter().any(|allowed| {
            let allowed_path = match std::path::Path::new(allowed).canonicalize() {
                Ok(p) => p,
                Err(_) => return false,
            };
            canonical.starts_with(allowed_path)
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
