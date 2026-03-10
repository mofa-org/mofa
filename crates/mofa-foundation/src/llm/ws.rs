/// WebSocket transport for streaming LLM responses
use futures::stream::{Stream, StreamExt};
use mofa_kernel::llm::transport::{SseFrame, TransportError};
use std::pin::Pin;
use tokio_tungstenite::tungstenite::Message;

/// Type alias for a boxed WebSocket frame stream
pub type WsStream = Pin<Box<dyn Stream<Item = Result<SseFrame, TransportError>> + Send>>;

/// Convert a `tokio_tungstenite` WebSocket stream into an `SseFrame` stream
pub fn decode_ws<S>(ws: S) -> WsStream
where
    S: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Send + 'static,
{
    let stream = ws.filter_map(|result| async move {
        match result {
            Ok(Message::Text(text)) => {
                let frame = parse_ws_message(&text);
                Some(Ok(frame))
            }
            Ok(Message::Binary(bytes)) => {
                match String::from_utf8(bytes.to_vec()) {
                    Ok(text) => Some(Ok(SseFrame::data_only(text))),
                    Err(e) => Some(Err(TransportError::Encoding(e.to_string()))),
                }
            }
            Ok(Message::Close(_)) => None,
            Ok(Message::Ping(_) | Message::Pong(_)) => None,
            Ok(Message::Frame(_)) => None,
            Err(e) => Some(Err(TransportError::Connection(e.to_string()))),
        }
    });

    Box::pin(stream)
}

/// Parse a WebSocket text message into an `SseFrame`
fn parse_ws_message(text: &str) -> SseFrame {
    let mut event_type = String::new();
    let mut data = String::new();

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event_type = rest.trim_start().to_string();
        } else if let Some(rest) = line.strip_prefix("data:") {
            let payload = rest.strip_prefix(' ').unwrap_or(rest);
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(payload);
        }
    }

    if data.is_empty() {
        SseFrame::data_only(text.to_string())
    } else {
        SseFrame::new(event_type, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    #[tokio::test]
    async fn decodes_text_messages() {
        let messages = vec![
            Ok(Message::Text("hello".into())),
            Ok(Message::Text("world".into())),
        ];

        let frames: Vec<_> = decode_ws(stream::iter(messages))
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].data, "hello");
        assert_eq!(frames[1].data, "world");
    }

    #[tokio::test]
    async fn parses_sse_formatted_ws_messages() {
        let msg = "event: content_block_delta\ndata: {\"text\":\"Hi\"}";
        let messages = vec![Ok(Message::Text(msg.into()))];

        let frames: Vec<_> = decode_ws(stream::iter(messages))
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].event_type, "content_block_delta");
        assert_eq!(frames[0].data, "{\"text\":\"Hi\"}");
    }

    #[tokio::test]
    async fn propagates_connection_errors() {
        let messages: Vec<Result<Message, tokio_tungstenite::tungstenite::Error>> = vec![
            Err(tokio_tungstenite::tungstenite::Error::ConnectionClosed),
        ];

        let results: Vec<_> = decode_ws(stream::iter(messages))
            .collect::<Vec<_>>()
            .await;

        assert_eq!(results.len(), 1);
        assert!(matches!(results[0], Err(TransportError::Connection(_))));
    }
}
