# Workflow Visualizer

Interactive web-based graph visualization for MoFA workflow definitions.

## Quick Start

```bash
cd examples && cargo run -p workflow_viz
# Open http://127.0.0.1:3030
```

## Features

### Phase 1 — Static Graph Visualization *(shipped)*
- Renders YAML workflow definitions as interactive DAG graphs
- Automatic Sugiyama-style layered layout with crossing minimization
- Node shapes per type: pill (start/end), diamond (condition), hexagon (loop), parallelogram (parallel/join)
- Edge rendering with Bézier curves and conditional/error styling
- Connected-node highlighting on hover
- Detail panel with node metadata, incoming/outgoing edges
- Minimap for large graphs
- Pan, zoom (mouse wheel), fit-to-view
- SVG export
- Fuzzy search across nodes

### Phase 2 — Live Execution Monitoring *(PR [#561](https://github.com/mofa-org/mofa/pull/561))*
- **WebSocket** (`ws://localhost:3030/ws`) for real-time event streaming (when enabled on the server)
- **Node status colors**: 🟢 running (green pulse), 🔵 completed (blue glow), 🔴 failed (red border), ⚪ pending (dimmed)
- **Duration badges** on completed nodes
- **Execution log**: collapsible bottom panel, filterable by status (All/Running/Completed/Failed)
- **Auto-pan**: camera follows the currently executing node (toggleable)
- **Connection indicator**: connected / reconnecting / disconnected with exponential backoff
- **Inspector v2**: inputs, outputs, execution time, and error details
- **Simulate**: `POST /api/simulate?workflow_id=...` triggers a synthetic topo-walk execution

## API

### Phase 1 (current)

| Endpoint | Method | Description |
|---|---|---|
| `/api/workflows` | GET | List all loaded workflow definitions |
| `/api/workflows/:id` | GET | Get full graph data for a workflow |

### Phase 2 (PR [#561](https://github.com/mofa-org/mofa/pull/561))

| Endpoint | Method | Description |
|---|---|---|
| `/api/simulate?workflow_id=...` | POST | Trigger simulated execution |
| `/api/execution/state` | GET | Current node states (for reconnection catch-up) |
| `/ws` | GET | WebSocket upgrade for live event stream |

## Keyboard Shortcuts

| Key | Action |
|---|---|
| `+` / `-` | Zoom in / out |
| `0` | Fit to view |
| `/` | Focus search |
| `S` | Simulate workflow execution *(Phase 2)* |
| `E` | Export SVG |
| `A` | Toggle auto-pan *(Phase 2)* |
| `L` | Toggle execution log *(Phase 2)* |
| `Esc` | Close panel |

## Architecture

```
examples/workflow_viz/
├── Cargo.toml          # axum + tokio
├── src/main.rs         # Server: REST API, static file serving
└── static/
    ├── index.html      # Layout: sidebar, canvas, inspector
    ├── style.css       # Design system + node styling
    ├── app.js          # Graph layout engine, interaction
    └── logo.png
```

Workflow definitions are loaded from `examples/workflow_dsl/*.yaml` at startup.
