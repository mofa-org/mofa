/* ================================================================
   MoFA Workflow Visualizer — Production Client v3
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
  let selectedNode = null, simRunning = false, simTimer = null;
  let viewX = 0, viewY = 0, scale = 1;
  let isPanning = false, panSX = 0, panSY = 0;

  const $ = id => document.getElementById(id);
  const wfList = $('workflow-list'), statusText = $('status-text');
  const graphTitle = $('graph-title'), statsBar = $('stats-bar');
  const statN = $('stat-nodes'), statE = $('stat-edges'), statL = $('stat-layers');
  const wrap = $('canvas-wrap'), svg = $('canvas'), gGroup = $('graph-group');
  const empty = $('empty-state'), loading = $('loading-overlay');
  const dPanel = $('detail-panel'), dTitle = $('detail-title'), dBody = $('detail-body');
  const dClose = $('detail-close');
  const bZI = $('btn-zoom-in'), bZO = $('btn-zoom-out'), bFit = $('btn-fit');
  const bSim = $('btn-simulate'), bExp = $('btn-export'), zPct = $('zoom-pct');
  const mmEl = $('minimap'), mmCvs = $('minimap-canvas'), mmVp = $('minimap-viewport');
  const sInput = $('search-input'), sResults = $('search-results');
  const tip = $('tooltip');

  // ── Init ──
  async function init() {
    try {
      const r = await fetch('/api/workflows');
      const d = await r.json();
      workflows = d.workflows || [];
      renderSidebar();
      statusText.textContent = `${workflows.length} workflow(s)`;
    } catch { statusText.textContent = 'Error'; }
    bindEvents();
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
    stopSim(); closeDetail();
    currentGraph = null;
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
    // Layout immediately — viewBox approach doesn't need DOM timing
    doLayout();
  }

  // ── Layout (Kahn's topo sort + longest path) ──
  function doLayout() {
    if (!currentGraph) return;
    const { nodes, edges, start_node } = currentGraph;
    if (!nodes.length) return;

    const adj = {}, radj = {}, inDeg = {};
    nodes.forEach(n => { adj[n.id] = []; radj[n.id] = []; inDeg[n.id] = 0; });
    edges.forEach(e => {
      if (adj[e.from] && radj[e.to]) {
        adj[e.from].push(e.to); radj[e.to].push(e.from);
        inDeg[e.to]++;
      }
    });

    // Kahn's BFS
    const topo = [], q = [], dc = { ...inDeg };
    nodes.forEach(n => { if (dc[n.id] === 0) q.push(n.id); });
    let qi = 0;
    while (qi < q.length) { const c = q[qi++]; topo.push(c); for (const nx of adj[c]) { dc[nx]--; if (dc[nx] === 0) q.push(nx); } }
    nodes.forEach(n => { if (!topo.includes(n.id)) topo.push(n.id); });

    // Longest-path layers
    const layer = {};
    topo.forEach(id => {
      const p = radj[id] || [];
      layer[id] = p.length ? Math.max(...p.map(x => (layer[x] || 0) + 1)) : 0;
    });

    // Group & sort
    const groups = {};
    nodes.forEach(n => { const l = layer[n.id] || 0; (groups[l] = groups[l] || []).push(n); });
    const maxL = Math.max(0, ...Object.keys(groups).map(Number));
    layoutLayers = maxL + 1;

    // Barycenter crossing reduction
    for (let pass = 0; pass < 4; pass++) {
      for (let l = 1; l <= maxL; l++) {
        const g = groups[l] || [], prev = groups[l - 1] || [], pp = {};
        prev.forEach((n, i) => { pp[n.id] = i; });
        g.forEach(n => {
          const par = (radj[n.id] || []).filter(x => pp[x] !== undefined);
          n._b = par.length ? par.reduce((s, x) => s + pp[x], 0) / par.length : Infinity;
        });
        g.sort((a, b) => a._b - b._b); groups[l] = g;
      }
      for (let l = maxL - 1; l >= 0; l--) {
        const g = groups[l] || [], nxt = groups[l + 1] || [], np = {};
        nxt.forEach((n, i) => { np[n.id] = i; });
        g.forEach(n => {
          const ch = (adj[n.id] || []).filter(x => np[x] !== undefined);
          n._b = ch.length ? ch.reduce((s, x) => s + np[x], 0) / ch.length : Infinity;
        });
        g.sort((a, b) => a._b - b._b); groups[l] = g;
      }
    }

    // Assign coords
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
    fitView(); // sets SVG viewBox — always works, no DOM dependency
    updateMinimap();
  }

  // ── Render ──
  function renderGraph() {
    gGroup.innerHTML = '';
    if (!currentGraph) return;
    const nm = {};
    layoutNodes.forEach(n => { nm[n.id] = n; });

    // Edges
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

    // Nodes
    layoutNodes.forEach((n, i) => {
      const g = svgEl('g');
      g.setAttribute('class', 'node-group');
      g.setAttribute('transform', `translate(${n.x - NODE_W / 2},${n.y - NODE_H / 2})`);
      g.dataset.id = n.id;
      g.style.animationDelay = `${i * 30}ms`;
      const m = TYPE_META[n.type] || TYPE_META.task;

      // Shape
      const s = makeShape(n.type, m);
      g.appendChild(s);

      // Label
      const lb = svgEl('text');
      lb.setAttribute('x', NODE_W / 2); lb.setAttribute('y', NODE_H / 2 - 5);
      lb.setAttribute('class', 'node-label');
      lb.textContent = n.name.length > 18 ? n.name.slice(0, 17) + '…' : n.name;
      g.appendChild(lb);

      // Type badge
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

    // viewBox will be set by fitView() called after renderGraph()
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
    gGroup.querySelectorAll('.node-group').forEach(g => {
      g.classList.toggle('dimmed', !cn.has(g.dataset.id));
    });
    gGroup.querySelectorAll('.edge-line').forEach(el => {
      const k = el.dataset.from + '->' + el.dataset.to;
      el.classList.toggle('dimmed', !ce.has(k));
      el.classList.toggle('highlighted', ce.has(k));
    });
  }
  function clearHL() {
    if (simRunning) return;
    gGroup.querySelectorAll('.dimmed,.highlighted').forEach(el => { el.classList.remove('dimmed', 'highlighted'); });
  }

  // ── Detail ──
  function openDetail(n) {
    selectedNode = n; dPanel.classList.remove('hidden');
    const m = TYPE_META[n.type] || TYPE_META.task;
    dTitle.textContent = n.name;
    let h = `<div class="detail-section"><h3>Type</h3>
    <span class="detail-badge" style="background:${m.fill};color:${m.stroke};border:1px solid ${m.stroke}">
    ${n.type.toUpperCase()}</span></div>
    <div class="detail-section"><h3>ID</h3><p><code>${esc(n.id)}</code></p></div>
    <div class="detail-section"><h3>Layer</h3><p>${(n._layer || 0) + 1} / ${layoutLayers}</p></div>`;
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

  // ── Simulation ──
  function toggleSim() { simRunning ? stopSim() : startSim(); }
  function startSim() {
    if (!currentGraph || !layoutNodes.length) return;
    simRunning = true; bSim.textContent = '⏹ Stop'; bSim.classList.add('running');
    clearHL();
    const a = {}; layoutNodes.forEach(n => { a[n.id] = []; });
    currentGraph.edges.forEach(e => { if (a[e.from]) a[e.from].push(e.to); });
    const order = [], vis = new Set(), bq = [currentGraph.start_node || layoutNodes[0].id];
    vis.add(bq[0]);
    while (bq.length) { const c = bq.shift(); order.push(c); for (const nx of (a[c] || [])) { if (!vis.has(nx)) { vis.add(nx); bq.push(nx); } } }
    layoutNodes.forEach(n => { if (!vis.has(n.id)) order.push(n.id); });
    const allN = gGroup.querySelectorAll('.node-group'), allE = gGroup.querySelectorAll('.edge-line');
    allN.forEach(g => g.classList.add('dimmed')); allE.forEach(el => el.classList.add('dimmed'));
    let step = 0;
    function adv() {
      if (step >= order.length || !simRunning) { stopSim(); return; }
      const id = order[step];
      allN.forEach(g => { if (g.dataset.id === id) { g.classList.remove('dimmed'); g.classList.add('active-sim'); } });
      allE.forEach(el => { if (el.dataset.to === id) { el.classList.remove('dimmed'); el.classList.add('active-sim'); } });
      step++; simTimer = setTimeout(adv, 550);
    }
    adv();
  }
  function stopSim() {
    simRunning = false; if (simTimer) { clearTimeout(simTimer); simTimer = null; }
    bSim.textContent = '▶ Simulate'; bSim.classList.remove('running');
    gGroup.querySelectorAll('.dimmed,.active-sim').forEach(el => el.classList.remove('dimmed', 'active-sim'));
  }

  // ── Export ──
  function exportSVG() {
    if (!currentGraph) return;
    const bb = gGroup.getBBox();
    const m = 50;
    const vx = bb.x - m, vy = bb.y - m;
    const vw = bb.width + m * 2, vh = bb.height + m * 2;

    // Build a clean SVG from scratch with inlined styles
    const ns = 'http://www.w3.org/2000/svg';
    const out = document.createElementNS(ns, 'svg');
    out.setAttribute('xmlns', ns);
    out.setAttribute('viewBox', `${vx} ${vy} ${vw} ${vh}`);
    out.setAttribute('width', vw);
    out.setAttribute('height', vh);

    // Embedded style for fonts
    const style = document.createElementNS(ns, 'style');
    style.textContent = `
      @import url('https://fonts.googleapis.com/css2?family=Inter:wght@500;600;700&family=JetBrains+Mono:wght@500;600&display=swap');
      text { font-family: 'Inter', system-ui, sans-serif; }
    `;
    out.appendChild(style);

    // White background
    const bg = document.createElementNS(ns, 'rect');
    bg.setAttribute('x', vx); bg.setAttribute('y', vy);
    bg.setAttribute('width', vw); bg.setAttribute('height', vh);
    bg.setAttribute('fill', '#f4f5f8');
    out.appendChild(bg);

    // Re-create defs (markers)
    const defs = document.createElementNS(ns, 'defs');
    const markers = [
      { id: 'exp-arrow', fill: '#c0c4cc' },
      { id: 'exp-arrow-cond', fill: '#F1C40F' },
      { id: 'exp-arrow-err', fill: '#E74C3C' },
    ];
    markers.forEach(mk => {
      const marker = document.createElementNS(ns, 'marker');
      setA(marker, { id: mk.id, markerWidth: 10, markerHeight: 7, refX: 10, refY: 3.5, orient: 'auto', markerUnits: 'strokeWidth' });
      const poly = document.createElementNS(ns, 'polygon');
      poly.setAttribute('points', '0 0, 10 3.5, 0 7');
      poly.setAttribute('fill', mk.fill);
      marker.appendChild(poly);
      defs.appendChild(marker);
    });
    out.appendChild(defs);

    const g = document.createElementNS(ns, 'g');
    const nm = {};
    layoutNodes.forEach(n => { nm[n.id] = n; });

    // Draw edges with inline styles
    currentGraph.edges.forEach(e => {
      const f = nm[e.from], t = nm[e.to];
      if (!f || !t) return;
      const x1 = f.x, y1 = f.y + NODE_H / 2, x2 = t.x, y2 = t.y - NODE_H / 2;
      const cp = Math.max(Math.abs(y2 - y1) * 0.4, 20);
      const p = document.createElementNS(ns, 'path');
      p.setAttribute('d', `M${x1} ${y1} C${x1} ${y1 + cp},${x2} ${y2 - cp},${x2} ${y2}`);
      p.setAttribute('fill', 'none');
      if (e.edge_type === 'conditional') {
        p.setAttribute('stroke', '#F1C40F');
        p.setAttribute('stroke-width', '1.8');
        p.setAttribute('stroke-dasharray', '8 5');
        p.setAttribute('marker-end', 'url(#exp-arrow-cond)');
      } else if (e.edge_type === 'error') {
        p.setAttribute('stroke', '#E74C3C');
        p.setAttribute('stroke-width', '1.5');
        p.setAttribute('stroke-dasharray', '4 4');
        p.setAttribute('marker-end', 'url(#exp-arrow-err)');
      } else {
        p.setAttribute('stroke', '#c0c4cc');
        p.setAttribute('stroke-width', '1.5');
        p.setAttribute('marker-end', 'url(#exp-arrow)');
      }
      g.appendChild(p);

      // Edge label
      if (e.label) {
        const mx = (x1 + x2) / 2, my = (y1 + y2) / 2;
        const tw = e.label.length * 6.5 + 12;
        const lbg = document.createElementNS(ns, 'rect');
        setA(lbg, { x: mx - tw / 2, y: my - 8, width: tw, height: 16, rx: 3 });
        lbg.setAttribute('fill', '#f4f5f8');
        g.appendChild(lbg);
        const lt = document.createElementNS(ns, 'text');
        setA(lt, { x: mx, y: my + 1 });
        lt.setAttribute('fill', '#5a6a7a');
        lt.setAttribute('font-family', "'JetBrains Mono', monospace");
        lt.setAttribute('font-size', '9');
        lt.setAttribute('font-weight', '500');
        lt.setAttribute('text-anchor', 'middle');
        lt.setAttribute('dominant-baseline', 'central');
        lt.textContent = e.label;
        g.appendChild(lt);
      }
    });

    // Draw nodes with inline styles
    layoutNodes.forEach(n => {
      const ng = document.createElementNS(ns, 'g');
      ng.setAttribute('transform', `translate(${n.x - NODE_W / 2},${n.y - NODE_H / 2})`);
      const meta = TYPE_META[n.type] || TYPE_META.task;

      // Shape
      const shape = makeShape(n.type, meta);
      shape.setAttribute('fill', meta.fill);
      shape.setAttribute('stroke', meta.stroke);
      shape.setAttribute('stroke-width', '2');
      ng.appendChild(shape);

      // Label
      const lb = document.createElementNS(ns, 'text');
      setA(lb, { x: NODE_W / 2, y: NODE_H / 2 - 5 });
      lb.setAttribute('fill', '#2c3e50');
      lb.setAttribute('font-family', "'Inter', sans-serif");
      lb.setAttribute('font-size', '11');
      lb.setAttribute('font-weight', '600');
      lb.setAttribute('text-anchor', 'middle');
      lb.setAttribute('dominant-baseline', 'central');
      lb.textContent = n.name.length > 18 ? n.name.slice(0, 17) + '…' : n.name;
      ng.appendChild(lb);

      // Type badge
      const bd = document.createElementNS(ns, 'text');
      setA(bd, { x: NODE_W / 2, y: NODE_H / 2 + 9 });
      bd.setAttribute('fill', '#8e99a4');
      bd.setAttribute('font-family', "'JetBrains Mono', monospace");
      bd.setAttribute('font-size', '8');
      bd.setAttribute('font-weight', '500');
      bd.setAttribute('text-anchor', 'middle');
      bd.setAttribute('dominant-baseline', 'hanging');
      bd.setAttribute('letter-spacing', '0.06em');
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
      // Map the current viewBox to minimap space
      const vx = (vbX - mnX) * s + (cw - gw * s) / 2;
      const vy = (vbY - mnY) * s + (ch - gh * s) / 2;
      const vw = vbW * s, vh = vbH * s;
      mmVp.style.left = Math.max(0, vx) + 'px'; mmVp.style.top = Math.max(0, vy) + 'px';
      mmVp.style.width = Math.min(vw, cw) + 'px'; mmVp.style.height = Math.min(vh, ch) + 'px';
    }
  }

  // ── ViewBox-based camera system ──
  // viewBox = [vbX, vbY, vbW, vbH] — defines visible area in graph coordinates
  let vbX = 0, vbY = 0, vbW = 800, vbH = 600;

  function applyTx() {
    // Remove any transform on the group — viewBox handles everything
    gGroup.removeAttribute('transform');
    svg.setAttribute('viewBox', `${vbX} ${vbY} ${vbW} ${vbH}`);
    svg.setAttribute('preserveAspectRatio', 'xMidYMid meet');
    // Compute effective scale for display
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

  function panTo(n) {
    // Center the viewBox on node n
    vbX = n.x - vbW / 2;
    vbY = n.y - vbH / 2;
    applyTx();
  }
  function zoom(f, cx, cy) {
    // cx, cy are in screen pixels — convert to graph coords
    const rect = wrap.getBoundingClientRect();
    const gx = vbX + (cx !== undefined ? (cx / rect.width) * vbW : vbW / 2);
    const gy = vbY + (cy !== undefined ? (cy / rect.height) * vbH : vbH / 2);
    const nw = vbW / f, nh = vbH / f;
    // Clamp zoom
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
    bFit.onclick = fitView; bSim.onclick = toggleSim; bExp.onclick = exportSVG;
    wrap.addEventListener('wheel', e => { e.preventDefault(); const r = wrap.getBoundingClientRect(); zoom(e.deltaY < 0 ? 1.08 : 1 / 1.08, e.clientX - r.left, e.clientY - r.top); }, { passive: false });
    wrap.onmousedown = e => { if (e.target.closest('.node-group')) return; isPanning = true; panSX = e.clientX; panSY = e.clientY; wrap.style.cursor = 'grabbing'; };
    window.onmousemove = e => { if (!isPanning) return; const rect = wrap.getBoundingClientRect(); const dx = (e.clientX - panSX) * (vbW / rect.width); const dy = (e.clientY - panSY) * (vbH / rect.height); vbX -= dx; vbY -= dy; panSX = e.clientX; panSY = e.clientY; applyTx(); };
    window.onmouseup = () => { if (isPanning) { isPanning = false; wrap.style.cursor = ''; } };
    wrap.onclick = e => { if (!e.target.closest('.node-group')) closeDetail(); };
    dClose.onclick = closeDetail;
    sInput.oninput = () => doSearch(sInput.value);
    sInput.onfocus = () => doSearch(sInput.value);
    sInput.onblur = () => setTimeout(() => sResults.classList.add('hidden'), 150);
    window.onkeydown = e => {
      if (document.activeElement === sInput) { if (e.key === 'Escape') { sInput.blur(); sResults.classList.add('hidden'); } return; }
      switch (e.key) {
        case 'Escape': closeDetail(); stopSim(); break;
        case '+': case '=': zoom(1.25); break; case '-': zoom(1 / 1.25); break;
        case '0': fitView(); break; case '/': e.preventDefault(); sInput.focus(); break;
        case 's': case 'S': toggleSim(); break; case 'e': case 'E': exportSVG(); break;
      }
    };
    window.onresize = updateMinimap;
  }

  function svgEl(t) { return document.createElementNS('http://www.w3.org/2000/svg', t); }
  function setA(el, o) { for (const [k, v] of Object.entries(o)) el.setAttribute(k, v); }
  function esc(s) { const d = document.createElement('div'); d.textContent = s; return d.innerHTML; }

  init();
})();
