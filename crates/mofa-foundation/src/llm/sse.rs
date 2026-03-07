/// Generic SSE (Server-Sent Events) decoder
use futures::stream::Stream;
use mofa_kernel::llm::transport::{SseFrame, TransportError};
use std::pin::Pin;

/// Type alias for a boxed SSE frame stream
pub type SseStream = Pin<Box<dyn Stream<Item = Result<SseFrame, TransportError>> + Send>>;

pub fn decode_sse(resp: reqwest::Response) -> SseStream {
    let stream = futures::stream::unfold(
        SseState::new(resp),
        |mut state| async move {
            loop {
                if let Some(newline_pos) = state.buf.find('\n') {
                    let line = state.buf[..newline_pos]
                        .trim_end_matches('\r')
                        .to_string();
                    state.buf = state.buf[newline_pos + 1..].to_string();
                    if line.is_empty() {
                        if !state.data.is_empty() {
                            let frame = SseFrame::new(
                                std::mem::take(&mut state.event_type),
                                std::mem::take(&mut state.data),
                            );
                            return Some((Ok(frame), state));
                        }
                        continue;
                    }

                    if let Some(rest) = line.strip_prefix("event:") {
                        state.event_type = rest.trim_start().to_string();
                        continue;
                    }
                    if let Some(rest) = line.strip_prefix("data:") {
                        let payload = rest.strip_prefix(' ').unwrap_or(rest);
                        if !state.data.is_empty() {
                            state.data.push('\n');
                        }
                        state.data.push_str(payload);
                        continue;
                    }
                    continue;
                }

                // more bytes from the network
                match state.resp.chunk().await {
                    Ok(Some(bytes)) => match std::str::from_utf8(&bytes) {
                        Ok(s) => state.buf.push_str(s),
                        Err(e) => {
                            return Some((
                                Err(TransportError::Encoding(e.to_string())),
                                state,
                            ));
                        }
                    },
                    Ok(None) => {
                        // Stream ended emit any buffered data as a final frame
                        if !state.data.is_empty() {
                            let frame = SseFrame::new(
                                std::mem::take(&mut state.event_type),
                                std::mem::take(&mut state.data),
                            );
                            return Some((Ok(frame), state));
                        }
                        return None;
                    }
                    Err(e) => {
                        return Some((
                            Err(TransportError::Connection(e.to_string())),
                            state,
                        ));
                    }
                }
            }
        },
    );

    Box::pin(stream)
}

/// Internal state for the SSE decoder unfold loop
struct SseState {
    resp: reqwest::Response,
    buf: String,
    event_type: String,
    data: String,
}

impl SseState {
    fn new(resp: reqwest::Response) -> Self {
        Self {
            resp,
            buf: String::with_capacity(1024),
            event_type: String::new(),
            data: String::new(),
        }
    }
}

pub fn transport_error_to_llm_error(err: TransportError) -> crate::llm::types::LLMError {
    use crate::llm::types::LLMError;
    match err {
        TransportError::Connection(msg) => LLMError::NetworkError(msg),
        TransportError::Parse(msg) => LLMError::SerializationError(msg),
        TransportError::Timeout(msg) => LLMError::NetworkError(format!("timeout: {msg}")),
        TransportError::Encoding(msg) => LLMError::SerializationError(format!("encoding: {msg}")),
        _ => LLMError::NetworkError(format!("transport error: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    /// Build a mock reqwest::Response from raw SSE text
    fn mock_response(body: &str) -> reqwest::Response {
        http::Response::builder()
            .status(200)
            .header("content-type", "text/event-stream")
            .body(body.to_string())
            .unwrap()
            .into()
    }

    #[tokio::test]
    async fn decodes_basic_sse_frames() {
        let raw = "\
event: message_start\n\
data: {\"type\":\"message_start\"}\n\
\n\
event: content_block_delta\n\
data: {\"text\":\"Hello\"}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n";

        let frames: Vec<_> = decode_sse(mock_response(raw))
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].event_type, "message_start");
        assert_eq!(frames[0].data, "{\"type\":\"message_start\"}");
        assert_eq!(frames[1].event_type, "content_block_delta");
        assert_eq!(frames[2].event_type, "message_stop");
    }

    #[tokio::test]
    async fn handles_data_only_frames() {
        let raw = "data: {\"text\":\"hello\"}\n\ndata: [DONE]\n\n";

        let frames: Vec<_> = decode_sse(mock_response(raw))
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(frames.len(), 2);
        assert!(frames[0].event_type.is_empty());
        assert_eq!(frames[0].data, "{\"text\":\"hello\"}");
        assert!(frames[1].is_done());
    }

    #[tokio::test]
    async fn handles_multiline_data() {
        let raw = "data: line1\ndata: line2\n\n";

        let frames: Vec<_> = decode_sse(mock_response(raw))
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "line1\nline2");
    }

    #[test]
    fn transport_error_maps_to_llm_error() {
        use crate::llm::types::LLMError;

        let err = transport_error_to_llm_error(TransportError::Connection("reset".into()));
        assert!(matches!(err, LLMError::NetworkError(msg) if msg == "reset"));

        let err = transport_error_to_llm_error(TransportError::Encoding("bad utf8".into()));
        assert!(matches!(err, LLMError::SerializationError(_)));
    }
}
