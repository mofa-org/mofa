//! Agent context builder framework
//!
//! This module provides:
//! - Flexible system prompt building
//! - Bootstrap file loading from workspace
//! - Agent identity integration
//! - Vision message support

use crate::llm::token_budget::ContextWindowManager;
use crate::llm::types::{ChatMessage, ContentPart, ImageUrl, MessageContent, Role};
use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Agent identity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    /// Agent name
    pub name: String,
    /// Agent description
    pub description: String,
    /// Agent icon (emoji)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

impl AgentIdentity {
    /// Create a new agent identity
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            icon: None,
        }
    }

    /// Set the icon
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// Default bootstrap files to load
const DEFAULT_BOOTSTRAP_FILES: &[&str] =
    &["AGENTS.md", "SOUL.md", "USER.md", "TOOLS.md", "IDENTITY.md"];

/// Skills manager trait for progressive loading
#[async_trait::async_trait]
pub trait SkillsManager: Send + Sync {
    /// Get skills that should always be loaded
    async fn get_always_skills(&self) -> Vec<String>;

    /// Load skills content for context
    async fn load_skills_for_context(&self, names: &[String]) -> String;

    /// Get skills summary for display
    async fn build_skills_summary(&self) -> String;
}

/// No-op skills manager for when skills aren't needed
pub struct NoOpSkillsManager;

#[async_trait::async_trait]
impl SkillsManager for NoOpSkillsManager {
    async fn get_always_skills(&self) -> Vec<String> {
        Vec::new()
    }

    async fn load_skills_for_context(&self, _names: &[String]) -> String {
        String::new()
    }

    async fn build_skills_summary(&self) -> String {
        String::new()
    }
}

/// Context builder for agent prompts
pub struct AgentContextBuilder {
    /// Workspace path
    workspace: PathBuf,
    /// Bootstrap files to load
    bootstrap_files: Vec<String>,
    /// Agent identity
    identity: AgentIdentity,
    /// Optional skills manager
    skills: Option<Arc<dyn SkillsManager>>,
    /// Cached system prompt
    cached_prompt: Arc<RwLock<Option<String>>>,
    /// Optional context window manager for token budget enforcement
    context_window_manager: Option<Arc<ContextWindowManager>>,
}

impl AgentContextBuilder {
    /// Create a new context builder
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            workspace,
            bootstrap_files: DEFAULT_BOOTSTRAP_FILES
                .iter()
                .map(|s| s.to_string())
                .collect(),
            identity: AgentIdentity {
                name: "agent".to_string(),
                description: "AI assistant".to_string(),
                icon: None,
            },
            skills: None,
            cached_prompt: Arc::new(RwLock::new(None)),
            context_window_manager: None,
        }
    }

    /// Set bootstrap files
    pub fn with_bootstrap_files(mut self, files: Vec<String>) -> Self {
        self.bootstrap_files = files;
        self
    }

    /// Set agent identity
    pub fn with_identity(mut self, identity: AgentIdentity) -> Self {
        self.identity = identity;
        self
    }

    /// Set skills manager
    pub fn with_skills(mut self, skills: Arc<dyn SkillsManager>) -> Self {
        self.skills = Some(skills);
        self
    }

    /// Set context window manager for token budget enforcement.
    ///
    /// When set, `build_messages()` and `build_messages_with_skills()` will
    /// automatically trim history to fit within the model's context window.
    pub fn with_context_window_manager(mut self, manager: Arc<ContextWindowManager>) -> Self {
        self.context_window_manager = Some(manager);
        self
    }

    /// Build system prompt from bootstrap files
    pub async fn build_system_prompt(&self) -> GlobalResult<String> {
        // Check cache
        {
            let cached = self.cached_prompt.read().await;
            if let Some(prompt) = cached.as_ref() {
                return Ok(prompt.clone());
            }
        }

        let mut parts = Vec::new();

        // Add identity header
        parts.push(format!("# Agent: {}", self.identity.name));
        parts.push(format!("{}\n", self.identity.description));

        // Load bootstrap files
        for filename in &self.bootstrap_files {
            let path = self.workspace.join(filename);
            if let Ok(content) = Self::load_file(&path) {
                parts.push(format!("## {}\n{}", filename, content));
            }
        }

        // Add skills section if available
        if let Some(skills) = &self.skills {
            let always_skills = skills.get_always_skills().await;
            if !always_skills.is_empty() {
                let content = skills.load_skills_for_context(&always_skills).await;
                if !content.is_empty() {
                    parts.push(format!("# Active Skills\n\n{}", content));
                }
            }

            let summary = skills.build_skills_summary().await;
            if !summary.is_empty() {
                parts.push(format!(
                    r#"# Skills

The following skills extend your capabilities. To use a skill, read its documentation.

{}"#,
                    summary
                ));
            }
        }

        let prompt = parts.join("\n\n---\n\n");

        // Cache the result
        let mut cached = self.cached_prompt.write().await;
        *cached = Some(prompt.clone());

        Ok(prompt)
    }

    /// Build messages with history and current input
    pub async fn build_messages(
        &self,
        history: Vec<ChatMessage>,
        current: &str,
        media: Option<Vec<String>>,
    ) -> GlobalResult<Vec<ChatMessage>> {
        let mut messages = Vec::new();

        // System prompt
        let system_prompt = self.build_system_prompt().await?;
        messages.push(ChatMessage::system(system_prompt));

        // History
        messages.extend(history);

        // Current message (with optional media)
        let user_msg = if let Some(media_paths) = media {
            if !media_paths.is_empty() {
                Self::build_vision_message(current, &media_paths)?
            } else {
                ChatMessage::user(current)
            }
        } else {
            ChatMessage::user(current)
        };

        messages.push(user_msg);

        // Apply context window management if configured
        let messages = if let Some(ref manager) = self.context_window_manager {
            let result = manager.apply(&messages);
            result.messages
        } else {
            messages
        };

        Ok(messages)
    }

    /// Build messages with skill names
    pub async fn build_messages_with_skills(
        &self,
        history: Vec<ChatMessage>,
        current: &str,
        media: Option<Vec<String>>,
        skill_names: Option<&[String]>,
    ) -> GlobalResult<Vec<ChatMessage>> {
        let mut messages = Vec::new();

        // Build system prompt with optional skills
        let system_prompt = self.build_system_prompt().await?;

        let final_prompt = if let Some(skills) = &self.skills {
            if let Some(names) = skill_names {
                if !names.is_empty() {
                    let skills_content = skills.load_skills_for_context(names).await;
                    if !skills_content.is_empty() {
                        format!(
                            "{}\n\n# Requested Skills\n\n{}",
                            system_prompt, skills_content
                        )
                    } else {
                        system_prompt
                    }
                } else {
                    system_prompt
                }
            } else {
                system_prompt
            }
        } else {
            system_prompt
        };

        messages.push(ChatMessage::system(final_prompt));

        // History
        messages.extend(history);

        // Current message (with optional media)
        let user_msg = if let Some(media_paths) = media {
            if !media_paths.is_empty() {
                Self::build_vision_message(current, &media_paths)?
            } else {
                ChatMessage::user(current)
            }
        } else {
            ChatMessage::user(current)
        };

        messages.push(user_msg);

        // Apply context window management if configured
        let messages = if let Some(ref manager) = self.context_window_manager {
            let result = manager.apply(&messages);
            result.messages
        } else {
            messages
        };

        Ok(messages)
    }

    /// Build a vision message with images
    fn build_vision_message(text: &str, image_paths: &[String]) -> GlobalResult<ChatMessage> {
        let mut parts = vec![ContentPart::Text {
            text: text.to_string(),
        }];

        for path in image_paths {
            let image_url = Self::encode_image_data_url(Path::new(path))?;
            parts.push(ContentPart::Image { image_url });
        }

        Ok(ChatMessage {
            role: Role::User,
            content: Some(MessageContent::Parts(parts)),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        })
    }

    /// Encode an image file as a data URL
    fn encode_image_data_url(path: &Path) -> GlobalResult<ImageUrl> {
        use base64::Engine;
        use base64::engine::general_purpose::STANDARD_NO_PAD;
        use std::fs;

        let bytes = fs::read(path)?;
        let mime_type = infer::get_from_path(path)?
            .ok_or_else(|| GlobalError::Other(format!("Unknown MIME type for: {:?}", path)))?
            .mime_type()
            .to_string();

        let base64 = STANDARD_NO_PAD.encode(&bytes);
        let url = format!("data:{};base64,{}", mime_type, base64);

        Ok(ImageUrl { url, detail: None })
    }

    /// Load a file's content
    fn load_file(path: &Path) -> GlobalResult<String> {
        std::fs::read_to_string(path)
            .map_err(|e| GlobalError::Other(format!("Failed to read {:?}: {}", path, e)))
    }

    /// Get the workspace path
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    /// Get the agent identity
    pub fn identity(&self) -> &AgentIdentity {
        &self.identity
    }

    /// Clear the cached prompt
    pub async fn clear_cache(&self) {
        let mut cached = self.cached_prompt.write().await;
        *cached = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_identity_new() {
        let identity = AgentIdentity::new("test", "Test agent");
        assert_eq!(identity.name, "test");
        assert_eq!(identity.description, "Test agent");
        assert!(identity.icon.is_none());
    }

    #[test]
    fn test_agent_identity_with_icon() {
        let identity = AgentIdentity::new("test", "Test agent").with_icon("🤖");
        assert_eq!(identity.icon, Some("🤖".to_string()));
    }

    #[tokio::test]
    async fn test_context_builder_new() {
        let workspace = std::env::temp_dir();
        let builder = AgentContextBuilder::new(workspace.clone());

        assert_eq!(builder.workspace(), &workspace);
        assert_eq!(builder.identity().name, "agent");
    }

    #[tokio::test]
    async fn test_context_builder_with_identity() {
        let workspace = std::env::temp_dir();
        let identity = AgentIdentity::new("custom", "Custom agent");
        let builder = AgentContextBuilder::new(workspace).with_identity(identity);

        assert_eq!(builder.identity().name, "custom");
    }
}
