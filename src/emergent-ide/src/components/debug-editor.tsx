"use client";
import Link from "next/link";
import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { sceneHierarchy } from "@/lib/scene-hierarchy";
import type { BrainNode, NodeCatalogue } from "@/lib/brains";

type Instance = { id: string; name?: string; parent: string | null; spawned?: boolean };
type Mode = { name: string; brain: string; active: boolean; done: boolean; running?: string[] };
type Selected = { id: string; exists: boolean; position?: number[]; rotation_degrees?: number[]; scale?: number[]; velocity?: number[]; modes?: Mode[]; intent?: Record<string, unknown>; flight?: Record<string, unknown>; tower?: Record<string, unknown>; warnings?: string[] };
type Live = { tick: number; paused: boolean; speed: number; selected?: Selected; spawned?: { id: string; name: string; parent?: string }[] };
type Status = { id: string; state: string; error: string | null; log: string; live: Live | null };

const KEY_NAMES: Record<string, string> = { KeyW: "w", KeyA: "a", KeyS: "s", KeyD: "d", KeyQ: "q", KeyE: "e", Space: "space", ShiftLeft: "shift", ShiftRight: "shift", ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right" };
const SPEEDS = [0.25, 0.5, 1, 2, 4];
const TREE_VIEW = "emergent.debug.tree";
type TreeView = { font: number; gap: number };
const clamp = (value: number, low: number, high: number) => Math.min(high, Math.max(low, value));
// Scene graph text size and row spacing, remembered per browser. The server renders the defaults.
const treeViewListeners = new Set<() => void>();
let treeViewFallback: string | null = null;
function readTreeView() { try { return localStorage.getItem(TREE_VIEW); } catch { return treeViewFallback; } }
function subscribeTreeView(listener: () => void) { treeViewListeners.add(listener); return () => { treeViewListeners.delete(listener); }; }
function parseTreeView(raw: string | null): TreeView {
  try { const saved = JSON.parse(raw ?? "null") as TreeView | null; if (saved && Number.isFinite(saved.font) && Number.isFinite(saved.gap)) return { font: clamp(saved.font, 8, 16), gap: clamp(saved.gap, 0, 8) }; } catch { /* Defaults. */ }
  return { font: 11, gap: 4 };
}
function saveTreeView(view: TreeView) {
  treeViewFallback = JSON.stringify(view);
  try { localStorage.setItem(TREE_VIEW, treeViewFallback); } catch { /* Kept for this page only. */ }
  for (const listener of treeViewListeners) listener();
}

/** Debug run: the game streamed from the engine, with the scene graph, controls and live values. */
export function DebugEditor({ projectId, projectName, scenePath, scenes }: { projectId: string; projectName: string; scenePath: string; scenes: { path: string; name: string }[] }) {
  const [status, setStatus] = useState<Status | null>(null), [error, setError] = useState("");
  const [instances, setInstances] = useState<Instance[]>([]), [selected, setSelected] = useState<string | null>(null);
  const [focused, setFocused] = useState(false), [fps, setFps] = useState(0), [locked, setLocked] = useState(false);
  const treeViewRaw = useSyncExternalStore(subscribeTreeView, readTreeView, () => null);
  const treeView = useMemo(() => parseTreeView(treeViewRaw), [treeViewRaw]);
  const canvas = useRef<HTMLCanvasElement>(null), view = useRef<HTMLDivElement>(null), held = useRef(new Set<string>()), session = useRef<string | null>(null), motion = useRef({ dx: 0, dy: 0 });
  const id = status?.id ?? null;

  const command = useCallback(async (value: Record<string, unknown>) => {
    if (!session.current) return;
    const response = await fetch("/api/debug", { method: "PUT", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ projectId, id: session.current, command: value }) });
    const data = await response.json();
    if (!response.ok) setError(data.error ?? "Command failed."); else { setError(""); setStatus(data); }
  }, [projectId]);

  // Start the run; stop it when the page goes away.
  useEffect(() => {
    let active = true, started: string | null = null;
    // Deferred so a mount that is immediately undone (Strict Mode) never starts a run: two racing
    // starts can finish in either order, and the newer one replaces the other.
    const timer = setTimeout(() => fetch("/api/debug", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ projectId, path: scenePath }) })
      .then(async response => { const data = await response.json(); if (!response.ok) throw new Error(data.error ?? "Debug run failed to start."); started = data.id; if (active) { session.current = data.id; setStatus(data); } else void stop(data.id); })
      .catch(failure => { if (active) setError(failure instanceof Error ? failure.message : "Debug run failed to start."); }), 0);
    function stop(sessionId: string) { return fetch("/api/debug", { method: "DELETE", keepalive: true, headers: { "Content-Type": "application/json" }, body: JSON.stringify({ projectId, id: sessionId }) }).catch(() => undefined); }
    return () => { active = false; clearTimeout(timer); session.current = null; if (started) void stop(started); };
  }, [projectId, scenePath]);

  function changeTreeView(change: Partial<TreeView>) {
    saveTreeView({ font: clamp(change.font ?? treeView.font, 8, 16), gap: clamp(change.gap ?? treeView.gap, 0, 8) });
  }

  // Mouse motion while the pointer is locked to the game view, batched to about 30 reports a second.
  useEffect(() => {
    const changed = () => setLocked(document.pointerLockElement === view.current);
    document.addEventListener("pointerlockchange", changed);
    const timer = setInterval(() => {
      const { dx, dy } = motion.current;
      if (!dx && !dy) return;
      motion.current = { dx: 0, dy: 0 };
      void command({ op: "mouse", dx: Math.round(dx * 100) / 100, dy: Math.round(dy * 100) / 100 });
    }, 33);
    return () => { document.removeEventListener("pointerlockchange", changed); clearInterval(timer); };
  }, [command]);

  // Scene graph from the scene file.
  useEffect(() => {
    fetch(`/api/scenes/placement?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(scenePath)}`, { cache: "no-store" })
      .then(async response => { const data = await response.json(); if (response.ok) setInstances(data.instances.map((item: Instance) => ({ id: item.id, name: item.name, parent: item.parent }))); })
      .catch(() => undefined);
  }, [projectId, scenePath]);

  // Live values and run state.
  useEffect(() => {
    if (!id) return;
    const timer = setInterval(async () => {
      try { const response = await fetch(`/api/debug?projectId=${encodeURIComponent(projectId)}&id=${encodeURIComponent(id)}`, { cache: "no-store" }); const data = await response.json(); if (response.ok) setStatus(data); } catch { /* The next poll retries. */ }
    }, 250);
    return () => clearInterval(timer);
  }, [projectId, id]);

  // Frames.
  useEffect(() => {
    if (!id) return;
    const abort = new AbortController();
    let sequence = 0, count = 0, since = performance.now();
    (async () => {
      try {
        while (!abort.signal.aborted) {
          const response = await fetch(`/api/debug/frame?projectId=${encodeURIComponent(projectId)}&id=${encodeURIComponent(id)}&after=${sequence}`, { signal: abort.signal, cache: "no-store" });
          if (response.status === 204) continue;
          if (!response.ok) { const data = await response.json(); throw new Error(data.error ?? "The game stopped."); }
          const bitmap = await createImageBitmap(await response.blob());
          try { canvas.current?.getContext("2d")?.drawImage(bitmap, 0, 0, 960, 540); } finally { bitmap.close(); }
          sequence = Number(response.headers.get("X-Debug-Frame"));
          count++; const now = performance.now(); if (now - since >= 1000) { setFps(Math.round(count * 1000 / (now - since))); count = 0; since = now; }
        }
      } catch (failure) { if (!abort.signal.aborted) setError(failure instanceof Error ? failure.message : "The game stopped."); }
    })();
    return () => abort.abort();
  }, [projectId, id]);

  function select(instance: string | null) { setSelected(instance); void command({ op: "select", id: instance }); }
  function keys(event: React.KeyboardEvent, down: boolean) {
    const name = KEY_NAMES[event.code];
    if (!name) return;
    event.preventDefault();
    if (down === held.current.has(name)) return;
    if (down) held.current.add(name); else held.current.delete(name);
    void command({ op: "keys", held: [...held.current] });
  }
  function release() { setFocused(false); if (document.pointerLockElement === view.current) document.exitPointerLock(); if (held.current.size) { held.current.clear(); void command({ op: "keys", held: [] }); } }

  const live = status?.live, running = status?.state === "running";

  const spawned = live?.spawned;
  const all = useMemo<Instance[]>(() => {
    const known = new Set(instances.map(item => item.id));
    const extra = (spawned ?? []).map(item => ({ id: item.id, name: item.name || item.id, parent: item.parent ?? null, spawned: true }));
    for (const item of extra) known.add(item.id);
    // A parent the graph doesn't know (e.g. a model part) shows the object at the top level.
    return [...instances, ...extra.map(item => item.parent && !known.has(item.parent) ? { ...item, parent: null } : item)];
  }, [instances, spawned]);
  const tree = useMemo(() => { try { return sceneHierarchy(all); } catch { return all.map(instance => ({ instance, depth: 0 })); } }, [all]);
  const current = live?.selected && live.selected.id === selected ? live.selected : undefined;
  const names = useMemo<Names>(() => new Map(all.map(item => [item.id, item.name ?? item.id])), [all]);
  const name = (instance: string) => names.get(instance) ?? instance;
  return <main className="debug-shell">
    <header className="settings-topbar"><Link className="brand" href="/"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></Link><span className="settings-breadcrumb">{projectName} / <b>Debug</b> / {scenes.find(item => item.path === scenePath)?.name ?? scenePath}</span><Link href={`/?scene=${encodeURIComponent(scenePath)}`} className="quiet-button">Back to scene ↗</Link></header>
    <div className="debug-layout">
      <aside className="panel debug-graph" aria-label="Scene graph">
        <div className="panel-head"><h2>Scene graph</h2>
          <div className="debug-tree-size" role="group" aria-label="Scene graph text size and row spacing">
            <button type="button" title="Smaller text" aria-label="Smaller text" disabled={treeView.font <= 8} onClick={() => changeTreeView({ font: treeView.font - 1 })}>A−</button>
            <button type="button" title="Larger text" aria-label="Larger text" disabled={treeView.font >= 16} onClick={() => changeTreeView({ font: treeView.font + 1 })}>A+</button>
            <button type="button" title="Tighter rows" aria-label="Tighter rows" disabled={treeView.gap <= 0} onClick={() => changeTreeView({ gap: treeView.gap - 1 })}>☰−</button>
            <button type="button" title="Looser rows" aria-label="Looser rows" disabled={treeView.gap >= 8} onClick={() => changeTreeView({ gap: treeView.gap + 1 })}>☰+</button>
          </div>
        </div>
        <div className="debug-tree" style={{ "--debug-tree-font": `${treeView.font}px`, "--debug-tree-gap": `${treeView.gap}px` } as React.CSSProperties}>{tree.map(({ instance, depth }) => <button type="button" key={instance.id} className={`debug-tree-row${selected === instance.id ? " selected" : ""}${instance.spawned ? " spawned" : ""}`} style={{ paddingLeft: 8 + depth * 14 }} aria-pressed={selected === instance.id} title={instance.spawned ? "Spawned while running" : undefined} onClick={() => select(selected === instance.id ? null : instance.id)}>{instance.name ?? instance.id}</button>)}</div>
      </aside>
      <section className="panel debug-stage">
        <div className="debug-toolbar">
          <button type="button" className="settings-save" disabled={!running} onClick={() => void command({ op: live?.paused ? "resume" : "pause" })}>{live?.paused ? "▶ Resume" : "❚❚ Pause"}</button>
          <button type="button" className="quiet-button" disabled={!running} title="Advance one tick (pauses)" onClick={() => void command({ op: "step", ticks: 1 })}>Step</button>
          <button type="button" className="quiet-button" disabled={!running} title="Advance one second of game time (pauses)" onClick={() => void command({ op: "step", ticks: 60 })}>+1 s</button>
          <label className="debug-speed">Speed <select value={live?.speed ?? 1} disabled={!running} onChange={event => void command({ op: "speed", factor: Number(event.target.value) })}>{SPEEDS.map(speed => <option key={speed} value={speed}>{speed}×</option>)}</select></label>
          <span className="debug-readout">{status ? status.state === "building" ? "Building the game…" : `tick ${live?.tick ?? 0} · ${((live?.tick ?? 0) / 60).toFixed(1)} s · ${fps} fps` : "Starting…"}</span>
        </div>
        {(error || status?.error) && <pre className="brain-error debug-error" role="alert">{error || status?.error}</pre>}
        <div ref={view} className={`debug-view${focused ? " focused" : ""}`} tabIndex={0} aria-label="Running game. Click to send keys (WASD, arrows, Space, Shift, Q, E) and mouse movement; Escape releases the mouse." onFocus={() => setFocused(true)} onBlur={release} onKeyDown={event => keys(event, true)} onKeyUp={event => keys(event, false)}
          onClick={() => { if (running && !locked) void view.current?.requestPointerLock()?.catch?.(() => undefined); }}
          onMouseMove={event => { if (locked) { motion.current.dx += event.movementX; motion.current.dy += event.movementY; } }}>
          <canvas ref={canvas} width={960} height={540} />
          {running && <span className="debug-hint">{locked ? "Mouse captured · Escape releases" : focused ? "Keys go to the game · click to capture the mouse" : "Click the game to send keys and mouse"}</span>}
        </div>
      </section>
      <aside className="panel debug-inspector" aria-label="Live values">
        <div className="panel-head"><h2>{selected ? name(selected) : "Live values"}</h2></div>
        <div className="debug-values">
          {!selected ? <p className="transform-note">Select an object in the scene graph to see its live values and running brain.</p>
            : !current ? <p className="transform-note">Waiting for the game…</p>
              : !current.exists ? <p className="brain-error">This object no longer exists in the running game.</p>
                : <SelectedValues projectId={projectId} value={current} names={names} />}
        </div>
      </aside>
    </div>
  </main>;
}

/** Instance IDs in engine text (e.g. flight orders) read as object names. */
type Names = Map<string, string>;
const named = (text: string, names: Names) => text.replace(/[a-z0-9][a-z0-9_.-]{7,}/gi, word => names.get(word) ?? word);
const show = (value: unknown, names: Names): string => typeof value === "string" ? named(value, names) : Array.isArray(value) ? value.map(n => typeof n === "number" ? n.toFixed(2) : String(n)).join(", ") : typeof value === "number" ? (Number.isInteger(value) ? String(value) : value.toFixed(2)) : String(value);
function Table({ title, values, names }: { title: string; values: Record<string, unknown>; names: Names }) {
  return <section className="debug-section"><h3>{title}</h3><dl>{Object.entries(values).filter(([, value]) => value !== undefined).map(([key, value]) => <div key={key}><dt>{key.replaceAll("_", " ")}</dt><dd>{show(value, names)}</dd></div>)}</dl></section>;
}

function SelectedValues({ projectId, value, names }: { projectId: string; value: Selected; names: Names }) {
  return <>
    {value.warnings?.map(warning => <p key={warning} className="brain-error" role="alert">{warning}</p>)}
    <Table names={names} title="Transform" values={{ position: value.position, rotation: value.rotation_degrees, scale: value.scale, velocity: value.velocity }} />
    {value.modes?.map(mode => <BrainTrace key={mode.name} projectId={projectId} mode={mode} />)}
    {value.intent && <Table names={names} title="Brain intent" values={value.intent} />}
    {value.flight && <Table names={names} title="Flight" values={value.flight} />}
    {value.tower && <Table names={names} title="Tower" values={value.tower} />}
  </>;
}

const brainCache = new Map<string, Promise<BrainNode[]>>();
let catalogue: Promise<NodeCatalogue | null> | null = null;
/** The brain tree read-only, with the nodes that produced output this tick highlighted. */
function BrainTrace({ projectId, mode }: { projectId: string; mode: Mode }) {
  const [nodes, setNodes] = useState<BrainNode[]>([]), [labels, setLabels] = useState<NodeCatalogue | null>(null);
  useEffect(() => {
    let active = true;
    const key = `${projectId}:${mode.brain}`;
    if (!brainCache.has(key)) brainCache.set(key, fetch(`/api/brains?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(mode.brain)}`, { cache: "no-store" }).then(r => r.json()).then(data => data.brain?.nodes ?? []).catch(() => []));
    catalogue ??= fetch(`/api/brains?projectId=${encodeURIComponent(projectId)}`, { cache: "no-store" }).then(r => r.json()).then(data => data.catalogue ?? null).catch(() => null);
    void brainCache.get(key)!.then(list => { if (active) setNodes(list); });
    void catalogue.then(list => { if (active) setLabels(list); });
    return () => { active = false; };
  }, [projectId, mode.brain]);
  const running = new Set(mode.running ?? []);
  // A reference node runs when any node of the brain it embeds does (ids `ref/inner`).
  const isRunning = (id: string) => running.has(id) || [...running].some(r => r.startsWith(`${id}/`));
  const children = (parent?: string) => nodes.filter(node => parent ? node.parent === parent : !node.parent);
  const label = (node: BrainNode) => (typeof node.fields.label === "string" && node.fields.label) || labels?.nodes.find(spec => spec.id === node.type)?.label || node.type;
  const render = (node: BrainNode, depth: number): React.ReactNode => <li key={node.id}>
    <span className={`debug-node${isRunning(node.id) ? " running" : ""}`} title={node.id}>{label(node)}</span>
    {depth < 32 && children(node.id).length > 0 && <ul>{children(node.id).map(child => render(child, depth + 1))}</ul>}
  </li>;
  return <section className="debug-section"><h3>Brain mode <b>{mode.name}</b> <small className={mode.active ? "debug-on" : ""}>{mode.active ? "running" : mode.done ? "finished" : "off"}</small></h3>
    <p className="debug-brain-path">{mode.brain.replace(/^.*\//, "")}</p>
    {nodes.length ? <ul className="debug-brain">{children(undefined).map(root => render(root, 0))}</ul> : <p className="transform-note">Loading brain…</p>}
  </section>;
}
