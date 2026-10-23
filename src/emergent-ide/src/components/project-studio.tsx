"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { AddAssetModal } from "@/components/add-asset-modal";
import { ProjectChat } from "@/components/project-chat";
import { ProjectRunControl } from "@/components/project-run-control";
import { StatementsPanel } from "@/components/statements-panel";
import { ModelPreview } from "@/components/model-preview";
import { projectScenes, type ScenePreview } from "@/lib/project-scenes";
import type { PublicAISettings } from "@/lib/ai-settings";
import type {
  ProjectConnection,
  ProjectFile,
  ProjectFileKind,
  ProjectSnapshot,
} from "@/lib/project";

type Props = { project: ProjectSnapshot; aiSettings: PublicAISettings };
type View = "overview" | "connections" | "source" | "scene";

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

export function ProjectStudio({ project, aiSettings }: Props) {
  const router = useRouter();
  const searchRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [selectedPath, setSelectedPath] = useState(project.entryScene);
  const [view, setView] = useState<View>("overview");
  const scenes = useMemo(() => projectScenes(project), [project]);
  const [scenePath, setScenePath] = useState(project.entryScene);
  const [loadedScene, setLoadedScene] = useState<ScenePreview | null>(null);
  const [sceneError, setSceneError] = useState("");
  const [sceneLoading, setSceneLoading] = useState(false);
  useEffect(() => () => { if (loadedScene) URL.revokeObjectURL(loadedScene.frame); }, [loadedScene]);
  const selected = project.files.find((file) => file.path === selectedPath) ?? project.files[0];
  const connected = connectionsFor(project.connections, project.files, selected.path);
  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return needle
      ? project.files.filter((file) => `${file.name} ${file.path} ${file.kind}`.toLowerCase().includes(needle))
      : project.files;
  }, [project.files, query]);
  const fileTree = useMemo(() => buildFileTree(filtered), [filtered]);

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
    else if (view === "scene") setView("overview");
  };

  async function loadScene() {
    setSceneLoading(true); setSceneError("");
    try {
      const path = scenes.some((scene) => scene.path === scenePath) ? scenePath : scenes[0]?.path;
      const response = await fetch("/api/scenes", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ projectId: project.id, path }) });
      if (!response.ok) { const result = await response.json(); throw new Error(result.error ?? "Scene could not be loaded."); }
      const frame = URL.createObjectURL(await response.blob());
      setLoadedScene({ path: path!, name: scenes.find((scene) => scene.path === path)!.name, frame }); setSelectedPath(path!); setView("scene");
    } catch (error) { setSceneError(error instanceof Error ? error.message : "Scene could not be loaded."); }
    finally { setSceneLoading(false); }
  }

  return (
    <main className="studio-shell">
      <header className="topbar">
        <div className="brand"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></div>
        <div className="project-pill"><span>◫</span>{project.name}<span className="muted">LOCAL</span></div>
        <div className="scene-selector"><label htmlFor="scene-picker">Scene viewer</label><select id="scene-picker" value={scenes.some((scene) => scene.path === scenePath) ? scenePath : scenes[0]?.path ?? ""} onChange={(event) => setScenePath(event.target.value)} disabled={!scenes.length || sceneLoading}>{!scenes.length && <option value="">No scenes</option>}{scenes.map((scene) => <option key={scene.path} value={scene.path}>{scene.name}</option>)}</select><button onClick={() => void loadScene()} disabled={!scenes.length || sceneLoading}>{sceneLoading ? "Loading…" : "Load"}</button></div>
        <div className="top-actions"><ProjectRunControl projectId={project.id} /><button className="quiet-button" onClick={() => searchRef.current?.focus()}>⌘ K</button><span className="avatar">FP</span></div>
      </header>

      <aside className="rail" aria-label="IDE sections">
        <button className="rail-button active" title="Project map" onClick={() => setView("overview")}>▦</button>
        <button className="rail-button" title="Connections" onClick={() => setView("connections")}>⌘</button>
        <button className="rail-button" title="Source" onClick={() => setView("source")}>⌗</button>
        <Link className="rail-button" href="/objects" title="Object Creator" aria-label="Object Creator">◇</Link>
        <span className="rail-spacer" />
        <Link className="rail-button" href="/settings" title="AI settings">⚙</Link>
        <button className="rail-button" title="Reload project from disk" onClick={() => router.refresh()}>↻</button>
      </aside>

      <section className="workspace">
        <div className="left-column">
          <aside className="panel project-panel">
            <div className="panel-head"><h2>Files &amp; assets</h2><button className="square-button" title="Reload project from disk" onClick={() => router.refresh()}>↻</button></div>
            <label className="search"><span>⌕</span><input ref={searchRef} value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Find…" /></label>
            <div className="asset-tree project-tree">
              {filtered.length > 0
                ? <details open className="tree-branch tree-project"><summary>{project.name}</summary><TreeContents node={fileTree} selected={selected.path} onSelect={selectFile} /></details>
                : <p className="empty">No match for “{query}”.</p>}
            </div>
            <AddAssetModal projectId={project.id} onCreated={(path) => {
              setQuery("");
              setSelectedPath(path);
              setView("overview");
              router.refresh();
            }} />
          </aside>
          <section className="panel tool-placeholder" aria-label="Reserved tool space">
            <span className="placeholder-mark">+</span>
            <div><span className="eyebrow">TOOL SPACE</span><h3>Reserved for what comes next</h3></div>
          </section>
        </div>

        <section className="canvas-panel">
          <div className="canvas-toolbar">
            <div className="crumb"><span>{selected.authored ? "AUTHORED" : selected.generated ? "GENERATED" : "PROJECT"}</span><b>/</b><strong>{selected.path}</strong></div>
            <div className="view-switch">{([...(loadedScene ? ["scene" as const] : []), "overview", "connections", "source"] as View[]).map((item) => <button key={item} className={view === item ? "selected" : ""} onClick={() => setView(item)}>{capitalize(item)}</button>)}</div>
          </div>
          {sceneError && <p className="scene-load-error" role="alert">{sceneError}</p>}
          {loadedScene && <section className="scene-canvas-slot" hidden={view !== "scene"} aria-label="Rust-rendered scene preview">
            {/* eslint-disable-next-line @next/next/no-img-element -- native renderer PNG delivered through an object URL */}
            <img className="native-scene-frame" src={loadedScene.frame} alt={`Rust-rendered scene: ${loadedScene.name}`} />
            <div className="scene-preview-caption"><span>{loadedScene.name} · Rust / wgpu · tick 0 · time stopped</span><span>Load to refresh</span></div>
          </section>}
          {view === "overview" && <Overview project={project} file={selected} connected={connected} onSelect={selectFile} />}
          {view === "connections" && <ConnectionView project={project} file={selected} connected={connected} onSelect={selectFile} />}
          {view === "source" && <SourceView file={selected} />}
          <ProjectChat
            projectId={project.id}
            selectedFile={selected}
            aiSettings={aiSettings}
          />
          <div className="canvas-footer"><span><i className="green-dot" />Live from the linked project folder</span><span>Runtime SlotMap handles are intentionally excluded.</span></div>
        </section>

        <aside className="right-column">
          <section className="panel details-panel">
            <div className="panel-head"><div><span className="eyebrow">SELECTED</span><h2>{selected.name}</h2></div><span className="kind-badge">{selected.kind}</span></div>
            <div className="connection-summary"><span className="eyebrow">CONNECTIONS</span>{connected.length === 0 ? <p className="empty compact">No known connections.</p> : connected.map(({ connection, peer, direction }) => <button key={connection.id} onClick={() => selectFile(peer, "connections")}><span className={`connection-direction ${direction}`}>{direction === "outgoing" ? "→" : "←"}</span><span><b>{connection.label}</b><small>{peer.name}</small></span></button>)}</div>
            <div className="outputs"><span className="eyebrow">DECLARED OUTPUTS</span>{project.outputs.map((output) => <div className="output" key={output.id}><span className={output.enabled ? "output-dot enabled" : "output-dot"} /><div><b>{output.id}</b><small>{output.os} · {output.architecture} · {output.profile}</small></div><em>{output.enabled ? "enabled" : "declared"}</em></div>)}</div>
          </section>
          <StatementsPanel key={selected.path} projectId={project.id} subject={`file:${selected.path}`} subjectLabel={selected.name} aiConfigured={aiSettings.configured} />
        </aside>

        <section className="diagnostics"><div><span className="terminal-caret">›</span><strong>Linked project</strong><span>{project.root}</span></div><div><span className="diagnostic-ok">●</span> Project asset authoring <b>•</b> {project.id}</div></section>
      </section>
    </main>
  );
}

function Overview({ project, file, connected, onSelect }: { project: ProjectSnapshot; file: ProjectFile; connected: ConnectedFile[]; onSelect: (file: ProjectFile, view?: View) => void }) {
  let modelPath = file.path.endsWith(".glb") ? file.path : null;
  if (file.name === "object.asset.json" && file.content) {
    try { const descriptor = JSON.parse(file.content); if (typeof descriptor.model === "string") modelPath = descriptor.model; } catch { /* Invalid descriptors still have a source view. */ }
  }
  if (modelPath) return <div className="model-overview"><ModelPreview key={modelPath} name={file.name} src={`/api/project-asset?projectId=${encodeURIComponent(project.id)}&path=${encodeURIComponent(modelPath)}`} /></div>;
  return <div className="canvas-grid"><div className={`file-orbit ${file.kind}`}><span>{icons[file.kind]}</span><b>{file.kind}</b></div><div className="canvas-copy"><span className="eyebrow">{file.authored ? "DURABLE PROJECT FILE" : file.generated ? "GENERATED IMPLEMENTATION" : "PROJECT CODE"}</span><h1>{file.name}</h1><p>{descriptions[file.kind]}</p></div><button className="stat-card primary-stat interactive" onClick={() => onSelect(file, "connections")}><span>KNOWN CONNECTIONS</span><strong>{connected.length}</strong><em>{connected.length ? "Inspect incoming and outgoing edges" : "Currently unreferenced"}</em></button><button className="stat-card interactive" onClick={() => onSelect(project.files.find((candidate) => candidate.path === project.entryScene) ?? file)}><span>ENTRY SCENE</span><strong>{project.entryScene.replace("assets/scenes/", "")}</strong><em>Declared by emergent.toml</em></button><div className="stat-card"><span>RUNTIME IDENTITY</span><strong>Resolved later</strong><em>No SlotMap keys are stored in project files</em></div></div>;
}

function ConnectionView({ project, file, connected, onSelect }: { project: ProjectSnapshot; file: ProjectFile; connected: ConnectedFile[]; onSelect: (file: ProjectFile, view?: View) => void }) {
  return <div className="connection-canvas"><div className="connection-center"><span>{icons[file.kind]}</span><strong>{file.name}</strong><small>{file.path}</small></div>{connected.length === 0 ? <div className="connection-empty"><b>No known edges</b><span>This file is present in the project but is not referenced by another discovered file.</span></div> : <div className="connection-list">{connected.map(({ connection, peer, direction }) => <button key={connection.id} onClick={() => onSelect(peer, "connections")}><span className="edge-label">{direction === "outgoing" ? connection.label : `used by · ${connection.label}`}</span><b>{peer.name}</b><small>{peer.path}</small><i>{direction === "outgoing" ? "→" : "←"}</i></button>)}</div>}<div className="graph-key"><span><i className="edge authored" /> authored dependency</span><span><i className="edge source" /> source relationship</span><span>{project.connections.length} project edges</span></div></div>;
}

function SourceView({ file }: { file: ProjectFile }) {
  return <div className="source-view"><div className="source-head"><span>{file.path}</span><b>{file.content ? `${file.content.split("\n").length} lines` : "binary or unavailable"}</b></div>{file.content ? <pre><code>{file.content}</code></pre> : <div className="connection-empty"><b>No text preview</b><span>The IDE keeps the reference visible without claiming it can decode this asset type.</span></div>}</div>;
}

type FileTree = { folders: Map<string, FileTree>; files: ProjectFile[] };

function buildFileTree(files: ProjectFile[]): FileTree {
  const root: FileTree = { folders: new Map(), files: [] };
  for (const file of files) {
    const parts = file.path.split("/");
    let node = root;
    for (const folder of parts.slice(0, -1)) {
      const existing = node.folders.get(folder);
      if (existing) {
        node = existing;
      } else {
        const created: FileTree = { folders: new Map(), files: [] };
        node.folders.set(folder, created);
        node = created;
      }
    }
    node.files.push(file);
  }
  return root;
}

function TreeContents({ node, selected, onSelect }: { node: FileTree; selected: string; onSelect: (file: ProjectFile) => void }) {
  const folders = [...node.folders.entries()].sort(([left], [right]) => left.localeCompare(right));
  const files = [...node.files].sort((left, right) => left.name.localeCompare(right.name));
  return <div className="tree-children">
    {folders.map(([name, child]) => <details open className="tree-branch" key={name}><summary>{name}</summary><TreeContents node={child} selected={selected} onSelect={onSelect} /></details>)}
    {files.map((file) => <button className={selected === file.path ? "tree-file selected" : "tree-file"} onClick={() => onSelect(file)} key={file.path}>{file.name}</button>)}
  </div>;
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
