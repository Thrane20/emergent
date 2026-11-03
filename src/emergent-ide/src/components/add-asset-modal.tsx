"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";
import type { ProjectSnapshot } from "@/lib/project";
import { parsePrefab } from "@/lib/prefabs";
import { ObjectIcon } from "./object-icon";
import { createPortal } from "react-dom";
import {
  assetTemplates,
  sceneCompatible,
  type AssetTemplate,
} from "@/lib/asset-catalog";

type Props = {
  projectId: string;
  scenePath?: string;
  files: ProjectSnapshot["files"];
  onInstantiate?: (asset: string) => Promise<void>;
  onBeforeOpen?: () => boolean;
  onCreated: (path: string, sceneError?: string) => void;
};

export function AddAssetModal({ projectId, scenePath, files, onInstantiate, onBeforeOpen, onCreated }: Props) {
  const nameRef = useRef<HTMLInputElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const [prefabPath, setPrefabPath] = useState<string | null>(null);
  const prefabs = useMemo(() => files.filter(file => file.path.endsWith(".prefab.toml")).map(file => {
    try { const prefab = parsePrefab(file.content ?? ""); return { path: file.path, name: prefab.name, parts: prefab.entities.length }; }
    catch { return { path: file.path, name: file.displayName ?? file.name, parts: null }; }
  }).sort((a, b) => a.name.localeCompare(b.name)), [files]);
  const prefab = prefabs.find(item => item.path === prefabPath);
  const [search,setSearch]=useState("");
  const matchingPrefabs = prefabs.filter(item => search.trim().toLowerCase().split(/\s+/).every(token => `prefabs ${item.name} ${item.path}`.toLowerCase().includes(token)));
  const [open, setOpen] = useState(false);
  const [selectedId, setSelectedId] = useState(assetTemplates[0].id);
  const [name, setName] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [includeInScene, setIncludeInScene] = useState(true);
  const selected = assetTemplates.find((template) => template.id === selectedId) ?? assetTemplates[0];
  const particleGenerator=selected.id==="effects.particle-emitter";
  const categories = useMemo(() => groupTemplates(assetTemplates.filter(template=>search.trim().toLowerCase().split(/\s+/).every(token=>`${template.category} ${template.label} ${template.id} ${template.description}`.toLowerCase().includes(token)))), [search]);

  useEffect(() => {
    if (!open) return;
    const previousBodyOverflow = document.body.style.overflow;
    const previousRootOverflow = document.documentElement.style.overflow;
    document.body.style.overflow = "hidden";
    document.documentElement.style.overflow = "hidden";
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !saving) setOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    searchRef.current?.focus();
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      document.body.style.overflow = previousBodyOverflow;
      document.documentElement.style.overflow = previousRootOverflow;
    };
  }, [open, saving]);

  const choose = (template: AssetTemplate) => {
    setPrefabPath(null);
    setSelectedId(template.id);
    setError("");
    if (!name.trim()) setName(template.label);
    requestAnimationFrame(() => nameRef.current?.select());
  };

  const close = () => {
    if (!saving) {
      setOpen(false);
      setError("");
    }
  };

  const create = async (event: React.FormEvent) => {
    event.preventDefault();
    if(prefabPath){
      if(!prefab || prefab.parts === null || !onInstantiate){setError("Select a valid prefab and load a scene first.");return;}
      setSaving(true); setError("");
      try { await onInstantiate(prefab.path); setOpen(false); }
      catch(caught){setError(caught instanceof Error ? caught.message : "Could not add prefab.");}
      finally {setSaving(false);}
      return;
    }
    if(particleGenerator){setError("Open the Particle Generator to author this effect.");return;}
    setSaving(true);
    setError("");
    try {
      const response = await fetch("/api/assets", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ projectId, templateId: selected.id, name, scenePath: includeInScene && sceneCompatible(selected) ? scenePath : undefined }),
      });
      const result = await response.json() as { path?: string; error?: string; sceneError?: string };
      if (!response.ok || !result.path) throw new Error(result.error ?? "Asset creation failed.");
      setOpen(false);
      setName("");
      onCreated(result.path, result.sceneError);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Asset creation failed.");
    } finally {
      setSaving(false);
    }
  };

  return <>
    <div className="asset-tree-actions">
      <button className="add-asset-button" onClick={() => { if(onBeforeOpen&&!onBeforeOpen())return;setSearch("");setPrefabPath(null);setError("");setOpen(true); setName(""); }}>
        <span>+</span> Add asset
      </button>
    </div>
    {open && typeof document !== "undefined" && createPortal(
      <div className="asset-modal-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) close(); }}>
        <section className="asset-modal" role="dialog" aria-modal="true" aria-labelledby="asset-modal-title">
          <header className="asset-modal-head">
            <div><span className="eyebrow">EMERGENT FOUNDATIONS</span><h2 id="asset-modal-title">Add a project asset</h2></div>
            <button aria-label="Close asset creator" onClick={close}>×</button>
          </header>
          <div className="asset-modal-search"><input ref={searchRef} type="search" aria-label="Search asset foundations" placeholder="Search assets, shapes, lights, cameras…" value={search} onChange={event=>setSearch(event.target.value)}/>{search&&<button type="button" aria-label="Clear asset search" onClick={()=>{setSearch("");searchRef.current?.focus();}}>Clear</button>}</div>
          <div className="asset-modal-body">
            <nav className="foundation-tree" aria-label="Asset foundations">
              {!categories.length&&!matchingPrefabs.length&&<p className="asset-search-empty" role="status">No matching assets.</p>}
              <details open>
                <summary>Prefabs<small>{matchingPrefabs.length}</small></summary>
                <div>{matchingPrefabs.map(item => <button key={item.path} aria-label={item.name} className={prefabPath === item.path ? "selected" : ""} disabled={saving} onClick={() => {setPrefabPath(item.path);setError("");}}>
                  <span><ObjectIcon prefab/> {item.name}</span>
                </button>)}{!matchingPrefabs.length&&<p className="asset-search-empty">{prefabs.length ? "No matching prefabs." : "Drop a scene object into the Prefabs card to create one."}</p>}</div>
              </details>
              {categories.map(([category, templates]) => <details key={`${search.trim().toLowerCase()}:${category}`} open={search.trim()?true:undefined}>
                <summary>{category}<small>{templates.length}</small></summary>
                <div>
                  {templates.map((template) => <button
                    className={!prefabPath && template.id === selected.id ? "selected" : ""}
                    key={template.id}
                    disabled={saving}
                    onClick={() => choose(template)}
                  >
                    <span>{template.label}</span>
                    <i className={`capability-dot ${template.status}`} title={template.status} />
                  </button>)}
                </div>
              </details>)}
            </nav>
            <form className="asset-create-form" onSubmit={create}>
              <div className="asset-create-fields">
                {prefabPath ? <>
                  <div className="asset-template-title"><span>Prefabs</span></div>
                  <h3>{prefab?.name ?? "Prefab unavailable"}</h3>
                  <p>Add the complete object hierarchy as an independent instance at the scene root.</p>
                  <div className="capability-note"><b>{prefab?.parts ?? "Invalid"} parts</b><span>Each instance gets its own objects and internal references.</span></div>
                  <div className="asset-path-preview"><span>PREFAB</span><code>{prefabPath}</code></div>
                  {!onInstantiate&&<p role="status">Load a scene before adding a prefab.</p>}
                </> : <>

                <div className="asset-template-title">
                <span className={`capability-badge ${selected.status}`}>{selected.status}</span>
                <span>{selected.category}</span>
              </div>
              <h3>{selected.label}</h3>
              <p>{selected.description}</p>
              <div className="capability-note">
                <b>{selected.capability}</b>
                <span>{supportNote(selected)}</span>
              </div>
              <div hidden={particleGenerator}>
              <label className="asset-name-field">
                <span>Display name</span>
                <input
                  ref={nameRef}
                  value={name}
                  maxLength={64}
                  onChange={(event) => { setName(event.target.value); setError(""); }}
                  placeholder={`My ${selected.label}`}
                  autoComplete="off"
                />
              </label>
              {scenePath && sceneCompatible(selected) && <label>
                <input type="checkbox" checked={includeInScene} onChange={(event) => setIncludeInScene(event.target.checked)} /> Include in selected scene
              </label>}
              <div className="asset-path-preview">
                <span>CREATES</span>
                <code>{`${selected.directory}/<unique-id>${selected.extension}`}</code>
              </div>
              </div>
                </>}
                {error && <p className="asset-create-error" role="alert">{error}</p>}
              </div>
              <footer>
                <button type="button" className="asset-cancel" onClick={close}>Cancel</button>
                {prefabPath?<button type="submit" className="asset-create" disabled={saving || !onInstantiate || !prefab || prefab.parts === null}>{saving ? "Adding…" : "Add prefab to scene"}</button>:particleGenerator?<Link className="asset-create" href={`/particles${scenePath?`?scene=${encodeURIComponent(scenePath)}`:""}`}>Open Particle Generator ↗</Link>:<button type="submit" className="asset-create" disabled={saving || !name.trim()}>{saving ? "Creating…" : "Create asset"}</button>}
              </footer>
            </form>
          </div>
        </section>
      </div>,
      document.body,
    )}
  </>;
}

function groupTemplates(templates: readonly AssetTemplate[]): [string, AssetTemplate[]][] {
  const groups = new Map<string, AssetTemplate[]>();
  for (const template of templates) {
    const group = groups.get(template.category) ?? [];
    group.push(template);
    groups.set(template.category, group);
  }
  return [...groups.entries()];
}

function supportNote(template: AssetTemplate): string {
  if(template.id==="effects.particle-emitter")return "The generator saves validated effect recipes and scene-ready entity assets together.";
  if(template.extension===".noise.toml")return "Select this filter to edit inputs and seeds, generate a 2D field or 3D volume, and inspect its grayscale slices.";
  if (sceneCompatible(template)) return "Included instances load from this file when you Load the scene or Run the game. Edit transforms and primitive/light values in the asset, then reload.";
  if (template.status === "available") {
    return "The engine API is available; this asset type is not yet supported by the primitive/light scene loader.";
  }
  if (template.status === "preview") {
    return "The engine API is in preview and its authored loading contract may still evolve.";
  }
  return "This creates a definition for the project graph. Runtime implementation and loading are still planned.";
}
