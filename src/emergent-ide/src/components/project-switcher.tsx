"use client";

import { useEffect, useState } from "react";
import { createPortal } from "react-dom";

type Summary = { id: string; name: string; root: string };
type State = { active: Summary | null; recent: (Summary & { openedAt: string })[]; pinned: boolean; defaultParent: string };

async function projectAction<T>(body: Record<string, unknown>) {
  const response = await fetch("/api/projects", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });
  const result = await response.json();
  if (!response.ok) throw new Error(result.error ?? "Project action failed.");
  return result as T;
}

export function ProjectSwitcher({ projectId, projectName, onBeforeOpen }: { projectId: string; projectName: string; onBeforeOpen?: () => boolean }) {
  const [open, setOpen] = useState(false), [state, setState] = useState<State>();
  const [busy, setBusy] = useState(false), [error, setError] = useState(""), [status, setStatus] = useState("");
  const [message, setMessage] = useState(""), [name, setName] = useState(""), [parent, setParent] = useState(""), [root, setRoot] = useState("");

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => { if (event.key === "Escape" && !busy) setOpen(false); };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, busy]);

  async function run(work: () => Promise<void>) {
    setBusy(true); setError(""); setStatus("");
    try { await work(); } catch (caught) { setError(caught instanceof Error ? caught.message : "Project action failed."); } finally { setBusy(false); }
  }
  // A full reload drops every client cache tied to the previous project.
  // eslint-disable-next-line @next/next/no-location-assign-relative-destination -- client-side navigation would keep the previous project's state.
  const switched = (warning?: string) => { if (warning) window.alert(warning); window.location.assign("/"); };
  const save = () => run(async () => { const result = await projectAction<{ message: string }>({ action: "save", projectId, message: message.trim() || undefined }); setStatus(result.message); setMessage(""); });
  const create = (action: "new" | "duplicate") => run(async () => { const result = await projectAction<{ warning?: string }>({ action, name, parent: parent.trim() || undefined }); switched(result.warning); });
  const openRoot = (path: string) => run(async () => { await projectAction({ action: "open", root: path }); switched(); });

  return <>
    <button type="button" className="project-pill project-switcher-button" aria-haspopup="dialog" onClick={() => {
      if (onBeforeOpen && !onBeforeOpen()) return;
      setOpen(true); setError(""); setStatus("");
      void fetch("/api/projects", { cache: "no-store" }).then(response => response.json()).then((value: State) => { setState(value); setParent(previous => previous || value.defaultParent); }).catch(() => setError("Projects could not be listed."));
    }}><span>◫</span>{projectName}<span className="muted">▾</span></button>
    {open && createPortal(
      <div className="asset-modal-backdrop" onMouseDown={event => { if (event.target === event.currentTarget && !busy) setOpen(false); }}>
        <section className="new-scene-modal project-switcher" role="dialog" aria-modal="true" aria-labelledby="project-switcher-title">
          <header className="asset-modal-head">
            <div><span className="eyebrow">PROJECT</span><h2 id="project-switcher-title">{projectName}</h2>{state?.active && <code>{state.active.root}</code>}</div>
            <button type="button" aria-label="Close projects" disabled={busy} onClick={() => setOpen(false)}>×</button>
          </header>
          <div className="project-switcher-body">
            {projectId && <section aria-labelledby="project-save-title">
              <h3 id="project-save-title">Save snapshot</h3>
              <form className="project-switcher-row" onSubmit={event => { event.preventDefault(); void save(); }}>
                <input aria-label="Save message" value={message} maxLength={200} placeholder="What changed?" disabled={busy} onChange={event => setMessage(event.target.value)} />
                <button type="submit" className="asset-create" disabled={busy}>{busy ? "Working…" : "Save"}</button>
              </form>
            </section>}
            {state?.pinned ? <p className="asset-create-error" role="status">Pinned by EMERGENT_PROJECT_PATH.</p> : <>
              <section aria-labelledby="project-new-title">
                <h3 id="project-new-title">New project</h3>
                <label><span>Name</span><input aria-label="Project name" value={name} maxLength={64} placeholder="Forest game" disabled={busy} onChange={event => setName(event.target.value)} /></label>
                <label><span>Parent folder</span><input aria-label="Parent folder" value={parent} disabled={busy} onChange={event => setParent(event.target.value)} /></label>
                <div className="project-switcher-row">
                  <button type="button" className="asset-create" disabled={busy || !name.trim()} onClick={() => void create("new")}>Create</button>
                  <button type="button" className="asset-cancel" disabled={busy || !name.trim() || !projectId} onClick={() => void create("duplicate")}>Duplicate current</button>
                </div>
              </section>
              <section aria-labelledby="project-open-title">
                <h3 id="project-open-title">Open</h3>
                <form className="project-switcher-row" onSubmit={event => { event.preventDefault(); void openRoot(root); }}>
                  <input aria-label="Project folder" value={root} placeholder="/path/to/project" disabled={busy} onChange={event => setRoot(event.target.value)} />
                  <button type="submit" className="asset-create" disabled={busy || !root.trim()}>Open</button>
                </form>
                {!!state?.recent.filter(item => item.root !== state.active?.root).length && <ul className="project-recent" aria-label="Recent projects">
                  {state.recent.filter(item => item.root !== state.active?.root).map(item => <li key={item.root}><button type="button" disabled={busy} title={item.root} onClick={() => void openRoot(item.root)}><b>{item.name}</b><small>{item.root}</small></button></li>)}
                </ul>}
              </section>
            </>}
            {error && <p className="asset-create-error" role="alert">{error}</p>}
            {status && <p role="status">{status}</p>}
          </div>
        </section>
      </div>,
      document.body,
    )}
  </>;
}
