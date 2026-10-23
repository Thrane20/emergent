"use client";

import { useMemo, useState } from "react";
import type { AssetKind, ProjectAsset, ProjectSnapshot } from "@/lib/project";

type Props = { project: ProjectSnapshot };

const icons: Record<AssetKind, string> = {
  scene: "◇",
  entity: "◌",
  data: "▤",
  audio: "⌁",
  model: "▱",
  texture: "▧",
  file: "·",
};

const descriptions: Record<AssetKind, string> = {
  scene: "Authored scene reference. Current runtime assembly remains generated Rust.",
  entity: "Reusable project-owned entity composition data.",
  data: "Project configuration and authored values.",
  audio: "Declared asset folder. Audio decoding is not available yet.",
  model: "Declared asset folder. Model importing is not available yet.",
  texture: "Declared asset folder. Texture importing is not available yet.",
  file: "Project asset reference.",
};

export function ProjectStudio({ project }: Props) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<ProjectAsset | undefined>(project.assets[0]);
  const [activeRail, setActiveRail] = useState("project");
  const assets = useMemo(
    () => project.assets.filter((asset) => asset.path.toLowerCase().includes(query.toLowerCase())),
    [project.assets, query],
  );

  return (
    <main className="studio-shell">
      <header className="topbar">
        <div className="brand"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></div>
        <div className="project-pill"><span>◫</span>{project.name}<span className="muted">⌄</span></div>
        <div className="top-status"><span className="pulse" />Project mode <i /> {project.assets.length} references <i /> manifest loaded</div>
        <div className="top-actions"><button className="quiet-button">⌘ K</button><button className="avatar">IS</button></div>
      </header>

      <aside className="rail" aria-label="IDE sections">
        {[["project", "▦", "Project"], ["graph", "⌘", "Graphs"], ["build", "▷", "Build"], ["notes", "◫", "Notes"]].map(([id, icon, label]) => (
          <button key={id} className={activeRail === id ? "rail-button active" : "rail-button"} onClick={() => setActiveRail(id)} title={label}>{icon}</button>
        ))}
        <span className="rail-spacer" />
        <button className="rail-button" title="Settings">⚙</button>
      </aside>

      <section className="workspace">
        <aside className="panel project-panel">
          <div className="panel-head"><div><span className="eyebrow">PROJECT MAP</span><h2>References</h2></div><button className="square-button" title="Refresh project">↻</button></div>
          <label className="search"><span>⌕</span><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Find scenes, entities, data…" /></label>
          <div className="tree-root"><span className="tree-dot" />{project.name}<small>ACTIVE</small></div>
          <div className="asset-tree">
            {assets.map((asset) => <AssetRow asset={asset} selected={selected?.path === asset.path} onSelect={() => setSelected(asset)} key={asset.path} />)}
            {assets.length === 0 && <p className="empty">No project reference matches “{query}”.</p>}
          </div>
          <div className="panel-footer"><span className="eyebrow">PROJECT ROOT</span><code>{project.root}</code></div>
        </aside>

        <section className="canvas-panel">
          <div className="canvas-toolbar"><div className="crumb"><span>PROJECT</span><b>/</b><strong>{selected?.path ?? project.entryScene}</strong></div><div className="view-switch"><button className="selected">Overview</button><button>Graph</button><button>Source</button></div></div>
          <div className="canvas-grid">
            <div className="signal-ring"><span /><span /><b>Project<br />mode</b></div>
            <div className="canvas-copy"><span className="eyebrow">AUTHORED REFERENCE</span><h1>{selected?.name ?? "Choose a project reference"}</h1><p>{selected ? descriptions[selected.kind] : "Select a reference from the project map."}</p></div>
            <div className="stat-card primary-stat"><span>ENTRY SCENE</span><strong>{project.entryScene.replace("assets/scenes/", "")}</strong><em>Manifest-declared</em></div>
            <div className="stat-card"><span>ASSET ROOTS</span><strong>assets/</strong><em>Durable project-relative paths</em></div>
            <div className="stat-card"><span>RUNTIME KEYS</span><strong>Not stored</strong><em>SlotMap identities are runtime-only</em></div>
          </div>
          <div className="canvas-footer"><span><i className="green-dot" />Reference graph is read-only</span><span>Use generated Rust to assemble the current runtime scene.</span></div>
        </section>

        <aside className="panel details-panel">
          <div className="panel-head"><div><span className="eyebrow">REFERENCE DETAILS</span><h2>{selected?.name ?? "No selection"}</h2></div><span className="kind-badge">{selected?.kind ?? "—"}</span></div>
          {selected ? <>
            <dl className="details"><div><dt>Path</dt><dd><code>{selected.path}</code></dd></div><div><dt>Kind</dt><dd>{selected.kind}</dd></div><div><dt>Size</dt><dd>{formatBytes(selected.bytes)}</dd></div><div><dt>Contract</dt><dd>{descriptions[selected.kind]}</dd></div></dl>
            <button className="open-button" onClick={() => navigator.clipboard?.writeText(selected.path)}>Copy reference path <span>→</span></button>
          </> : <p className="empty">Choose an asset reference to inspect it.</p>}
          <div className="outputs"><span className="eyebrow">DECLARED OUTPUTS</span>{project.outputs.map((output) => <div className="output" key={output.id}><span className={output.enabled ? "output-dot enabled" : "output-dot"} /><div><b>{output.id}</b><small>{output.os} · {output.architecture} · {output.profile}</small></div><em>{output.enabled ? "enabled" : "declared"}</em></div>)}</div>
        </aside>

        <section className="diagnostics"><div><span className="terminal-caret">›</span><strong>Project mode</strong><span>Manifest and asset references loaded from disk.</span></div><div><span className="diagnostic-ok">●</span> No mutations performed <b>•</b> {project.id}</div></section>
      </section>
    </main>
  );
}

function AssetRow({ asset, selected, onSelect }: { asset: ProjectAsset; selected: boolean; onSelect: () => void }) {
  return <button className={selected ? "asset-row selected" : "asset-row"} onClick={onSelect}><span className={`asset-icon ${asset.kind}`}>{icons[asset.kind]}</span><span className="asset-name">{asset.name}<small>{asset.path.replace(`/${asset.name}`, "")}</small></span><span className="asset-size">{formatBytes(asset.bytes)}</span></button>;
}

function formatBytes(bytes: number) { return bytes === 0 ? "folder" : `${Math.max(1, Math.ceil(bytes / 1024))} KB`; }
