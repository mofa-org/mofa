//! Document loaders for ingesting text from various sources
//!
//! Provides document loaders that read from files and convert them into
//! the kernel `Document` type for use in RAG pipelines.

use mofa_kernel::rag::Document;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

// =============================================================================
// Errors
// =============================================================================

/// Errors from document loading.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LoaderError {
    /// File IO error (includes the path that failed)
    #[error("Failed to read '{path}': {source}")]
    IoError {
        /// The path that caused the IO error
        path: String,
        /// The underlying IO error
        source: std::io::Error,
    },

    /// Unsupported file format
    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),

    /// Empty document
    #[error("Document is empty: {0}")]
    EmptyDocument(String),
}

/// Result type for loader operations.
pub type LoaderResult<T> = Result<T, LoaderError>;

// =============================================================================
// DocumentLoader Trait
// =============================================================================

/// Trait for loading documents from various sources.
pub trait DocumentLoader: Send + Sync {
    /// Load a document from the given path.
    fn load(&self, path: &Path) -> LoaderResult<Vec<Document>>;
}

// =============================================================================
// TextLoader
// =============================================================================

/// Loads plain text files into documents.
///
/// Each file becomes a single `Document` with the file path stored in metadata.
#[derive(Debug, Clone, Default)]
pub struct TextLoader;

impl TextLoader {
    /// Create a new text loader.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl DocumentLoader for TextLoader {
    fn load(&self, path: &Path) -> LoaderResult<Vec<Document>> {
        let content = std::fs::read_to_string(path).map_err(|e| LoaderError::IoError {
            path: path.display().to_string(),
            source: e,
        })?;

        if content.trim().is_empty() {
            return Err(LoaderError::EmptyDocument(
                path.display().to_string(),
            ));
        }

        // Use full path for unique IDs across directories
        let id = path.display().to_string();

        let mut metadata = HashMap::new();
        metadata.insert("source".into(), path.display().to_string());
        metadata.insert("format".into(), "text".into());

        Ok(vec![Document {
            id,
            text: content,
            metadata,
        }])
    }
}

// =============================================================================
// MarkdownLoader
// =============================================================================

/// Loads markdown files with heading-aware section splitting.
///
/// Splits markdown documents at heading boundaries (`#`, `##`, `###`, etc.)
/// so each section becomes a separate `Document` with heading metadata.
#[derive(Debug, Clone)]
pub struct MarkdownLoader {
    /// Maximum heading level to split at (1 = `#`, 2 = `##`, etc.)
    pub split_level: usize,
}

impl Default for MarkdownLoader {
    fn default() -> Self {
        Self { split_level: 2 }
    }
}

impl MarkdownLoader {
    /// Create a new markdown loader that splits at the given heading level.
    #[must_use]
    pub fn new(split_level: usize) -> Self {
        Self { split_level }
    }

    /// Parse heading level from a line (e.g. "## Foo" -> Some(2))
    fn heading_level(line: &str) -> Option<usize> {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') {
            return None;
        }
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        // Must be followed by a space or end of line to be a valid heading
        let rest = &trimmed[level..];
        if rest.is_empty() || rest.starts_with(' ') {
            Some(level)
        } else {
            None
        }
    }

    /// Extract heading text (without the `#` prefix).
    fn heading_text(line: &str) -> String {
        let trimmed = line.trim_start();
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        trimmed[level..].trim().to_string()
    }
}

impl DocumentLoader for MarkdownLoader {
    fn load(&self, path: &Path) -> LoaderResult<Vec<Document>> {
        let content = std::fs::read_to_string(path).map_err(|e| LoaderError::IoError {
            path: path.display().to_string(),
            source: e,
        })?;

        if content.trim().is_empty() {
            return Err(LoaderError::EmptyDocument(
                path.display().to_string(),
            ));
        }

        let source = path.display().to_string();
        // Use full path for unique IDs across directories
        let base_id = path.display().to_string();

        let mut documents = Vec::new();
        let mut current_heading = String::new();
        let mut current_content = String::new();
        let mut section_index = 0usize;

        for line in content.lines() {
            if let Some(level) = Self::heading_level(line) {
                if level <= self.split_level {
                    // Save previous section
                    if !current_content.trim().is_empty() {
                        let mut metadata = HashMap::new();
                        metadata.insert("source".into(), source.clone());
                        metadata.insert("format".into(), "markdown".into());
                        metadata.insert("section_index".into(), section_index.to_string());
                        if !current_heading.is_empty() {
                            metadata.insert("heading".into(), current_heading.clone());
                        }

                        documents.push(Document {
                            id: format!("{base_id}:s{section_index}"),
                            text: current_content.trim().to_string(),
                            metadata,
                        });
                        section_index += 1;
                    }

                    current_heading = Self::heading_text(line);
                    current_content = format!("{line}\n");
                    continue;
                }
            }

            current_content.push_str(line);
            current_content.push('\n');
        }

        // Save last section
        if !current_content.trim().is_empty() {
            let mut metadata = HashMap::new();
            metadata.insert("source".into(), source);
            metadata.insert("format".into(), "markdown".into());
            metadata.insert("section_index".into(), section_index.to_string());
            if !current_heading.is_empty() {
                metadata.insert("heading".into(), current_heading);
            }

            documents.push(Document {
                id: format!("{base_id}:s{section_index}"),
                text: current_content.trim().to_string(),
                metadata,
            });
        }

        if documents.is_empty() {
            return Err(LoaderError::EmptyDocument(
                path.display().to_string(),
            ));
        }

        Ok(documents)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    // --- TextLoader tests ---

    #[test]
    fn text_loader_loads_file() {
        let f = write_temp("Hello, world!\nSecond line.");
        let loader = TextLoader::new();
        let docs = loader.load(f.path()).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].text, "Hello, world!\nSecond line.");
        assert_eq!(docs[0].metadata.get("format").unwrap(), "text");
    }

    #[test]
    fn text_loader_rejects_empty() {
        let f = write_temp("   \n  \n  ");
        let loader = TextLoader::new();
        let result = loader.load(f.path());
        assert!(result.is_err());
    }

    #[test]
    fn text_loader_sets_source_metadata() {
        let f = write_temp("content");
        let loader = TextLoader::new();
        let docs = loader.load(f.path()).unwrap();
        assert!(docs[0].metadata.get("source").unwrap().len() > 0);
    }

    #[test]
    fn text_loader_missing_file() {
        let loader = TextLoader::new();
        let result = loader.load(Path::new("/nonexistent/file.txt"));
        assert!(result.is_err());
    }

    // --- MarkdownLoader tests ---

    #[test]
    fn markdown_splits_at_headings() {
        let content = "# Chapter 1\nFirst content.\n\n# Chapter 2\nSecond content.\n";
        let f = write_temp(content);
        let loader = MarkdownLoader::new(1);
        let docs = loader.load(f.path()).unwrap();
        assert_eq!(docs.len(), 2);
        assert!(docs[0].text.contains("Chapter 1"));
        assert!(docs[1].text.contains("Chapter 2"));
    }

    #[test]
    fn markdown_respects_split_level() {
        let content = "# Title\n\n## Section A\nContent A.\n\n## Section B\nContent B.\n";
        let f = write_temp(content);
        let loader = MarkdownLoader::new(2);
        let docs = loader.load(f.path()).unwrap();
        // Should split at ## level: title preamble + Section A + Section B
        assert!(docs.len() >= 2);
    }

    #[test]
    fn markdown_preserves_heading_metadata() {
        let content = "# Introduction\nSome intro text.\n\n# Methods\nSome methods.\n";
        let f = write_temp(content);
        let loader = MarkdownLoader::new(1);
        let docs = loader.load(f.path()).unwrap();
        assert_eq!(docs[0].metadata.get("heading").unwrap(), "Introduction");
        assert_eq!(docs[1].metadata.get("heading").unwrap(), "Methods");
    }

    #[test]
    fn markdown_sets_section_index() {
        let content = "# A\nContent.\n# B\nContent.\n# C\nContent.\n";
        let f = write_temp(content);
        let loader = MarkdownLoader::new(1);
        let docs = loader.load(f.path()).unwrap();
        assert_eq!(docs[0].metadata.get("section_index").unwrap(), "0");
        assert_eq!(docs[1].metadata.get("section_index").unwrap(), "1");
        assert_eq!(docs[2].metadata.get("section_index").unwrap(), "2");
    }

    #[test]
    fn markdown_rejects_empty() {
        let f = write_temp("   ");
        let loader = MarkdownLoader::default();
        assert!(loader.load(f.path()).is_err());
    }

    #[test]
    fn markdown_no_headings_single_doc() {
        let content = "Just some text without headings.\nMore text.\n";
        let f = write_temp(content);
        let loader = MarkdownLoader::new(1);
        let docs = loader.load(f.path()).unwrap();
        assert_eq!(docs.len(), 1);
    }

    #[test]
    fn heading_level_parsing() {
        assert_eq!(MarkdownLoader::heading_level("# Title"), Some(1));
        assert_eq!(MarkdownLoader::heading_level("## Section"), Some(2));
        assert_eq!(MarkdownLoader::heading_level("### Sub"), Some(3));
        assert_eq!(MarkdownLoader::heading_level("Not a heading"), None);
        assert_eq!(MarkdownLoader::heading_level("#NoSpace"), None);
        assert_eq!(MarkdownLoader::heading_level("#"), Some(1));
    }
}
