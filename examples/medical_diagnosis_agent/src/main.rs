use mofa_plugins::{
    tools::{
        medical_knowledge::MedicalKnowledgeTool,
        ToolPlugin,
    },
    ToolCall, ToolResult,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 创建并配置工具插件
    let mut tool_plugin = ToolPlugin::new("medical_tool_plugin");

    // 注册医疗知识工具
    tool_plugin.register_tool(MedicalKnowledgeTool::new());

    // 初始化插件
    tool_plugin.init_plugin().await?;
    tool_plugin.start().await?;

    println!("医疗诊断Agent领域知识动态注入演示");
    println!("====================================");

    // 1. 从JSON文件注入知识
    println!("\n1. 从JSON文件注入医疗知识...");
    let inject_call = ToolCall {
        tool_name: "medical_knowledge".to_string(),
        arguments: serde_json::json!({
            "action": "inject_knowledge",
            "file_path": "../test_medical_knowledge.json"
        }),
    };

    let result: ToolResult = serde_json::from_str(&tool_plugin.execute(serde_json::to_string(&inject_call)?.to_string()).await?)?;
    println!("注入结果: {}", result.result);

    // 2. 查询糖尿病的诊断标准
    println!("\n2. 查询糖尿病的诊断标准...");
    let query_diabetes = ToolCall {
        tool_name: "medical_knowledge".to_string(),
        arguments: serde_json::json!({
            "action": "query_diagnosis",
            "disease": "糖尿病"
        }),
    };

    let result: ToolResult = serde_json::from_str(&tool_plugin.execute(serde_json::to_string(&query_diabetes)?.to_string()).await?)?;
    println!("诊断标准: {:?}", result.result);

    // 3. 查询高血压的治疗方案
    println!("\n3. 查询高血压的治疗方案...");
    let query_hypertension = ToolCall {
        tool_name: "medical_knowledge".to_string(),
        arguments: serde_json::json!({
            "action": "query_treatment",
            "disease": "高血压"
        }),
    };

    let result: ToolResult = serde_json::from_str(&tool_plugin.execute(serde_json::to_string(&query_hypertension)?.to_string()).await?)?;
    println!("治疗方案: {:?}", result.result);

    // 4. 演示动态更新知识
    println!("\n4. 演示动态更新知识（模拟从新的研究论文更新）...");
    let update_knowledge = ToolCall {
        tool_name: "medical_knowledge".to_string(),
        arguments: serde_json::json!({
            "action": "inject_knowledge",
            "knowledge": {
                "diagnoses": [
                    {
                        "disease_name": "糖尿病",
                        "criteria": [
                            "空腹血糖 ≥7.0 mmol/L",
                            "餐后2小时血糖 ≥11.1 mmol/L",
                            "糖化血红蛋白 ≥6.5%"
                        ],
                        "update_date": "2025-12-31",
                        "source": "最新医学研究论文（2025年12月）"
                    }
                ],
                "treatments": []
            }
        }),
    };

    let result: ToolResult = serde_json::from_str(&tool_plugin.execute(serde_json::to_string(&update_knowledge)?.to_string()).await?)?;
    println!("更新结果: {}", result.result);

    // 5. 查询更新后的糖尿病诊断标准
    println!("\n5. 查询更新后的糖尿病诊断标准...");
    let query_diabetes_updated = ToolCall {
        tool_name: "medical_knowledge".to_string(),
        arguments: serde_json::json!({
            "action": "query_diagnosis",
            "disease": "糖尿病"
        }),
    };

    let result: ToolResult = serde_json::from_str(&tool_plugin.execute(serde_json::to_string(&query_diabetes_updated)?.to_string()).await?)?;
    println!("更新后的诊断标准: {:?}", result.result);

    // 停止插件
    tool_plugin.stop().await?;

    Ok(())
}
