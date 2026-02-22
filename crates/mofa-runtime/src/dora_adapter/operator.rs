//! DoraOperator 封装
//! DoraOperator Encapsulation
//!
//! 将 MoFA 插件 system 与 dora-rs Operator API 集成
//! Integrates MoFA plugin system with dora-rs Operator API

use crate::dora_adapter::error::{DoraError, DoraResult};
use ::tracing::info;
use mofa_kernel::plugin::AgentPlugin;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Operator 配置
/// Operator Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorConfig {
    /// Operator 唯一标识
    /// Unique Operator Identifier
    pub operator_id: String,
    /// Operator 名称
    /// Operator Name
    pub name: String,
    /// 输入端口映射
    /// Input port mapping
    pub input_mapping: HashMap<String, String>,
    /// 输出端口映射
    /// Output port mapping
    pub output_mapping: HashMap<String, String>,
    /// 自定义配置
    /// Custom configuration
    pub custom_config: HashMap<String, String>,
}

impl Default for OperatorConfig {
    fn default() -> Self {
        Self {
            operator_id: uuid::Uuid::now_v7().to_string(),
            name: "default_operator".to_string(),
            input_mapping: HashMap::new(),
            output_mapping: HashMap::new(),
            custom_config: HashMap::new(),
        }
    }
}

/// Operator 输入数据封装
/// Operator input data encapsulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorInput {
    /// 输入端口 ID
    /// Input port ID
    pub input_id: String,
    /// 原始数据
    /// Raw data
    pub data: Vec<u8>,
    /// 元数据
    /// Metadata
    pub metadata: HashMap<String, String>,
}

impl OperatorInput {
    pub fn new(input_id: String, data: Vec<u8>) -> Self {
        Self {
            input_id,
            data,
            metadata: HashMap::new(),
        }
    }

    /// 反序列化数据
    /// Deserialize data
    pub fn deserialize<T: for<'de> Deserialize<'de>>(&self) -> DoraResult<T> {
        bincode::deserialize(&self.data).map_err(|e| DoraError::DeserializationError(e.to_string()))
    }

    /// 反序列化为 JSON
    /// Deserialize to JSON
    pub fn deserialize_json<T: for<'de> Deserialize<'de>>(&self) -> DoraResult<T> {
        serde_json::from_slice(&self.data)
            .map_err(|e| DoraError::DeserializationError(e.to_string()))
    }
}

/// Operator 输出数据封装
/// Operator output data encapsulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorOutput {
    /// 输出端口 ID
    /// Output port ID
    pub output_id: String,
    /// 原始数据
    /// Raw data
    pub data: Vec<u8>,
    /// 元数据
    /// Metadata
    pub metadata: HashMap<String, String>,
}

impl OperatorOutput {
    pub fn new(output_id: String, data: Vec<u8>) -> Self {
        Self {
            output_id,
            data,
            metadata: HashMap::new(),
        }
    }

    /// 从可序列化类型创建
    /// Create from serializable type
    pub fn from_serializable<T: Serialize>(output_id: String, value: &T) -> DoraResult<Self> {
        let data = bincode::serialize(value)?;
        Ok(Self::new(output_id, data))
    }

    /// 从 JSON 可序列化类型创建
    /// Create from JSON serializable type
    pub fn from_json<T: Serialize>(output_id: String, value: &T) -> DoraResult<Self> {
        let data = serde_json::to_vec(value)?;
        Ok(Self::new(output_id, data))
    }
}

/// 封装 MoFA 插件为 dora-rs Operator
/// Wrap MoFA plugin as dora-rs Operator
pub struct DoraPluginOperator {
    config: OperatorConfig,
    plugin: Arc<RwLock<Box<dyn AgentPlugin>>>,
    initialized: bool,
}

impl DoraPluginOperator {
    pub fn new(config: OperatorConfig, plugin: Box<dyn AgentPlugin>) -> Self {
        Self {
            config,
            plugin: Arc::new(RwLock::new(plugin)),
            initialized: false,
        }
    }

    /// 获取配置
    /// Get configuration
    pub fn config(&self) -> &OperatorConfig {
        &self.config
    }

    /// 初始化 Operator
    /// Initialize Operator
    pub async fn init(&mut self) -> DoraResult<()> {
        if self.initialized {
            return Ok(());
        }

        let mut plugin = self.plugin.write().await;
        plugin
            .init_plugin()
            .await
            .map_err(|e| DoraError::OperatorError(e.to_string()))?;
        self.initialized = true;
        info!("DoraPluginOperator {} initialized", self.config.operator_id);
        Ok(())
    }

    /// 处理输入数据
    /// Process input data
    pub async fn on_input(&self, input: OperatorInput) -> DoraResult<Vec<OperatorOutput>> {
        if !self.initialized {
            return Err(DoraError::OperatorError(
                "Operator not initialized".to_string(),
            ));
        }

        // 将输入数据转换为字符串（假设插件期望字符串输入）
        // Convert input data to string (assuming plugin expects string input)
        let input_str = String::from_utf8(input.data.clone())
            .unwrap_or_else(|_| format!("binary_data_{}", input.data.len()));

        // 调用插件执行
        // Invoke plugin execution
        let mut plugin = self.plugin.write().await;
        let result = plugin
            .execute(input_str)
            .await
            .map_err(|e| DoraError::OperatorError(e.to_string()))?;

        // 构建输出
        // Build output
        let output = OperatorOutput::new("default_output".to_string(), result.into_bytes());
        Ok(vec![output])
    }

    /// 处理批量输入
    /// Process batch inputs
    pub async fn on_inputs(&self, inputs: Vec<OperatorInput>) -> DoraResult<Vec<OperatorOutput>> {
        let mut outputs = Vec::new();
        for input in inputs {
            let mut output = self.on_input(input).await?;
            outputs.append(&mut output);
        }
        Ok(outputs)
    }
}

/// MoFA Operator trait - 扩展 dora-rs DoraOperator
/// MoFA Operator trait - extends dora-rs DoraOperator
#[async_trait::async_trait]
pub trait MoFAOperator: Send + Sync {
    /// 获取 Operator ID
    /// Get Operator ID
    fn operator_id(&self) -> &str;

    /// 初始化
    /// Initialize
    async fn init_operator(&mut self) -> DoraResult<()>;

    /// 处理输入
    /// Process input
    async fn process(&mut self, input: OperatorInput) -> DoraResult<Vec<OperatorOutput>>;

    /// 清理资源
    /// Cleanup resources
    async fn cleanup(&mut self) -> DoraResult<()>;
}

/// 为 AgentPlugin 实现 MoFAOperator
/// Implement MoFAOperator for AgentPlugin
pub struct PluginOperatorAdapter {
    plugin: Box<dyn AgentPlugin>,
    operator_id: String,
}

impl PluginOperatorAdapter {
    pub fn new(operator_id: String, plugin: Box<dyn AgentPlugin>) -> Self {
        Self {
            plugin,
            operator_id,
        }
    }
}

#[async_trait::async_trait]
impl MoFAOperator for PluginOperatorAdapter {
    fn operator_id(&self) -> &str {
        &self.operator_id
    }

    async fn init_operator(&mut self) -> DoraResult<()> {
        self.plugin
            .init_plugin()
            .await
            .map_err(|e| DoraError::OperatorError(e.to_string()))
    }

    async fn process(&mut self, input: OperatorInput) -> DoraResult<Vec<OperatorOutput>> {
        let input_str =
            String::from_utf8(input.data).unwrap_or_else(|_| "invalid_utf8".to_string());

        let result = self
            .plugin
            .execute(input_str)
            .await
            .map_err(|e| DoraError::OperatorError(e.to_string()))?;

        let output = OperatorOutput::new("output".to_string(), result.into_bytes());
        Ok(vec![output])
    }

    async fn cleanup(&mut self) -> DoraResult<()> {
        Ok(())
    }
}

/// Operator 链 - 支持多个 Operator 串联执行
/// Operator Chain - Supports sequential execution of multiple Operators
pub struct OperatorChain {
    operators: Vec<Box<dyn MoFAOperator>>,
}

impl OperatorChain {
    pub fn new() -> Self {
        Self {
            operators: Vec::new(),
        }
    }

    /// 添加 Operator 到链
    /// Add Operator to chain
    pub fn add_operator(&mut self, operator: Box<dyn MoFAOperator>) {
        self.operators.push(operator);
    }

    /// 初始化所有 Operator
    /// Initialize all Operators
    pub async fn init_all(&mut self) -> DoraResult<()> {
        for op in &mut self.operators {
            op.init_operator().await?;
        }
        Ok(())
    }

    /// 链式执行
    /// Chained execution
    pub async fn process(&mut self, input: OperatorInput) -> DoraResult<Vec<OperatorOutput>> {
        if self.operators.is_empty() {
            return Ok(vec![]);
        }

        let mut current_outputs = vec![OperatorOutput::new(
            input.input_id.clone(),
            input.data.clone(),
        )];

        for op in &mut self.operators {
            let mut next_outputs = Vec::new();
            for output in current_outputs {
                let input = OperatorInput::new(output.output_id, output.data);
                let mut results = op.process(input).await?;
                next_outputs.append(&mut results);
            }
            current_outputs = next_outputs;
        }

        Ok(current_outputs)
    }
}

impl Default for OperatorChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{MoFAOperator, OperatorChain, OperatorInput, PluginOperatorAdapter};
    use mofa_plugins::LLMPlugin;

    #[tokio::test]
    async fn test_plugin_operator_adapter() {
        let plugin = Box::new(LLMPlugin::new("test_llm"));
        let mut adapter = PluginOperatorAdapter::new("test_op".to_string(), plugin);

        adapter.init_operator().await.unwrap();

        let input = OperatorInput::new("input".to_string(), b"Hello".to_vec());
        let outputs = adapter.process(input).await.unwrap();

        assert!(!outputs.is_empty());
    }

    #[tokio::test]
    async fn test_operator_chain() {
        let mut chain = OperatorChain::new();

        let plugin1 = Box::new(LLMPlugin::new("llm1"));
        let adapter1 = PluginOperatorAdapter::new("op1".to_string(), plugin1);
        chain.add_operator(Box::new(adapter1));

        chain.init_all().await.unwrap();

        let input = OperatorInput::new("input".to_string(), b"Test".to_vec());
        let outputs = chain.process(input).await.unwrap();

        assert!(!outputs.is_empty());
    }
}
