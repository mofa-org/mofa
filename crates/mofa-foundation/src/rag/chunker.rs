//! Text chunking utilities for splitting documents before embedding
//!
//! Provides configurable text splitting strategies for preparing documents
//! for embedding and storage in a vector store.

/// Configuration for text chunking.
#[derive(Debug, Clone)]
pub struct ChunkConfig {
    /// Maximum number of characters per chunk
    pub chunk_size: usize,
    /// Number of characters to overlap between consecutive chunks
    pub chunk_overlap: usize,
}

impl ChunkConfig {
    /// Create a new chunk config with the given size and overlap.
    pub fn new(chunk_size: usize, chunk_overlap: usize) -> Self {
        Self {
            chunk_size,
            chunk_overlap,
        }
    }
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            chunk_size: 512,
            chunk_overlap: 64,
        }
    }
}

/// Utility for splitting text into chunks suitable for embedding.
pub struct TextChunker {
    config: ChunkConfig,
}

impl TextChunker {
    /// Create a new text chunker with the given configuration.
    pub fn new(config: ChunkConfig) -> Self {
        Self { config }
    }

    /// Create a text chunker with default settings (512 chars, 64 overlap).
    pub fn with_defaults() -> Self {
        Self {
            config: ChunkConfig::default(),
        }
    }

    /// Split text into chunks by character count with overlap.
    ///
    /// Each chunk will be at most `chunk_size` characters long.
    /// Consecutive chunks overlap by `chunk_overlap` characters so that
    /// context is not lost at chunk boundaries.
    pub fn chunk_by_chars(&self, text: &str) -> Vec<String> {
        if text.is_empty() {
            return vec![];
        }

        if text.len() <= self.config.chunk_size {
            return vec![text.to_string()];
        }

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
            chunks.push(chunk);

            if end >= chars.len() {
                break;
            }

            start += step;
        }

        chunks
    }

    /// Split text into chunks at sentence boundaries.
    ///
    /// Tries to keep chunks under `chunk_size` characters while splitting
    /// at sentence endings (periods, question marks, exclamation marks
    /// followed by whitespace or end of string).
    pub fn chunk_by_sentences(&self, text: &str) -> Vec<String> {
        if text.is_empty() {
            return vec![];
        }

        if text.len() <= self.config.chunk_size {
            return vec![text.to_string()];
        }

        let sentences = split_sentences(text);
        let mut chunks = Vec::new();
        let mut current = String::new();

        for sentence in sentences {
            if current.is_empty() {
                current = sentence;
            } else if current.len() + sentence.len() <= self.config.chunk_size {
                current.push_str(&sentence);
            } else {
                if !current.is_empty() {
                    chunks.push(current.trim().to_string());
                }
                current = sentence;
            }
        }

        if !current.is_empty() {
            chunks.push(current.trim().to_string());
        }

        chunks.retain(|c| !c.is_empty());
        chunks
    }
}

impl Default for TextChunker {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Split text into sentences at common sentence-ending punctuation.
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    for i in 0..len {
        current.push(chars[i]);

        let is_sentence_end = (chars[i] == '.' || chars[i] == '?' || chars[i] == '!')
            && (i + 1 >= len || chars[i + 1].is_whitespace());

        if is_sentence_end {
            sentences.push(current.clone());
            current.clear();
        }
    }

    if !current.is_empty() {
        sentences.push(current);
    }

    sentences
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_by_chars_short_text() {
        let chunker = TextChunker::new(ChunkConfig::new(100, 10));
        let chunks = chunker.chunk_by_chars("short text");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "short text");
    }

    #[test]
    fn test_chunk_by_chars_empty() {
        let chunker = TextChunker::with_defaults();
        let chunks = chunker.chunk_by_chars("");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_by_chars_splits_correctly() {
        let chunker = TextChunker::new(ChunkConfig::new(10, 3));
        let text = "abcdefghijklmnopqrstuvwxyz";
        let chunks = chunker.chunk_by_chars(text);

        assert!(chunks.len() > 1);
        assert!(chunks[0].len() <= 10);
        for chunk in &chunks {
            assert!(chunk.len() <= 10);
        }
    }

    #[test]
    fn test_chunk_by_chars_overlap() {
        let chunker = TextChunker::new(ChunkConfig::new(10, 4));
        let text = "abcdefghijklmnopqrst";
        let chunks = chunker.chunk_by_chars(text);

        assert!(chunks.len() >= 2);
        // With chunk_size=10 and overlap=4, step=6
        // chunk 0: abcdefghij (0..10)
        // chunk 1: ghijklmnop (6..16)
        // The last 4 chars of chunk 0 should appear at the start of chunk 1
        let overlap = &chunks[0][6..10];
        assert!(chunks[1].starts_with(overlap));
    }

    #[test]
    fn test_chunk_by_sentences_short() {
        let chunker = TextChunker::new(ChunkConfig::new(200, 0));
        let text = "Hello world.";
        let chunks = chunker.chunk_by_sentences(text);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_chunk_by_sentences_splits() {
        let chunker = TextChunker::new(ChunkConfig::new(30, 0));
        let text = "First sentence. Second sentence. Third sentence.";
        let chunks = chunker.chunk_by_sentences(text);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_chunk_by_sentences_empty() {
        let chunker = TextChunker::with_defaults();
        let chunks = chunker.chunk_by_sentences("");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_split_sentences() {
        let sentences = split_sentences("Hello world. How are you? I'm fine!");
        assert_eq!(sentences.len(), 3);
    }

    #[test]
    fn test_split_sentences_no_ending_punctuation() {
        let sentences = split_sentences("No ending punctuation");
        assert_eq!(sentences.len(), 1);
        assert_eq!(sentences[0], "No ending punctuation");
    }

    #[test]
    fn test_default_config() {
        let config = ChunkConfig::default();
        assert_eq!(config.chunk_size, 512);
        assert_eq!(config.chunk_overlap, 64);
    }
}
