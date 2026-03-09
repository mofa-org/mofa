//! Swarm Executor: sequential and parallel coordination pattern schedulers

use std::sync::Arc;

use mofa_kernel::agent::{
    AgentContext,
    core::MoFAAgent,
    types::{AgentInput, error::GlobalError},
};

use crate::swarm::dag::{SubtaskDAG, SubtaskStatus};
use mofa_kernel::agent::types::error::GlobalResult;

#[derive(Debug, Clone)]
pub struct SubtaskOutput {
    pub subtask_id: String,
    pub agent_id: String,
    pub output: String,
}

#[derive(Debug)]
pub struct ExecutionResult {
    pub dag_id: String,
    pub task_count: usize,
    pub completed: usize,
    pub failed: usize,
    pub outputs: Vec<SubtaskOutput>,
}

fn find_matching_agent<'a>(
    agents: &'a mut Vec<Box<dyn MoFAAgent>>,
    required: &[String],
) -> Option<&'a mut Box<dyn MoFAAgent>> {
    agents
        .iter_mut()
        .find(|a| required.iter().all(|cap| a.capabilities().has_tag(cap)))
}

/// Execute a [`SubtaskDAG`] sequentially. Aborts on first failure
pub async fn run_sequential(
    dag: &mut SubtaskDAG,
    agents: &mut Vec<Box<dyn MoFAAgent>>,
    ctx: &AgentContext,
) -> GlobalResult<ExecutionResult> {
    let dag_id = dag.id.clone();
    let task_count = dag.task_count();
    let mut outputs: Vec<SubtaskOutput> = Vec::new();

    while !dag.is_complete() {
        let ready = dag.ready_tasks();
        if ready.is_empty() {
            break;
        }

        let idx = ready[0];
        let (id, desc, caps) = {
            let t = dag.get_task(idx).unwrap();
            (t.id.clone(), t.description.clone(), t.required_capabilities.clone())
        };

        let agent = find_matching_agent(agents, &caps).ok_or_else(|| {
            GlobalError::Other(format!(
                "No agent satisfies capabilities {:?} for subtask '{id}'",
                caps
            ))
        })?;

        let agent_id = agent.id().to_string();
        dag.mark_running(idx);

        let input = AgentInput::text(format!("[{id}] {desc}"));
        match agent.execute(input, ctx).await {
            Ok(out) => {
                let text = out.to_text();
                dag.mark_complete_with_output(idx, Some(text.clone()));
                outputs.push(SubtaskOutput { subtask_id: id, agent_id, output: text });
            }
            Err(e) => {
                let reason = e.to_string();
                dag.mark_failed(idx, &reason);
                return Err(GlobalError::Other(format!("Subtask '{id}' failed: {reason}")));
            }
        }
    }

    let completed = dag
        .all_tasks()
        .iter()
        .filter(|(_, t)| t.status == SubtaskStatus::Completed)
        .count();
    let failed = dag
        .all_tasks()
        .iter()
        .filter(|(_, t)| matches!(t.status, SubtaskStatus::Failed(_)))
        .count();

    Ok(ExecutionResult { dag_id, task_count, completed, failed, outputs })
}

/// Execute a [`SubtaskDAG`] with parallelism via `join_all`
pub async fn run_parallel(
    dag: &mut SubtaskDAG,
    agents: &mut Vec<Box<dyn MoFAAgent>>,
    ctx: &AgentContext,
) -> GlobalResult<ExecutionResult> {
    let dag_id = dag.id.clone();
    let task_count = dag.task_count();
    let mut outputs: Vec<SubtaskOutput> = Vec::new();

    while !dag.is_complete() {
        let ready = dag.ready_tasks();
        if ready.is_empty() {
            break;
        }

        let mut assignments: Vec<(petgraph::graph::NodeIndex, String, String, usize)> = Vec::new();
        let mut used_agent_indices: Vec<usize> = Vec::new();

        for idx in &ready {
            let (id, desc, caps) = {
                let t = dag.get_task(*idx).unwrap();
                (t.id.clone(), t.description.clone(), t.required_capabilities.clone())
            };

            let agent_pos = agents
                .iter()
                .enumerate()
                .find(|(i, a)| {
                    !used_agent_indices.contains(i)
                        && caps.iter().all(|c| a.capabilities().has_tag(c))
                })
                .map(|(i, _)| i)
                .ok_or_else(|| {
                    GlobalError::Other(format!(
                        "No available agent for capabilities {:?} (subtask '{id}')",
                        caps
                    ))
                })?;

            used_agent_indices.push(agent_pos);
            assignments.push((*idx, id, desc, agent_pos));
        }

        for (idx, _, _, _) in &assignments {
            dag.mark_running(*idx);
        }

        // Remove agents from pool in descending index order to keep indices stable
        let mut sorted_indices = used_agent_indices.clone();
        sorted_indices.sort_unstable_by(|a, b| b.cmp(a));
        let mut wave_agents: Vec<(usize, Box<dyn MoFAAgent>)> = sorted_indices
            .iter()
            .map(|&i| (i, agents.remove(i)))
            .collect();
        wave_agents.sort_by_key(|(i, _)| *i);

        type WaveResult = (
            usize,
            Box<dyn MoFAAgent>,
            String,
            petgraph::graph::NodeIndex,
            Result<String, String>,
        );

        let mut futures: Vec<std::pin::Pin<Box<dyn std::future::Future<Output = WaveResult> + Send>>> = Vec::new();

        for (i, (pool_idx, mut agent)) in wave_agents.into_iter().enumerate() {
            let (idx, id, desc, _) = assignments[i].clone();
            let input = AgentInput::text(format!("[{id}] {desc}"));
            let ctx = ctx.clone();

            futures.push(Box::pin(async move {
                let outcome = match agent.execute(input, &ctx).await {
                    Ok(out) => Ok(out.to_text()),
                    Err(e) => Err(e.to_string()),
                };
                (pool_idx, agent, id, idx, outcome)
            }));
        }

        let results = futures::future::join_all(futures).await;

        let mut restore_pairs: Vec<(usize, Box<dyn MoFAAgent>)> = Vec::new();
        let mut first_error: Option<String> = None;

        for (pool_idx, agent, id, idx, outcome) in results {
            let agent_id = agent.id().to_string();
            restore_pairs.push((pool_idx, agent));
            match outcome {
                Ok(text) => {
                    dag.mark_complete_with_output(idx, Some(text.clone()));
                    outputs.push(SubtaskOutput {
                        subtask_id: id.clone(),
                        agent_id,
                        output: text,
                    });
                }
                Err(reason) => {
                    dag.mark_failed(idx, &reason);
                    if first_error.is_none() {
                        first_error = Some(format!("Subtask '{id}' failed: {reason}"));
                    }
                }
            }
        }

        restore_pairs.sort_by_key(|(i, _)| *i);
        for (pool_idx, agent) in restore_pairs {
            agents.insert(pool_idx, agent);
        }

        if let Some(err) = first_error {
            return Err(GlobalError::Other(err));
        }
    }

    let completed = dag
        .all_tasks()
        .iter()
        .filter(|(_, t)| t.status == SubtaskStatus::Completed)
        .count();
    let failed = dag
        .all_tasks()
        .iter()
        .filter(|(_, t)| matches!(t.status, SubtaskStatus::Failed(_)))
        .count();

    Ok(ExecutionResult { dag_id, task_count, completed, failed, outputs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::dag::{SubtaskDAG, SubtaskStatus, SwarmSubtask};
    use mofa_kernel::agent::{
        AgentCapabilities,
        capabilities::AgentCapabilitiesBuilder,
        error::{AgentError, AgentResult},
        types::{AgentOutput, AgentState, InterruptResult},
    };
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    struct InMemoryAgentBuilder {
        id: String,
        tags: Vec<String>,
        responses: Vec<String>,
        latency_ms: u64,
        fail_after: Option<usize>,
        log: Arc<Mutex<Vec<String>>>,
    }
    impl InMemoryAgentBuilder {
        fn new(id: impl Into<String>) -> Self {
            let id = id.into();
            Self { id, tags: Vec::new(), responses: vec!["ok".to_string()],
                   latency_ms: 0, fail_after: None, log: Arc::new(Mutex::new(Vec::new())) }
        }
        fn tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
            self.tags = tags.into_iter().map(|t| t.into()).collect(); self
        }
        fn fail_after(mut self, n: usize) -> Self { self.fail_after = Some(n); self }
        fn log(mut self, log: Arc<Mutex<Vec<String>>>) -> Self { self.log = log; self }
        fn build(self) -> InMemoryAgent {
            let mut b = AgentCapabilitiesBuilder::new();
            for tag in &self.tags { b = b.tag(tag); }
            InMemoryAgent { id: self.id, capabilities: b.build(), responses: self.responses,
                            latency_ms: self.latency_ms, fail_after: self.fail_after,
                            call_count: 0, log: self.log }
        }
    }

    struct InMemoryAgent {
        id: String,
        capabilities: AgentCapabilities,
        responses: Vec<String>,
        latency_ms: u64,
        fail_after: Option<usize>,
        call_count: usize,
        log: Arc<Mutex<Vec<String>>>,
    }
    impl InMemoryAgent {
        fn simple(id: impl Into<String>) -> Self { InMemoryAgentBuilder::new(id).build() }
        fn next_response(&self) -> AgentOutput {
            let idx = (self.call_count - 1).min(self.responses.len() - 1);
            AgentOutput::text(self.responses[idx].clone())
        }
    }
    #[async_trait::async_trait]
    impl MoFAAgent for InMemoryAgent {
        fn id(&self) -> &str { &self.id }
        fn name(&self) -> &str { &self.id }
        fn capabilities(&self) -> &AgentCapabilities { &self.capabilities }
        fn state(&self) -> AgentState { AgentState::Ready }
        async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> { Ok(()) }
        async fn execute(
            &mut self,
            _input: mofa_kernel::agent::types::AgentInput,
            _ctx: &AgentContext,
        ) -> AgentResult<AgentOutput> {
            self.call_count += 1;
            self.log.lock().unwrap().push(format!("{}:start", self.id));
            if self.latency_ms > 0 {
                tokio::time::sleep(Duration::from_millis(self.latency_ms)).await;
            }
            let result = match self.fail_after {
                Some(limit) if self.call_count > limit =>
                    Err(AgentError::ExecutionFailed(format!("'{}' intentional failure", self.id))),
                _ => Ok(self.next_response()),
            };
            let status = if result.is_ok() { "end" } else { "err" };
            self.log.lock().unwrap().push(format!("{}:{status}", self.id));
            result
        }
        async fn shutdown(&mut self) -> AgentResult<()> { Ok(()) }
        async fn interrupt(&mut self) -> AgentResult<InterruptResult> {
            Ok(InterruptResult::Acknowledged)
        }
    }

    fn ctx() -> AgentContext {
        AgentContext::new("test-exec")
    }

    fn simple_agent(id: &str) -> Box<dyn MoFAAgent> {
        Box::new(InMemoryAgent::simple(id))
    }

    fn tagged_agent(id: &str, tags: &[&str]) -> Box<dyn MoFAAgent> {
        Box::new(InMemoryAgentBuilder::new(id).tags(tags.to_vec()).build())
    }

    #[tokio::test]
    async fn test_seq_single_task() {
        let mut dag = SubtaskDAG::new("single");
        dag.add_task(SwarmSubtask::new("t1", "Do the thing"));

        let mut agents: Vec<Box<dyn MoFAAgent>> = vec![simple_agent("a1")];
        let result = run_sequential(&mut dag, &mut agents, &ctx()).await.unwrap();

        assert_eq!(result.completed, 1);
        assert_eq!(result.failed, 0);
        assert_eq!(result.outputs.len(), 1);

        let t1_idx = dag.find_by_id("t1").unwrap();
        assert_eq!(dag.get_task(t1_idx).unwrap().status, SubtaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_seq_linear_chain_runs_in_order() {
        let log = Arc::new(Mutex::new(Vec::<String>::new()));

        let mut dag = SubtaskDAG::new("chain");
        let a = dag.add_task(SwarmSubtask::new("a", "Step A"));
        let b = dag.add_task(SwarmSubtask::new("b", "Step B"));
        let c = dag.add_task(SwarmSubtask::new("c", "Step C"));
        dag.add_dependency(a, b).unwrap();
        dag.add_dependency(b, c).unwrap();

        let mut agents: Vec<Box<dyn MoFAAgent>> = vec![
            Box::new(InMemoryAgentBuilder::new("agent").log(log.clone()).build()),
        ];

        let result = run_sequential(&mut dag, &mut agents, &ctx()).await.unwrap();
        assert_eq!(result.completed, 3);

        let entries = log.lock().unwrap().clone();
        let starts: Vec<&str> = entries.iter().filter(|e| e.ends_with(":start")).map(|s| s.as_str()).collect();
        let ends: Vec<&str>   = entries.iter().filter(|e| e.ends_with(":end")).map(|s| s.as_str()).collect();
        assert_eq!(starts.len(), 3);
        assert_eq!(ends.len(), 3);
        let end_pos = entries.iter().position(|e| e == "agent:end").unwrap();
        assert!(entries[end_pos + 1..].iter().any(|e| e.ends_with(":start")));
    }

    #[tokio::test]
    async fn test_seq_failure_halts_execution() {
        let mut dag = SubtaskDAG::new("halt");
        let a = dag.add_task(SwarmSubtask::new("a", "Will fail"));
        let b = dag.add_task(SwarmSubtask::new("b", "Should not run"));
        dag.add_dependency(a, b).unwrap();

        let mut agents: Vec<Box<dyn MoFAAgent>> = vec![
            Box::new(InMemoryAgentBuilder::new("a1").fail_after(0).build()),
        ];

        let result = run_sequential(&mut dag, &mut agents, &ctx()).await;
        assert!(result.is_err());
        assert_eq!(dag.get_task(b).unwrap().status, SubtaskStatus::Pending);
    }

    #[tokio::test]
    async fn test_seq_no_matching_agent_returns_error() {
        let mut dag = SubtaskDAG::new("nomatch");
        let mut task = SwarmSubtask::new("t1", "Needs vision");
        task.required_capabilities = vec!["vision".to_string()];
        dag.add_task(task);

        let mut agents: Vec<Box<dyn MoFAAgent>> = vec![tagged_agent("a1", &["llm"])];
        let result = run_sequential(&mut dag, &mut agents, &ctx()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("vision"));
    }

    #[tokio::test]
    async fn test_par_independent_tasks_all_complete() {
        let mut dag = SubtaskDAG::new("independent");
        dag.add_task(SwarmSubtask::new("t1", "Task 1"));
        dag.add_task(SwarmSubtask::new("t2", "Task 2"));
        dag.add_task(SwarmSubtask::new("t3", "Task 3"));

        let mut agents: Vec<Box<dyn MoFAAgent>> = vec![
            simple_agent("a1"), simple_agent("a2"), simple_agent("a3"),
        ];

        let result = run_parallel(&mut dag, &mut agents, &ctx()).await.unwrap();
        assert_eq!(result.completed, 3);
        assert_eq!(result.failed, 0);
    }

    #[tokio::test]
    async fn test_par_diamond_respects_dependencies() {
        let mut dag = SubtaskDAG::new("diamond");
        let root  = dag.add_task(SwarmSubtask::new("root",  "Root"));
        let left  = dag.add_task(SwarmSubtask::new("left",  "Left branch"));
        let right = dag.add_task(SwarmSubtask::new("right", "Right branch"));
        let merge = dag.add_task(SwarmSubtask::new("merge", "Merge"));
        dag.add_dependency(root, left).unwrap();
        dag.add_dependency(root, right).unwrap();
        dag.add_dependency(left, merge).unwrap();
        dag.add_dependency(right, merge).unwrap();

        let mut agents: Vec<Box<dyn MoFAAgent>> = vec![simple_agent("a1"), simple_agent("a2")];

        let result = run_parallel(&mut dag, &mut agents, &ctx()).await.unwrap();
        assert_eq!(result.completed, 4);
        assert_eq!(result.failed, 0);
        assert_eq!(dag.get_task(merge).unwrap().status, SubtaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_par_failure_propagated() {
        let mut dag = SubtaskDAG::new("parfail");
        dag.add_task(SwarmSubtask::new("t1", "Will fail"));
        dag.add_task(SwarmSubtask::new("t2", "May succeed"));

        let mut agents: Vec<Box<dyn MoFAAgent>> = vec![
            Box::new(InMemoryAgentBuilder::new("a1").fail_after(0).build()),
            simple_agent("a2"),
        ];

        assert!(run_parallel(&mut dag, &mut agents, &ctx()).await.is_err());
    }

    #[tokio::test]
    async fn test_par_outputs_carry_correct_agent_ids() {
        let mut dag = SubtaskDAG::new("agent-id-check");
        dag.add_task(SwarmSubtask::new("t1", "Task 1"));
        dag.add_task(SwarmSubtask::new("t2", "Task 2"));

        let mut agents: Vec<Box<dyn MoFAAgent>> =
            vec![simple_agent("alpha"), simple_agent("beta")];

        let result = run_parallel(&mut dag, &mut agents, &ctx()).await.unwrap();
        assert_eq!(result.completed, 2);

        for out in &result.outputs {
            assert!(
                !out.agent_id.is_empty(),
                "agent_id was empty for subtask '{}'",
                out.subtask_id
            );
        }

        let valid_ids = ["alpha", "beta"];
        for out in &result.outputs {
            assert!(
                valid_ids.contains(&out.agent_id.as_str()),
                "unexpected agent_id '{}' for subtask '{}'",
                out.agent_id,
                out.subtask_id
            );
        }
    }

    // API integration tests
    struct LLMAgent {
        id: String,
        model: String,
        api_key: String,
        base_url: String,
        capabilities: AgentCapabilities,
        client: reqwest::Client,
    }

    impl LLMAgent {
        fn new(
            id: impl Into<String>,
            model: impl Into<String>,
            api_key: impl Into<String>,
            base_url: impl Into<String>,
        ) -> Self {
            Self {
                id: id.into(),
                model: model.into(),
                api_key: api_key.into(),
                base_url: base_url.into(),
                capabilities: AgentCapabilities::default(),
                client: reqwest::Client::new(),
            }
        }
    }

    #[async_trait::async_trait]
    impl MoFAAgent for LLMAgent {
        fn id(&self) -> &str { &self.id }
        fn name(&self) -> &str { &self.id }
        fn capabilities(&self) -> &AgentCapabilities { &self.capabilities }
        fn state(&self) -> mofa_kernel::agent::types::AgentState {
            mofa_kernel::agent::types::AgentState::Ready
        }
        async fn initialize(&mut self, _ctx: &AgentContext)
            -> mofa_kernel::agent::error::AgentResult<()> { Ok(()) }

        async fn execute(
            &mut self,
            input: mofa_kernel::agent::types::AgentInput,
            _ctx: &AgentContext,
        ) -> mofa_kernel::agent::error::AgentResult<mofa_kernel::agent::types::AgentOutput> {
            use mofa_kernel::agent::error::AgentError;

            let body = serde_json::json!({
                "model": self.model,
                "messages": [{"role": "user", "content": input.as_text().unwrap_or_default()}],
                "max_tokens": 256,
            });

            let endpoint = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
            let resp = self.client
                .post(&endpoint)
                .bearer_auth(&self.api_key)
                .json(&body)
                .send()
                .await
                .map_err(|e| AgentError::ExecutionFailed(e.to_string()))?;

            let status = resp.status();
            let json: serde_json::Value = resp.json().await
                .map_err(|e| AgentError::ExecutionFailed(e.to_string()))?;

            if !status.is_success() {
                return Err(AgentError::ExecutionFailed(format!(
                    "API {} — {}",
                    status,
                    json["error"]["message"].as_str().unwrap_or("unknown")
                )));
            }

            Ok(mofa_kernel::agent::types::AgentOutput::text(
                json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string()
            ))
        }

        async fn shutdown(&mut self)
            -> mofa_kernel::agent::error::AgentResult<()> { Ok(()) }
        async fn interrupt(&mut self)
            -> mofa_kernel::agent::error::AgentResult<mofa_kernel::agent::types::InterruptResult> {
            Ok(mofa_kernel::agent::types::InterruptResult::Acknowledged)
        }
    }

    #[tokio::test]
    #[ignore = "requires LLM_API_KEY"]
    async fn test_llm_seq_two_tasks() {
        let Ok(key) = std::env::var("LLM_API_KEY") else { return };
        let base_url = std::env::var("LLM_BASE_URL")
            .unwrap_or_else(|_| "https://api.groq.com/openai/v1".to_string());

        let mut dag = SubtaskDAG::new("llm-seq");
        let a = dag.add_task(SwarmSubtask::new("a", "Reply with exactly one word: hello"));
        let b = dag.add_task(SwarmSubtask::new("b", "Reply with exactly one word: world"));
        dag.add_dependency(a, b).unwrap();

        let mut agents: Vec<Box<dyn MoFAAgent>> = vec![
            Box::new(LLMAgent::new("llm-1", "llama-3.1-8b-instant", &key, &base_url)),
        ];

        let result = run_sequential(&mut dag, &mut agents, &ctx()).await.unwrap();
        assert_eq!(result.completed, 2);
        assert_eq!(result.failed, 0);
        for out in &result.outputs { println!("[{}] {}", out.subtask_id, out.output); }
    }

    #[tokio::test]
    #[ignore = "requires LLM_API_KEY"]
    async fn test_llm_par_diamond() {
        let Ok(key) = std::env::var("LLM_API_KEY") else { return };
        let base_url = std::env::var("LLM_BASE_URL")
            .unwrap_or_else(|_| "https://api.groq.com/openai/v1".to_string());

        let mut dag = SubtaskDAG::new("llm-diamond");
        let root  = dag.add_task(SwarmSubtask::new("root",  "Reply with one word: start"));
        let left  = dag.add_task(SwarmSubtask::new("left",  "Reply with one word: left"));
        let right = dag.add_task(SwarmSubtask::new("right", "Reply with one word: right"));
        let merge = dag.add_task(SwarmSubtask::new("merge", "Reply with one word: done"));
        dag.add_dependency(root, left).unwrap();
        dag.add_dependency(root, right).unwrap();
        dag.add_dependency(left, merge).unwrap();
        dag.add_dependency(right, merge).unwrap();

        let mut agents: Vec<Box<dyn MoFAAgent>> = vec![
            Box::new(LLMAgent::new("llm-1", "llama-3.1-8b-instant", &key, &base_url)),
            Box::new(LLMAgent::new("llm-2", "llama-3.1-8b-instant", &key, &base_url)),
        ];

        let result = run_parallel(&mut dag, &mut agents, &ctx()).await.unwrap();
        assert_eq!(result.completed, 4);
        assert_eq!(result.failed, 0);
        assert_eq!(dag.get_task(merge).unwrap().status, SubtaskStatus::Completed);
        for out in &result.outputs { println!("[{}] {}", out.subtask_id, out.output); }
    }
}
