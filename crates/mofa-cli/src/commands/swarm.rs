use crate::error::{CliError, CliResult, IntoCliReport as _};
use error_stack::ResultExt as _;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    CapabilityExecutionPolicy, CoordinationPattern, DependencyKind, SubtaskDAG, SubtaskExecutorFn,
    SwarmScheduler, SwarmSubtask,
};
use mofa_foundation::{GatewayCapabilityRegistry, built_in_capability_registry_from_env};
use mofa_kernel::agent::types::error::GlobalResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize)]
struct SwarmRunConfig {
    name: String,
    task: String,
    #[serde(default)]
    pattern: CoordinationPattern,
    dag: SwarmDagFile,
}

#[derive(Debug, Clone, Deserialize)]
struct SwarmDagFile {
    tasks: Vec<SwarmTaskFile>,
    #[serde(default)]
    dependencies: Vec<SwarmDependencyFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct SwarmTaskFile {
    id: String,
    description: String,
    #[serde(default, alias = "required_capabilities")]
    capabilities: Vec<String>,
    #[serde(default)]
    capability_params: HashMap<String, Value>,
    #[serde(default)]
    capability_policy: CapabilityExecutionPolicy,
    #[serde(default)]
    deps: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SwarmDependencyFile {
    from: String,
    to: String,
    #[serde(default)]
    kind: Option<DependencyKind>,
}

#[derive(Debug, Serialize)]
struct SwarmRunSummary {
    name: String,
    task: String,
    pattern: CoordinationPattern,
    total_tasks: usize,
    succeeded: usize,
    failed: usize,
    skipped: usize,
    outputs: Vec<String>,
}

pub async fn run(file: &Path, json: bool) -> CliResult<()> {
    let raw = std::fs::read_to_string(file)
        .map_err(CliError::from)
        .into_report()
        .attach_with(|| format!("reading swarm config '{}'", file.display()))?;

    let config: SwarmRunConfig = serde_yaml::from_str(&raw)
        .map_err(CliError::from)
        .into_report()
        .attach_with(|| format!("parsing swarm YAML '{}'", file.display()))?;

    let registry = built_in_capability_registry_from_env();
    let summary = run_config_with_registry(config, registry)
        .await
        .attach_with(|| format!("executing swarm '{}'", file.display()))?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&summary)
                .map_err(CliError::from)
                .into_report()
                .attach("serializing swarm summary to JSON")?
        );
    } else {
        println!("Swarm: {}", summary.name);
        println!("Task: {}", summary.task);
        println!("Pattern: {:?}", summary.pattern);
        println!(
            "Tasks: total={} succeeded={} failed={} skipped={}",
            summary.total_tasks, summary.succeeded, summary.failed, summary.skipped
        );
        for output in &summary.outputs {
            println!("output: {output}");
        }
    }

    Ok(())
}

async fn run_config_with_registry(
    config: SwarmRunConfig,
    registry: Arc<GatewayCapabilityRegistry>,
) -> CliResult<SwarmRunSummary> {
    let SwarmRunConfig {
        name,
        task,
        pattern,
        dag,
    } = config;

    let mut dag = build_dag(&name, dag)
        .map_err(|e| CliError::Other(e.to_string()))
        .into_report()
        .attach("building SubtaskDAG from YAML")?;

    let executor: SubtaskExecutorFn = Arc::new(move |_idx, task: SwarmSubtask| {
        let registry = Arc::clone(&registry);
        Box::pin(async move { execute_task(task, registry).await })
            as BoxFuture<'static, GlobalResult<String>>
    });

    let scheduler = pattern.clone().into_scheduler();
    let summary = scheduler
        .execute(&mut dag, executor)
        .await
        .map_err(|e| CliError::Other(e.to_string()))
        .into_report()
        .attach("running swarm scheduler")?;

    Ok(SwarmRunSummary {
        name,
        task,
        pattern,
        total_tasks: summary.total_tasks,
        succeeded: summary.succeeded,
        failed: summary.failed,
        skipped: summary.skipped,
        outputs: summary
            .successful_outputs()
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
    })
}

async fn execute_task(
    task: SwarmSubtask,
    registry: Arc<GatewayCapabilityRegistry>,
) -> GlobalResult<String> {
    match registry
        .invoke_task(&task, format!("swarm-{}", task.id))
        .await?
    {
        Some(response) => Ok(response.output),
        None => Ok(format!("local: {}", task.description)),
    }
}

fn build_dag(name: &str, dag_file: SwarmDagFile) -> GlobalResult<SubtaskDAG> {
    let tasks = dag_file
        .tasks
        .iter()
        .map(|task| {
            SwarmSubtask::new(task.id.clone(), task.description.clone())
            .with_capabilities(task.capabilities.clone())
            .with_capability_params(task.capability_params.clone())
            .with_capability_policy(task.capability_policy)
        })
        .collect();
    let dependencies = dag_file
        .tasks
        .iter()
        .flat_map(|task| {
            task.deps.iter().map(|dep_id| {
                (
                    dep_id.clone(),
                    task.id.clone(),
                    DependencyKind::Sequential,
                )
            })
        })
        .chain(dag_file.dependencies.iter().map(|dependency| {
            (
                dependency.from.clone(),
                dependency.to.clone(),
                dependency.kind.clone().unwrap_or(DependencyKind::Sequential),
            )
        }))
        .collect();

    SubtaskDAG::from_subtasks(name, tasks, dependencies)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use mofa_foundation::{CapabilityRequest, CapabilityResponse, GatewayCapability};

    struct EchoCapability;

    #[async_trait]
    impl GatewayCapability for EchoCapability {
        fn name(&self) -> &str {
            "web_search"
        }

        async fn invoke(&self, input: CapabilityRequest) -> GlobalResult<CapabilityResponse> {
            Ok(CapabilityResponse {
                output: format!(
                    "capability:{}:{}",
                    input.input,
                    input.params["query"]
                ),
                metadata: HashMap::new(),
                latency_ms: 1,
            })
        }
    }

    #[tokio::test]
    async fn run_uses_capability_registry_and_task_params() {
        let config: SwarmRunConfig = serde_yaml::from_str(
            r#"
name: gateway-demo
task: "Search for AI news"
pattern: sequential
dag:
  tasks:
    - id: search
      description: "latest AI news"
      capabilities: [web_search]
      capability_policy: require_capability
      capability_params:
        query: "2025 ai news"
"#,
        )
        .unwrap();

        let registry = Arc::new(GatewayCapabilityRegistry::new());
        registry.register(Arc::new(EchoCapability));

        let summary = run_config_with_registry(config, registry).await.unwrap();
        assert_eq!(summary.succeeded, 1);
        assert_eq!(
            summary.outputs,
            vec!["capability:latest AI news:\"2025 ai news\"".to_string()]
        );
    }

    #[tokio::test]
    async fn run_falls_back_to_local_when_policy_allows_it() {
        let config: SwarmRunConfig = serde_yaml::from_str(
            r#"
name: local-demo
task: "Write summary"
pattern: sequential
dag:
  tasks:
    - id: write
      description: "Write a summary"
"#,
        )
        .unwrap();

        let registry = Arc::new(GatewayCapabilityRegistry::new());
        let summary = run_config_with_registry(config, registry).await.unwrap();
        assert_eq!(summary.outputs, vec!["local: Write a summary".to_string()]);
    }
}
