"use client";

import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

type Props = {
  projectId: string;
  disabled?: boolean;
  onBeforeOpen?: () => boolean;
  onCreated: (path: string) => void;
};

export function NewSceneModal({ projectId, disabled, onBeforeOpen, onCreated }: Props) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

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
    requestAnimationFrame(() => inputRef.current?.focus());
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      document.body.style.overflow = previousBodyOverflow;
      document.documentElement.style.overflow = previousRootOverflow;
    };
  }, [open]);

  const close = () => {
    if (saving) return;
    setOpen(false);
    setError("");
  };

  const create = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    setError("");
    try {
      const response = await fetch("/api/assets", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ projectId, templateId: "scene.blank", name }),
      });
      const result = await response.json() as { path?: string; error?: string };
      if (!response.ok || !result.path) throw new Error(result.error ?? "Scene creation failed.");
      setOpen(false);
      setName("");
      onCreated(result.path);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Scene creation failed.");
    } finally {
      setSaving(false);
    }
  };

  return <>
    <button type="button" disabled={disabled} onClick={() => {
      if (onBeforeOpen && !onBeforeOpen()) return;
      setName("");
      setError("");
      setOpen(true);
    }}>+ New Scene</button>
    {open && typeof document !== "undefined" && createPortal(
      <div className="asset-modal-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) close(); }}>
        <section className="new-scene-modal" role="dialog" aria-modal="true" aria-labelledby="new-scene-title">
          <header className="asset-modal-head">
            <div><span className="eyebrow">PROJECT SCENES</span><h2 id="new-scene-title">Create a new scene</h2></div>
            <button type="button" aria-label="Close scene creator" onClick={close}>×</button>
          </header>
          <form className="new-scene-form" onSubmit={create}>
            <label>
              <span>Scene name</span>
              <input ref={inputRef} value={name} maxLength={64} onChange={(event) => { setName(event.target.value); setError(""); }} placeholder="Forest clearing" autoComplete="off" />
            </label>
            {error && <p className="asset-create-error" role="alert">{error}</p>}
            <footer>
              <button type="button" className="asset-cancel" onClick={close}>Cancel</button>
              <button type="submit" className="asset-create" disabled={saving || !name.trim()}>{saving ? "Creating…" : "Create scene"}</button>
            </footer>
          </form>
        </section>
      </div>,
      document.body,
    )}
  </>;
}
