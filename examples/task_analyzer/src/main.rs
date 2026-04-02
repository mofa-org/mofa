use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, Level};

use mofa_sdk::llm::{openai_from_env, LLMClient};

// ============================================================================
// Core Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Complexity {
    Low,
    Medium,
    High,
}

/// A sub-task decomposed from the original task
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubTask {
    id: String,
    name: String,
    description: String,
    dependencies: Vec<String>, // IDs of tasks this depends on
    estimated_complexity: Complexity,
    required_capabilities: Vec<String>,
}

/// DAG representation of the execution plan
#[derive(Debug)]
struct TaskDAG {
    tasks: HashMap<String, SubTask>,
    // task_id -> list of dependent tasks (tasks that depend on this one)
    adjacency: HashMap<String, Vec<String>>,
    // task_id -> list of dependencies (tasks this one depends on)
    in_degree: HashMap<String, usize>,
}

// ============================================================================
// Components
// ============================================================================

/// 1. TaskDecomposer - Uses LLM to break down a task
struct TaskDecomposer {
    llm_client: Arc<LLMClient>,
}

impl TaskDecomposer {
    fn new(llm_client: Arc<LLMClient>) -> Self {
        Self { llm_client }
    }

    /// Ask the LLM to decompose a task description into structured sub-tasks.
    async fn decompose(&self, task_description: &str) -> Result<Vec<SubTask>, Box<dyn std::error::Error>> {
        let prompt = format!(
            r#"You are a seasoned software architect and project manager.
Your job is to break down a large high-level task into a set of 4-8 granular, actionable sub-tasks.

For each sub-task, provide:
- id: a unique short ascii snake_case string (e.g. "db_schema")
- name: a human-readable name 
- description: what needs to be done
- dependencies: list of parent sub-task IDs that MUST be completed before this one can start. If none, leave empty.
- estimated_complexity: "Low", "Medium", or "High"
- required_capabilities: list of skills needed (e.g. ["database"], ["frontend", "react"])

Ensure that there are NO circular dependencies.

Return ONLY a valid JSON array of these objects. 
Do not include markdown codeblocks like ```json ... ```, just output the raw JSON array.

The task to decompose is:
"{}"
"#,
            task_description
        );

        info!("Sending decomposition prompt to LLM...");

        let response = self
            .llm_client
            .chat()
            .system("You are an AI planner. Output raw JSON only without markdown formatting.")
            .user(&prompt)
            .temperature(0.2)
            .send()
            .await?;

        let response_text = response.content().unwrap_or("[]").trim();
        
        // Strip out ```json if the model ignores our instruction
        let cleaned_json = if response_text.starts_with("```json") {
            response_text
                .trim_start_matches("```json")
                .trim_end_matches("```")
                .trim()
        } else if response_text.starts_with("```") {
            response_text
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim()
        } else {
            response_text
        };

        info!("Parsing JSON response...");
        let subtasks: Vec<SubTask> = serde_json::from_str(cleaned_json)?;

        Ok(subtasks)
    }
}

// 2. TaskDAG - Construction and execution order
impl TaskDAG {
    /// Build a DAG from a list of sub-tasks
    fn from_subtasks(subtasks: Vec<SubTask>) -> Result<Self, String> {
        let mut tasks = HashMap::new();
        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        // Initialize maps validation
        for task in &subtasks {
            tasks.insert(task.id.clone(), task.clone());
            adjacency.insert(task.id.clone(), Vec::new());
            in_degree.insert(task.id.clone(), 0);
        }

        // Build edges
        for task in &subtasks {
            in_degree.insert(task.id.clone(), task.dependencies.len());
            
            for dep in &task.dependencies {
                if !tasks.contains_key(dep) {
                    return Err(format!("Task {} depends on unknown task {}", task.id, dep));
                }
                
                // Add this task to the dependent's adjacency list
                if let Some(adj_list) = adjacency.get_mut(dep) {
                    adj_list.push(task.id.clone());
                }
            }
        }

        Ok(Self {
            tasks,
            adjacency,
            in_degree,
        })
    }

    /// Output a topological sort layered by iteration (parallel execution groups)
    /// Returns a list of layers, where each layer is a list of independent tasks
    fn execution_layers(&self) -> Result<Vec<Vec<&SubTask>>, String> {
        let mut in_degree = self.in_degree.clone();
        let mut queue = Vec::new();
        let mut layers = Vec::new();
        
        // Find tasks with 0 in-degree (no dependencies)
        for (id, &deg) in &in_degree {
            if deg == 0 {
                queue.push(id.clone());
            }
        }

        let mut processed_count = 0;

        while !queue.is_empty() {
            let mut next_queue = Vec::new();
            let mut current_layer = Vec::new();
            
            for id in queue {
                current_layer.push(self.tasks.get(&id).unwrap());
                processed_count += 1;
                
                // Decrease in-degree for dependents
                if let Some(adj_list) = self.adjacency.get(&id) {
                    for neighbor in adj_list {
                        if let Some(deg) = in_degree.get_mut(neighbor) {
                            *deg -= 1;
                            if *deg == 0 {
                                next_queue.push(neighbor.clone());
                            }
                        }
                    }
                }
            }
            layers.push(current_layer);
            queue = next_queue;
        }

        if processed_count != self.tasks.len() {
            return Err("Cycle detected in task dependencies!".into());
        }

        Ok(layers)
    }

    fn print_execution_plan(&self) {
        info!("📊 Execution DAG Plan:");
        
        match self.execution_layers() {
            Ok(layers) => {
                for (i, layer) in layers.iter().enumerate() {
                    let task_names: Vec<String> = layer.iter().map(|t| t.id.clone()).collect();
                    let parallel_note = if layer.len() > 1 { " (Parallel Context)" } else { "" };
                    info!("  Layer {}{}: [{}]", i, parallel_note, task_names.join(", "));
                    
                    for task in layer {
                        info!("    - {}: {} ({:?})", task.id, task.name, task.estimated_complexity);
                    }
                }
            }
            Err(e) => tracing::error!("Failed to generate execution plan: {}", e)
        }
    }
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("==========================================================");
    info!(" MoFA Task Analyzer - LLM Decomposition & DAG Generation  ");
    info!("==========================================================");

    // 1. Initialize LLM Provider 
    // Uses OpenAI API key from the environment
    let provider = match openai_from_env() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Failed to create OpenAI provider. Did you set OPENAI_API_KEY? Error: {}", e);
            return Ok(());
        }
    };
    
    let llm_client = Arc::new(LLMClient::new(Arc::new(provider)));
    let decomposer = TaskDecomposer::new(llm_client);

    // 2. Define High-Level Task
    let target_task = "Build a user authentication system with OAuth2, JWT tokens, and role-based access control";
    info!("\nInput Task: \"{}\"\n", target_task);

    // 3. Decompose via LLM
    info!("🔍 Decomposing task using LLM (this may take a few seconds)...");
    let subtasks = decomposer.decompose(target_task).await?;
    info!("✅ Decomposed into {} sub-tasks\n", subtasks.len());

    // 4. Validate and construct DAG
    match TaskDAG::from_subtasks(subtasks) {
        Ok(dag) => {
            // 5. Output Execution Plan
            dag.print_execution_plan();
            
            info!("\n🔗 Dependency Map:");
            for (node, adj_list) in &dag.adjacency {
                if !adj_list.is_empty() {
                    info!("  {} -> {}", node, adj_list.join(", "));
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to build task DAG: {}", e);
        }
    }

    Ok(())
}
