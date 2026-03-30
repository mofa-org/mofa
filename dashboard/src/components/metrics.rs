use leptos::*;

// 在现有metrics页面添加新卡片
// Add new card to existing metrics page
#[component]
pub fn ReactAgentMetrics() -> impl IntoView {
    // 假设 AgentTraceStore 已经通过 provide_context 提供
    // Assume AgentTraceStore is provided via context
    let agent_traces = use_context::<AgentTraceStore>()
        .expect("Agent trace store required");

    view! {
        <MetricCard title="ReAct Agent Traces">
            <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                <StatCard
                    title="Active Traces"
                    value={move || agent_traces.active_traces()}
                    trend="up"
                    description="Currently running ReAct reasoning loops"
                />
                <StatCard
                    title="Avg Steps"
                    value={move || format!("{:.1}", agent_traces.avg_steps())}
                    trend="neutral"
                    description="Average steps per successful task"
                />
                <StatCard
                    title="Error Rate"
                    value={move || format!("{:.1}%", agent_traces.error_rate() * 100.0)}
                    trend="down"
                    description="Percentage of failed reasoning steps"
                />
            </div>
        </MetricCard>
    }
}

// 基础组件定义 (模拟)
// Base component definitions (mock)
#[component]
fn MetricCard(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="bg-card rounded-xl p-6 border border-border shadow-sm">
            <h3 class="text-lg font-semibold mb-4 text-primary">{title}</h3>
            {children()}
        </div>
    }
}

#[component]
fn StatCard(
    title: &'static str,
    value: impl Fn() -> String + 'static,
    trend: &'static str,
    description: &'static str,
) -> impl IntoView {
    let trend_class = match trend {
        "up" => "text-green-500",
        "down" => "text-red-500",
        _ => "text-gray-400",
    };

    view! {
        <div class="flex flex-col p-4 bg-secondary/50 rounded-lg">
            <span class="text-sm text-secondary mb-1">{title}</span>
            <span class="text-2xl font-bold text-primary">{value}</span>
            <span class={format!("text-xs mt-2 {}", trend_class)}>{description}</span>
        </div>
    }
}

// 模拟 Store
// Mock Store
#[derive(Clone, Copy)]
pub struct AgentTraceStore {}
impl AgentTraceStore {
    pub fn active_traces(&self) -> String { "12".to_string() }
    pub fn avg_steps(&self) -> f64 { 4.5 }
    pub fn error_rate(&self) -> f64 { 0.05 }
}
