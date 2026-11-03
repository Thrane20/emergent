"use client";

/* eslint-disable @next/next/no-img-element -- local image blobs and scoped reference routes */
import {FalBalance} from "@/components/fal-balance";
import Link from "next/link";
import { useEffect, useRef, useState } from "react";
import { ModelPreview } from "./model-preview";
import { MAX_IMAGE_BYTES, type ObjectJob, type ObjectSettings } from "@/lib/object-types";
import { ReferenceImagePicker } from "./reference-image-picker";
import type { LibraryImage } from "@/lib/image-library";

const activeStates = ["uploading", "submitting", "queued", "generating", "downloading"];
async function jsonResponse<T>(response: Response): Promise<T> {
  const result = await response.json();
  if (!response.ok) throw new Error(result.error ?? "Object Creator could not complete the request.");
  return result as T;
}

export function ObjectCreator({ projectId, projectName, initialJobs, settings, images }: { projectId: string; projectName: string; initialJobs: ObjectJob[]; settings: ObjectSettings; images: LibraryImage[] }) {
  const [jobs, setJobs] = useState(initialJobs);
  const [selectedId, setSelectedId] = useState(initialJobs[0]?.id ?? "");
  const [name, setName] = useState("");
  const [saveName, setSaveName] = useState("");
  const [image, setImage] = useState<File | null>(null);
  const [imageUrl, setImageUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [pollError, setPollError] = useState("");
  const pendingId = useRef<string | null>(null);
  const selected = jobs.find((job) => job.id === selectedId) ?? jobs[0];
  const query = `projectId=${encodeURIComponent(projectId)}`;
  const artifact = (job: ObjectJob, kind: "model" | "reference") => `/api/objects/${job.id}/artifact?${query}&kind=${kind}`;
  const update = (job: ObjectJob) => setJobs((previous) => [job, ...previous.filter((item) => item.id !== job.id)].sort((a, b) => b.createdAt.localeCompare(a.createdAt)));

  useEffect(() => () => { if (imageUrl) URL.revokeObjectURL(imageUrl); }, [imageUrl]);
  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      try {
        const list = await jsonResponse<ObjectJob[]>(await fetch(`/api/objects?projectId=${encodeURIComponent(projectId)}`, { cache: "no-store" }));
        const refreshed = await Promise.all(list.map(async (job) => {
          if (!activeStates.includes(job.state)) return job;
          return jsonResponse<ObjectJob>(await fetch(`/api/objects/${job.id}`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ projectId, action: "refresh" }) }));
        }));
        if (!cancelled) { setJobs(refreshed); setPollError(""); }
      } catch { if (!cancelled) setPollError("Progress updates are temporarily unavailable. Your existing jobs are retained; checking will resume automatically."); }
      if (!cancelled) timer = setTimeout(poll, 4_000);
    }
    void poll();
    return () => { cancelled = true; clearTimeout(timer); };
  }, [projectId]);

  async function generate(event: React.FormEvent) {
    event.preventDefault();
    if (!image) return;
    setBusy(true); setError("");
    const id = pendingId.current ?? crypto.randomUUID();
    pendingId.current = id;
    const form = new FormData();
    form.set("id", id); form.set("projectId", projectId); form.set("name", name); form.set("image", image);
    try {
      const job = await jsonResponse<ObjectJob>(await fetch("/api/objects", { method: "POST", body: form }));
      update(job); setSelectedId(job.id); setSaveName(job.name); pendingId.current = null;
    } catch (error) { setError(error instanceof Error ? error.message : "Unable to submit generation. Retrying keeps the same submission ID."); }
    finally { setBusy(false); }
  }

  async function save() {
    if (!selected) return;
    setSaving(true); setError("");
    try {
      const job = await jsonResponse<ObjectJob>(await fetch(`/api/objects/${selected.id}`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ projectId, action: "save", name: saveName || selected.name }) }));
      update(job);
    } catch (error) { setError(error instanceof Error ? error.message : "Unable to save object."); }
    finally { setSaving(false); }
  }

  return <main className="object-shell">
    <header className="settings-topbar"><Link className="brand" href="/"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></Link><span className="settings-breadcrumb">{projectName} / <b>Object Creator</b></span><Link href="/terrain" className="quiet-button">Terrain Builder</Link><Link href="/settings#object-generation" className="settings-back">Generation settings</Link><Link href="/" className="quiet-button">Project map ↗</Link><FalBalance/></header>
    <div className="object-heading"><div><span className="eyebrow">IMAGE TO OBJECT · TRELLIS</span><h1>Give your world a new shape.</h1><p>Start from a reference. Explore the result. Keep what belongs.</p></div><span className={settings.configured ? "settings-state connected" : "settings-state"}>{settings.configured ? "fal key configured" : "Connect fal to begin"}</span></div>
    <div className="object-layout">
      <aside className="object-sidebar"><section className="panel object-input-panel"><div className="panel-head"><h2>Create an object</h2><span className="kind-badge">01 / REFERENCE</span></div>
        <form className="object-form" onSubmit={generate}>
          <label className="settings-field"><span>Object name</span><input value={name} onChange={(event) => setName(event.target.value)} placeholder="Moon rover" maxLength={64} required disabled={busy} /></label>
          <ReferenceImagePicker projectId={projectId} images={images} preview={imageUrl} disabled={busy} maxBytes={MAX_IMAGE_BYTES} onError={setError} onPick={(file) => {
            setImage(file); setImageUrl(URL.createObjectURL(file)); pendingId.current = null; setError("");
          }} />
          {image && <p className="object-muted">{image.name}</p>}
          <button className="settings-save object-generate" disabled={busy || !image || !name.trim() || !settings.configured}>{busy ? "Submitting…" : "✦ Generate object"}</button>
          {!settings.configured && <Link className="object-link" href="/settings#object-generation">Add your fal key in Settings →</Link>}
        </form>
      </section>
      <section className="panel object-history"><div className="panel-head"><h2>Explorations</h2><span className="kind-badge">{jobs.length}</span></div><div>{jobs.map((job) => <button key={job.id} className={selected?.id === job.id ? "object-history-item selected" : "object-history-item"} onClick={() => { setSelectedId(job.id); setSaveName(job.name); setError(""); }}><img src={artifact(job, "reference")} alt="" /><span><b>{job.name}</b><small>{job.savedPath ? "Saved to project" : job.state}</small></span><i>{job.state === "ready" ? "◇" : "·"}</i></button>)}</div></section></aside>
      <section className="panel object-stage"><div className="object-stage-head"><div><span className="eyebrow">02 / EXPLORE</span><h2>{selected?.name ?? "Your next object starts here"}</h2></div><span className="kind-badge">{selected?.state ?? "AWAITING REFERENCE"}</span></div>
        <div className="object-viewport">{selected?.state === "ready" ? <ModelPreview key={selected.id} src={artifact(selected, "model")} name={selected.name} /> : <div className="object-empty"><div className="object-wire">◇</div><h3>{selected ? selected.state === "failed" ? "This attempt needs another look" : selected.state === "uncertain" ? "Submission needs checking" : "Shaping your object" : "A space for new possibilities"}</h3>{selected?.error && <p>{selected.error}</p>}</div>}</div>
        {selected?.error && selected.state !== "failed" && selected.state !== "uncertain" && <p className="object-message" role="status">{selected.error}</p>}
        {error && <p className="object-error" role="alert">{error}</p>}
        {pollError && <p className="object-message" role="status">{pollError}</p>}
        <footer className="object-stage-footer"><div><span className="eyebrow">03 / KEEP</span>{selected?.savedPath && <><p>{selected.savedPath}</p><Link className="object-link" href={`/models?model=${encodeURIComponent(selected.savedPath.replace(/model\.glb$/, "object.asset.json"))}`}>Split into parts ↗</Link></>}</div>{selected?.state === "ready" && !selected.savedPath && <input aria-label="Saved object name" value={saveName || selected.name} onChange={(event) => setSaveName(event.target.value)} maxLength={64} />}<button className="settings-save" disabled={selected?.state !== "ready" || !!selected?.savedPath || saving} onClick={() => void save()}>{saving ? "Saving…" : selected?.savedPath ? "Saved ✓" : "Save to project"}</button></footer>
      </section>
    </div>
  </main>;
}
