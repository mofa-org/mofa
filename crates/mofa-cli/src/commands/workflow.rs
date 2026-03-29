//! Workflow management command implementation

use crate::CliError;
use crate::context::CliContext;
use crate::plugin_catalog::catalog_entries;
use mofa_foundation::workflow::executor::{WorkflowExecutor, ExecutorConfig};
use mofa_foundation::workflow::state::{WorkflowContext, WorkflowStatus, WorkflowValue};
use mofa_foundation::workflow::graph::WorkflowGraph;
use mofa_foundation::workflow::dsl::WorkflowDslParser;
use mofa_foundation::hitl::handlers::WorkflowReviewHandler;
use mofa_foundation::hitl::manager::{ReviewManager, ReviewManagerConfig};
use mofa_foundation::hitl::notifier::{ReviewNotifier, NotificationChannel};
use mofa_foundation::hitl::policy_engine::ReviewPolicyEngine;
use mofa_foundation::hitl::store::ReviewStore;
use mofa_kernel::hitl::{ReviewRequest, ReviewRequestId, ReviewStatus, ReviewResponse};
use async_trait::async_trait;
use std::sync::Arc;
use std::path::Path;
use colored::Colorize;
use uuid::Uuid;

/// Resume a paused workflow execution
pub async fn resume(ctx: &CliContext, id: &str, file: Option<&Path>, human_input: Option<String>) -> Result<(), CliError> {
    // 1. Find the execution record in the workflow store
    let record = ctx.workflow_store.get(id).map_err(|e| {
        CliError::Other(format!("Failed to query workflow store: {}", e))
    })?.ok_or_else(|| {
        CliError::Other(format!("Execution record '{}' not found", id))
    })?;
    
    // 2. Check if paused
    if record.status != WorkflowStatus::Paused {
        return Err(CliError::Other(format!("Execution record '{}' is not paused (status: {:?})", id, record.status)));
    }

    let waiting_node_id = record.context_snapshot.as_ref()
        .and_then(|snap| snap.last_waiting_node.clone())
        .ok_or_else(|| CliError::Other("No waiting node found in session context".to_string()))?;

    println!("{} {} Session ID: {}", "→".green(), "Resuming".bold(), id.cyan());
    println!("{} {} Node ID: {}", "→".green(), "Waiting at".bold(), waiting_node_id.yellow());

    // 3. Load the graph
    let graph = if let Some(path) = file {
        let def = WorkflowDslParser::from_file(path).await.map_err(|e| CliError::Other(format!("Failed to parse workflow file: {}", e)))?;
        // For CLI, we use an empty agent registry as builtin agents are handled by the executor settings
        let agent_registry = std::collections::HashMap::new();
        WorkflowDslParser::build_with_agents(def, &agent_registry).await
            .map_err(|e| CliError::Other(format!("Failed to build graph: {}", e)))?
    } else {
        return Err(CliError::Other("Workflow graph file required to resume. Use --file <PATH>".to_string()));
    };

    // 4. Prepare HITL handler
    let review_store = Arc::new(CliReviewStore { store: ctx.review_store.clone() });
    let notifier = Arc::new(ReviewNotifier::new(vec![NotificationChannel::Log]));
    let policy_engine = Arc::new(ReviewPolicyEngine::default());
    let manager = Arc::new(ReviewManager::new(
        review_store,
        notifier,
        policy_engine,
        None,
        ReviewManagerConfig::default(),
    ));
    let handler = Arc::new(WorkflowReviewHandler::new(manager));

    // 5. Initialize executor
    let executor = WorkflowExecutor::new(ExecutorConfig::default())
        .with_review_manager(handler);

    // 6. Restore context from snapshot
    let snapshot = record.context_snapshot.as_ref().ok_or_else(|| CliError::Other("Missing context snapshot in execution record".to_string()))?;
    let workflow_ctx = WorkflowContext::from_snapshot(snapshot.clone());
    
    // 7. Resume
    let input_value = human_input.map(WorkflowValue::String).unwrap_or(WorkflowValue::Null);
    
    match executor.resume_with_human_input(&graph, workflow_ctx, &waiting_node_id, input_value).await {
        Ok(output) => {
            println!("{} Workflow completed successfully", "✓".green());
            println!("{}: {:?}", "Final Outputs".bold(), output.outputs);
            Ok(())
        }
        Err(e) => {
            Err(CliError::Other(format!("Workflow failed during resume: {}", e)))
        }
    }
}

/// Implementation of ReviewStore that wraps the CLI's PersistedStore
struct CliReviewStore {
    store: crate::store::PersistedStore<ReviewRequest>,
}

#[async_trait]
impl ReviewStore for CliReviewStore {
    async fn create_review(&self, request: &ReviewRequest) -> Result<(), mofa_foundation::hitl::store::ReviewStoreError> {
        self.store.save(request.id.as_str(), request).map_err(|e| {
            mofa_foundation::hitl::store::ReviewStoreError::Query(e.to_string())
        })
    }

    async fn get_review(&self, id: &ReviewRequestId) -> Result<Option<ReviewRequest>, mofa_foundation::hitl::store::ReviewStoreError> {
        self.store.get(id.as_str()).map_err(|e| {
            mofa_foundation::hitl::store::ReviewStoreError::Query(e.to_string())
        })
    }

    async fn update_review(&self, id: &ReviewRequestId, status: ReviewStatus, response: Option<ReviewResponse>, resolved_by: Option<String>) -> Result<(), mofa_foundation::hitl::store::ReviewStoreError> {
        if let Some(mut review) = self.store.get(id.as_str()).map_err(|e| mofa_foundation::hitl::store::ReviewStoreError::Query(e.to_string()))? {
            review.status = status;
            review.response = response;
            review.resolved_by = resolved_by;
            review.resolved_at = Some(chrono::Utc::now());
            self.store.save(id.as_str(), &review).map_err(|e| mofa_foundation::hitl::store::ReviewStoreError::Query(e.to_string()))?;
            Ok(())
        } else {
            Err(mofa_foundation::hitl::store::ReviewStoreError::NotFound(id.as_str().to_string()))
        }
    }

    async fn list_pending(&self, _tenant_id: Option<Uuid>, _limit: Option<u64>) -> Result<Vec<ReviewRequest>, mofa_foundation::hitl::store::ReviewStoreError> {
        let reviews = self.store.list().map_err(|e| mofa_foundation::hitl::store::ReviewStoreError::Query(e.to_string()))?;
        Ok(reviews.into_iter().map(|(_, r)| r).filter(|r| matches!(r.status, ReviewStatus::Pending)).collect())
    }

    async fn list_by_execution(&self, execution_id: &str) -> Result<Vec<ReviewRequest>, mofa_foundation::hitl::store::ReviewStoreError> {
        let reviews = self.store.list().map_err(|e| mofa_foundation::hitl::store::ReviewStoreError::Query(e.to_string()))?;
        Ok(reviews.into_iter().map(|(_, r)| r).filter(|r| r.execution_id == execution_id).collect())
    }

    async fn list_expired(&self) -> Result<Vec<ReviewRequest>, mofa_foundation::hitl::store::ReviewStoreError> {
        let reviews = self.store.list().map_err(|e| mofa_foundation::hitl::store::ReviewStoreError::Query(e.to_string()))?;
        Ok(reviews.into_iter().map(|(_, r)| r).filter(|r| r.is_expired()).collect())
    }

    async fn cleanup_old_reviews(&self, before: chrono::DateTime<chrono::Utc>) -> Result<u64, mofa_foundation::hitl::store::ReviewStoreError> {
        let reviews = self.store.list().map_err(|e| mofa_foundation::hitl::store::ReviewStoreError::Query(e.to_string()))?;
        let mut count = 0;
        for (id, review) in reviews {
            if review.created_at < before {
                self.store.delete(&id).map_err(|e| mofa_foundation::hitl::store::ReviewStoreError::Query(e.to_string()))?;
                count += 1;
            }
        }
        Ok(count)
    }
}
