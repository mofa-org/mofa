//! Bridges `BoxTokenStream` into foundation `TextStream` and `StreamEvent` streams

use futures::StreamExt;
use mofa_kernel::llm::streaming::{BoxTokenStream, StreamError};

use super::agent::{StreamEvent, TextStream};
use super::types::{LLMError, LLMResult};

/// `StreamError` to `LLMError`
pub fn stream_error_to_llm_error(err: StreamError) -> LLMError {
    match err {
        StreamError::Provider { message, .. } => LLMError::ApiError { code: None, message },
        StreamError::Connection(msg) => LLMError::NetworkError(msg),
        StreamError::Parse(msg) => LLMError::SerializationError(msg),
        StreamError::Timeout(msg) => LLMError::Timeout(msg),
        StreamError::Cancelled => LLMError::Other("stream cancelled".into()),
        _ => LLMError::Other(err.to_string()),
    }
}

/// `LLMError` to `StreamError`
pub fn llm_error_to_stream_error(provider: &str, err: LLMError) -> StreamError {
    match err {
        LLMError::NetworkError(msg) | LLMError::Timeout(msg) => StreamError::Connection(msg),
        LLMError::SerializationError(msg) => StreamError::Parse(msg),
        other => StreamError::provider(provider, other.to_string()),
    }
}

pub fn token_stream_to_text(stream: BoxTokenStream) -> TextStream {
    let mapped = stream.filter_map(|result| async move {
        match result {
            Ok(chunk) => {
                if chunk.delta.is_empty() || chunk.is_done() {
                    None
                } else {
                    Some(Ok(chunk.delta))
                }
            }
            Err(err) => Some(Err(stream_error_to_llm_error(err))),
        }
    });
    Box::pin(mapped)
}

/// `BoxTokenStream`to stream of `StreamEvent`s
pub fn token_stream_to_events(
    stream: BoxTokenStream,
) -> std::pin::Pin<Box<dyn futures::Stream<Item = LLMResult<StreamEvent>> + Send>> {
    let mapped = stream.flat_map(|result| {
        let events: Vec<LLMResult<StreamEvent>> = match result {
            Ok(chunk) => {
                let mut v = Vec::new();

                // Tool call events
                if let Some(ref tcs) = chunk.tool_calls {
                    for tc in tcs {
                        if tc.id.is_some() {
                            v.push(Ok(StreamEvent::ToolCallStart {
                                id: tc.id.clone().unwrap_or_default(),
                                name: tc
                                    .function
                                    .as_ref()
                                    .and_then(|f| f.name.clone())
                                    .unwrap_or_default(),
                            }));
                        } else if let Some(ref f) = tc.function {
                            v.push(Ok(StreamEvent::ToolCallDelta {
                                id: tc.id.clone().unwrap_or_default(),
                                arguments_delta: f.arguments.clone().unwrap_or_default(),
                            }));
                        }
                    }
                }

                if !chunk.delta.is_empty() {
                    v.push(Ok(StreamEvent::Text(chunk.delta.clone())));
                }

                if chunk.is_done() {
                    v.push(Ok(StreamEvent::Done(
                        chunk.finish_reason.as_ref().map(|fr| format!("{:?}", fr)),
                    )));
                }

                v
            }
            Err(err) => vec![Err(stream_error_to_llm_error(err))],
        };
        futures::stream::iter(events)
    });
    Box::pin(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use mofa_kernel::llm::streaming::StreamChunk;

    fn text(s: &str) -> Result<StreamChunk, StreamError> {
        Ok(StreamChunk { delta: s.into(), ..Default::default() })
    }

    fn done() -> Result<StreamChunk, StreamError> {
        Ok(StreamChunk {
            finish_reason: Some(mofa_kernel::llm::types::FinishReason::Stop),
            ..Default::default()
        })
    }

    fn tool_start() -> Result<StreamChunk, StreamError> {
        Ok(StreamChunk {
            tool_calls: Some(vec![mofa_kernel::llm::types::ToolCallDelta {
                index: 0,
                id: Some("tc1".into()),
                call_type: Some("function".into()),
                function: Some(mofa_kernel::llm::types::FunctionCallDelta {
                    name: Some("search".into()),
                    arguments: None,
                }),
            }]),
            ..Default::default()
        })
    }

    #[tokio::test]
    async fn text_bridge_filters_empty_and_done() {
        let stream: BoxTokenStream = Box::pin(futures::stream::iter(vec![
            text("a"),
            text(""),     // filtered
            text("b"),
            done(),
        ]));
        let items: Vec<_> = token_stream_to_text(stream)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(items, vec!["a", "b"]);
    }

    #[tokio::test]
    async fn text_bridge_propagates_errors() {
        let stream: BoxTokenStream = Box::pin(futures::stream::iter(vec![
            text("ok"),
            Err(StreamError::Connection("oops".into())),
        ]));
        let items: Vec<_> = token_stream_to_text(stream).collect().await;
        assert!(items[0].is_ok());
        assert!(items[1].is_err());
    }

    #[tokio::test]
    async fn events_bridge_text_and_done() {
        let stream: BoxTokenStream = Box::pin(futures::stream::iter(vec![
            text("hi"),
            done(),
        ]));
        let evts: Vec<_> = token_stream_to_events(stream)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();
        assert!(matches!(evts[0], StreamEvent::Text(ref s) if s == "hi"));
        assert!(matches!(evts[1], StreamEvent::Done(_)));
    }

    #[tokio::test]
    async fn events_bridge_tool_calls() {
        let stream: BoxTokenStream = Box::pin(futures::stream::iter(vec![tool_start()]));
        let evts: Vec<_> = token_stream_to_events(stream)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();
        assert!(matches!(evts[0], StreamEvent::ToolCallStart { ref name, .. } if name == "search"));
    }

    #[tokio::test]
    async fn error_roundtrip() {
        let se = StreamError::provider("test", "fail");
        let le = stream_error_to_llm_error(se);
        let se2 = llm_error_to_stream_error("test", le);
        assert!(matches!(se2, StreamError::Provider { .. }));
    }
}
