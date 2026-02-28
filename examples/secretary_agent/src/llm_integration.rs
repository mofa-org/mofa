// 为了演示，我们实现一个简单的模拟LLM提供者
// For demonstration, we implement a simple mock LLM provider
use mofa_sdk::secretary::{ChatMessage, LLMProvider, ModelInfo};
use mofa_sdk::kernel::GlobalResult;
use std::sync::Arc;
use tracing::info;

// 模拟LLM提供者实现
// Mock LLM provider implementation
#[derive(Debug, Clone)]
pub struct MockLLMProvider {
    model_name: String,
}

impl MockLLMProvider {
    pub fn new() -> Self {
        Self {
            model_name: "mock-llm-1.0".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl LLMProvider for MockLLMProvider {
    fn name(&self) -> &str {
        "mock-llm"
    }

    async fn chat(&self, messages: Vec<ChatMessage>) -> GlobalResult<String> {
        // 模拟LLM响应，实际上这里应该调用真实的LLM API
        // Mock LLM response, in practice this should call a real LLM API
        info!("模拟LLM接收消息: {:?}", messages);

        // 对于演示，我们根据消息内容生成模拟响应
        // For demonstration, we generate mock responses based on message content
        let last_message = messages.last().unwrap();
        if last_message.content.contains("用户需求") {
            // 模拟需求分析的JSON响应
            // Mock requirement analysis JSON response
            return Ok(r#"{
                "title": "用户管理系统开发",
                "description": "开发一个包含注册、登录、权限管理功能的用户管理系统",
                "acceptance_criteria": ["用户可以成功注册", "用户可以登录", "权限可以正确分配", "系统稳定可靠"],
                "subtasks": [
                    {
                        "id": "subtask_1",
                        "description": "设计用户数据库结构",
                        "required_capabilities": ["database", "backend"],
                        "order": 1,
                        "depends_on": []
                    },
                    {
                        "id": "subtask_2",
                        "description": "实现用户注册功能",
                        "required_capabilities": ["backend"],
                        "order": 2,
                        "depends_on": ["subtask_1"]
                    },
                    {
                        "id": "subtask_3",
                        "description": "实现用户登录功能",
                        "required_capabilities": ["backend"],
                        "order": 3,
                        "depends_on": ["subtask_1"]
                    },
                    {
                        "id": "subtask_4",
                        "description": "实现权限管理功能",
                        "required_capabilities": ["backend"],
                        "order": 4,
                        "depends_on": ["subtask_2", "subtask_3"]
                    }
                ],
                "dependencies": [],
                "estimated_effort": null,
                "resources": []
            }"#.to_string())
        } else if last_message.content.contains("分析用户需求") {
            // 模拟需求分析的JSON响应
            // Mock requirement analysis JSON response
            return Ok(r#"{
                "core_objective": "开发一个功能完整的用户管理系统",
                "functional_requirements": ["注册", "登录", "权限管理", "密码重置"],
                "constraints": ["支持1000并发", "数据加密传输"],
                "acceptance_criteria": ["用户可以成功注册", "用户可以登录", "权限可以正确分配", "密码可以重置"],
                "dependencies": []
            }"#.to_string())
        } else {
            // 默认响应 - 对于需求澄清场景，我们也需要返回JSON格式
            // Default response - For requirement clarification scenarios, we also need to return JSON format
            return Ok(r#"{
                "title": "默认需求",
                "description": "默认需求描述",
                "acceptance_criteria": ["功能按预期工作"],
                "subtasks": [
                    {
                        "id": "subtask_default",
                        "description": "完成默认任务",
                        "required_capabilities": ["general"],
                        "order": 1,
                        "depends_on": []
                    }
                ],
                "dependencies": [],
                "estimated_effort": null,
                "resources": []
            }"#.to_string())
        }
    }

    fn model_info(&self) -> Option<ModelInfo> {
        Some(ModelInfo {
            name: self.model_name.clone(),
            version: Some("1.0".to_string()),
            context_window: Some(4096),
            max_output_tokens: Some(1024),
        })
    }
}

// 创建LLM提供者的便捷函数
// Convenience function to create LLM provider
pub fn create_llm_provider() -> Arc<dyn LLMProvider> {
    Arc::new(MockLLMProvider::new())
}

// 您也可以实现真实的LLM提供者，例如OpenAI
// You can also implement a real LLM provider, such as OpenAI
// use reqwest::Client;
// use serde_json::json;

// pub struct OpenAIProvider {
//     client: Client,
//     api_key: String,
//     model: String,
// }

// impl OpenAIProvider {
//     pub fn new(api_key: String, model: String) -> Self {
//         Self {
//             client: Client::new(),
//             api_key,
//             model,
//         }
//     }
// }

// #[async_trait::async_trait]
// impl LLMProvider for OpenAIProvider {
//     fn name(&self) -> &str {
//         "openai"
//     }

//     async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String, Box<dyn std::error::Error>> {
//         let res = self.client.post("https://api.openai.com/v1/chat/completions")
//             .bearer_auth(&self.api_key)
//             .json(&json!({
//                 "model": self.model,
//                 "messages": messages
//             }))
//             .send()
//             .await?;

//         let json: serde_json::Value = res.json().await?;
//         Ok(json["choices"][0]["message"]["content"].as_str().unwrap().to_string())
//     }
// }
