use leptos::*;

// 新增工具调用直方图
// New Tool Call Latency Histogram component
#[component]
pub fn ToolLatencyChart() -> impl IntoView {
    let tool_metrics = use_context::<ToolMetricsStore>()
        .expect("Tool metrics required");

    // 获取直方图数据 (假设 store 返回 Vec<(f64, u64)>)
    // Fetch histogram data (assume store returns Vec<(f64, u64)>)
    let chart_data = create_memo(move |_| tool_metrics.tool_latencies());

    view! {
        <ChartCard title="Tool Call Latency Distribution">
            <div class="h-64 w-full">
                // 假设有一个现成的 HistogramChart 组件
                // Assume there's an existing HistogramChart component
                <HistogramChart
                    data=chart_data
                    x_label="Latency (ms)"
                    y_label="Frequency"
                    buckets=20
                    color="rgba(233, 69, 96, 0.7)"
                />
            </div>
            <p class="text-xs text-secondary mt-2 text-center">
                Distribution of time taken by tools to respond to agent calls.
            </p>
        </ChartCard>
    }
}

// 基础图表容器
// Base chart container component
#[component]
fn ChartCard(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="bg-card rounded-xl p-6 border border-border shadow-md">
            <h3 class="text-lg font-semibold mb-4 text-primary">{title}</h3>
            {children()}
        </div>
    }
}

// 直方图模拟组件
// Histogram chart mock component
#[component]
fn HistogramChart(
    data: Memo<Vec<(f64, u64)>>,
    x_label: &'static str,
    y_label: &'static str,
    buckets: usize,
    color: &'static str,
) -> impl IntoView {
    view! {
        <div class="w-full h-full bg-secondary/20 rounded flex items-end justify-between p-2">
            {move || data.get().into_iter().map(|(val, count)| {
                let height = format!("{}%", (count as f64 / 100.0 * 100.0).min(100.0));
                view! {
                    <div class="w-2 bg-accent mx-0.5 rounded-t transition-all"
                         style={format!("height: {}; background-color: {};", height, color)}
                         title={format!("{}: {} ({} calls)", x_label, val, count)}>
                    </div>
                }
            }).collect_view()}
        </div>
    }
}

// 模拟 Store
// Mock Store
#[derive(Clone, Copy)]
pub struct ToolMetricsStore {}
impl ToolMetricsStore {
    pub fn tool_latencies(&self) -> Vec<(f64, u64)> {
        vec![
            (10.0, 5), (20.0, 15), (30.0, 25), (40.0, 40), (50.0, 60),
            (60.0, 45), (70.0, 30), (80.0, 15), (90.0, 8), (100.0, 3)
        ]
    }
}
