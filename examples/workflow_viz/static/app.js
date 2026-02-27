/* ================================================================
   MoFA Workflow Visualizer — Phase 2: Live Execution Monitoring
   ================================================================ */
(() => {
  'use strict';

  const NODE_W = 160, NODE_H = 52, LAYER_GAP_Y = 100, NODE_GAP_X = 50, PAD = 60;

  const TYPE_META = {
    start: { fill: '#d5f5f0', stroke: '#1ABC9C', label: 'START' },
    end: { fill: '#fde8e5', stroke: '#E74C3C', label: 'END' },
    task: { fill: '#d6eaf8', stroke: '#3498DB', label: 'TASK' },
    agent: { fill: '#f4ecf7', stroke: '#9B59B6', label: 'AGENT' },
    llm_agent: { fill: '#f4ecf7', stroke: '#9B59B6', label: 'LLM' },
    condition: { fill: '#fef9e7', stroke: '#F1C40F', label: 'COND' },
    parallel: { fill: '#d5f5f0', stroke: '#1ABC9C', label: 'PARALLEL' },
    join: { fill: '#d5f5f0', stroke: '#1ABC9C', label: 'JOIN' },
    loop: { fill: '#fdebd0', stroke: '#E67E22', label: 'LOOP' },
    wait: { fill: '#f2f3f4', stroke: '#7f8c8d', label: 'WAIT' },
    transform: { fill: '#d5f5f0', stroke: '#1ABC9C', label: 'XFORM' },
    sub_workflow: { fill: '#d6eaf8', stroke: '#3498DB', label: 'SUB' },
  };

  let workflows = [], currentGraph = null, layoutNodes = [], layoutLayers = 0;
  let selectedNode = null, simRunning = false;
  let isPanning = false, panSX = 0, panSY = 0, scale = 1;
  let autoPan = false;

  // Live execution state
  let wsConn = null;
  let wsRetryDelay = 1000;
  let nodeExecData = {};   // node_id -> { status, duration_ms, error, inputs, outputs }
  let logEvents = [];
  let logFilter = 'all';

  const $ = id => document.getElementById(id);

  // Core DOM
  const wfList = $('workflow-list');
  const graphTitle = $('graph-title'), statsBar = $('stats-bar');
  const statN = $('stat-nodes'), statE = $('stat-edges'), statL = $('stat-layers');
  const wrap = $('canvas-wrap'), svg = $('canvas'), gGroup = $('graph-group');
  const empty = $('empty-state'), loading = $('loading-overlay');
  const dPanel = $('detail-panel'), dTitle = $('detail-title'), dBody = $('detail-body');
  const dClose = $('detail-close');
  const bZI = $('btn-zoom-in'), bZO = $('btn-zoom-out'), bFit = $('btn-fit');
  const bExec = $('btn-execute'), bExp = $('btn-export'), zPct = $('zoom-pct');
  const bAutoPan = $('btn-autopan');
  const mmEl = $('minimap'), mmCvs = $('minimap-canvas'), mmVp = $('minimap-viewport');
  const sInput = $('search-input'), sResults = $('search-results');
  const tip = $('tooltip');

  // Live monitoring DOM
  const wsDot = $('ws-dot'), wsStatus = $('ws-status');
  const execLog = $('exec-log'), logToggle = $('log-toggle');
  const logList = $('log-list'), logClear = $('log-clear');

  // ── init ──
  async function init() {
    try {
      const r = await fetch('/api/workflows');
      const d = await r.json();
      workflows = d.workflows || [];
      renderSidebar();
    } catch { /* ignore */ }
    bindEvents();
    connectWS();
  }

  function renderSidebar() {
    wfList.innerHTML = '';
    workflows.forEach(w => {
      const li = document.createElement('li');
      li.dataset.id = w.id;
      li.innerHTML = `<span class="wf-icon">⬡</span><span>${esc(w.name)}</span>
      <span class="wf-meta"><span class="badge badge-nodes">${w.node_count}n</span>
      <span class="badge badge-edges">${w.edge_count}e</span></span>`;
      li.onclick = () => loadWorkflow(w.id);
      wfList.appendChild(li);
    });
  }

  // ── Load Workflow ──
  async function loadWorkflow(id) {
    wfList.querySelectorAll('li').forEach(l => l.classList.toggle('active', l.dataset.id === id));
    empty.classList.add('hidden');
    loading.classList.remove('hidden');
    currentGraph = null;
    nodeExecData = {};
    try {
      const r = await fetch(`/api/workflows/${id}`);
      currentGraph = await r.json();
      graphTitle.textContent = currentGraph.name;
      statsBar.classList.remove('hidden');
    } catch (e) {
      console.error(e);
      graphTitle.textContent = '';
      statsBar.classList.add('hidden');
      empty.classList.remove('hidden');
      return;
    } finally {
      loading.classList.add('hidden');
    }
    doLayout();
    // Fetch current execution state for catch-up
    try {
      const r = await fetch('/api/execution/state');
      const d = await r.json();
      if (d.states) {
        for (const [id, entry] of Object.entries(d.states)) {
          nodeExecData[id] = entry;
          applyNodeStatus(id, entry.status);
        }
      }
      if (d.sim_running) {
        bExec.textContent = '⏹ Running…'; bExec.classList.add('running');
        simRunning = true;
      }
    } catch { /* ignore */ }
  }

  // ── Layout ──
  function doLayout() {
    if (!currentGraph) return;
    const { nodes, edges } = currentGraph;
    if (!nodes.length) return;

    const adj = {}, radj = {}, inDeg = {};
    nodes.forEach(n => { adj[n.id] = []; radj[n.id] = []; inDeg[n.id] = 0; });
    edges.forEach(e => {
      if (adj[e.from] && radj[e.to]) {
        adj[e.from].push(e.to); radj[e.to].push(e.from);
        inDeg[e.to]++;
      }
    });
    const topo = [], q = [], dc = { ...inDeg };
    nodes.forEach(n => { if (dc[n.id] === 0) q.push(n.id); });
    let qi = 0;
    while (qi < q.length) { const c = q[qi++]; topo.push(c); for (const nx of adj[c]) { dc[nx]--; if (dc[nx] === 0) q.push(nx); } }
    nodes.forEach(n => { if (!topo.includes(n.id)) topo.push(n.id); });

    const layer = {};
    topo.forEach(id => {
      const p = radj[id] || [];
      layer[id] = p.length ? Math.max(...p.map(x => (layer[x] || 0) + 1)) : 0;
    });
    const groups = {};
    nodes.forEach(n => { const l = layer[n.id] || 0; (groups[l] = groups[l] || []).push(n); });
    const maxL = Math.max(0, ...Object.keys(groups).map(Number));
    layoutLayers = maxL + 1;

    for (let pass = 0; pass < 4; pass++) {
      for (let l = 1; l <= maxL; l++) {
        const g = groups[l] || [], prev = groups[l - 1] || [], pp = {};
        prev.forEach((n, i) => { pp[n.id] = i; });
        g.forEach(n => { const par = (radj[n.id] || []).filter(x => pp[x] !== undefined); n._b = par.length ? par.reduce((s, x) => s + pp[x], 0) / par.length : Infinity; });
        g.sort((a, b) => a._b - b._b); groups[l] = g;
      }
      for (let l = maxL - 1; l >= 0; l--) {
        const g = groups[l] || [], nxt = groups[l + 1] || [], np = {};
        nxt.forEach((n, i) => { np[n.id] = i; });
        g.forEach(n => { const ch = (adj[n.id] || []).filter(x => np[x] !== undefined); n._b = ch.length ? ch.reduce((s, x) => s + np[x], 0) / ch.length : Infinity; });
        g.sort((a, b) => a._b - b._b); groups[l] = g;
      }
    }

    layoutNodes = [];
    for (let l = 0; l <= maxL; l++) {
      const g = groups[l] || [];
      const tw = g.length * NODE_W + Math.max(0, g.length - 1) * NODE_GAP_X;
      const sx = -tw / 2 + NODE_W / 2;
      g.forEach((n, i) => {
        layoutNodes.push({ ...n, x: sx + i * (NODE_W + NODE_GAP_X), y: l * LAYER_GAP_Y, _layer: l });
      });
    }

    statN.textContent = nodes.length;
    statE.textContent = edges.length;
    statL.textContent = layoutLayers;
    renderGraph();
    fitView();
    updateMinimap();
  }

  // ── Render ──
  function renderGraph() {
    gGroup.innerHTML = '';
    if (!currentGraph) return;
    const nm = {};
    layoutNodes.forEach(n => { nm[n.id] = n; });

    currentGraph.edges.forEach((e, i) => {
      const f = nm[e.from], t = nm[e.to];
      if (!f || !t) return;
      const x1 = f.x, y1 = f.y + NODE_H / 2, x2 = t.x, y2 = t.y - NODE_H / 2;
      const cp = Math.max(Math.abs(y2 - y1) * 0.4, 20);
      const p = svgEl('path');
      p.setAttribute('d', `M${x1} ${y1} C${x1} ${y1 + cp},${x2} ${y2 - cp},${x2} ${y2}`);
      let cls = 'edge-line';
      if (e.edge_type === 'conditional') cls += ' conditional';
      else if (e.edge_type === 'error') cls += ' error';
      p.setAttribute('class', cls);
      p.dataset.from = e.from; p.dataset.to = e.to;
      p.style.animationDelay = `${i * 25}ms`;
      gGroup.appendChild(p);
      if (e.label) {
        const mx = (x1 + x2) / 2, my = (y1 + y2) / 2;
        const bg = svgEl('rect');
        const tw = e.label.length * 6 + 10;
        setA(bg, { x: mx - tw / 2, y: my - 7, width: tw, height: 14, rx: 3, class: 'edge-label-bg' });
        gGroup.appendChild(bg);
        const txt = svgEl('text');
        txt.setAttribute('x', mx); txt.setAttribute('y', my);
        txt.setAttribute('class', 'edge-label');
        txt.textContent = e.label;
        gGroup.appendChild(txt);
      }
    });

    layoutNodes.forEach((n, i) => {
      const g = svgEl('g');
      g.setAttribute('class', 'node-group');
      g.setAttribute('transform', `translate(${n.x - NODE_W / 2},${n.y - NODE_H / 2})`);
      g.dataset.id = n.id;
      g.style.animationDelay = `${i * 30}ms`;
      const m = TYPE_META[n.type] || TYPE_META.task;
      const s = makeShape(n.type, m);
      g.appendChild(s);
      const lb = svgEl('text');
      lb.setAttribute('x', NODE_W / 2); lb.setAttribute('y', NODE_H / 2 - 5);
      lb.setAttribute('class', 'node-label');
      lb.textContent = n.name.length > 18 ? n.name.slice(0, 17) + '…' : n.name;
      g.appendChild(lb);
      const bd = svgEl('text');
      bd.setAttribute('x', NODE_W / 2); bd.setAttribute('y', NODE_H / 2 + 9);
      bd.setAttribute('class', 'node-type-badge');
      bd.textContent = m.label;
      g.appendChild(bd);

      g.onclick = ev => { ev.stopPropagation(); openDetail(n); };
      g.onmouseenter = () => hlConnect(n.id);
      g.onmouseleave = clearHL;
      g.onmousemove = ev => showTip(ev, `${n.name} (${n.type})`);
      g.addEventListener('mouseleave', hideTip);
      gGroup.appendChild(g);
    });
  }

  function makeShape(type, m) {
    let s;
    if (type === 'start' || type === 'end') {
      s = svgEl('rect');
      setA(s, { x: 0, y: 0, width: NODE_W, height: NODE_H, rx: NODE_H / 2 });
    } else if (type === 'condition') {
      const cx = NODE_W / 2, cy = NODE_H / 2, dx = NODE_W / 2 + 6, dy = NODE_H / 2;
      s = svgEl('polygon');
      s.setAttribute('points', `${cx},${cy - dy} ${cx + dx},${cy} ${cx},${cy + dy} ${cx - dx},${cy}`);
    } else if (type === 'loop') {
      const ind = 18, cy = NODE_H / 2;
      s = svgEl('polygon');
      s.setAttribute('points', `${ind},0 ${NODE_W - ind},0 ${NODE_W},${cy} ${NODE_W - ind},${NODE_H} ${ind},${NODE_H} 0,${cy}`);
    } else if (type === 'parallel' || type === 'join') {
      const sk = 12;
      s = svgEl('polygon');
      s.setAttribute('points', `${sk},0 ${NODE_W},0 ${NODE_W - sk},${NODE_H} 0,${NODE_H}`);
    } else {
      s = svgEl('rect');
      setA(s, { x: 0, y: 0, width: NODE_W, height: NODE_H, rx: 8 });
    }
    s.setAttribute('class', 'node-shape');
    s.style.fill = m.fill; s.style.stroke = m.stroke; s.style.strokeWidth = '2';
    return s;
  }

  // ── Highlight ──
  function hlConnect(id) {
    if (simRunning) return;
    const cn = new Set([id]), ce = new Set();
    currentGraph.edges.forEach(e => {
      if (e.from === id || e.to === id) { cn.add(e.from); cn.add(e.to); ce.add(e.from + '->' + e.to); }
    });
    gGroup.querySelectorAll('.node-group').forEach(g => g.classList.toggle('dimmed', !cn.has(g.dataset.id)));
    gGroup.querySelectorAll('.edge-line').forEach(el => {
      const k = el.dataset.from + '->' + el.dataset.to;
      el.classList.toggle('dimmed', !ce.has(k));
      el.classList.toggle('highlighted', ce.has(k));
    });
  }
  function clearHL() {
    if (simRunning) return;
    gGroup.querySelectorAll('.dimmed,.highlighted').forEach(el => el.classList.remove('dimmed', 'highlighted'));
  }

  // ── Detail / Inspector ──
  function openDetail(n) {
    selectedNode = n; dPanel.classList.remove('hidden');
    const m = TYPE_META[n.type] || TYPE_META.task;
    dTitle.textContent = n.name;
    let h = `<div class="detail-section"><h3>Type</h3>
    <span class="detail-badge" style="background:${m.fill};color:${m.stroke};border:1px solid ${m.stroke}">
    ${n.type.toUpperCase()}</span></div>
    <div class="detail-section"><h3>ID</h3><p><code>${esc(n.id)}</code></p></div>
    <div class="detail-section"><h3>Layer</h3><p>${(n._layer || 0) + 1} / ${layoutLayers}</p></div>`;

    // Incoming / Outgoing edges
    const inc = currentGraph.edges.filter(e => e.to === n.id);
    const out = currentGraph.edges.filter(e => e.from === n.id);
    if (inc.length) {
      h += `<div class="detail-section"><h3>Incoming (${inc.length})</h3><ul class="detail-list">`;
      inc.forEach(e => { h += `<li data-node="${esc(e.from)}"><span class="arrow">←</span><code>${esc(e.from)}</code>${eTag(e.edge_type)}</li>`; });
      h += '</ul></div>';
    }
    if (out.length) {
      h += `<div class="detail-section"><h3>Outgoing (${out.length})</h3><ul class="detail-list">`;
      out.forEach(e => { h += `<li data-node="${esc(e.to)}"><span class="arrow">→</span><code>${esc(e.to)}</code>${e.label ? ` <em style="color:var(--text-muted)">${esc(e.label)}</em>` : ''}${eTag(e.edge_type)}</li>`; });
      h += '</ul></div>';
    }

    // Inspector v2: Execution data
    const ed = nodeExecData[n.id];
    if (ed) {
      const statusColors = { pending: '#8e99a4', running: '#1ABC9C', completed: '#3498DB', failed: '#E74C3C' };
      const sc = statusColors[ed.status] || '#8e99a4';
      h += `<div class="detail-section"><h3>Execution Status</h3>
      <span class="detail-badge" style="background:${sc}18;color:${sc};border:1px solid ${sc}">${(ed.status || 'pending').toUpperCase()}</span></div>`;
      if (ed.duration_ms != null) {
        h += `<div class="detail-section"><h3>Execution Time</h3><p><code>${ed.duration_ms}ms</code></p></div>`;
      }
      if (ed.inputs) {
        h += `<div class="detail-section"><h3>Input Data</h3>
        <details><summary style="cursor:pointer;font-size:11px;color:var(--text-muted)">Show JSON</summary>
        <pre style="font-family:var(--mono);font-size:10px;background:var(--bg-hover);padding:8px;border-radius:4px;overflow:auto;max-height:150px;margin-top:4px">${esc(JSON.stringify(ed.inputs, null, 2))}</pre>
        </details></div>`;
      }
      if (ed.outputs) {
        h += `<div class="detail-section"><h3>Output Data</h3>
        <details><summary style="cursor:pointer;font-size:11px;color:var(--text-muted)">Show JSON</summary>
        <pre style="font-family:var(--mono);font-size:10px;background:var(--bg-hover);padding:8px;border-radius:4px;overflow:auto;max-height:150px;margin-top:4px">${esc(JSON.stringify(ed.outputs, null, 2))}</pre>
        </details></div>`;
      }
      if (ed.error) {
        h += `<div class="detail-section"><h3>Error</h3>
        <div style="background:rgba(231,76,60,.08);border:1px solid rgba(231,76,60,.2);border-radius:6px;padding:8px 10px;color:#E74C3C;font-size:11px;font-family:var(--mono)">${esc(ed.error)}</div></div>`;
      }
    }

    dBody.innerHTML = h;
    dBody.querySelectorAll('li[data-node]').forEach(li => {
      li.onclick = () => { const t = layoutNodes.find(x => x.id === li.dataset.node); if (t) { panTo(t); openDetail(t); } };
    });
  }
  function eTag(t) {
    if (!t || t === 'normal') return '';
    const c = { conditional: 'background:var(--accent-yellow-dim);color:var(--accent-yellow)', error: 'background:var(--accent-red-dim);color:var(--accent-red)' };
    return `<span class="edge-tag" style="${c[t] || ''}">${t}</span>`;
  }
  function closeDetail() { selectedNode = null; dPanel.classList.add('hidden'); }

  // ================================================================
  // WEBSOCKET CLIENT
  // ================================================================

  function connectWS() {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    const url = `${proto}//${location.host}/ws`;
    wsConn = new WebSocket(url);
    setWSStatus('reconnecting');

    wsConn.onopen = () => {
      setWSStatus('connected');
      wsRetryDelay = 1000;
    };

    wsConn.onmessage = (evt) => {
      try {
        const data = JSON.parse(evt.data);
        handleWSMessage(data);
      } catch { /* ignore parse errors */ }
    };

    wsConn.onclose = () => {
      setWSStatus('disconnected');
      scheduleReconnect();
    };

    wsConn.onerror = () => {
      setWSStatus('disconnected');
    };
  }

  function scheduleReconnect() {
    setWSStatus('reconnecting');
    setTimeout(() => {
      wsRetryDelay = Math.min(wsRetryDelay * 2, 10000);
      connectWS();
    }, wsRetryDelay);
  }

  function setWSStatus(status) {
    wsDot.className = 'status-dot ' + status;
    const labels = { connected: 'Connected', reconnecting: 'Reconnecting…', disconnected: 'Disconnected' };
    wsStatus.textContent = labels[status] || status;
  }

  function handleWSMessage(data) {
    if (data.event_type === 'heartbeat') return;

    if (data.event_type === 'state_snapshot') {
      // Full state catch-up on connect
      if (data.states) {
        for (const [id, entry] of Object.entries(data.states)) {
          nodeExecData[id] = entry;
          applyNodeStatus(id, entry.status);
        }
      }
      return;
    }

    if (data.event_type === 'workflow_start') {
      simRunning = true;
      bExec.textContent = '⏹ Running…'; bExec.classList.add('running');
      // Reset all node states to pending
      nodeExecData = {};
      gGroup.querySelectorAll('.node-group').forEach(g => {
        g.classList.remove('status-running', 'status-completed', 'status-failed');
        g.classList.add('status-pending');
      });
      // Remove old duration badges
      gGroup.querySelectorAll('.node-duration-badge').forEach(el => el.remove());
      addLogEntry('workflow', 'started', null);
      return;
    }

    if (data.event_type === 'workflow_end') {
      simRunning = false;
      bExec.textContent = '▶ Execute'; bExec.classList.remove('running');
      addLogEntry('workflow', 'ended', null);
      return;
    }

    if (data.event_type === 'node_status' && data.node_id) {
      const status = data.status;
      nodeExecData[data.node_id] = {
        status,
        duration_ms: data.duration_ms || nodeExecData[data.node_id]?.duration_ms,
        error: data.error || nodeExecData[data.node_id]?.error,
        inputs: data.inputs || nodeExecData[data.node_id]?.inputs,
        outputs: data.outputs || nodeExecData[data.node_id]?.outputs,
      };
      applyNodeStatus(data.node_id, status);
      addLogEntry(data.node_id, status, data.duration_ms);

      // Auto-pan to running node
      if (autoPan && status === 'running') {
        const n = layoutNodes.find(x => x.id === data.node_id);
        if (n) panTo(n);
      }

      // Update detail panel if viewing this node
      if (selectedNode && selectedNode.id === data.node_id) {
        openDetail(selectedNode);
      }
    }
  }

  function applyNodeStatus(nodeId, status) {
    const g = gGroup.querySelector(`.node-group[data-id="${nodeId}"]`);
    if (!g) return;
    g.classList.remove('status-pending', 'status-running', 'status-completed', 'status-failed');
    g.classList.add(`status-${status}`);

    // Add duration badge on completion
    if ((status === 'completed' || status === 'failed') && nodeExecData[nodeId]?.duration_ms) {
      // Remove existing badge
      const existing = g.querySelector('.node-duration-badge');
      if (existing) existing.remove();

      const dur = svgEl('text');
      dur.setAttribute('x', NODE_W / 2);
      dur.setAttribute('y', NODE_H + 4);
      dur.setAttribute('class', 'node-duration-badge');
      dur.textContent = `${nodeExecData[nodeId].duration_ms}ms`;
      if (status === 'failed') dur.style.fill = '#E74C3C';
      g.appendChild(dur);
    }
  }

  // ── Execute ──
  async function triggerExecute() {
    if (!currentGraph) return;
    if (simRunning) return; // let the server handle completion
    try {
      await fetch(`/api/simulate?workflow_id=${encodeURIComponent(currentGraph.id)}`, { method: 'POST' });
    } catch (e) {
      console.error('Failed to start execution:', e);
    }
  }

  // ── Execution Log ──
  function addLogEntry(nodeId, status, durationMs) {
    const now = new Date();
    const ts = `${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}.${String(now.getMilliseconds()).padStart(3, '0')}`;
    const entry = { nodeId, status, ts, durationMs };
    logEvents.push(entry);
    if (logEvents.length > 500) logEvents.shift();

    const li = document.createElement('li');
    li.dataset.status = status;
    li.dataset.node = nodeId;
    li.innerHTML = `<span class="log-time">${ts}</span>
      <span class="log-node">${esc(nodeId)}</span>
      <span class="log-arrow">→</span>
      <span class="log-status ${status}">${status}</span>
      ${durationMs != null ? `<span class="log-dur">(${durationMs}ms)</span>` : ''}`;
    li.onclick = () => {
      const n = layoutNodes.find(x => x.id === nodeId);
      if (n) { panTo(n); openDetail(n); }
    };
    logList.appendChild(li);
    applyLogFilter();
    logList.scrollTop = logList.scrollHeight;
  }

  function applyLogFilter() {
    logList.querySelectorAll('li').forEach(li => {
      if (logFilter === 'all' || li.dataset.status === logFilter) {
        li.classList.remove('hidden');
      } else {
        li.classList.add('hidden');
      }
    });
  }

  function toggleLog() {
    execLog.classList.toggle('collapsed');
    execLog.classList.toggle('expanded');
  }

  // ── Export ──
  function exportSVG() {
    if (!currentGraph) return;
    const bb = gGroup.getBBox();
    const m = 50;
    const vx = bb.x - m, vy = bb.y - m;
    const vw = bb.width + m * 2, vh = bb.height + m * 2;
    const ns = 'http://www.w3.org/2000/svg';
    const out = document.createElementNS(ns, 'svg');
    out.setAttribute('xmlns', ns);
    out.setAttribute('viewBox', `${vx} ${vy} ${vw} ${vh}`);
    out.setAttribute('width', vw); out.setAttribute('height', vh);
    const style = document.createElementNS(ns, 'style');
    style.textContent = `@import url('https://fonts.googleapis.com/css2?family=Inter:wght@500;600;700&family=JetBrains+Mono:wght@500;600&display=swap'); text { font-family: 'Inter', system-ui, sans-serif; }`;
    out.appendChild(style);
    const bg = document.createElementNS(ns, 'rect');
    bg.setAttribute('x', vx); bg.setAttribute('y', vy);
    bg.setAttribute('width', vw); bg.setAttribute('height', vh);
    bg.setAttribute('fill', '#f4f5f8');
    out.appendChild(bg);
    const defs = document.createElementNS(ns, 'defs');
    [{ id: 'a1', fill: '#c0c4cc' }, { id: 'a2', fill: '#F1C40F' }, { id: 'a3', fill: '#E74C3C' }].forEach(mk => {
      const marker = document.createElementNS(ns, 'marker');
      setA(marker, { id: mk.id, markerWidth: 10, markerHeight: 7, refX: 10, refY: 3.5, orient: 'auto', markerUnits: 'strokeWidth' });
      const poly = document.createElementNS(ns, 'polygon');
      poly.setAttribute('points', '0 0, 10 3.5, 0 7'); poly.setAttribute('fill', mk.fill);
      marker.appendChild(poly); defs.appendChild(marker);
    });
    out.appendChild(defs);
    const g = document.createElementNS(ns, 'g');
    const nm = {}; layoutNodes.forEach(n => { nm[n.id] = n; });
    currentGraph.edges.forEach(e => {
      const f = nm[e.from], t = nm[e.to]; if (!f || !t) return;
      const x1 = f.x, y1 = f.y + NODE_H / 2, x2 = t.x, y2 = t.y - NODE_H / 2;
      const cp = Math.max(Math.abs(y2 - y1) * 0.4, 20);
      const p = document.createElementNS(ns, 'path');
      p.setAttribute('d', `M${x1} ${y1} C${x1} ${y1 + cp},${x2} ${y2 - cp},${x2} ${y2}`);
      p.setAttribute('fill', 'none');
      if (e.edge_type === 'conditional') { p.setAttribute('stroke', '#F1C40F'); p.setAttribute('stroke-width', '1.8'); p.setAttribute('stroke-dasharray', '8 5'); p.setAttribute('marker-end', 'url(#a2)'); }
      else if (e.edge_type === 'error') { p.setAttribute('stroke', '#E74C3C'); p.setAttribute('stroke-width', '1.5'); p.setAttribute('stroke-dasharray', '4 4'); p.setAttribute('marker-end', 'url(#a3)'); }
      else { p.setAttribute('stroke', '#c0c4cc'); p.setAttribute('stroke-width', '1.5'); p.setAttribute('marker-end', 'url(#a1)'); }
      g.appendChild(p);
    });
    layoutNodes.forEach(n => {
      const ng = document.createElementNS(ns, 'g');
      ng.setAttribute('transform', `translate(${n.x - NODE_W / 2},${n.y - NODE_H / 2})`);
      const meta = TYPE_META[n.type] || TYPE_META.task;
      const shape = makeShape(n.type, meta);
      shape.setAttribute('fill', meta.fill); shape.setAttribute('stroke', meta.stroke); shape.setAttribute('stroke-width', '2');
      ng.appendChild(shape);
      const lb = document.createElementNS(ns, 'text');
      setA(lb, { x: NODE_W / 2, y: NODE_H / 2 - 5 });
      lb.setAttribute('fill', '#2c3e50'); lb.setAttribute('font-family', "'Inter', sans-serif"); lb.setAttribute('font-size', '11'); lb.setAttribute('font-weight', '600'); lb.setAttribute('text-anchor', 'middle'); lb.setAttribute('dominant-baseline', 'central');
      lb.textContent = n.name.length > 18 ? n.name.slice(0, 17) + '…' : n.name;
      ng.appendChild(lb);
      const bd = document.createElementNS(ns, 'text');
      setA(bd, { x: NODE_W / 2, y: NODE_H / 2 + 9 });
      bd.setAttribute('fill', '#8e99a4'); bd.setAttribute('font-family', "'JetBrains Mono', monospace"); bd.setAttribute('font-size', '8'); bd.setAttribute('font-weight', '500'); bd.setAttribute('text-anchor', 'middle'); bd.setAttribute('dominant-baseline', 'hanging'); bd.setAttribute('letter-spacing', '0.06em');
      bd.textContent = meta.label;
      ng.appendChild(bd);
      g.appendChild(ng);
    });
    out.appendChild(g);
    const svgStr = new XMLSerializer().serializeToString(out);
    const blob = new Blob([svgStr], { type: 'image/svg+xml' });
    const u = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = u; a.download = `${currentGraph.id || 'wf'}.svg`; a.click();
    URL.revokeObjectURL(u);
  }

  // ── Search ──
  function doSearch(q) {
    q = q.trim().toLowerCase();
    if (!q || !currentGraph) { sResults.classList.add('hidden'); return; }
    const r = layoutNodes.filter(n => n.name.toLowerCase().includes(q) || n.id.includes(q) || n.type.includes(q));
    if (!r.length) { sResults.classList.add('hidden'); return; }
    sResults.classList.remove('hidden'); sResults.innerHTML = '';
    r.slice(0, 8).forEach(n => {
      const m = TYPE_META[n.type] || TYPE_META.task, li = document.createElement('li');
      li.innerHTML = `<span class="sr-type" style="background:${m.fill};color:${m.stroke};border:1px solid ${m.stroke}">${m.label}</span><span>${esc(n.name)}</span>`;
      li.onclick = () => { sInput.value = ''; sResults.classList.add('hidden'); panTo(n); openDetail(n); };
      sResults.appendChild(li);
    });
  }

  // ── Minimap ──
  function updateMinimap() {
    if (!layoutNodes.length) { mmEl.classList.remove('visible'); return; }
    mmEl.classList.add('visible');
    const ctx = mmCvs.getContext('2d'), cw = mmCvs.width, ch = mmCvs.height;
    ctx.clearRect(0, 0, cw, ch);
    const xs = layoutNodes.map(n => n.x), ys = layoutNodes.map(n => n.y);
    const mnX = Math.min(...xs) - NODE_W / 2 - 10, mxX = Math.max(...xs) + NODE_W / 2 + 10;
    const mnY = Math.min(...ys) - NODE_H / 2 - 10, mxY = Math.max(...ys) + NODE_H / 2 + 10;
    const gw = mxX - mnX || 1, gh = mxY - mnY || 1;
    const s = Math.min(cw / gw, ch / gh) * 0.88;
    const ox = (cw - gw * s) / 2 - mnX * s, oy = (ch - gh * s) / 2 - mnY * s;
    const nm = {}; layoutNodes.forEach(n => { nm[n.id] = n; });
    ctx.strokeStyle = '#c0c2cc'; ctx.lineWidth = .6;
    currentGraph.edges.forEach(e => { const f = nm[e.from], t = nm[e.to]; if (!f || !t) return; ctx.beginPath(); ctx.moveTo(f.x * s + ox, f.y * s + oy); ctx.lineTo(t.x * s + ox, t.y * s + oy); ctx.stroke(); });
    layoutNodes.forEach(n => { const m = TYPE_META[n.type] || TYPE_META.task; ctx.fillStyle = m.stroke; ctx.globalAlpha = .8; ctx.beginPath(); ctx.arc(n.x * s + ox, n.y * s + oy, 3, 0, Math.PI * 2); ctx.fill(); });
    ctx.globalAlpha = 1;
    const rect = wrap.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) {
      const vx = (vbX - mnX) * s + (cw - gw * s) / 2;
      const vy = (vbY - mnY) * s + (ch - gh * s) / 2;
      const vw = vbW * s, vh = vbH * s;
      mmVp.style.left = Math.max(0, vx) + 'px'; mmVp.style.top = Math.max(0, vy) + 'px';
      mmVp.style.width = Math.min(vw, cw) + 'px'; mmVp.style.height = Math.min(vh, ch) + 'px';
    }
  }

  // ── ViewBox camera ──
  let vbX = 0, vbY = 0, vbW = 800, vbH = 600;

  function applyTx() {
    gGroup.removeAttribute('transform');
    svg.setAttribute('viewBox', `${vbX} ${vbY} ${vbW} ${vbH}`);
    svg.setAttribute('preserveAspectRatio', 'xMidYMid meet');
    const rect = wrap.getBoundingClientRect();
    if (rect.width > 0) scale = rect.width / vbW;
    zPct.textContent = Math.round(scale * 100) + '%';
    updateMinimap();
  }

  function fitView() {
    if (!layoutNodes.length) return;
    const xs = layoutNodes.map(n => n.x), ys = layoutNodes.map(n => n.y);
    vbX = Math.min(...xs) - NODE_W / 2 - PAD;
    const x1 = Math.max(...xs) + NODE_W / 2 + PAD;
    vbY = Math.min(...ys) - NODE_H / 2 - PAD;
    const y1 = Math.max(...ys) + NODE_H / 2 + PAD;
    vbW = (x1 - vbX) || 1;
    vbH = (y1 - vbY) || 1;
    applyTx();
  }

  function panTo(n) { vbX = n.x - vbW / 2; vbY = n.y - vbH / 2; applyTx(); }
  function zoom(f, cx, cy) {
    const rect = wrap.getBoundingClientRect();
    const gx = vbX + (cx !== undefined ? (cx / rect.width) * vbW : vbW / 2);
    const gy = vbY + (cy !== undefined ? (cy / rect.height) * vbH : vbH / 2);
    const nw = vbW / f, nh = vbH / f;
    if (nw < 50 || nw > 10000) return;
    vbX = gx - (gx - vbX) * (nw / vbW);
    vbY = gy - (gy - vbY) * (nh / vbH);
    vbW = nw; vbH = nh;
    applyTx();
  }

  // ── Tooltip ──
  function showTip(ev, t) { tip.textContent = t; tip.classList.remove('hidden'); tip.style.left = (ev.clientX + 12) + 'px'; tip.style.top = (ev.clientY - 8) + 'px'; }
  function hideTip() { tip.classList.add('hidden'); }

  // ── Events ──
  function bindEvents() {
    bZI.onclick = () => zoom(1.25); bZO.onclick = () => zoom(1 / 1.25);
    bFit.onclick = fitView; bExec.onclick = triggerExecute; bExp.onclick = exportSVG;
    bAutoPan.onclick = () => { autoPan = !autoPan; bAutoPan.classList.toggle('active', autoPan); };
    wrap.addEventListener('wheel', e => { e.preventDefault(); const r = wrap.getBoundingClientRect(); zoom(e.deltaY < 0 ? 1.08 : 1 / 1.08, e.clientX - r.left, e.clientY - r.top); }, { passive: false });
    wrap.onmousedown = e => { if (e.target.closest('.node-group')) return; isPanning = true; panSX = e.clientX; panSY = e.clientY; wrap.style.cursor = 'grabbing'; };
    window.onmousemove = e => { if (!isPanning) return; const rect = wrap.getBoundingClientRect(); const dx = (e.clientX - panSX) * (vbW / rect.width); const dy = (e.clientY - panSY) * (vbH / rect.height); vbX -= dx; vbY -= dy; panSX = e.clientX; panSY = e.clientY; applyTx(); };
    window.onmouseup = () => { if (isPanning) { isPanning = false; wrap.style.cursor = ''; } };
    wrap.onclick = e => { if (!e.target.closest('.node-group')) closeDetail(); };
    dClose.onclick = closeDetail;
    sInput.oninput = () => doSearch(sInput.value);
    sInput.onfocus = () => doSearch(sInput.value);
    sInput.onblur = () => setTimeout(() => sResults.classList.add('hidden'), 150);

    // Log controls
    logToggle.onclick = toggleLog;
    logClear.onclick = () => { logEvents = []; logList.innerHTML = ''; };
    document.querySelectorAll('.log-filter').forEach(btn => {
      btn.onclick = () => {
        document.querySelectorAll('.log-filter').forEach(b => b.classList.remove('active'));
        btn.classList.add('active');
        logFilter = btn.dataset.filter;
        applyLogFilter();
      };
    });

    window.onkeydown = e => {
      if (document.activeElement === sInput) { if (e.key === 'Escape') { sInput.blur(); sResults.classList.add('hidden'); } return; }
      switch (e.key) {
        case 'Escape': closeDetail(); break;
        case '+': case '=': zoom(1.25); break;
        case '-': zoom(1 / 1.25); break;
        case '0': fitView(); break;
        case '/': e.preventDefault(); sInput.focus(); break;
        case 'x': case 'X': triggerExecute(); break;
        case 'e': case 'E': exportSVG(); break;
        case 'a': case 'A': autoPan = !autoPan; bAutoPan.classList.toggle('active', autoPan); break;
        case 'l': case 'L': toggleLog(); break;
      }
    };
    window.onresize = updateMinimap;
  }

  // ── Helpers ──
  function svgEl(t) { return document.createElementNS('http://www.w3.org/2000/svg', t); }
  function setA(el, o) { for (const [k, v] of Object.entries(o)) el.setAttribute(k, v); }
  function esc(s) { const d = document.createElement('div'); d.textContent = s; return d.innerHTML; }
  function pad(n) { return String(n).padStart(2, '0'); }

  init();
})();
