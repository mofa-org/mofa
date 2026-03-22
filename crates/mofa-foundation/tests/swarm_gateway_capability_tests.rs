use async_trait::async_trait;
use futures::future::BoxFuture;
use std::sync::Arc;

use mofa_foundation::{
    CapabilityRequest, CapabilityResponse, GatewayCapability, GatewayCapabilityRegistry,
};
use mofa_foundation::swarm::{
    CoordinationPattern, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};

struct EchoCapability;

#[async_trait]
impl GatewayCapability for EchoCapability {
    fn name(&self) -> &str {
        "web_search"
    }

    async fn invoke(&self, input: CapabilityRequest) -> GlobalResult<CapabilityResponse> {
        Ok(CapabilityResponse {
            output: format!("capability: {}", input.input),
            metadata: Default::default(),
            latency_ms: 1,
        })
    }
}

#[tokio::test]
async fn swarm_executor_can_resolve_and_invoke_registered_capability() {
    let registry = Arc::new(GatewayCapabilityRegistry::new());
    registry.register(Arc::new(EchoCapability));

    let mut dag = SubtaskDAG::new("gateway-capability");
    dag.add_task(
        SwarmSubtask::new("search", "latest AI news").with_capabilities(vec!["web_search".into()]),
    );

    let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
        let registry = Arc::clone(&registry);
        Box::pin(async move {
            let response = registry
                .invoke_task(&task, format!("trace-{}", task.id))
                .await?
                .ok_or_else(|| {
                    GlobalError::Other(format!(
                        "no registered capability for task '{}'",
                        task.id
                    ))
                })?;
            Ok(response.output)
        }) as BoxFuture<'static, GlobalResult<String>>
    });

    let scheduler = CoordinationPattern::Sequential.into_scheduler();
    let summary = scheduler.execute(&mut dag, executor).await.unwrap();

    assert!(summary.is_fully_successful());
    assert_eq!(summary.successful_outputs(), vec!["capability: latest AI news"]);
}

#[tokio::test]
async fn swarm_executor_surfaces_missing_required_capability() {
    let registry = Arc::new(GatewayCapabilityRegistry::new());

    let mut dag = SubtaskDAG::new("gateway-capability-missing");
    dag.add_task(
        SwarmSubtask::new("search", "latest AI news").with_capabilities(vec!["web_search".into()]),
    );

    let executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
        let registry = Arc::clone(&registry);
        Box::pin(async move {
            let response = registry
                .invoke_task(&task, format!("trace-{}", task.id))
                .await?
                .ok_or_else(|| {
                    GlobalError::Other(format!(
                        "no registered capability for task '{}'",
                        task.id
                    ))
                })?;
            Ok(response.output)
        }) as BoxFuture<'static, GlobalResult<String>>
    });

    let scheduler = CoordinationPattern::Sequential.into_scheduler();
    let summary = scheduler.execute(&mut dag, executor).await.unwrap();

    assert_eq!(summary.failed, 1);
    assert_eq!(summary.succeeded, 0);
}
