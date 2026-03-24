//! Browser UI for viewing and triggering swarm testing artifacts.
//!
//! This Leptos app can:
//! - trigger a demo swarm run over HTTP
//! - fetch the latest `SwarmRunArtifact`
//! - render swarm summary, graph, task state, agent ownership, metrics, and audit data

use gloo_file::callbacks::read_as_text;
use gloo_file::File;
use gloo_net::http::Request;
use leptos::*;
use serde_json::Value;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::prelude::*;
use web_sys::HtmlInputElement;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = renderMermaid)]
    fn render_mermaid(id: &str, code: &str);
}

fn main() {
    mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let (json_text, set_json_text) = create_signal(String::new());
    let (artifact, set_artifact) = create_signal::<Option<Value>>(None);
    let (error, set_error) = create_signal::<Option<String>>(None);
    let (mermaid_code, set_mermaid_code) = create_signal(String::new());
    let (server_url, set_server_url) = create_signal(String::from("http://127.0.0.1:3001"));
    let (status_msg, set_status_msg) = create_signal::<Option<String>>(None);
    let (_file_readers, set_file_readers) = create_signal::<Vec<gloo_file::callbacks::FileReader>>(
        Vec::new(),
    );

    // Keep the text area as the single source of truth and derive parsed state from it.
    create_effect(move |_| {
        let text = json_text.get();
        if text.trim().is_empty() {
            set_artifact.set(None);
            set_error.set(None);
            set_mermaid_code.set(String::new());
            return;
        }

        match serde_json::from_str::<Value>(&text) {
            Ok(value) => {
                set_error.set(None);
                set_artifact.set(Some(value.clone()));
                let code = build_mermaid(&value);
                set_mermaid_code.set(code);
            }
            Err(err) => {
                set_error.set(Some(format!("JSON parse error: {err}")));
                set_artifact.set(None);
                set_mermaid_code.set(String::new());
            }
        }
    });

    // Mermaid is initialized in `index.html`; this effect pushes updated graph code into it.
    create_effect(move |_| {
        let code = mermaid_code.get();
        if !code.trim().is_empty() {
            render_mermaid("mermaid-graph", &code);
        }
    });

    let on_file_change = move |ev| {
        let input = event_target::<HtmlInputElement>(&ev);
        if let Some(file) = input.files().and_then(|list| list.get(0)) {
            let file = File::from(file);
            let setter = set_json_text.clone();
            let reader = read_as_text(&file, move |res| {
                if let Ok(contents) = res {
                    setter.set(contents);
                }
            });
            set_file_readers.update(|readers| readers.push(reader));
        }
    };

    let run_swarm = move |_| {
        let url = server_url.get();
        let set_status_msg = set_status_msg.clone();
        let set_error = set_error.clone();
        let set_json_text = set_json_text.clone();
        spawn_local(async move {
            set_status_msg.set(Some("Running swarm...".to_string()));
            set_error.set(None);
            let endpoint = format!("{}/swarm/run", url.trim_end_matches('/'));
            let resp = Request::post(&endpoint).send().await;
            match resp {
                Ok(resp) if resp.ok() => {
                    match resp.json::<Value>().await {
                        Ok(value) => {
                            let text = pretty_json(value);
                            set_json_text.set(text);
                            set_status_msg.set(Some("Swarm run complete.".to_string()));
                        }
                        Err(err) => {
                            set_error.set(Some(format!("Failed to parse artifact: {err}")));
                            set_status_msg.set(None);
                        }
                    }
                }
                Ok(resp) => {
                    set_error.set(Some(format!("Run failed: HTTP {}", resp.status())));
                    set_status_msg.set(None);
                }
                Err(err) => {
                    set_error.set(Some(format!("Run request failed: {err}")));
                    set_status_msg.set(None);
                }
            }
        });
    };

    let fetch_artifact = move |_| {
        let url = server_url.get();
        let set_status_msg = set_status_msg.clone();
        let set_error = set_error.clone();
        let set_json_text = set_json_text.clone();
        spawn_local(async move {
            set_status_msg.set(Some("Fetching latest artifact...".to_string()));
            set_error.set(None);
            let endpoint = format!("{}/swarm/artifact", url.trim_end_matches('/'));
            // This path is useful when the swarm was triggered outside the current browser session.
            let resp = Request::get(&endpoint).send().await;
            match resp {
                Ok(resp) if resp.ok() => match resp.json::<Value>().await {
                    Ok(value) => {
                        let text = pretty_json(value);
                        set_json_text.set(text);
                        set_status_msg.set(Some("Artifact loaded.".to_string()));
                    }
                    Err(err) => {
                        set_error.set(Some(format!("Failed to parse artifact: {err}")));
                        set_status_msg.set(None);
                    }
                },
                Ok(resp) => {
                    set_error.set(Some(format!("Fetch failed: HTTP {}", resp.status())));
                    set_status_msg.set(None);
                }
                Err(err) => {
                    set_error.set(Some(format!("Fetch request failed: {err}")));
                    set_status_msg.set(None);
                }
            }
        });
    };

    view! {
        <div class="header">
            <div>
                <h1>"Swarm Artifact Viewer"</h1>
                <p>"Load a SwarmRunArtifact JSON file and explore task state, metrics, and orchestration flow."</p>
            </div>
        </div>

        <section class="panel">
            <div class="controls">
                <label class="badge">"JSON Input"</label>
                <input type="file" accept="application/json" on:change=on_file_change />
                <label class="badge">"Server"</label>
                <input
                    class="text-input"
                    type="text"
                    prop:value=server_url
                    on:input=move |ev| {
                        set_server_url.set(event_target_value(&ev));
                    }
                />
                <button class="button" on:click=run_swarm>"Run Swarm"</button>
                <button class="button secondary" on:click=fetch_artifact>"Fetch Latest"</button>
            </div>
            <textarea
                placeholder="Paste SwarmRunArtifact JSON here..."
                prop:value=json_text
                on:input=move |ev| {
                    set_json_text.set(event_target_value(&ev));
                }
            ></textarea>
            <Show
                when=move || error.get().is_some()
                fallback=|| ()
            >
                <p class="error">{move || error.get().unwrap_or_default()}</p>
            </Show>
            <Show
                when=move || status_msg.get().is_some()
                fallback=|| ()
            >
                <p class="status">{move || status_msg.get().unwrap_or_default()}</p>
            </Show>
        </section>

        <Show
            when=move || artifact.get().is_some()
            fallback=|| ()
        >
            <section class="panel">
                <div class="grid">
                    {move || artifact.get().map(render_summary)}
                </div>
            </section>

            <section class="panel">
                <h2 class="section-title">"Dependency Graph"</h2>
                <div id="mermaid-graph" class="mermaid"></div>
            </section>

            <section class="panel">
                <h2 class="section-title">"Tasks"</h2>
                {move || artifact.get().map(render_tasks)}
            </section>

            <section class="panel">
                <h2 class="section-title">"Execution Trace"</h2>
                {move || artifact.get().map(render_execution)}
            </section>

            <section class="panel">
                <h2 class="section-title">"Agent Collaboration"</h2>
                {move || artifact.get().map(render_agents)}
            </section>

            <section class="panel">
                <h2 class="section-title">"Metrics"</h2>
                {move || artifact.get().map(render_metrics)}
            </section>

            <section class="panel">
                <h2 class="section-title">"Audit Trail"</h2>
                {move || artifact.get().map(render_audit)}
            </section>

            <section class="panel">
                <h2 class="section-title">"Raw JSON"</h2>
                <pre>{move || artifact.get().map(pretty_json).unwrap_or_default()}</pre>
            </section>
        </Show>
    }
}

/// Render top-level run metadata cards for quick inspection.
fn render_summary(value: Value) -> impl IntoView {
    let name = pick_string(&value, "name");
    let pattern = pick_string(&value, "pattern");
    let status = pick_string(&value, "swarm_status");
    let total_tasks = pick_number(&value, "total_tasks");
    let succeeded = pick_number(&value, "succeeded");
    let failed = pick_number(&value, "failed");
    let skipped = pick_number(&value, "skipped");
    let wall_time = pick_number(&value, "total_wall_time_ms");
    let success_rate = value
        .get("success_rate")
        .and_then(|v| v.as_f64())
        .map(|v| format!("{:.1}%", v * 100.0))
        .unwrap_or_else(|| "-".to_string());
    let output = pick_string(&value, "output");

    view! {
        <div class="card">
            <h3>"Run Summary"</h3>
            <div class="stat-list">
                <p><strong>"Name"</strong><span>{name}</span></p>
                <p><strong>"Pattern"</strong><span>{pattern}</span></p>
                <p><strong>"Swarm Status"</strong><span>{status}</span></p>
                <p><strong>"Total Tasks"</strong><span>{total_tasks}</span></p>
            </div>
        </div>
        <div class="card">
            <h3>"Outcomes"</h3>
            <div class="stat-list">
                <p><strong>"Succeeded"</strong><span>{succeeded}</span></p>
                <p><strong>"Failed"</strong><span>{failed}</span></p>
                <p><strong>"Skipped"</strong><span>{skipped}</span></p>
                <p><strong>"Success Rate"</strong><span>{success_rate}</span></p>
            </div>
        </div>
        <div class="card">
            <h3>"Timing"</h3>
            <div class="stat-list">
                <p><strong>"Wall Time (ms)"</strong><span>{wall_time}</span></p>
                <p><strong>"Final Output"</strong><span>{output}</span></p>
            </div>
        </div>
    }
}

/// Render the canonical task list captured in the artifact.
fn render_tasks(value: Value) -> impl IntoView {
    let tasks = value.get("tasks").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if tasks.is_empty() {
        return view! { <div><p>"No tasks found in artifact."</p></div> };
    }

    view! {
        <div>
        <table class="table">
            <thead>
                <tr>
                    <th>"Task"</th>
                    <th>"Status"</th>
                    <th>"Agent"</th>
                    <th>"Dependencies"</th>
                    <th>"Capabilities"</th>
                    <th>"Risk"</th>
                    <th>"HITL"</th>
                </tr>
            </thead>
            <tbody>
                {tasks.into_iter().map(|task| {
                    let id = task.get("id").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                    let status = task.get("status").map(value_to_string).unwrap_or_else(|| "-".to_string());
                    let agent = task.get("assigned_agent").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                    let deps = task.get("dependencies").map(join_array).unwrap_or_else(|| "-".to_string());
                    let caps = task.get("required_capabilities").map(join_array).unwrap_or_else(|| "-".to_string());
                    let risk = task.get("risk_level").map(value_to_string).unwrap_or_else(|| "-".to_string());
                    let hitl = task
                        .get("hitl_required")
                        .and_then(|v| v.as_bool())
                        .map(|v| if v { "required" } else { "no" })
                        .unwrap_or("-")
                        .to_string();

                    view! {
                        <tr>
                            <td>{id}</td>
                            <td>{status}</td>
                            <td>{agent}</td>
                            <td>{deps}</td>
                            <td>{caps}</td>
                            <td>{risk}</td>
                            <td>{hitl}</td>
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>
        </div>
    }
}

/// Render the ordered execution trace emitted by the scheduler.
fn render_execution(value: Value) -> impl IntoView {
    let rows = value
        .get("execution")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if rows.is_empty() {
        return view! { <div><p>"No execution records found."</p></div> };
    }

    view! {
        <div>
        <table class="table">
            <thead>
                <tr>
                    <th>"Order"</th>
                    <th>"Task"</th>
                    <th>"Outcome"</th>
                    <th>"Detail"</th>
                    <th>"Duration (ms)"</th>
                    <th>"Attempt"</th>
                </tr>
            </thead>
            <tbody>
                {rows.into_iter().enumerate().map(|(idx, row)| {
                    let task_id = row.get("task_id").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                    let outcome = row.get("outcome").map(value_to_string).unwrap_or_else(|| "-".to_string());
                    let detail = row.get("detail").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                    let duration = row.get("wall_time_ms").map(value_to_string).unwrap_or_else(|| "-".to_string());
                    let attempt = row.get("attempt").map(value_to_string).unwrap_or_else(|| "-".to_string());

                    view! {
                        <tr>
                            <td>{(idx + 1).to_string()}</td>
                            <td>{task_id}</td>
                            <td>{outcome}</td>
                            <td>{detail}</td>
                            <td>{duration}</td>
                            <td>{attempt}</td>
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>
        </div>
    }
}

/// Group tasks by assigned agent to show ownership across the swarm.
fn render_agents(value: Value) -> impl IntoView {
    let tasks = value.get("tasks").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if tasks.is_empty() {
        return view! { <div><p>"No tasks to group by agent."</p></div> };
    }

    use std::collections::BTreeMap;
    let mut by_agent: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for task in tasks {
        let agent = task.get("assigned_agent").and_then(|v| v.as_str()).unwrap_or("unassigned").to_string();
        let id = task.get("id").and_then(|v| v.as_str()).unwrap_or("-").to_string();
        let status = task.get("status").map(value_to_string).unwrap_or_else(|| "-".to_string());
        by_agent.entry(agent).or_default().push((id, status));
    }

    view! {
        <div>
        <table class="table">
            <thead>
                <tr>
                    <th>"Agent"</th>
                    <th>"Tasks"</th>
                    <th>"Terminal States"</th>
                </tr>
            </thead>
            <tbody>
                {by_agent.into_iter().map(|(agent, tasks)| {
                    let task_ids: Vec<String> = tasks.iter().map(|(id, _)| id.clone()).collect();
                    let states: Vec<String> = tasks
                        .iter()
                        .map(|(id, status)| format!("{}:{}", id, status))
                        .collect();

                    view! {
                        <tr>
                            <td>{agent}</td>
                            <td>{task_ids.join(", ")}</td>
                            <td>{states.join(", ")}</td>
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>
        </div>
    }
}

/// Render aggregate swarm metrics and per-agent token usage when present.
fn render_metrics(value: Value) -> impl IntoView {
    let metrics = match value.get("metrics") {
        Some(v) => v.clone(),
        None => return view! { <div><p>"No metrics snapshot in artifact."</p></div> },
    };

    let total_tokens = metrics.get("total_tokens").map(value_to_string).unwrap_or_else(|| "-".to_string());
    let duration = metrics.get("duration_ms").map(value_to_string).unwrap_or_else(|| "-".to_string());
    let tasks_completed = metrics.get("tasks_completed").map(value_to_string).unwrap_or_else(|| "-".to_string());
    let tasks_failed = metrics.get("tasks_failed").map(value_to_string).unwrap_or_else(|| "-".to_string());
    let hitl = metrics.get("hitl_interventions").map(value_to_string).unwrap_or_else(|| "-".to_string());
    let reassignments = metrics.get("reassignments").map(value_to_string).unwrap_or_else(|| "-".to_string());

    let agent_tokens = metrics.get("agent_tokens").cloned().unwrap_or(Value::Null);

    view! {
        <div>
            <table class="table">
                <thead>
                    <tr>
                        <th>"Total Tokens"</th>
                        <th>"Duration"</th>
                        <th>"Tasks Completed"</th>
                        <th>"Tasks Failed"</th>
                        <th>"HITL"</th>
                        <th>"Reassignments"</th>
                    </tr>
                </thead>
                <tbody>
                    <tr>
                        <td>{total_tokens}</td>
                        <td>{duration}</td>
                        <td>{tasks_completed}</td>
                        <td>{tasks_failed}</td>
                        <td>{hitl}</td>
                        <td>{reassignments}</td>
                    </tr>
                </tbody>
            </table>
            <h3>"Agent Tokens"</h3>
            <pre>{pretty_json(agent_tokens)}</pre>
        </div>
    }
}

/// Render normalized audit events captured during the swarm run.
fn render_audit(value: Value) -> impl IntoView {
    let events = value
        .get("audit_events")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if events.is_empty() {
        return view! { <div><p>"No audit events recorded."</p></div> };
    }

    view! {
        <div>
        <table class="table">
            <thead>
                <tr>
                    <th>"Kind"</th>
                    <th>"Description"</th>
                    <th>"Timestamp"</th>
                    <th>"Data"</th>
                </tr>
            </thead>
            <tbody>
                {events.into_iter().map(|event| {
                    let kind = event.get("kind").map(value_to_string).unwrap_or_else(|| "-".to_string());
                    let description = event.get("description").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                    let timestamp = event.get("timestamp_ms").map(value_to_string).unwrap_or_else(|| "-".to_string());
                    let data = event.get("data").cloned().unwrap_or(Value::Null);
                    view! {
                        <tr>
                            <td>{kind}</td>
                            <td>{description}</td>
                            <td>{timestamp}</td>
                            <td><pre>{pretty_json(data)}</pre></td>
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>
        </div>
    }
}

/// Convert task dependencies into Mermaid graph syntax for browser rendering.
fn build_mermaid(value: &Value) -> String {
    let tasks = value.get("tasks").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if tasks.is_empty() {
        return String::new();
    }

    let mut lines = vec!["graph TD".to_string()];
    for task in tasks {
        let id = task.get("id").and_then(|v| v.as_str()).unwrap_or("task");
        let safe_id = sanitize_id(id);
        let deps = task.get("dependencies").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        // Mirror the markdown artifact graph so the browser view stays aligned with PR output.
        if deps.is_empty() {
            lines.push(format!("    {}[\"{}\"]", safe_id, id));
        } else {
            for dep in deps {
                let dep_id = dep.as_str().unwrap_or("dep");
                let dep_safe = sanitize_id(dep_id);
                lines.push(format!("    {}[\"{}\"] --> {}[\"{}\"]", dep_safe, dep_id, safe_id, id));
            }
        }
    }

    lines.join("\n")
}

/// Sanitize task ids so they are valid Mermaid node identifiers.
fn sanitize_id(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push_str("task");
    }
    out
}

/// Read a string field from the artifact and provide a stable fallback for missing data.
fn pick_string(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("-")
        .to_string()
}

/// Read numeric fields used in summary cards and table cells.
fn pick_number(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_i64())
        .map(|v| v.to_string())
        .or_else(|| value.get(key).and_then(|v| v.as_u64()).map(|v| v.to_string()))
        .unwrap_or_else(|| "-".to_string())
}

/// Join string arrays from artifact fields into a compact display value.
fn join_array(value: &Value) -> String {
    value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "-".to_string())
}

/// Normalize mixed JSON values into plain display strings for the tables.
fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "-".to_string(),
        other => other.to_string(),
    }
}

/// Pretty-print JSON for the text area and raw artifact panel.
fn pretty_json(value: Value) -> String {
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
}
