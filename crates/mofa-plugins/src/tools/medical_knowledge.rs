use super::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// 疾病诊断标准
/// Disease diagnosis criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiseaseDiagnosis {
    /// 疾病名称
    /// Disease name
    pub disease_name: String,
    /// 诊断标准
    /// Diagnosis criteria
    pub criteria: Vec<String>,
    /// 最新更新日期
    /// Latest update date
    pub update_date: String,
    /// 来源
    /// Source
    pub source: String,
}

/// 治疗方案
/// Treatment plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreatmentPlan {
    /// 疾病名称
    /// Disease name
    pub disease_name: String,
    /// 治疗方案
    /// Treatment plan details
    pub plan: Vec<String>,
    /// 最新更新日期
    /// Latest update date
    pub update_date: String,
    /// 来源
    /// Source
    pub source: String,
}

/// 医疗知识存储
/// Medical knowledge storage
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MedicalKnowledge {
    /// 疾病诊断标准
    /// Disease diagnosis criteria
    pub diagnoses: Vec<DiseaseDiagnosis>,
    /// 治疗方案
    /// Treatment plans
    pub treatments: Vec<TreatmentPlan>,
}

/// 医疗知识动态注入工具
/// Medical knowledge dynamic injection tool
pub struct MedicalKnowledgeTool {
    definition: ToolDefinition,
    knowledge: Arc<RwLock<MedicalKnowledge>>,
    knowledge_path: Arc<RwLock<PathBuf>>,
}

impl Default for MedicalKnowledgeTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MedicalKnowledgeTool {
    pub fn new() -> Self {
        let tool_def = ToolDefinition {
            name: "medical_knowledge".to_string(),
            description: "Medical diagnosis knowledge management: dynamically injects and updates the latest disease diagnosis standards and treatment plans at runtime.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["inject_knowledge", "query_diagnosis", "query_treatment", "refresh_knowledge"],
                        "description": "Action to perform on medical knowledge"
                    },
                    "knowledge": {
                        "type": "object",
                        "description": "Medical knowledge data to inject (required for inject_knowledge)"
                    },
                    "disease": {
                        "type": "string",
                        "description": "Disease name (required for query_diagnosis, query_treatment)"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Path to JSON file containing medical knowledge (for inject_knowledge and refresh_knowledge)"
                    }
                },
                "required": ["action"]
            }),
            requires_confirmation: false,
        };

        Self {
            definition: tool_def,
            knowledge: Arc::new(RwLock::new(MedicalKnowledge::default())),
            knowledge_path: Arc::new(RwLock::new(PathBuf::from("medical_knowledge.json"))),
        }
    }

    /// 从JSON文件加载医疗知识
    /// Load medical knowledge from a JSON file
    async fn load_knowledge_from_file(&self, file_path: &str) -> PluginResult<MedicalKnowledge> {
        let content = fs::read_to_string(file_path)?;
        let knowledge: MedicalKnowledge = serde_json::from_str(&content)?;
        Ok(knowledge)
    }

    /// 保存医疗知识到文件
    /// Save medical knowledge to a file
    async fn save_knowledge_to_file(&self, knowledge: &MedicalKnowledge) -> PluginResult<()> {
        let content = serde_json::to_string_pretty(knowledge)?;
        let path = self.knowledge_path.read().unwrap();
        fs::write(&*path, content)?; // 解引用RwLockReadGuard
        // Dereference RwLockReadGuard
        Ok(())
    }
}

#[async_trait::async_trait]
impl ToolExecutor for MedicalKnowledgeTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, arguments: serde_json::Value) -> PluginResult<serde_json::Value> {
        let action = arguments["action"]
            .as_str()
            .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Action is required".to_string()))?;

        match action {
            // 注入知识（支持JSON数据或文件路径）
            // Inject knowledge (supports JSON data or file path)
            "inject_knowledge" => {
                let new_knowledge = if let Some(knowledge_json) = arguments.get("knowledge") {
                    // 从JSON数据注入
                    // Inject from JSON data
                    serde_json::from_value(knowledge_json.clone())?
                } else if let Some(file_path) = arguments["file_path"].as_str() {
                    // 从文件注入
                    // Inject from file
                    self.load_knowledge_from_file(file_path).await?
                } else {
                    return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(
                        "Either knowledge JSON or file_path must be provided for inject_knowledge".into(),
                    ));
                };

                // 获取统计信息
                // Get statistical information
                let diagnoses_count = new_knowledge.diagnoses.len();
                let treatments_count = new_knowledge.treatments.len();

                // 保存到文件
                // Save to file
                self.save_knowledge_to_file(&new_knowledge).await?;

                // 更新内存中的知识
                // Update knowledge in memory
                let mut knowledge = self.knowledge.write().unwrap();
                knowledge.diagnoses = new_knowledge.diagnoses;
                knowledge.treatments = new_knowledge.treatments;

                Ok(json!({
                    "success": true,
                    "message": format!("Injected medical knowledge successfully. {} diagnoses and {} treatments loaded.", diagnoses_count, treatments_count)
                }))
            }

            // 查询诊断标准
            // Query diagnosis criteria
            "query_diagnosis" => {
                let disease = arguments["disease"].as_str().ok_or_else(|| {
                    mofa_kernel::plugin::PluginError::ExecutionFailed("Disease name is required for query_diagnosis".to_string())
                })?;

                let knowledge = self.knowledge.read().unwrap();

                if let Some(diagnosis) = knowledge
                    .diagnoses
                    .iter()
                    .find(|d| d.disease_name.to_lowercase() == disease.to_lowercase())
                {
                    Ok(serde_json::to_value(diagnosis)?)
                } else {
                    Ok(json!({
                        "success": false,
                        "message": format!("No diagnosis information found for disease: {}", disease)
                    }))
                }
            }

            // 查询治疗方案
            // Query treatment plan
            "query_treatment" => {
                let disease = arguments["disease"].as_str().ok_or_else(|| {
                    mofa_kernel::plugin::PluginError::ExecutionFailed("Disease name is required for query_treatment".to_string())
                })?;

                let knowledge = self.knowledge.read().unwrap();

                if let Some(treatment) = knowledge
                    .treatments
                    .iter()
                    .find(|t| t.disease_name.to_lowercase() == disease.to_lowercase())
                {
                    Ok(serde_json::to_value(treatment)?)
                } else {
                    Ok(json!({
                        "success": false,
                        "message": format!("No treatment information found for disease: {}", disease)
                    }))
                }
            }

            // 刷新知识（从文件重新加载）
            // Refresh knowledge (reload from file)
            "refresh_knowledge" => {
                // 使用当前知识文件路径
                // Use current knowledge file path
                let file_path = if let Some(path) = arguments["file_path"].as_str() {
                    path.to_string()
                } else {
                    // 使用当前知识文件路径
                    // Use current knowledge file path
                    let path = self.knowledge_path.read().unwrap();
                    path.to_str()
                        .unwrap_or("medical_knowledge.json")
                        .to_string()
                };

                let new_knowledge = self.load_knowledge_from_file(&file_path).await?;

                // 获取统计信息
                // Get statistical information
                let diagnoses_count = new_knowledge.diagnoses.len();
                let treatments_count = new_knowledge.treatments.len();

                // 更新内存中的知识
                // Update knowledge in memory
                let mut knowledge = self.knowledge.write().unwrap();
                knowledge.diagnoses = new_knowledge.diagnoses;
                knowledge.treatments = new_knowledge.treatments;

                Ok(json!({
                    "success": true,
                    "message": format!("Refreshed medical knowledge successfully from {}. {} diagnoses and {} treatments loaded.", file_path, diagnoses_count, treatments_count)
                }))
            }

            _ => Err(mofa_kernel::plugin::PluginError::ExecutionFailed(format!("Unsupported action: {}", action))),
        }
    }
}
