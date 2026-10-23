"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  assetPath,
  assetTemplates,
  type AssetTemplate,
} from "@/lib/asset-catalog";

type Props = {
  projectId: string;
  onCreated: (path: string) => void;
};

export function AddAssetModal({ projectId, onCreated }: Props) {
  const nameRef = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
  const [selectedId, setSelectedId] = useState(assetTemplates[0].id);
  const [name, setName] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const selected = assetTemplates.find((template) => template.id === selectedId) ?? assetTemplates[0];
  const categories = useMemo(() => groupTemplates(assetTemplates), []);

  useEffect(() => {
    if (!open) return;
    const previousBodyOverflow = document.body.style.overflow;
    const previousRootOverflow = document.documentElement.style.overflow;
    document.body.style.overflow = "hidden";
    document.documentElement.style.overflow = "hidden";
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    nameRef.current?.focus();
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      document.body.style.overflow = previousBodyOverflow;
      document.documentElement.style.overflow = previousRootOverflow;
    };
  }, [open]);

  const choose = (template: AssetTemplate) => {
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
    setSaving(true);
    setError("");
    try {
      const response = await fetch("/api/assets", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ projectId, templateId: selected.id, name }),
      });
      const result = await response.json() as { path?: string; error?: string };
      if (!response.ok || !result.path) throw new Error(result.error ?? "Asset creation failed.");
      setOpen(false);
      setName("");
      onCreated(result.path);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Asset creation failed.");
    } finally {
      setSaving(false);
    }
  };

  return <>
    <div className="asset-tree-actions">
      <button className="add-asset-button" onClick={() => { setOpen(true); setName(""); }}>
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
          <div className="asset-modal-body">
            <nav className="foundation-tree" aria-label="Asset foundations">
              {categories.map(([category, templates]) => <details open key={category}>
                <summary>{category}<small>{templates.length}</small></summary>
                <div>
                  {templates.map((template) => <button
                    className={template.id === selected.id ? "selected" : ""}
                    key={template.id}
                    onClick={() => choose(template)}
                  >
                    <span>{template.label}</span>
                    <i className={`capability-dot ${template.status}`} title={template.status} />
                  </button>)}
                </div>
              </details>)}
            </nav>
            <form className="asset-create-form" onSubmit={create}>
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
              <label className="asset-name-field">
                <span>Name</span>
                <input
                  ref={nameRef}
                  value={name}
                  maxLength={64}
                  onChange={(event) => { setName(event.target.value); setError(""); }}
                  placeholder={`My ${selected.label}`}
                  autoComplete="off"
                />
              </label>
              <div className="asset-path-preview">
                <span>CREATES</span>
                <code>{name.trim() ? assetPath(selected, name) : `${selected.directory}/…${selected.extension}`}</code>
              </div>
              {error && <p className="asset-create-error" role="alert">{error}</p>}
              <footer>
                <button type="button" className="asset-cancel" onClick={close}>Cancel</button>
                <button type="submit" className="asset-create" disabled={saving || !name.trim()}>{saving ? "Creating…" : "Create asset"}</button>
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
  if (template.status === "available") {
    return "The engine API is available. This file is authored now; generic project loading is a later integration step.";
  }
  if (template.status === "preview") {
    return "The engine API is in preview and its authored loading contract may still evolve.";
  }
  return "This creates a definition for the project graph. Runtime implementation and loading are still planned.";
}
