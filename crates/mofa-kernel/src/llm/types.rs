use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Role {
    System,
    #[default]
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContentPart {
    Text { text: String },
    Image { image_url: ImageUrl },
    Audio { audio: AudioData },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<ImageDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    Low,
    High,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioData {
    pub data: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}
impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: Some(MessageContent::Text(content.into())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Some(MessageContent::Text(content.into())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user_with_content(content: MessageContent) -> Self {
        Self {
            role: Role::User,
            content: Some(content),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user_with_parts(parts: Vec<ContentPart>) -> Self {
        Self::user_with_content(MessageContent::Parts(parts))
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: Some(MessageContent::Text(content.into())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant_with_tool_calls(tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content: None,
            name: None,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: Some(MessageContent::Text(content.into())),
            name: None,
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    pub fn user_with_image(text: impl Into<String>, image_url: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Some(MessageContent::Parts(vec![
                ContentPart::Text { text: text.into() },
                ContentPart::Image {
                    image_url: ImageUrl {
                        url: image_url.into(),
                        detail: None,
                    },
                },
            ])),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn text_content(&self) -> Option<&str> {
        match &self.content {
            Some(MessageContent::Text(s)) => Some(s),
            Some(MessageContent::Parts(parts)) => {
                for part in parts {
                    if let ContentPart::Text { text } = part {
                        return Some(text);
                    }
                }
                None
            }
            None => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

impl Tool {
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        let parameters = parameters;
        Self {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: name.into(),
                description: Some(description.into()),
                parameters: Some(parameters),
                strict: None,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    Auto,
    None,
    Required,
    Specific {
        #[serde(rename = "type")]
        choice_type: String,
        function: ToolChoiceFunction,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceFunction {
    pub name: String,
}


#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatCompletionRequest {
    pub model: String,

    #[serde(default)]
    pub messages: Vec<ChatMessage>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
}

impl ChatCompletionRequest {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }
    pub fn message(mut self, message: ChatMessage) -> Self {
        self.messages.push(message);
        self
    }

    pub fn system(mut self, content: impl Into<String>) -> Self {
        self.messages.push(ChatMessage::system(content));
        self
    }

    pub fn user(mut self, content: impl Into<String>) -> Self {
        self.messages.push(ChatMessage::user(content));
        self
    }

    pub fn temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    pub fn tool(mut self, tool: Tool) -> Self {
        self.tools.get_or_insert_with(Vec::new).push(tool);
        self
    }

    pub fn tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn stream(mut self) -> Self {
        self.stream = Some(true);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub format_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<serde_json::Value>,
}

impl ResponseFormat {
    pub fn text() -> Self {
        Self {
            format_type: "text".to_string(),
            json_schema: None,
        }
    }

    pub fn json() -> Self {
        Self {
            format_type: "json_object".to_string(),
            json_schema: None,
        }
    }

    pub fn json_schema(schema: serde_json::Value) -> Self {
        Self {
            format_type: "json_schema".to_string(),
            json_schema: Some(schema),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<Choice>,
}

impl ChatCompletionResponse {
    pub fn content(&self) -> Option<&str> {
        self.choices.first()?.message.text_content()
    }

    pub fn tool_calls(&self) -> Option<&Vec<ToolCall>> {
        self.choices.first()?.message.tool_calls.as_ref()
    }

    pub fn has_tool_calls(&self) -> bool {
        self.tool_calls().map(|t| !t.is_empty()).unwrap_or(false)
    }

    pub fn finish_reason(&self) -> Option<&FinishReason> {
        self.choices.first()?.finish_reason.as_ref()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    pub choices: Vec<ChunkChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: ChunkDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub call_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<FunctionCallDelta>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: EmbeddingInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub data: Vec<EmbeddingData>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<EmbeddingUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]

pub enum EmbeddingInput {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingData {
    pub object: String,
    pub index: u32,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    pub prompt_tokens: u32,
    pub total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- Role ---

    #[test]
    fn role_default_is_user() {
        assert_eq!(Role::default(), Role::User);
    }

    #[test]
    fn role_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Role::System).unwrap(), "\"system\"");
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            "\"assistant\""
        );
        assert_eq!(serde_json::to_string(&Role::Tool).unwrap(), "\"tool\"");
    }

    #[test]
    fn role_roundtrip() {
        for role in [Role::System, Role::User, Role::Assistant, Role::Tool] {
            let json = serde_json::to_string(&role).unwrap();
            let back: Role = serde_json::from_str(&json).unwrap();
            assert_eq!(role, back);
        }
    }

    // --- ChatMessage constructors ---

    #[test]
    fn system_message_sets_role_and_content() {
        let msg = ChatMessage::system("You are helpful.");
        assert_eq!(msg.role, Role::System);
        assert_eq!(msg.text_content(), Some("You are helpful."));
        assert!(msg.tool_calls.is_none());
        assert!(msg.tool_call_id.is_none());
    }

    #[test]
    fn user_message_sets_role_and_content() {
        let msg = ChatMessage::user("Hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.text_content(), Some("Hello"));
    }

    #[test]
    fn assistant_message_sets_role_and_content() {
        let msg = ChatMessage::assistant("Hi there");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.text_content(), Some("Hi there"));
    }

    #[test]
    fn tool_result_sets_role_and_id() {
        let msg = ChatMessage::tool_result("call_123", "result data");
        assert_eq!(msg.role, Role::Tool);
        assert_eq!(msg.text_content(), Some("result data"));
        assert_eq!(msg.tool_call_id.as_deref(), Some("call_123"));
    }

    #[test]
    fn assistant_with_tool_calls_has_no_content() {
        let tc = ToolCall {
            id: "call_1".into(),
            call_type: "function".into(),
            function: FunctionCall {
                name: "get_weather".into(),
                arguments: r#"{"city":"NYC"}"#.into(),
            },
        };
        let msg = ChatMessage::assistant_with_tool_calls(vec![tc]);
        assert_eq!(msg.role, Role::Assistant);
        assert!(msg.content.is_none());
        assert_eq!(msg.tool_calls.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn user_with_image_creates_multipart() {
        let msg = ChatMessage::user_with_image("describe this", "https://img.example.com/a.png");
        assert_eq!(msg.role, Role::User);
        if let Some(MessageContent::Parts(parts)) = &msg.content {
            assert_eq!(parts.len(), 2);
            assert!(matches!(&parts[0], ContentPart::Text { text } if text == "describe this"));
            assert!(matches!(&parts[1], ContentPart::Image { image_url } if image_url.url == "https://img.example.com/a.png"));
        } else {
            panic!("expected Parts content");
        }
    }

    #[test]
    fn user_with_parts_delegates_correctly() {
        let parts = vec![ContentPart::Text {
            text: "hello".into(),
        }];
        let msg = ChatMessage::user_with_parts(parts);
        assert_eq!(msg.role, Role::User);
        assert!(matches!(msg.content, Some(MessageContent::Parts(_))));
    }

    // --- text_content extraction ---

    #[test]
    fn text_content_from_text_variant() {
        let msg = ChatMessage::user("simple text");
        assert_eq!(msg.text_content(), Some("simple text"));
    }

    #[test]
    fn text_content_from_parts_returns_first_text() {
        let msg = ChatMessage::user_with_image("first text", "https://example.com/img.png");
        assert_eq!(msg.text_content(), Some("first text"));
    }

    #[test]
    fn text_content_returns_none_when_no_content() {
        let msg = ChatMessage::assistant_with_tool_calls(vec![]);
        assert!(msg.text_content().is_none());
    }

    #[test]
    fn text_content_returns_none_for_image_only_parts() {
        let msg = ChatMessage {
            role: Role::User,
            content: Some(MessageContent::Parts(vec![ContentPart::Image {
                image_url: ImageUrl {
                    url: "https://example.com/img.png".into(),
                    detail: None,
                },
            }])),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        };
        assert!(msg.text_content().is_none());
    }

    // --- ChatMessage serialization ---

    #[test]
    fn chat_message_roundtrip_json() {
        let msg = ChatMessage::user("roundtrip test");
        let json = serde_json::to_string(&msg).unwrap();
        let back: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.role, Role::User);
        assert_eq!(back.text_content(), Some("roundtrip test"));
    }

    #[test]
    fn chat_message_skips_none_fields() {
        let msg = ChatMessage::user("hi");
        let val: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert!(val.get("tool_calls").is_none());
        assert!(val.get("tool_call_id").is_none());
        assert!(val.get("name").is_none());
    }

    // --- Tool / FunctionDefinition ---

    #[test]
    fn tool_function_constructor() {
        let t = Tool::function(
            "search",
            "Search the web",
            json!({"type": "object", "properties": {"q": {"type": "string"}}}),
        );
        assert_eq!(t.tool_type, "function");
        assert_eq!(t.function.name, "search");
        assert_eq!(t.function.description.as_deref(), Some("Search the web"));
        assert!(t.function.parameters.is_some());
        assert!(t.function.strict.is_none());
    }

    // --- ChatCompletionRequest builder ---

    #[test]
    fn request_builder_chain() {
        let req = ChatCompletionRequest::new("gpt-4")
            .system("Be helpful")
            .user("Hello")
            .temperature(0.7)
            .max_tokens(100)
            .stream();

        assert_eq!(req.model, "gpt-4");
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.temperature, Some(0.7));
        assert_eq!(req.max_tokens, Some(100));
        assert_eq!(req.stream, Some(true));
    }

    #[test]
    fn request_builder_tool_appends() {
        let t = Tool::function("a", "desc", json!({}));
        let req = ChatCompletionRequest::new("model").tool(t);
        assert_eq!(req.tools.as_ref().unwrap().len(), 1);

        let t2 = Tool::function("b", "desc2", json!({}));
        let req = req.tool(t2);
        assert_eq!(req.tools.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn request_builder_tools_replaces() {
        let t1 = Tool::function("a", "d", json!({}));
        let t2 = Tool::function("b", "d", json!({}));
        let req = ChatCompletionRequest::new("m")
            .tool(t1)
            .tools(vec![t2]);
        assert_eq!(req.tools.as_ref().unwrap().len(), 1);
        assert_eq!(req.tools.as_ref().unwrap()[0].function.name, "b");
    }

    // --- ResponseFormat ---

    #[test]
    fn response_format_text() {
        let f = ResponseFormat::text();
        assert_eq!(f.format_type, "text");
        assert!(f.json_schema.is_none());
    }

    #[test]
    fn response_format_json() {
        let f = ResponseFormat::json();
        assert_eq!(f.format_type, "json_object");
    }

    #[test]
    fn response_format_json_schema() {
        let schema = json!({"type": "object"});
        let f = ResponseFormat::json_schema(schema.clone());
        assert_eq!(f.format_type, "json_schema");
        assert_eq!(f.json_schema.unwrap(), schema);
    }

    // --- ChatCompletionResponse ---

    #[test]
    fn response_content_extracts_first_choice() {
        let resp = ChatCompletionResponse {
            choices: vec![Choice {
                index: 0,
                message: ChatMessage::assistant("answer"),
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
        };
        assert_eq!(resp.content(), Some("answer"));
        assert_eq!(resp.finish_reason(), Some(&FinishReason::Stop));
        assert!(!resp.has_tool_calls());
    }

    #[test]
    fn response_content_none_when_empty_choices() {
        let resp = ChatCompletionResponse { choices: vec![] };
        assert!(resp.content().is_none());
        assert!(resp.finish_reason().is_none());
        assert!(!resp.has_tool_calls());
    }

    #[test]
    fn response_has_tool_calls_when_present() {
        let tc = ToolCall {
            id: "c1".into(),
            call_type: "function".into(),
            function: FunctionCall {
                name: "f".into(),
                arguments: "{}".into(),
            },
        };
        let resp = ChatCompletionResponse {
            choices: vec![Choice {
                index: 0,
                message: ChatMessage::assistant_with_tool_calls(vec![tc]),
                finish_reason: Some(FinishReason::ToolCalls),
                logprobs: None,
            }],
        };
        assert!(resp.has_tool_calls());
        assert_eq!(resp.tool_calls().unwrap().len(), 1);
    }

    // --- FinishReason serialization ---

    #[test]
    fn finish_reason_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&FinishReason::Stop).unwrap(),
            "\"stop\""
        );
        assert_eq!(
            serde_json::to_string(&FinishReason::ToolCalls).unwrap(),
            "\"tool_calls\""
        );
        assert_eq!(
            serde_json::to_string(&FinishReason::ContentFilter).unwrap(),
            "\"content_filter\""
        );
    }

    // --- EmbeddingInput ---

    #[test]
    fn embedding_input_single_roundtrip() {
        let input = EmbeddingInput::Single("hello".into());
        let json = serde_json::to_string(&input).unwrap();
        let back: EmbeddingInput = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, EmbeddingInput::Single(s) if s == "hello"));
    }

    #[test]
    fn embedding_input_multiple_roundtrip() {
        let input = EmbeddingInput::Multiple(vec!["a".into(), "b".into()]);
        let json = serde_json::to_string(&input).unwrap();
        let back: EmbeddingInput = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, EmbeddingInput::Multiple(v) if v.len() == 2));
    }

    // --- ImageDetail ---

    #[test]
    fn image_detail_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&ImageDetail::Low).unwrap(),
            "\"low\""
        );
        assert_eq!(
            serde_json::to_string(&ImageDetail::High).unwrap(),
            "\"high\""
        );
        assert_eq!(
            serde_json::to_string(&ImageDetail::Auto).unwrap(),
            "\"auto\""
        );
    }

    // --- ChunkDelta defaults ---

    #[test]
    fn chunk_delta_default_all_none() {
        let d = ChunkDelta::default();
        assert!(d.role.is_none());
        assert!(d.content.is_none());
        assert!(d.tool_calls.is_none());
    }
}
