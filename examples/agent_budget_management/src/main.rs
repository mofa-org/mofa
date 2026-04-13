use mofa_sdk::prelude::*;
use mofa_sdk::foundation::cost::BudgetEnforcer;
use mofa_sdk::kernel::budget::BudgetConfig;
use std::sync::Arc;
use tracing::{info, error};

struct BudgetAgent {
    caps: AgentCapabilities,
    state: AgentState,
    enforcer: Arc<BudgetEnforcer>,
}

impl BudgetAgent {
    fn new(enforcer: Arc<BudgetEnforcer>) -> Self {
        Self {
            caps: AgentCapabilitiesBuilder::new().tag("budget").build(),
            state: AgentState::Created,
            enforcer,
        }
    }
}

#[async_trait]
impl MoFAAgent for BudgetAgent {
    fn id(&self) -> &str { "budget-agent" }
    fn name(&self) -> &str { "Budget-Limited Agent" }
    fn capabilities(&self) -> &AgentCapabilities { &self.caps }

    async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(&mut self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
        self.state = AgentState::Executing;
        
        // 1. Check budget before execution
        if let Err(e) = self.enforcer.check_budget(self.id()).await {
            error!("Budget exceeded: {}", e);
            return Err(AgentError::ExecutionFailed(format!("Budget limit reached: {}", e)));
        }

        info!("Agent executing task: {}", input.to_text());

        // 2. Simulate LLM cost and token usage
        // In a real scenario, this would come from the LLM provider response.
        let simulated_cost = 2.50; // USD
        let simulated_tokens = 5000;
        
        // 3. Record usage
        self.enforcer.record_usage(self.id(), simulated_cost, simulated_tokens).await;

        let status = self.enforcer.get_status(self.id()).await;
        info!("Current status: ${:.2} spent, {} tokens used", status.session_cost, status.session_tokens);

        Ok(AgentOutput::text(format!("Processed: {}", input.to_text())))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }

    fn state(&self) -> AgentState { self.state.clone() }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging (show info by default)
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    info!("--- Agent Budget Management Example ---");

    // 1. Initialize Budget Enforcer
    let enforcer = Arc::new(BudgetEnforcer::new());

    // 2. Configure a tight budget for the agent
    // We allow a max session cost of $6.00 and max 15,000 tokens.
    let config = BudgetConfig::default()
        .with_max_cost_per_session(6.0)?
        .with_max_tokens_per_session(15_000)?;
    
    enforcer.set_budget("budget-agent", config).await;

    // 3. Create the agent
    let mut agent = BudgetAgent::new(enforcer.clone());
    let ctx = AgentContext::with_session("exec-001", "session-001");
    agent.initialize(&ctx).await?;

    // 4. Run multiple tasks until the budget is hit
    for i in 1..=5 {
        info!("Running task #{}", i);
        match agent.execute(AgentInput::text(format!("Task contents for #{}", i)), &ctx).await {
            Ok(output) => {
                info!("Task result: {}", output.to_text());
            }
            Err(e) => {
                error!("Execution failed as expected: {}", e);
                break;
            }
        }
    }

    // 5. Check final status
    let status = enforcer.get_status("budget-agent").await;
    info!("--- Final Summary ---");
    info!("Total Spent: ${:.2}", status.session_cost);
    info!("Total Tokens: {}", status.session_tokens);
    info!("Budget Exceeded: {}", status.is_exceeded());

    agent.shutdown().await?;
    Ok(())
}
