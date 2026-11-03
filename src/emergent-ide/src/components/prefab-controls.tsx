"use client";

import {ObjectIcon} from "./object-icon";
import { useMemo, useRef, useState } from "react";
import type { ProjectSnapshot } from "@/lib/project";
import type { SceneInstance, ScenePlacement } from "@/lib/scene-placement";
import { parsePrefab } from "@/lib/prefabs";
import { prefabMime, sceneInstanceDrop, sceneInstanceMime } from "@/lib/scene-drag";

export function PrefabControls({ project, placement, selected, disabled, beforeAction, onBusy, onChanged, onInstantiate }: {
  project: ProjectSnapshot; placement: ScenePlacement; selected?: SceneInstance; disabled: boolean;
  beforeAction: () => boolean; onBusy: (busy: boolean) => void; onChanged: () => void;
  onInstantiate: (asset: string, parent: string | null, revision: string) => Promise<void>;
}) {
  const pending = useRef(false);
  const [expanded,setExpanded]=useState(true);
  const [message, setMessage] = useState(""), [over, setOver] = useState(false);
  const prefabs = useMemo(() => project.files.filter(file => file.path.endsWith(".prefab.toml")).map(file => {
    try { const prefab = parsePrefab(file.content ?? ""); return { file, name: prefab.name, parts: prefab.entities.length }; }
    catch { return { file, name: file.displayName ?? file.name, parts: null }; }
  }).sort((a, b) => a.name.localeCompare(b.name)), [project.files]);
  async function capture(id: string) {
    if (disabled || pending.current || !beforeAction()) return;
    pending.current = true; onBusy(true); setMessage("Creating prefab…");
    try {
      const response = await fetch("/api/prefabs", {
        method: "POST", headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ projectId: project.id, path: placement.path, revision: placement.revision, root: id }),
      });
      const result = await response.json();
      if (!response.ok) throw new Error(result.error ?? "Could not create prefab.");
      setMessage(`Created ${result.name}. Original object retained.`); onChanged();
    } catch (error) { setMessage(error instanceof Error ? error.message : "Could not create prefab."); }
    finally { pending.current = false; onBusy(false); }
  }
  async function instantiate(asset: string) {
    try { await onInstantiate(asset, null, placement.revision); setMessage("Prefab added to scene."); }
    catch (error) { setMessage(error instanceof Error ? error.message : "Could not add prefab."); }
  }
  return <section className="prefab-controls sidebar-card" aria-label="Prefabs card"
      onDragOver={event => { if (!disabled && event.dataTransfer.types.includes(sceneInstanceMime)) { event.preventDefault(); event.dataTransfer.dropEffect = "copy"; setOver(true); } }}
      onDragLeave={event => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setOver(false); }}
      onDrop={event => {
        if (!event.dataTransfer.types.includes(sceneInstanceMime)) return;
        event.preventDefault(); event.stopPropagation(); setOver(false); if (disabled) return;
        try { void capture(sceneInstanceDrop(event.dataTransfer.getData(sceneInstanceMime), project.id, placement)); }
        catch (error) { setMessage(error instanceof Error ? error.message : "Drag a scene object here."); }
      }}
    >
    <div className="prefab-card-heading"><button type="button" className="prefab-collapse" aria-label={expanded?"Collapse Prefabs":"Expand Prefabs"} aria-expanded={expanded} onClick={()=>setExpanded(value=>!value)}><span aria-hidden="true">{expanded?"▾":"▸"}</span><ObjectIcon prefab/>Prefabs</button>
    <div className="prefab-dropzone" role="region" aria-label="Create prefab from scene object" tabIndex={0} aria-busy={disabled} data-drag-over={over} title="Drop a scene object and its children to create a prefab. Or select an object, focus here and press Enter."
      onKeyDown={event => { if (event.key === "Enter" && event.target === event.currentTarget && selected) { event.preventDefault(); void capture(selected.id); } }}>
      <span aria-hidden="true">＋</span> Drop object

    </div>
    </div>
    {expanded&&<>
    <div className="prefab-table-scroll" tabIndex={0} role="region" aria-label="Prefab library">
      <table><thead><tr><th scope="col">Name</th><th scope="col">Parts</th></tr></thead>
        <tbody>{prefabs.map(({ file, name, parts }) => <tr key={file.path} tabIndex={0} draggable={!disabled && parts !== null} aria-disabled={disabled || parts === null} aria-label={`Prefab ${name}`} title={file.path}
          onDragStart={event => {
            if (disabled || parts === null || !beforeAction()) { event.preventDefault(); return; }
            event.dataTransfer.effectAllowed = "copy";
            event.dataTransfer.setData(prefabMime, JSON.stringify({ projectId: project.id, scene: placement.path, revision: placement.revision, asset: file.path }));
          }}
          onKeyDown={event => { if (event.key === "Enter" && !disabled && parts !== null) { event.preventDefault(); void instantiate(file.path); } }}>
          <td><span className="prefab-name"><ObjectIcon prefab/>{name}</span></td><td>{parts ?? "Invalid"}</td>
        </tr>)}</tbody>
      </table>
      {!prefabs.length && <p>No prefabs yet.</p>}
    </div>
    <p className="prefab-hint">Drag a row into the scene graph to instantiate. Enter adds it at the scene root.</p>
    </>}
    {message && <p role="status">{message}</p>}
  </section>;
}
