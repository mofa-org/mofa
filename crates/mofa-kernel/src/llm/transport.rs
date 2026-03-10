/// Transport-layer abstractions for streaming LLM responses

/// Single parsed Server-Sent Events frame
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseFrame {
    pub event_type: String,
    pub data: String,
}

impl SseFrame {
    pub fn new(event_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            data: data.into(),
        }
    }
    
    pub fn data_only(data: impl Into<String>) -> Self {
        Self {
            event_type: String::new(),
            data: data.into(),
        }
    }

    pub fn is_done(&self) -> bool {
        self.data.trim() == "[DONE]"
    }
}

/// Errors from the transport layer (SSE / WebSocket / HTTP)
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TransportError {
    /// Underlying network / connection error
    #[error("connection error: {0}")]
    Connection(String),

    /// Malformed frame that could not be parsed
    #[error("parse error: {0}")]
    Parse(String),

    /// Stream timed out waiting for next frame
    #[error("timeout: {0}")]
    Timeout(String),

    /// UTF-8 decoding failure on incoming bytes
    #[error("encoding error: {0}")]
    Encoding(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_constructors() {
        let f = SseFrame::new("message_start", r#"{"type":"message_start"}"#);
        assert_eq!(f.event_type, "message_start");
        assert!(!f.is_done());

        let d = SseFrame::data_only("[DONE]");
        assert!(d.event_type.is_empty());
        assert!(d.is_done());
    }

}
