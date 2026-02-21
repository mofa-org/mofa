//! 持久化实体定义
//!
//! 对应数据库表结构的实体类型

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 消息角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// 系统消息
    System,
    /// 用户消息
    User,
    /// 助手消息
    Assistant,
    /// 工具消息
    Tool,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageRole::System => write!(f, "system"),
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
            MessageRole::Tool => write!(f, "tool"),
        }
    }
}

impl std::str::FromStr for MessageRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "system" => Ok(MessageRole::System),
            "user" => Ok(MessageRole::User),
            "assistant" => Ok(MessageRole::Assistant),
            "tool" => Ok(MessageRole::Tool),
            _ => Err(format!("Unknown role: {}", s)),
        }
    }
}

/// API 调用状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ApiCallStatus {
    /// 成功
    #[default]
    Success,
    /// 失败
    Failed,
    /// 超时
    Timeout,
    /// 限流
    RateLimited,
    /// 取消
    Cancelled,
}

impl std::fmt::Display for ApiCallStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiCallStatus::Success => write!(f, "success"),
            ApiCallStatus::Failed => write!(f, "failed"),
            ApiCallStatus::Timeout => write!(f, "timeout"),
            ApiCallStatus::RateLimited => write!(f, "rate_limited"),
            ApiCallStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// 消息内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageContent {
    /// 文本内容
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// 工具调用
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallContent>>,
    /// 工具结果
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_result: Option<ToolResultContent>,
    /// 附加数据
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl MessageContent {
    /// 创建文本消息内容
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            text: Some(content.into()),
            tool_calls: None,
            tool_result: None,
            extra: HashMap::new(),
        }
    }

    /// 创建工具调用消息内容
    pub fn tool_calls(calls: Vec<ToolCallContent>) -> Self {
        Self {
            text: None,
            tool_calls: Some(calls),
            tool_result: None,
            extra: HashMap::new(),
        }
    }

    /// 创建工具结果消息内容
    pub fn tool_result(result: ToolResultContent) -> Self {
        Self {
            text: None,
            tool_calls: None,
            tool_result: Some(result),
            extra: HashMap::new(),
        }
    }
}

/// 工具调用内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallContent {
    /// 工具调用 ID
    pub id: String,
    /// 工具名称
    pub name: String,
    /// 工具参数
    pub arguments: serde_json::Value,
}

/// 工具结果内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultContent {
    /// 工具调用 ID
    pub tool_call_id: String,
    /// 结果内容
    pub content: String,
    /// 是否错误
    #[serde(default)]
    pub is_error: bool,
}

/// LLM 消息实体
///
/// 对应 `entity_llm_message` 表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMMessage {
    /// 消息 ID
    pub id: Uuid,
    /// 父消息 ID (用于构建对话树)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_message_id: Option<Uuid>,
    /// 会话 ID
    pub chat_session_id: Uuid,
    /// Agent ID
    pub agent_id: Uuid,
    /// 消息内容
    pub content: MessageContent,
    /// 消息角色
    pub role: MessageRole,
    /// 用户 ID
    pub user_id: Uuid,
    /// 租户 ID
    pub tenant_id: Uuid,
    /// 创建时间
    pub create_time: chrono::DateTime<chrono::Utc>,
    /// 更新时间
    pub update_time: chrono::DateTime<chrono::Utc>,
}

impl LLMMessage {
    /// 创建新消息
    pub fn new(
        chat_session_id: Uuid,
        agent_id: Uuid,
        user_id: Uuid,
        tenant_id: Uuid,
        role: MessageRole,
        content: MessageContent,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::now_v7(),
            parent_message_id: None,
            chat_session_id,
            agent_id,
            content,
            role,
            user_id,
            tenant_id,
            create_time: now,
            update_time: now,
        }
    }

    /// 设置父消息 ID
    pub fn with_parent(mut self, parent_id: Uuid) -> Self {
        self.parent_message_id = Some(parent_id);
        self
    }

    /// 设置租户 ID
    pub fn with_tenant(mut self, tenant_id: Uuid) -> Self {
        self.tenant_id = tenant_id;
        self
    }
}

/// Token 使用详情
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenDetails {
    /// 缓存 tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<i32>,
    /// 推理 tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<i32>,
    /// 附加信息
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 价格详情
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PriceDetails {
    /// 输入价格
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_price: Option<f64>,
    /// 输出价格
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_price: Option<f64>,
    /// 货币单位
    #[serde(default = "default_currency")]
    pub currency: String,
    /// 附加信息
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_currency() -> String {
    "USD".to_string()
}

/// LLM API 调用记录实体
///
/// 对应 `entity_llm_api_call` 表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMApiCall {
    /// 记录 ID
    pub id: Uuid,
    /// 会话 ID
    pub chat_session_id: Uuid,
    /// Agent ID
    pub agent_id: Uuid,
    /// 用户 ID
    pub user_id: Uuid,
    // 租户 ID
    pub tenant_id: Uuid,
    /// 请求消息 ID
    pub request_message_id: Uuid,
    /// 响应消息 ID
    pub response_message_id: Uuid,
    /// 模型名称
    pub model_name: String,
    /// 提示词 tokens
    pub prompt_tokens: i32,
    /// 提示词 tokens 详情
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<TokenDetails>,
    /// 补全 tokens
    pub completion_tokens: i32,
    /// 补全 tokens 详情
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens_details: Option<TokenDetails>,
    /// 总 tokens
    pub total_tokens: i32,
    /// 总价格
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_price: Option<f64>,
    /// 价格详情
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_details: Option<PriceDetails>,
    /// 延迟 (毫秒)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<i32>,
    /// 首 token 时间 (毫秒)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_to_first_token_ms: Option<i32>,
    /// tokens/秒
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_per_second: Option<f64>,
    /// API 响应 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_response_id: Option<String>,
    /// 调用状态
    pub status: ApiCallStatus,
    /// 错误消息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// 错误代码
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// 创建时间
    pub create_time: chrono::DateTime<chrono::Utc>,
    /// 更新时间
    pub update_time: chrono::DateTime<chrono::Utc>,
}

impl LLMApiCall {
    /// 创建成功的 API 调用记录
    #[allow(clippy::too_many_arguments)]
    pub fn success(
        chat_session_id: Uuid,
        agent_id: Uuid,
        user_id: Uuid,
        tenant_id: Uuid,
        request_message_id: Uuid,
        response_message_id: Uuid,
        model_name: impl Into<String>,
        prompt_tokens: i32,
        completion_tokens: i32,
        request_time: chrono::DateTime<chrono::Utc>,
        response_time: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        let latency_ms = (response_time - request_time).num_milliseconds() as i32;
        let tokens_per_second = if latency_ms > 0 {
            Some(completion_tokens as f64 / (latency_ms as f64 / 1000.0))
        } else {
            None
        };

        Self {
            id: Uuid::now_v7(),
            chat_session_id,
            agent_id,
            user_id,
            tenant_id,
            request_message_id,
            response_message_id,
            model_name: model_name.into(),
            prompt_tokens,
            prompt_tokens_details: None,
            completion_tokens,
            completion_tokens_details: None,
            total_tokens: prompt_tokens + completion_tokens,
            total_price: None,
            price_details: None,
            latency_ms: Some(latency_ms),
            time_to_first_token_ms: None,
            tokens_per_second,
            api_response_id: None,
            status: ApiCallStatus::Success,
            error_message: None,
            error_code: None,
            create_time: request_time,
            update_time: response_time,
        }
    }

    /// 创建失败的 API 调用记录
    #[allow(clippy::too_many_arguments)]
    pub fn failed(
        chat_session_id: Uuid,
        agent_id: Uuid,
        user_id: Uuid,
        tenant_id: Uuid,
        request_message_id: Uuid,
        model_name: impl Into<String>,
        error_message: impl Into<String>,
        error_code: Option<String>,
        request_time: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::now_v7(),
            chat_session_id,
            agent_id,
            user_id,
            tenant_id,
            request_message_id,
            response_message_id: Uuid::nil(),
            model_name: model_name.into(),
            prompt_tokens: 0,
            prompt_tokens_details: None,
            completion_tokens: 0,
            completion_tokens_details: None,
            total_tokens: 0,
            total_price: None,
            price_details: None,
            latency_ms: Some((now - request_time).num_milliseconds() as i32),
            time_to_first_token_ms: None,
            tokens_per_second: None,
            api_response_id: None,
            status: ApiCallStatus::Failed,
            error_message: Some(error_message.into()),
            error_code,
            create_time: request_time,
            update_time: now,
        }
    }

    /// 设置 API 响应 ID
    pub fn with_api_response_id(mut self, id: impl Into<String>) -> Self {
        self.api_response_id = Some(id.into());
        self
    }

    /// 设置价格信息
    pub fn with_price(mut self, total_price: f64, details: Option<PriceDetails>) -> Self {
        self.total_price = Some(total_price);
        self.price_details = details;
        self
    }

    /// 设置首 token 时间
    pub fn with_time_to_first_token(mut self, ttft_ms: i32) -> Self {
        self.time_to_first_token_ms = Some(ttft_ms);
        self
    }

    /// 设置 token 详情
    pub fn with_token_details(
        mut self,
        prompt_details: Option<TokenDetails>,
        completion_details: Option<TokenDetails>,
    ) -> Self {
        self.prompt_tokens_details = prompt_details;
        self.completion_tokens_details = completion_details;
        self
    }
}

/// 会话实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    /// 会话 ID
    pub id: Uuid,
    /// 用户 ID
    pub user_id: Uuid,
    /// Agent ID
    pub agent_id: Uuid,
    /// 租户 ID
    pub tenant_id: Uuid,
    /// 会话标题
    pub title: Option<String>,
    /// 会话元数据
    pub metadata: HashMap<String, serde_json::Value>,
    /// 创建时间
    pub create_time: chrono::DateTime<chrono::Utc>,
    /// 更新时间
    pub update_time: chrono::DateTime<chrono::Utc>,
}

impl ChatSession {
    /// 创建新会话
    pub fn new(user_id: Uuid, agent_id: Uuid) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::now_v7(),
            user_id,
            agent_id,
            tenant_id: Uuid::nil(), // 默认为 nil UUID，可以通过 with_tenant_id 设置
            title: None,
            metadata: HashMap::new(),
            create_time: now,
            update_time: now,
        }
    }

    /// 设置标题
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// 设置 ID
    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    /// 设置租户 ID
    pub fn with_tenant_id(mut self, tenant_id: Uuid) -> Self {
        self.tenant_id = tenant_id;
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// 查询过滤器
#[derive(Debug, Clone, Default)]
pub struct QueryFilter {
    /// 用户 ID
    pub user_id: Option<Uuid>,
    /// 会话 ID
    pub session_id: Option<Uuid>,
    /// Agent ID
    pub agent_id: Option<Uuid>,
    /// 开始时间
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    /// 结束时间
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    /// 状态过滤
    pub status: Option<ApiCallStatus>,
    /// 模型名称
    pub model_name: Option<String>,
    /// 分页: 偏移量
    pub offset: Option<i64>,
    /// 分页: 限制数量
    pub limit: Option<i64>,
}

impl QueryFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn user(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn session(mut self, session_id: Uuid) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn agent(mut self, agent_id: Uuid) -> Self {
        self.agent_id = Some(agent_id);
        self
    }

    pub fn time_range(
        mut self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        self.start_time = Some(start);
        self.end_time = Some(end);
        self
    }

    pub fn with_status(mut self, status: ApiCallStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn model(mut self, model_name: impl Into<String>) -> Self {
        self.model_name = Some(model_name.into());
        self
    }

    pub fn paginate(mut self, offset: i64, limit: i64) -> Self {
        self.offset = Some(offset);
        self.limit = Some(limit);
        self
    }
}

/// 统计摘要
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageStatistics {
    /// 总调用次数
    pub total_calls: i64,
    /// 成功次数
    pub success_count: i64,
    /// 失败次数
    pub failed_count: i64,
    /// 总 tokens
    pub total_tokens: i64,
    /// 总提示词 tokens
    pub total_prompt_tokens: i64,
    /// 总补全 tokens
    pub total_completion_tokens: i64,
    /// 总费用
    pub total_cost: Option<f64>,
    /// 平均延迟 (毫秒)
    pub avg_latency_ms: Option<f64>,
    /// 平均 tokens/秒
    pub avg_tokens_per_second: Option<f64>,
}

/// Provider Entity - maps to entity_provider table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub provider_name: String,
    pub provider_type: String,
    pub api_base: String,
    pub api_key: String,
    pub enabled: bool,
    pub create_time: chrono::DateTime<chrono::Utc>,
    pub update_time: chrono::DateTime<chrono::Utc>,
}

/// Agent Entity - maps to entity_agent table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub agent_code: String,
    pub agent_name: String,
    pub agent_order: i32,
    pub agent_status: bool,
    pub context_limit: Option<i32>,
    pub custom_params: Option<serde_json::Value>,
    pub max_completion_tokens: Option<i32>,
    pub model_name: String,
    pub provider_id: Uuid,
    pub response_format: Option<String>,
    pub system_prompt: String,
    pub temperature: Option<f32>,
    pub stream: Option<bool>,
    pub thinking: Option<serde_json::Value>,
    pub create_time: chrono::DateTime<chrono::Utc>,
    pub update_time: chrono::DateTime<chrono::Utc>,
}

/// Agent Configuration with Provider
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub provider: Provider,
    pub agent: Agent,
}
