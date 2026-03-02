//! Recursive text chunker
//!
//! Splits text using a hierarchy of separators, trying the most meaningful
//! split first (paragraphs → sentences → words → characters).
//! Inspired by LangChain's RecursiveCharacterTextSplitter.

// =============================================================================
// RecursiveChunker
// =============================================================================

/// Configuration for recursive chunking.
#[derive(Debug, Clone)]
pub struct RecursiveChunkConfig {
    /// Maximum number of characters per chunk.
    pub chunk_size: usize,
    /// Number of characters to overlap between consecutive chunks.
    pub chunk_overlap: usize,
    /// Ordered list of separators to try (most → least meaningful).
    pub separators: Vec<String>,
}

impl Default for RecursiveChunkConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1000,
            chunk_overlap: 200,
            separators: vec![
                "\n\n".into(),  // Paragraph
                "\n".into(),    // Line
                ". ".into(),    // Sentence
                ", ".into(),    // Clause
                " ".into(),     // Word
            ],
        }
    }
}

impl RecursiveChunkConfig {
    /// Create a new config with custom chunk size and overlap.
    #[must_use]
    pub fn new(chunk_size: usize, chunk_overlap: usize) -> Self {
        Self {
            chunk_size,
            chunk_overlap,
            separators: Self::default().separators,
        }
    }

    /// Set custom separators.
    #[must_use]
    pub fn with_separators(mut self, separators: Vec<String>) -> Self {
        self.separators = separators;
        self
    }
}

/// Recursive text chunker that tries multiple separator levels.
///
/// Splits text using a hierarchy of separators. First tries to split at
/// paragraph boundaries (`\n\n`), then at line boundaries (`\n`), then
/// at sentence boundaries (`. `), and falls back to word and character
/// boundaries as needed.
///
/// This produces higher-quality chunks than fixed-size splitting because
/// it preserves semantic boundaries where possible.
#[derive(Debug, Clone)]
pub struct RecursiveChunker {
    config: RecursiveChunkConfig,
}

impl Default for RecursiveChunker {
    fn default() -> Self {
        Self {
            config: RecursiveChunkConfig::default(),
        }
    }
}

impl RecursiveChunker {
    /// Create a new recursive chunker with the given configuration.
    #[must_use]
    pub fn new(config: RecursiveChunkConfig) -> Self {
        Self { config }
    }

    /// Split text into chunks using recursive separator hierarchy.
    pub fn chunk(&self, text: &str) -> Vec<String> {
        if text.is_empty() {
            return vec![];
        }

        if text.chars().count() <= self.config.chunk_size {
            return vec![text.to_string()];
        }

        self.recursive_split(text, 0)
    }

    fn recursive_split(&self, text: &str, separator_idx: usize) -> Vec<String> {
        if text.chars().count() <= self.config.chunk_size {
            return vec![text.to_string()];
        }

        // If we've exhausted all separators, do a hard character split
        if separator_idx >= self.config.separators.len() {
            return self.hard_split(text);
        }

        let separator = &self.config.separators[separator_idx];
        let parts: Vec<&str> = text.split(separator.as_str()).collect();

        // If the separator didn't actually split anything useful, try next
        if parts.len() <= 1 {
            return self.recursive_split(text, separator_idx + 1);
        }

        let mut chunks = Vec::new();
        let mut current = String::new();

        for (i, part) in parts.iter().enumerate() {
            let candidate = if current.is_empty() {
                part.to_string()
            } else {
                format!("{current}{separator}{part}")
            };

            if candidate.chars().count() <= self.config.chunk_size {
                current = candidate;
            } else {
                // Save current chunk
                if !current.is_empty() {
                    chunks.push(current.trim().to_string());
                }

                // If this single part is still too large, recurse with next separator
                if part.chars().count() > self.config.chunk_size {
                    let sub_chunks = self.recursive_split(part, separator_idx + 1);
                    chunks.extend(sub_chunks);
                    current = String::new();
                } else {
                    current = part.to_string();
                }
            }

            // Add overlap from the end of the previous chunk (UTF-8 safe)
            if i > 0 && current.is_empty() && !chunks.is_empty() {
                let last = chunks.last().unwrap();
                let char_count = last.chars().count();
                let overlap_chars = char_count.min(self.config.chunk_overlap);
                let overlap_text: String = last.chars().skip(char_count - overlap_chars).collect();
                // Only use overlap if it doesn't make next chunk too large
                if overlap_text.chars().count() < self.config.chunk_size / 2 {
                    current = overlap_text;
                }
            }
        }

        // Save the last chunk
        if !current.trim().is_empty() {
            chunks.push(current.trim().to_string());
        }

        chunks.retain(|c| !c.is_empty());
        chunks
    }

    fn hard_split(&self, text: &str) -> Vec<String> {
        let mut chunks = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let step = self
            .config
            .chunk_size
            .saturating_sub(self.config.chunk_overlap)
            .max(1);
        let mut start = 0;

        while start < chars.len() {
            let end = (start + self.config.chunk_size).min(chars.len());
            let chunk: String = chars[start..end].iter().collect();
            if !chunk.trim().is_empty() {
                chunks.push(chunk);
            }
            if end >= chars.len() {
                break;
            }
            start += step;
        }

        chunks
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_no_split() {
        let chunker = RecursiveChunker::default();
        let chunks = chunker.chunk("Short text");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Short text");
    }

    #[test]
    fn empty_text() {
        let chunker = RecursiveChunker::default();
        let chunks = chunker.chunk("");
        assert!(chunks.is_empty());
    }

    #[test]
    fn splits_at_paragraph_boundary() {
        let config = RecursiveChunkConfig::new(50, 0);
        let chunker = RecursiveChunker::new(config);
        let text = "First paragraph content.\n\nSecond paragraph content.\n\nThird paragraph content.";
        let chunks = chunker.chunk(text);
        assert!(chunks.len() >= 2);
        // Each chunk should be under the size limit
        for chunk in &chunks {
            assert!(chunk.len() <= 50, "Chunk too large: {}", chunk.len());
        }
    }

    #[test]
    fn splits_at_sentence_boundary() {
        let config = RecursiveChunkConfig::new(30, 0);
        let chunker = RecursiveChunker::new(config);
        let text = "First sentence. Second sentence. Third sentence.";
        let chunks = chunker.chunk(text);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn respects_chunk_size() {
        let config = RecursiveChunkConfig::new(100, 0);
        let chunker = RecursiveChunker::new(config);
        let text = "A ".repeat(200); // 400 chars
        let chunks = chunker.chunk(&text);
        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(
                chunk.len() <= 110, // Allow small margin due to separator handling
                "Chunk too large: {} chars",
                chunk.len()
            );
        }
    }

    #[test]
    fn custom_separators() {
        let config = RecursiveChunkConfig::new(20, 0)
            .with_separators(vec!["|".into(), " ".into()]);
        let chunker = RecursiveChunker::new(config);
        let text = "part one content|part two content|part three content here";
        let chunks = chunker.chunk(text);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn hard_split_fallback() {
        let config = RecursiveChunkConfig::new(5, 0)
            .with_separators(vec![]); // No separators → immediate hard split
        let chunker = RecursiveChunker::new(config);
        let text = "abcdefghijklmnopqrst"; // 20 chars, no separators
        let chunks = chunker.chunk(text);
        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(chunk.len() <= 5);
        }
    }

    #[test]
    fn preserves_content() {
        let config = RecursiveChunkConfig::new(50, 0);
        let chunker = RecursiveChunker::new(config);
        let text = "Hello world.\n\nThis is a test.\n\nFinal paragraph.";
        let chunks = chunker.chunk(text);
        let reconstructed: String = chunks.join(" ");
        // All original words should be present
        assert!(reconstructed.contains("Hello"));
        assert!(reconstructed.contains("test"));
        assert!(reconstructed.contains("Final"));
    }

    #[test]
    fn default_config_values() {
        let config = RecursiveChunkConfig::default();
        assert_eq!(config.chunk_size, 1000);
        assert_eq!(config.chunk_overlap, 200);
        assert_eq!(config.separators.len(), 5);
    }
}
