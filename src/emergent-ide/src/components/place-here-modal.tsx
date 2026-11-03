"use client";

import { useMemo, useState } from "react";
import { createPortal } from "react-dom";
import type { ProjectFile } from "@/lib/project";

// Right-click "add an asset here": places an existing entity asset at the viewport surface hit.
export function PlaceHereModal({ projectId, scenePath, revision, files, hit, surfaceName, onClose }: {
  projectId: string; scenePath: string; revision: string; files: ProjectFile[]; hit: { id: string; point: number[] }; surfaceName: string; onClose: () => void;
}) {
  const [search, setSearch] = useState(""), [chosen, setChosen] = useState<string | null>(null), [error, setError] = useState(""), [saving, setSaving] = useState(false);
  const label = (file: ProjectFile) => file.displayName ?? file.name.replace(/\.(entity|prefab)\.toml$/, "");
  const assets = useMemo(() => {
    const tokens = search.trim().toLowerCase().split(/\s+/);
    return files.filter(file => /\.(entity|prefab)\.toml$/.test(file.path) && tokens.every(token => `${label(file)} ${file.path}`.toLowerCase().includes(token)))
      .sort((a, b) => label(a).localeCompare(label(b)));
  }, [files, search]);
  const point = hit.point.map(n => Math.round(n * 1000) / 1000);
  async function place() {
    if (!chosen) return;
    setSaving(true); setError("");
    try {
      const response = await fetch("/api/scenes/place", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ projectId, path: scenePath, asset: chosen, revision, translation: point }) });
      const data = await response.json();
      if (!response.ok) throw new Error(data.error || "Placement failed.");
      onClose(); // The project-events stream refreshes the graph and inspection.
    } catch (failure) { setError(failure instanceof Error ? failure.message : "Placement failed."); setSaving(false); }
  }
  return createPortal(
    <div className="asset-modal-backdrop" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }} onKeyDown={event => { if (event.key === "Escape") onClose(); }}>
      <section className="asset-modal place-here-modal" role="dialog" aria-modal="true" aria-labelledby="place-here-title">
        <header className="asset-modal-head">
          <div><span className="eyebrow">ON {surfaceName.toUpperCase()} AT {point.join(", ")}</span><h2 id="place-here-title">Add an asset here</h2></div>
          <button aria-label="Close" onClick={onClose}>×</button>
        </header>
        <div className="asset-modal-search"><input autoFocus type="search" aria-label="Search assets and prefabs" placeholder="Search assets and prefabs…" value={search} onChange={event => setSearch(event.target.value)} /></div>
        <nav className="foundation-tree place-here-list" aria-label="Entity assets">
          {!assets.length && <p className="asset-search-empty" role="status">No matching assets or prefabs.</p>}
          {assets.map(file => <button key={file.path} className={chosen === file.path ? "selected" : ""} onClick={() => setChosen(file.path)}><span>{label(file)}</span></button>)}
        </nav>
        {error && <p className="asset-create-error" role="alert">{error}</p>}
        <footer className="place-here-footer">
          <button type="button" className="asset-cancel" onClick={onClose}>Cancel</button>
          <button type="button" className="asset-create" disabled={!chosen || saving} onClick={() => void place()}>{saving ? "Placing…" : "Place here"}</button>
        </footer>
      </section>
    </div>,
    document.body,
  );
}
