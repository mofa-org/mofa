//! Converts provider `ChatStream` into unified `BoxTokenStream`

use futures::StreamExt;
use mofa_kernel::llm::streaming::{BoxTokenStream, StreamChunk, StreamError, UsageDelta};

use super::provider::ChatStream;
use super::stream_bridge::llm_error_to_stream_error;
use super::types::{ChatCompletionChunk, LLMError};

/// Normalises a `ChatStream` into a `BoxTokenStream`
pub trait StreamAdapter: Send + Sync {
    fn provider_name(&self) -> &str;
    fn adapt(&self, raw: ChatStream) -> BoxTokenStream;
}

#[derive(Debug, Clone)]
pub struct GenericStreamAdapter {
    name: String,
}

impl GenericStreamAdapter {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl StreamAdapter for GenericStreamAdapter {
    fn provider_name(&self) -> &str {
        &self.name
    }

    fn adapt(&self, raw: ChatStream) -> BoxTokenStream {
        let name = self.name.clone();
        let stream = raw.map(move |result| match result {
            Ok(chunk) => Ok(chunk_to_stream_chunk(chunk)),
            Err(err) => Err(llm_error_to_stream_error(&name, err)),
        });
        Box::pin(stream)
    }
}

/// Return adapter for the given provider name
pub fn adapter_for_provider(provider_name: &str) -> GenericStreamAdapter {
    let canonical = match provider_name {
        "claude" => "anthropic",
        "google" => "gemini",
        other => other,
    };
    GenericStreamAdapter::new(canonical)
}

/// Map `ChatCompletionChunk` to `StreamChunk`
fn chunk_to_stream_chunk(chunk: ChatCompletionChunk) -> StreamChunk {
    let choice = chunk.choices.first();

    let delta = choice
        .and_then(|c| c.delta.content.clone())
        .unwrap_or_default();

    let finish_reason = choice.and_then(|c| c.finish_reason.clone()).map(|fr| match fr {
        super::types::FinishReason::Stop => mofa_kernel::llm::types::FinishReason::Stop,
        super::types::FinishReason::Length => mofa_kernel::llm::types::FinishReason::Length,
        super::types::FinishReason::ToolCalls => mofa_kernel::llm::types::FinishReason::ToolCalls,
        super::types::FinishReason::ContentFilter => mofa_kernel::llm::types::FinishReason::ContentFilter,
    });

    let usage = chunk.usage.map(|u| UsageDelta {
        prompt_tokens: Some(u.prompt_tokens),
        completion_tokens: Some(u.completion_tokens),
        total_tokens: Some(u.total_tokens),
    });

    let tool_calls = choice
        .and_then(|c| c.delta.tool_calls.clone())
        .map(|tcs| {
            tcs.into_iter()
                .map(|tc| mofa_kernel::llm::types::ToolCallDelta {
                    index: tc.index,
                    id: tc.id,
                    call_type: tc.call_type,
                    function: tc.function.map(|f| mofa_kernel::llm::types::FunctionCallDelta {
                        name: f.name,
                        arguments: f.arguments,
                    }),
                })
                .collect()
        });

    StreamChunk { delta, finish_reason, usage, tool_calls }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::{ChunkChoice, ChunkDelta, FinishReason, Usage};
    use futures::StreamExt;

    fn text_chunk(text: &str) -> ChatCompletionChunk {
        ChatCompletionChunk {
            id: "id".into(), object: "chat.completion.chunk".into(),
            created: 0, model: "m".into(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: ChunkDelta { role: None, content: Some(text.into()), tool_calls: None },
                finish_reason: None,
            }],
            usage: None,
        }
    }

    fn done_chunk(reason: FinishReason, usage: Option<Usage>) -> ChatCompletionChunk {
        ChatCompletionChunk {
            id: "id".into(), object: "chat.completion.chunk".into(),
            created: 0, model: "m".into(),
            choices: vec![ChunkChoice {
                index: 0, delta: ChunkDelta::default(), finish_reason: Some(reason),
            }],
            usage,
        }
    }

    #[tokio::test]
    async fn adapter_maps_text_and_done() {
        let chunks: Vec<Result<ChatCompletionChunk, LLMError>> = vec![
            Ok(text_chunk("Hello")),
            Ok(text_chunk(" world")),
            Ok(done_chunk(FinishReason::Stop, None)),
        ];
        let mut s = adapter_for_provider("openai")
            .adapt(Box::pin(futures::stream::iter(chunks)));

        assert_eq!(s.next().await.unwrap().unwrap().delta, "Hello");
        assert_eq!(s.next().await.unwrap().unwrap().delta, " world");
        let done = s.next().await.unwrap().unwrap();
        assert!(done.is_done());
        assert_eq!(done.finish_reason, Some(mofa_kernel::llm::types::FinishReason::Stop));
        assert!(s.next().await.is_none());
    }

    #[tokio::test]
    async fn adapter_maps_usage_and_tool_calls() {
        let tool_chunk = ChatCompletionChunk {
            id: "id".into(), object: "c".into(), created: 0, model: "m".into(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: ChunkDelta {
                    role: None, content: None,
                    tool_calls: Some(vec![crate::llm::types::ToolCallDelta {
                        index: 0, id: Some("tc".into()),
                        call_type: Some("function".into()),
                        function: Some(crate::llm::types::FunctionCallDelta {
                            name: Some("search".into()), arguments: Some("{}".into()),
                        }),
                    }]),
                },
                finish_reason: None,
            }],
            usage: None,
        };
        let usage_chunk = done_chunk(FinishReason::Stop, Some(Usage {
            prompt_tokens: 10, completion_tokens: 20, total_tokens: 30,
        }));
        let mut s = adapter_for_provider("anthropic")
            .adapt(Box::pin(futures::stream::iter(vec![Ok(tool_chunk), Ok(usage_chunk)])));

        let tc = s.next().await.unwrap().unwrap();
        assert_eq!(tc.tool_calls.as_ref().unwrap()[0].id.as_deref(), Some("tc"));

        let u = s.next().await.unwrap().unwrap();
        let usage = u.usage.unwrap();
        assert_eq!(usage.prompt_tokens, Some(10));
        assert_eq!(usage.total_tokens, Some(30));
    }

    #[tokio::test]
    async fn adapter_maps_errors() {
        let chunks: Vec<Result<ChatCompletionChunk, LLMError>> = vec![
            Ok(text_chunk("ok")),
            Err(LLMError::NetworkError("reset".into())),
        ];
        let mut s = adapter_for_provider("ollama")
            .adapt(Box::pin(futures::stream::iter(chunks)));

        assert!(s.next().await.unwrap().is_ok());
        match s.next().await.unwrap().unwrap_err() {
            StreamError::Connection(msg) => assert_eq!(msg, "reset"),
            other => panic!("expected Connection, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn adapter_for_provider_canonicalises_names() {
        assert_eq!(adapter_for_provider("claude").provider_name(), "anthropic");
        assert_eq!(adapter_for_provider("google").provider_name(), "gemini");
        assert_eq!(adapter_for_provider("openai").provider_name(), "openai");
        assert_eq!(adapter_for_provider("custom").provider_name(), "custom");
    }

    #[tokio::test]
    async fn all_finish_reasons_mapped() {
        for (src, expected) in [
            (FinishReason::Stop, mofa_kernel::llm::types::FinishReason::Stop),
            (FinishReason::Length, mofa_kernel::llm::types::FinishReason::Length),
            (FinishReason::ToolCalls, mofa_kernel::llm::types::FinishReason::ToolCalls),
            (FinishReason::ContentFilter, mofa_kernel::llm::types::FinishReason::ContentFilter),
        ] {
            let mut s = adapter_for_provider("test")
                .adapt(Box::pin(futures::stream::iter(vec![Ok(done_chunk(src, None))])));
            assert_eq!(s.next().await.unwrap().unwrap().finish_reason, Some(expected));
        }
    }
}
