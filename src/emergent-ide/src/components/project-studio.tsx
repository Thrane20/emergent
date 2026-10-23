"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import type {
  ProjectConnection,
  ProjectFile,
  ProjectFileKind,
  ProjectSnapshot,
} from "@/lib/project";

type Props = { project: ProjectSnapshot };
type View = "overview" | "connections" | "source";

const icons: Record<ProjectFileKind, string> = {
  manifest: "◆",
  scene: "◇",
  entity: "◌",
  data: "▤",
  audio: "⌁",
  model: "▱",
  texture: "▧",
  source: "rs",
  documentation: "¶",
  file: "·",
};

const descriptions: Record<ProjectFileKind, string> = {
  manifest: "Versioned project identity, asset roots, entry scene, and declared outputs.",
  scene: "Authored scene reference. Runtime construction currently uses its generated Rust builder.",
  entity: "Reusable project-owned entity composition data.",
  data: "Project configuration read through the public Emergent project boundary.",
  audio: "Declared audio reference. Runtime audio decoding is not available yet.",
  model: "Declared model reference. Runtime model importing is not available yet.",
  texture: "Declared texture reference. Runtime texture importing is not available yet.",
  source: "Game-owned or generated Rust source compiled as part of this project.",
  documentation: "Project documentation. It is visible in the map but is not a runtime asset.",
  file: "Project file reference.",
};

export function ProjectStudio({ project }: Props) {
  const router = useRouter();
  const searchRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [selectedPath, setSelectedPath] = useState(project.entryScene);
  const [view, setView] = useState<View>("overview");
  const selected = project.files.find((file) => file.path === selectedPath) ?? project.files[0];
  const connected = connectionsFor(project.connections, project.files, selected.path);
  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return needle
      ? project.files.filter((file) => `${file.name} ${file.path} ${file.kind}`.toLowerCase().includes(needle))
      : project.files;
  }, [project.files, query]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const selectFile = (file: ProjectFile, nextView?: View) => {
    setSelectedPath(file.path);
    if (nextView) setView(nextView);
  };

  return (
    <main className="studio-shell">
      <header className="topbar">
        <div className="brand"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></div>
        <div className="project-pill"><span>◫</span>{project.name}<span className="muted">LOCAL</span></div>
        <div className="top-status"><span className="pulse" />Linked project <i /> {project.files.length} files <i /> {project.connections.length} connections</div>
        <div className="top-actions"><button className="quiet-button" onClick={() => searchRef.current?.focus()}>⌘ K</button><span className="avatar">FP</span></div>
      </header>

      <aside className="rail" aria-label="IDE sections">
        <button className="rail-button active" title="Project map" onClick={() => setView("overview")}>▦</button>
        <button className="rail-button" title="Connections" onClick={() => setView("connections")}>⌘</button>
        <button className="rail-button" title="Source" onClick={() => setView("source")}>⌗</button>
        <span className="rail-spacer" />
        <button className="rail-button" title="Reload project from disk" onClick={() => router.refresh()}>↻</button>
      </aside>

      <section className="workspace">
        <aside className="panel project-panel">
          <div className="panel-head"><div><span className="eyebrow">PROJECT MAP</span><h2>Files &amp; assets</h2></div><button className="square-button" title="Reload project from disk" onClick={() => router.refresh()}>↻</button></div>
          <label className="search"><span>⌕</span><input ref={searchRef} value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Find assets and source…" /></label>
          <div className="tree-root"><span className="tree-dot" />{project.name}<small>LINKED</small></div>
          <div className="asset-tree">
            <FileGroup title="Project" files={filtered.filter((file) => file.kind === "manifest")} selected={selected.path} onSelect={selectFile} />
            <FileGroup title="Authored assets" files={filtered.filter((file) => file.authored && file.kind !== "manifest")} selected={selected.path} onSelect={selectFile} />
            <FileGroup title="Project source" files={filtered.filter((file) => !file.authored && file.kind === "source")} selected={selected.path} onSelect={selectFile} />
            {filtered.length === 0 && <p className="empty">No project file matches “{query}”.</p>}
          </div>
          <div className="panel-footer"><span className="eyebrow">LINKED ROOT</span><code>{project.root}</code></div>
        </aside>

        <section className="canvas-panel">
          <div className="canvas-toolbar">
            <div className="crumb"><span>{selected.authored ? "AUTHORED" : selected.generated ? "GENERATED" : "PROJECT"}</span><b>/</b><strong>{selected.path}</strong></div>
            <div className="view-switch">{(["overview", "connections", "source"] as View[]).map((item) => <button key={item} className={view === item ? "selected" : ""} onClick={() => setView(item)}>{capitalize(item)}</button>)}</div>
          </div>
          {view === "overview" && <Overview project={project} file={selected} connected={connected} onSelect={selectFile} />}
          {view === "connections" && <ConnectionView project={project} file={selected} connected={connected} onSelect={selectFile} />}
          {view === "source" && <SourceView file={selected} />}
          <div className="canvas-footer"><span><i className="green-dot" />Live from the linked project folder</span><span>Runtime SlotMap handles are intentionally excluded.</span></div>
        </section>

        <aside className="panel details-panel">
          <div className="panel-head"><div><span className="eyebrow">FILE DETAILS</span><h2>{selected.name}</h2></div><span className="kind-badge">{selected.kind}</span></div>
          <dl className="details">
            <div><dt>Project path</dt><dd><code>{selected.path}</code></dd></div>
            <div><dt>Ownership</dt><dd>{selected.generated ? "Generator-owned Rust" : selected.authored ? "Authored project input" : "Game-owned source"}</dd></div>
            <div><dt>Size &amp; modified</dt><dd>{formatBytes(selected.bytes)} · {formatDate(selected.modifiedAt)}</dd></div>
            <div><dt>Contract</dt><dd>{descriptions[selected.kind]}</dd></div>
          </dl>
          <button className="open-button" onClick={() => navigator.clipboard?.writeText(selected.path)}>Copy project path <span>→</span></button>
          <div className="connection-summary"><span className="eyebrow">CONNECTIONS</span>{connected.length === 0 ? <p className="empty compact">No known file references this asset yet.</p> : connected.map(({ connection, peer, direction }) => <button key={connection.id} onClick={() => selectFile(peer, "connections")}><span className={`connection-direction ${direction}`}>{direction === "outgoing" ? "→" : "←"}</span><span><b>{connection.label}</b><small>{peer.path}</small></span></button>)}</div>
          <div className="outputs"><span className="eyebrow">DECLARED OUTPUTS</span>{project.outputs.map((output) => <div className="output" key={output.id}><span className={output.enabled ? "output-dot enabled" : "output-dot"} /><div><b>{output.id}</b><small>{output.os} · {output.architecture} · {output.profile}</small></div><em>{output.enabled ? "enabled" : "declared"}</em></div>)}</div>
        </aside>

        <section className="diagnostics"><div><span className="terminal-caret">›</span><strong>Linked project</strong><span>{project.root}</span></div><div><span className="diagnostic-ok">●</span> Read-only inspection <b>•</b> {project.id}</div></section>
      </section>
    </main>
  );
}

function Overview({ project, file, connected, onSelect }: { project: ProjectSnapshot; file: ProjectFile; connected: ConnectedFile[]; onSelect: (file: ProjectFile, view?: View) => void }) {
  return <div className="canvas-grid"><div className={`file-orbit ${file.kind}`}><span>{icons[file.kind]}</span><b>{file.kind}</b></div><div className="canvas-copy"><span className="eyebrow">{file.authored ? "DURABLE PROJECT FILE" : file.generated ? "GENERATED IMPLEMENTATION" : "PROJECT CODE"}</span><h1>{file.name}</h1><p>{descriptions[file.kind]}</p></div><button className="stat-card primary-stat interactive" onClick={() => onSelect(file, "connections")}><span>KNOWN CONNECTIONS</span><strong>{connected.length}</strong><em>{connected.length ? "Inspect incoming and outgoing edges" : "Currently unreferenced"}</em></button><button className="stat-card interactive" onClick={() => onSelect(project.files.find((candidate) => candidate.path === project.entryScene) ?? file)}><span>ENTRY SCENE</span><strong>{project.entryScene.replace("assets/scenes/", "")}</strong><em>Declared by emergent.toml</em></button><div className="stat-card"><span>RUNTIME IDENTITY</span><strong>Resolved later</strong><em>No SlotMap keys are stored in project files</em></div></div>;
}

function ConnectionView({ project, file, connected, onSelect }: { project: ProjectSnapshot; file: ProjectFile; connected: ConnectedFile[]; onSelect: (file: ProjectFile, view?: View) => void }) {
  return <div className="connection-canvas"><div className="connection-center"><span>{icons[file.kind]}</span><strong>{file.name}</strong><small>{file.path}</small></div>{connected.length === 0 ? <div className="connection-empty"><b>No known edges</b><span>This file is present in the project but is not referenced by another discovered file.</span></div> : <div className="connection-list">{connected.map(({ connection, peer, direction }) => <button key={connection.id} onClick={() => onSelect(peer, "connections")}><span className="edge-label">{direction === "outgoing" ? connection.label : `used by · ${connection.label}`}</span><b>{peer.name}</b><small>{peer.path}</small><i>{direction === "outgoing" ? "→" : "←"}</i></button>)}</div>}<div className="graph-key"><span><i className="edge authored" /> authored dependency</span><span><i className="edge source" /> source relationship</span><span>{project.connections.length} project edges</span></div></div>;
}

function SourceView({ file }: { file: ProjectFile }) {
  return <div className="source-view"><div className="source-head"><span>{file.path}</span><b>{file.content ? `${file.content.split("\n").length} lines` : "binary or unavailable"}</b></div>{file.content ? <pre><code>{file.content}</code></pre> : <div className="connection-empty"><b>No text preview</b><span>The IDE keeps the reference visible without claiming it can decode this asset type.</span></div>}</div>;
}

function FileGroup({ title, files, selected, onSelect }: { title: string; files: ProjectFile[]; selected: string; onSelect: (file: ProjectFile) => void }) {
  if (files.length === 0) return null;
  return <section className="file-group"><h3>{title}<span>{files.length}</span></h3>{files.map((file) => <button className={selected === file.path ? "asset-row selected" : "asset-row"} onClick={() => onSelect(file)} key={file.path}><span className={`asset-icon ${file.kind}`}>{icons[file.kind]}</span><span className="asset-name">{file.name}<small>{file.path.replace(`/${file.name}`, "")}</small></span><span className="asset-size">{formatBytes(file.bytes)}</span></button>)}</section>;
}

type ConnectedFile = { connection: ProjectConnection; peer: ProjectFile; direction: "incoming" | "outgoing" };

function connectionsFor(connections: ProjectConnection[], files: ProjectFile[], path: string): ConnectedFile[] {
  const connected: ConnectedFile[] = [];
  for (const connection of connections) {
    const direction = connection.source === path
      ? "outgoing"
      : connection.target === path
        ? "incoming"
        : undefined;
    if (!direction) continue;
    const peerPath = direction === "outgoing" ? connection.target : connection.source;
    const peer = files.find((file) => file.path === peerPath);
    if (peer) connected.push({ connection, direction, peer });
  }
  return connected;
}

function capitalize(value: string) { return `${value[0].toUpperCase()}${value.slice(1)}`; }
function formatBytes(bytes: number) { return bytes === 0 ? "empty" : bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KB`; }
function formatDate(value: string) { return new Intl.DateTimeFormat("en-AU", { day: "numeric", month: "short", year: "numeric" }).format(new Date(value)); }
