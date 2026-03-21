# Human-in-the-Loop (HITL) System

## Overview

The Human-in-the-Loop (HITL) system enables **"pause at any node for manual review"** across workflows, agents, and tool execution in MoFA.

### Key Features

- **Unified Abstraction** - Single interface across all layers  
- **Production-Ready** - Rate limiting, webhooks, multi-tenancy  
- **Type-Safe** - Strong typing with Rust best practices  
- **Scalable** - Database partitioning, pagination, async  
- **Secure** - HMAC signatures, row-level security, audit trail  
- **Extensible** - Plugin system for custom handlers  

---

## Use Case Analysis

**Primary Use Cases** (High Priority):
- **Workflow Reviews**: Pause workflow execution at nodes for approval (payments, publishing, deployments)
- **Tool Execution Reviews**: Approve tool calls before execution (database ops, file ops, API calls)

**Conditional Use Cases** (Validate First):
- **Agent Reviews**: Review agent actions/outputs (may not be needed for service integrations)
  - **When Needed**: Autonomous agents, critical decisions, regulatory compliance
  - **When NOT Needed**: Service integrations, stateless agents, background workers
  - **Status**: Deferred until validated through real-world use cases

---

## Architecture

```
┌────────────────────────────────────────────────────────────┐
│                    mofa-sdk                                │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Re-exports: hitl::* (ReviewManager, ReviewRequest)   │  │
│  └──────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────┘
                            ↓
┌────────────────────────────────────────────────────────────┐
│              mofa-foundation                               │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ NEW: hitl/                                           │  │
│  │  ├── ReviewManager (orchestration + timeouts)        │  │
│  │  ├── ReviewStore (PostgreSQL + partitioning)         │  │
│  │  ├── ReviewNotifier (multi-channel)                  │  │
│  │  ├── RateLimiter (token bucket)                      │  │
│  │  ├── WebhookDelivery (retries + HMAC)                │  │
│  │  ├── ReviewPolicyEngine                              │  │
│  │  └── handlers/ (Workflow, Tool)                      │  │
│  ├──────────────────────────────────────────────────────┤  │
│  │ workflow/     → Enhanced with ReviewManager          │  │
│  │ persistence/  → Extended for review storage          │  │
│  └──────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────┘
                            ↓
┌────────────────────────────────────────────────────────────┐
│              mofa-runtime                                  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ AgentRunner → Enhanced with ReviewManager            │  │
│  │              (conditional - validate use cases)      │  │
│  │              (backward compatible)                   │  │
│  └──────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────┘
                            ↓
┌────────────────────────────────────────────────────────────┐
│              mofa-kernel                                   │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ NEW: hitl/                                           │  │
│  │  ├── ReviewRequest, ReviewResponse (types)           │  │
│  │  ├── ReviewPolicy (trait)                            │  │
│  │  ├── ReviewContext, ReviewMetadata                   │  │
│  │  └── HitlError (with rate limiting)                  │  │
│  ├──────────────────────────────────────────────────────┤  │
│  │ agent/      → Unchanged (can use HITL)               │  │
│  │ workflow/   → Unchanged                              │  │
│  └──────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────┘
```

---

## Quick Start

### Basic Review Manager Setup

```rust
use mofa_foundation::hitl::*;
use std::sync::Arc;

// Create components
let store = Arc::new(InMemoryReviewStore::new());
let notifier = Arc::new(ReviewNotifier::default());
let policy_engine = Arc::new(ReviewPolicyEngine::default());
let rate_limiter = Some(Arc::new(RateLimiter::new(10.0, 100.0))); // 10 req/sec, max 100 tokens

// Create review manager
let manager = ReviewManager::new(
    store,
    notifier,
    policy_engine,
    rate_limiter,
    ReviewManagerConfig::default(),
);
```

### With Audit Trail

```rust
use mofa_foundation::hitl::*;
use std::sync::Arc;

let store = Arc::new(InMemoryReviewStore::new());
let audit_store = Arc::new(InMemoryAuditStore::new());
let notifier = Arc::new(ReviewNotifier::default());
let policy_engine = Arc::new(ReviewPolicyEngine::default());

let manager = ReviewManager::with_audit_store(
    store,
    notifier,
    policy_engine,
    None, // No rate limiting
    audit_store,
    ReviewManagerConfig::default(),
);
```

---

## Usage Examples

### Scenario 1: Workflow Node Review

```rust
use mofa_foundation::hitl::*;
use mofa_foundation::workflow::*;
use mofa_kernel::hitl::*;
use std::sync::Arc;

// Setup
let store = Arc::new(InMemoryReviewStore::new());
let manager = Arc::new(ReviewManager::new(...));
let handler = Arc::new(WorkflowReviewHandler::new(manager));

// Create executor with review support
let executor = WorkflowExecutor::new(ExecutorConfig::default())
    .with_review_manager(handler);

// When workflow reaches a Wait node, it will automatically:
// 1. Create a review request
// 2. Pause execution
// 3. Wait for human resolution

// Resume workflow after review
let ctx = workflow_context; // From paused workflow
executor.resume_with_human_input(&graph, ctx, waiting_node_id, WorkflowValue::Null).await?;
```

### Scenario 2: Tool Execution Review

```rust
use mofa_foundation::hitl::*;
use mofa_foundation::agent::components::tool::SimpleToolRegistry;
use mofa_kernel::hitl::*;
use std::sync::Arc;

// Setup
let store = Arc::new(InMemoryReviewStore::new());
let manager = Arc::new(ReviewManager::new(...));
let handler = Arc::new(ToolReviewHandler::new(manager));

let registry = SimpleToolRegistry::new()
    .with_review_manager(handler);

// Execute tool with review (for destructive operations)
let result = registry.execute_with_review(
    "database_delete",
    tool_input,
    &agent_context,
    "execution-123",
).await?;
```

### Scenario 3: Query Audit Trail

```rust
use mofa_foundation::hitl::*;
use mofa_kernel::hitl::AuditLogQuery;
use uuid::Uuid;

let audit_store = Arc::new(InMemoryAuditStore::new());
let manager = ReviewManager::with_audit_store(..., audit_store.clone(), ...);

// Query audit events
let query = AuditLogQuery {
    tenant_id: Some(tenant_uuid),
    start_time_ms: Some(start_timestamp),
    end_time_ms: Some(end_timestamp),
    limit: Some(100),
    ..Default::default()
};

let events = manager.query_audit_events(&query).await?;

// Get events for specific review
let review_events = manager.get_review_audit_events("review-123").await?;
```

### Scenario 4: Analytics and Metrics

```rust
use mofa_foundation::hitl::*;
use uuid::Uuid;

let audit_store = Arc::new(InMemoryAuditStore::new());
let analytics = ReviewAnalytics::new(audit_store);

// Calculate metrics
let metrics = analytics.calculate_metrics(
    Some(tenant_id),
    Some(start_time_ms),
    Some(end_time_ms),
).await?;

println!("Approval rate: {:.2}%", metrics.approval_rate * 100.0);
println!("Average review time: {:?}ms", metrics.average_review_time_ms);

// Get reviewer activity
let reviewer_metrics = analytics.get_reviewer_metrics(
    Some(tenant_id),
    Some(start_time_ms),
    Some(end_time_ms),
).await?;

for reviewer in reviewer_metrics {
    println!("{}: {} reviews resolved", reviewer.reviewer, reviewer.total_resolved);
}
```

### Scenario 5: Multi-Tenant Setup

```rust
use mofa_foundation::hitl::*;
use mofa_kernel::hitl::*;
use uuid::Uuid;

let store = Arc::new(InMemoryReviewStore::new());
let manager = Arc::new(ReviewManager::new(...));

// Create review with tenant ID
let mut review = ReviewRequest::new("exec-1", ReviewType::Approval, context);
review.metadata.tenant_id = Some(tenant_uuid);

let review_id = manager.request_review(review).await?;

// List reviews for tenant
let tenant_reviews = manager.list_pending(Some(tenant_uuid), None).await?;
```

### Scenario 6: Webhook Notifications

```rust
use mofa_foundation::hitl::*;

let webhook_config = WebhookConfig {
    url: "https://example.com/webhook".to_string(),
    secret: Some("webhook-secret".to_string()),
    timeout: Duration::from_secs(5),
    retry_count: 3,
};

let webhook_delivery = Arc::new(WebhookDelivery::new(webhook_config));

let notifier = ReviewNotifier::new(vec![
    NotificationChannel::Webhook(webhook_config),
    NotificationChannel::Log,
]);

let manager = ReviewManager::new(
    store,
    Arc::new(notifier),
    policy_engine,
    rate_limiter,
    config,
);
```

---

## API Reference

### ReviewManager

Central orchestration for review requests.

```rust
pub struct ReviewManager {
    // ...
}

impl ReviewManager {
    pub fn new(
        store: Arc<dyn ReviewStore>,
        notifier: Arc<ReviewNotifier>,
        policy_engine: Arc<ReviewPolicyEngine>,
        rate_limiter: Option<Arc<RateLimiter>>,
        config: ReviewManagerConfig,
    ) -> Self;
    
    pub fn with_audit_store(
        store: Arc<dyn ReviewStore>,
        notifier: Arc<ReviewNotifier>,
        policy_engine: Arc<ReviewPolicyEngine>,
        rate_limiter: Option<Arc<RateLimiter>>,
        audit_store: Arc<dyn AuditStore>,
        config: ReviewManagerConfig,
    ) -> Self;
    
    pub async fn request_review(
        &self,
        request: ReviewRequest,
    ) -> HitlResult<ReviewRequestId>;
    
    pub async fn get_review(
        &self,
        id: &ReviewRequestId,
    ) -> HitlResult<Option<ReviewRequest>>;
    
    pub async fn resolve_review(
        &self,
        id: &ReviewRequestId,
        response: ReviewResponse,
        resolved_by: String,
    ) -> HitlResult<()>;
    
    pub async fn wait_for_review(
        &self,
        id: &ReviewRequestId,
        timeout: Option<Duration>,
    ) -> HitlResult<ReviewResponse>;
    
    pub async fn list_pending(
        &self,
        tenant_id: Option<Uuid>,
        limit: Option<u64>,
    ) -> HitlResult<Vec<ReviewRequest>>;
    
    pub async fn query_audit_events(
        &self,
        query: &AuditLogQuery,
    ) -> HitlResult<Vec<ReviewAuditEvent>>;
    
    pub async fn get_review_audit_events(
        &self,
        review_id: &str,
    ) -> HitlResult<Vec<ReviewAuditEvent>>;
}
```

### WorkflowReviewHandler

Integration handler for workflow-level reviews.

```rust
pub struct WorkflowReviewHandler {
    manager: Arc<ReviewManager>,
}

impl WorkflowReviewHandler {
    pub fn new(manager: Arc<ReviewManager>) -> Self;
    
    pub async fn request_node_review(
        &self,
        execution_id: &str,
        node_id: &str,
        context: ReviewContext,
    ) -> HitlResult<ReviewRequestId>;
    
    pub async fn wait_for_review(
        &self,
        review_id: &ReviewRequestId,
    ) -> HitlResult<ReviewResponse>;
    
    pub async fn is_resolved(
        &self,
        review_id: &ReviewRequestId,
    ) -> HitlResult<bool>;
    
    pub async fn is_approved(
        &self,
        review_id: &ReviewRequestId,
    ) -> HitlResult<bool>;
}
```

### ToolReviewHandler

Integration handler for tool execution reviews.

```rust
pub struct ToolReviewHandler {
    manager: Arc<ReviewManager>,
}

impl ToolReviewHandler {
    pub fn new(manager: Arc<ReviewManager>) -> Self;
    
    pub async fn request_tool_call_review(
        &self,
        execution_id: &str,
        tool_name: &str,
        tool_args: serde_json::Value,
        context: ReviewContext,
    ) -> HitlResult<ReviewRequestId>;
    
    pub async fn request_tool_output_review(
        &self,
        execution_id: &str,
        tool_name: &str,
        tool_output: serde_json::Value,
        context: ReviewContext,
    ) -> HitlResult<ReviewRequestId>;
    
    pub async fn wait_for_review(
        &self,
        review_id: &ReviewRequestId,
    ) -> HitlResult<ReviewResponse>;
}
```

### ReviewAnalytics

Analytics engine for review metrics.

```rust
pub struct ReviewAnalytics {
    audit_store: Arc<dyn AuditStore>,
}

impl ReviewAnalytics {
    pub fn new(audit_store: Arc<dyn AuditStore>) -> Self;
    
    pub async fn calculate_metrics(
        &self,
        tenant_id: Option<Uuid>,
        start_time_ms: Option<u64>,
        end_time_ms: Option<u64>,
    ) -> Result<ReviewMetrics, AuditStoreError>;
    
    pub async fn get_reviewer_metrics(
        &self,
        tenant_id: Option<Uuid>,
        start_time_ms: Option<u64>,
        end_time_ms: Option<u64>,
    ) -> Result<Vec<ReviewerMetrics>, AuditStoreError>;
    
    pub async fn get_review_volume(
        &self,
        tenant_id: Option<Uuid>,
        start_time_ms: u64,
        end_time_ms: u64,
        interval_ms: u64,
    ) -> Result<Vec<TimeSeriesPoint>, AuditStoreError>;
}
```

### REST API Endpoints (http-api feature)

#### GET /api/reviews

List reviews.

**Query Parameters:**
- `execution_id` (optional): Filter by execution ID
- `tenant_id` (optional): Filter by tenant ID
- `status` (optional): Filter by status
- `limit` (optional): Maximum number of results
- `offset` (optional): Pagination offset

**Response:**
```json
{
  "reviews": [
    {
      "id": "review-123",
      "execution_id": "exec-456",
      "node_id": "node-789",
      "status": "Pending",
      "review_type": "Approval",
      "created_at": 1234567890,
      "expires_at": 1234567890,
      "resolved_at": null,
      "resolved_by": null,
      "priority": 5
    }
  ],
  "total": 1
}
```

#### GET /api/reviews/:id

Get review details.

**Response:**
```json
{
  "id": "review-123",
  "execution_id": "exec-456",
  "node_id": "node-789",
  "review_type": "Approval",
  "status": "Pending",
  "context": {...},
  "metadata": {...},
  "created_at": 1234567890,
  "expires_at": 1234567890,
  "resolved_at": null,
  "resolved_by": null,
  "response": null
}
```

#### POST /api/reviews/:id/resolve

Resolve a review.

**Request Body:**
```json
{
  "response": {
    "Approved": {
      "comment": "Looks good!"
    }
  },
  "resolved_by": "reviewer@example.com"
}
```

**Response:**
```json
{
  "message": "Review resolved successfully"
}
```

#### GET /api/reviews/:id/audit

Get audit events for a review.

**Response:**
```json
{
  "events": [
    {
      "event_id": "event-123",
      "review_id": "review-123",
      "event_type": "Created",
      "timestamp_ms": 1234567890,
      "actor": null,
      "data": {...}
    }
  ],
  "total": 1
}
```

#### GET /api/audit/events

Query audit events.

**Query Parameters:**
- `review_id` (optional): Filter by review ID
- `execution_id` (optional): Filter by execution ID
- `tenant_id` (optional): Filter by tenant ID
- `event_type` (optional): Filter by event type
- `actor` (optional): Filter by actor
- `start_time_ms` (optional): Start timestamp
- `end_time_ms` (optional): End timestamp
- `limit` (optional): Maximum results
- `offset` (optional): Pagination offset

### Error Types

```rust
pub enum FoundationHitlError {
    Store(ReviewStoreError),
    RateLimit(String),
    WebhookDelivery(String),
    Notification(String),
    PolicyEvaluation(String),
    Serialization(String),
    InvalidConfig(String),
    Audit(String),
}
```

### Configuration

#### ReviewManagerConfig

```rust
pub struct ReviewManagerConfig {
    pub default_expiration: Duration,
    pub expiration_check_interval: Duration,
    pub enable_rate_limiting: bool,
}

impl Default for ReviewManagerConfig {
    fn default() -> Self {
        Self {
            default_expiration: Duration::from_secs(3600), // 1 hour
            expiration_check_interval: Duration::from_secs(60), // 1 minute
            enable_rate_limiting: true,
        }
    }
}
```

#### RateLimiter

```rust
pub struct RateLimiter {
    // Token bucket algorithm
}

impl RateLimiter {
    pub fn new(tokens_per_second: f64, max_tokens: f64) -> Self;
    
    pub async fn check(&self, tenant_id: &str) -> Result<(), RateLimitError>;
}
```

#### WebhookConfig

```rust
pub struct WebhookConfig {
    pub url: String,
    pub secret: Option<String>,
    pub timeout: Duration,
    pub retry_count: u32,
}
```

---

## Migration Guide

### Migrating from Legacy Wait Nodes

This guide helps you migrate from the legacy `Wait` node approach to the unified HITL system.

#### Before: Legacy Wait Node

```rust
use mofa_foundation::workflow::*;

// Old approach: Wait node pauses workflow
let graph = WorkflowGraph::new("my_workflow")
    .add_node(WorkflowNode::wait("wait_node", "Wait for approval"));

// Workflow pauses at Wait node
// Manual resume required
executor.resume_with_human_input(&graph, ctx, "wait_node", input).await?;
```

#### After: Unified HITL System

```rust
use mofa_foundation::hitl::*;
use mofa_foundation::workflow::*;

// New approach: Review manager handles pauses
let store = Arc::new(InMemoryReviewStore::new());
let manager = Arc::new(ReviewManager::new(...));
let handler = Arc::new(WorkflowReviewHandler::new(manager));

let executor = WorkflowExecutor::new(config)
    .with_review_manager(handler);

// Workflow automatically creates review at Wait nodes
// Review can be resolved via API or programmatically
```

### Migration Steps

#### Step 1: Setup Review Manager

```rust
// Add to your application setup
let store = Arc::new(InMemoryReviewStore::new());
let notifier = Arc::new(ReviewNotifier::default());
let policy_engine = Arc::new(ReviewPolicyEngine::default());

let manager = ReviewManager::new(
    store,
    notifier,
    policy_engine,
    None, // Add rate limiter if needed
    ReviewManagerConfig::default(),
);
```

#### Step 2: Integrate with WorkflowExecutor

```rust
// Replace old executor creation
let handler = Arc::new(WorkflowReviewHandler::new(manager));
let executor = WorkflowExecutor::new(config)
    .with_review_manager(handler);
```

#### Step 3: Update Resume Logic

**Old approach:**
```rust
// Manual resume with input
executor.resume_with_human_input(&graph, ctx, "wait_node", input).await?;
```

**New approach:**
```rust
// Option 1: Resolve review first, then resume
manager.resolve_review(
    &review_id,
    ReviewResponse::Approved { comment: None },
    "reviewer@example.com".to_string(),
).await?;

// Then resume workflow
executor.resume_with_human_input(&graph, ctx, "wait_node", input).await?;

// Option 2: Use API endpoint (if http-api feature enabled)
// POST /api/reviews/{id}/resolve
```

#### Step 4: Handle Review IDs

The new system stores review IDs in workflow context variables:

```rust
// Get review ID from context
if let Some(review_id_value) = ctx.get_variable("review_id").await {
    if let WorkflowValue::String(review_id) = review_id_value {
        // Use review_id to query or resolve review
    }
}
```

### Compatibility

#### Backward Compatibility

The unified HITL system is **backward compatible**:
- If `ReviewManager` is not configured, workflows fall back to legacy `Wait` node behavior
- Existing workflows with `Wait` nodes continue to work
- No breaking changes to existing code

#### Gradual Migration

You can migrate incrementally:

1. **Phase 1**: Add `ReviewManager` alongside existing code
2. **Phase 2**: Update new workflows to use review system
3. **Phase 3**: Migrate existing workflows one by one
4. **Phase 4**: Remove legacy `Wait` node handling (optional)

### Feature Comparison

| Feature | Legacy Wait Node | Unified HITL |
|---------|------------------|--------------|
| Pause workflow | Yes | Yes |
| Resume workflow | Yes | Yes |
| Review tracking | No | Yes |
| Audit trail | No | Yes |
| Multi-tenancy | No | Yes |
| Rate limiting | No | Yes |
| Webhook notifications | No | Yes |
| Review policies | No | Yes |
| Analytics | No | Yes |
| REST API | No | Yes |

---

## Best Practices

1. **Always use audit trail in production** - Enable audit store for compliance and debugging
2. **Set appropriate expiration times** - Configure `default_expiration` based on your use case
3. **Use rate limiting** - Prevent abuse with `RateLimiter`
4. **Multi-tenancy** - Always set `tenant_id` in review metadata for isolation
5. **Error handling** - Review operations can fail; handle errors gracefully
6. **Webhook retries** - Configure appropriate retry counts for webhook delivery
7. **Review policies** - Use `ReviewPolicy` to automate review decisions when possible

### Error Handling

```rust
use mofa_foundation::hitl::error::FoundationHitlError;

match manager.request_review(review).await {
    Ok(review_id) => {
        // Success
    }
    Err(FoundationHitlError::RateLimit(msg)) => {
        // Rate limit exceeded - retry later
    }
    Err(FoundationHitlError::Store(err)) => {
        // Storage error - check database connection
    }
    Err(e) => {
        // Other errors
    }
}
```

### Performance Tips

1. **Use PostgreSQL store** - For production, use `PostgresReviewStore` instead of `InMemoryReviewStore`
2. **Batch queries** - Use `list_pending` with limits instead of individual queries
3. **Index audit events** - Ensure database indexes on `tenant_id`, `execution_id`, `timestamp_ms`
4. **Cleanup old reviews** - Periodically clean up expired reviews to maintain performance

---

## Examples

See `examples/hitl_workflow/` for comprehensive examples demonstrating:
- ReviewManager usage
- Workflow integration
- Tool execution integration
- Webhook notifications
- Rate limiting
- Multi-tenant isolation
- End-to-end workflows

Run examples:
```bash
cd examples
cargo run --bin hitl_workflow -- integration all
```
