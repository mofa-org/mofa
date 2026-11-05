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
                "\n\n".into(), // Paragraph
                "\n".into(),   // Line
                ". ".into(),   // Sentence (period)
                "? ".into(),   // Sentence (question)
                "! ".into(),   // Sentence (exclamation)
                ", ".into(),   // Clause
                " ".into(),    // Word
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
#[derive(Debug, Clone, Default)]
pub struct RecursiveChunker {
    config: RecursiveChunkConfig,
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
        let (parts, joiner) = split_with_separator(text, separator);

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
                format!("{current}{joiner}{part}")
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

/// Splits text using `separator` while preserving punctuation boundaries (Issue #1705).
///
/// If `separator` contains punctuation / non-whitespace characters (e.g. `". "`, `"? "`, `"! "`, `", "`),
/// the punctuation prefix is kept and attached to the preceding slice.
/// Trailing whitespace in the separator is discarded as the boundary delimiter and returned as the `joiner`.
/// If `separator` is pure whitespace (e.g. `"\n\n"`, `"\n"`, `" "`), it is discarded during splitting
/// and returned as the `joiner`.
fn split_with_separator<'a, 's>(text: &'a str, separator: &'s str) -> (Vec<&'a str>, &'s str) {
    if separator.is_empty() || text.is_empty() {
        return (vec![text], "");
    }

    let punct_len = separator.trim_end_matches(char::is_whitespace).len();
    let is_punctuation = punct_len > 0;

    if is_punctuation {
        let mut parts = Vec::new();
        let mut last_end = 0;

        for (match_idx, _) in text.match_indices(separator) {
            let end_idx = match_idx + punct_len;
            if end_idx > last_end {
                let slice = text[last_end..end_idx].trim_start_matches(char::is_whitespace);
                if !slice.is_empty() {
                    parts.push(slice);
                }
            }
            last_end = match_idx + separator.len();
        }

        if last_end < text.len() {
            let remainder = text[last_end..].trim_start_matches(char::is_whitespace);
            if !remainder.is_empty() {
                parts.push(remainder);
            }
        }

        (parts, &separator[punct_len..])
    } else {
        let parts: Vec<&str> = text
            .split(separator)
            .filter(|s| !s.is_empty())
            .collect();
        (parts, separator)
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
        let text =
            "First paragraph content.\n\nSecond paragraph content.\n\nThird paragraph content.";
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
        // Ensure sentence boundaries retain punctuation
        for chunk in &chunks {
            assert!(chunk.ends_with('.'), "Expected chunk to retain period: {chunk}");
        }
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
        let config = RecursiveChunkConfig::new(20, 0).with_separators(vec!["|".into(), " ".into()]);
        let chunker = RecursiveChunker::new(config);
        let text = "part one content|part two content|part three content here";
        let chunks = chunker.chunk(text);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn hard_split_fallback() {
        let config = RecursiveChunkConfig::new(5, 0).with_separators(vec![]); // No separators → immediate hard split
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
        assert_eq!(config.separators.len(), 7);
    }

    // =========================================================================
    // Issue #1705 Regression Tests: Text Boundary Truncation & Punctuation
    // =========================================================================

    #[test]
    fn issue_1705_sentences_retain_all_punctuation_marks() {
        // Prove that sentences ending in ., ?, and ! strictly retain their marks
        let config = RecursiveChunkConfig::new(15, 0);
        let chunker = RecursiveChunker::new(config);
        let text = "What is Rust? It is safe! It is fast. Works.";
        let chunks = chunker.chunk(text);

        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0], "What is Rust?");
        assert_eq!(chunks[1], "It is safe!");
        assert_eq!(chunks[2], "It is fast.");
        assert_eq!(chunks[3], "Works.");

        // Verify ending punctuation invariant
        assert!(chunks[0].ends_with('?'));
        assert!(chunks[1].ends_with('!'));
        assert!(chunks[2].ends_with('.'));
        assert!(chunks[3].ends_with('.'));

        for chunk in &chunks {
            assert!(
                chunk.chars().count() <= 15,
                "Chunk exceeded size limit: {chunk}"
            );
        }
    }

    #[test]
    fn issue_1705_clauses_retain_commas() {
        // Prove that clauses ending in , strictly retain their commas
        let config = RecursiveChunkConfig::new(30, 0);
        let chunker = RecursiveChunker::new(config);
        let text = "When writing Rust, safety comes first, and speed follows.";
        let chunks = chunker.chunk(text);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "When writing Rust,");
        assert_eq!(chunks[1], "safety comes first,");
        assert_eq!(chunks[2], "and speed follows.");

        // Verify comma retention
        assert!(chunks[0].ends_with(','));
        assert!(chunks[1].ends_with(','));
        assert!(chunks[2].ends_with('.'));

        for chunk in &chunks {
            assert!(
                chunk.chars().count() <= 30,
                "Chunk exceeded size limit: {chunk}"
            );
        }
    }

    #[test]
    fn issue_1705_strictly_respects_chunk_size_with_punctuation() {
        // Exact boundary condition: chunk_size is 15 chars.
        // Sentences with punctuation attached must not overflow the hard limit.
        let config = RecursiveChunkConfig::new(15, 0);
        let chunker = RecursiveChunker::new(config);
        let text = "12345678901234. 12345678901234? 12345678901234!";
        let chunks = chunker.chunk(text);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "12345678901234.");
        assert_eq!(chunks[1], "12345678901234?");
        assert_eq!(chunks[2], "12345678901234!");

        for chunk in &chunks {
            assert_eq!(
                chunk.chars().count(),
                15,
                "Expected chunk length to match exact boundary: {chunk}"
            );
            assert!(chunk.chars().count() <= 15);
        }
    }

    #[test]
    fn issue_1705_whitespace_discarded_while_punctuation_survives() {
        // Prove that pure whitespace separators (\n\n) are discarded
        // while sentence-ending punctuation (.) survives
        let config = RecursiveChunkConfig::new(40, 0);
        let chunker = RecursiveChunker::new(config);
        let text = "Paragraph one content.\n\nParagraph two content.\n\nParagraph three content.";
        let chunks = chunker.chunk(text);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "Paragraph one content.");
        assert_eq!(chunks[1], "Paragraph two content.");
        assert_eq!(chunks[2], "Paragraph three content.");

        for chunk in &chunks {
            assert!(!chunk.contains('\n'), "Whitespace should be discarded: {chunk}");
            assert!(chunk.ends_with('.'), "Punctuation should be preserved: {chunk}");
        }
    }

    #[test]
    fn issue_1705_merged_chunks_retain_intermediate_punctuation() {
        // When multiple clauses or sentences fit into a single chunk,
        // intermediate punctuation and spacing must be preserved naturally.
        let config = RecursiveChunkConfig::new(60, 0);
        let chunker = RecursiveChunker::new(config);
        let text = "First clause, second clause. Third sentence.";
        let chunks = chunker.chunk(text);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "First clause, second clause. Third sentence.");
    }
}
