"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type { RigReport } from "@/lib/model-rig";
import type { SplitJob } from "@/lib/model-split";

type Rig = { report: RigReport; jobs: SplitJob[] };

async function call<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, init), body = await response.json();
  if (!response.ok) throw new Error(body.error ?? "Request failed.");
  return body as T;
}

/** The engine's skeleton report for a model or BVH take, a skin-weight view and AI auto-rigging. */
export function RigPanel({ projectId, path, assetUrl, onImported }: { projectId: string; path: string; assetUrl?: string; onImported: (path: string) => void }) {
  const [rig, setRig] = useState<Rig | null>(null), [error, setError] = useState(""), [busy, setBusy] = useState("");
  const [weights, setWeights] = useState(false), [joint, setJoint] = useState<string | null>(null);
  const load = useCallback(async () => {
    setError("");
    try { setRig(await call<Rig>(`/api/models/rig?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(path)}`)); }
    catch (e) { setError(e instanceof Error ? e.message : "Inspection failed."); }
  }, [projectId, path]);
  // eslint-disable-next-line react-hooks/set-state-in-effect -- Inspect once per selected file.
  useEffect(() => { void load(); }, [load]);
  async function act(label: string, body: Record<string, unknown>) {
    setBusy(label); setError("");
    try { return await call<{ path?: string }>("/api/models/rig", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ projectId, ...body }) }); }
    catch (e) { setError(e instanceof Error ? e.message : "Request failed."); return null; }
    finally { setBusy(""); await load(); }
  }
  const report = rig?.report, mapped = report?.humanoid ? Object.keys(report.humanoid).length : 0;
  const unskinned = report && !path.endsWith(".bvh") && !report.skinned;
  return <section className="rig-panel panel" aria-label="Rig">
    <div className="panel-head"><h3>Rig</h3>{report && <span className="kind-badge">{report.joints ? `${report.joints.length} joints` : "no skeleton"}</span>}</div>
    {error && <p className="object-error" role="alert">{error}</p>}
    {!report && !error && <p className="particle-job-status" role="status"><span className="particle-spinner" aria-hidden="true" />Asking the engine…</p>}
    {report && <>
      <p className="transform-note">
        {report.vertices !== undefined && `${report.vertices} vertices · ${report.triangles} triangles · `}
        {report.joints ? `humanoid ${mapped}/${mapped + (report.humanoid_missing?.length ?? 0)} bones mapped` : "Not skinned: the engine draws it rigid."}
        {report.clips.length ? ` · clips ${report.clips.map(clip => `${clip.name} (${clip.duration_seconds}s)`).join(", ")}` : ""}
      </p>
      {!!report.humanoid_missing?.length && <p className="transform-note">Missing: {report.humanoid_missing.join(", ")}. Mocap and foot IK skip unmapped bones.</p>}
      {report.joints && assetUrl && <label className="audio-toggle"><input type="checkbox" checked={weights} onChange={event => setWeights(event.target.checked)} />Show skin weights</label>}
      {weights && assetUrl && report.joints && <>
        <WeightView src={assetUrl} joint={joint} />
        <div className="rig-joints" role="listbox" aria-label="Joints">{report.joints.map(item => <button type="button" role="option" aria-selected={joint === item.name} key={item.name} className={joint === item.name ? "selected" : ""} onClick={() => setJoint(joint === item.name ? null : item.name)}>{item.name}</button>)}</div>
        <p className="transform-note">{joint ? `Red is fully bound to ${joint}, blue is not bound.` : "Each colour is the joint that moves that part most. Pick a joint to see its weights."}</p>
      </>}
      {unskinned && path.endsWith(".glb") && <button type="button" className="quiet-button" disabled={!!busy} onClick={() => { if (window.confirm("Auto-rig this model with Meshy on fal? It costs about $0.20 and needs a textured humanoid in a T or A pose.")) void act("Submitting…", { action: "autorig", id: crypto.randomUUID(), model: path }); }}>Auto-rig humanoid · fal ≈ $0.20</button>}
      {rig?.jobs.map(job => <div className="particle-actions" key={job.id}>
        <span className="particle-job-status" role="status">Auto-rig {job.state}{job.error ? `: ${job.error}` : ""}</span>
        {["queued", "splitting", "downloading"].includes(job.state) && <button type="button" className="quiet-button" disabled={!!busy} onClick={() => void act("Checking…", { action: "refresh", id: job.id })}>Check</button>}
        {job.state === "ready" && <button type="button" className="quiet-button" disabled={!!busy} onClick={async () => { const name = window.prompt("Name for the rigged model", `${path.split("/").at(-2) ?? "model"} rigged`); if (name) { const saved = await act("Saving…", { action: "save", id: job.id, name }); if (saved?.path) onImported(saved.path); } }}>Save rigged model</button>}
      </div>)}
      {busy && <p className="particle-job-status" role="status"><span className="particle-spinner" aria-hidden="true" />{busy}</p>}
    </>}
  </section>;
}

/** File input that imports a rigged .glb/.vrm (plus a model entity) or a .bvh take through the engine. */
export function RigImport({ projectId, onImported }: { projectId: string; onImported: (path: string) => void }) {
  const [busy, setBusy] = useState(false), [error, setError] = useState("");
  async function importFile(file: File) {
    setBusy(true); setError("");
    try {
      const extension = file.name.split(".").pop() ?? "", name = file.name.replace(/\.[^.]+$/, "").replace(/[_-]+/g, " ").trim();
      const result = await call<{ path: string }>(`/api/models/rig?action=import&projectId=${encodeURIComponent(projectId)}&name=${encodeURIComponent(name)}&extension=${encodeURIComponent(extension)}`, { method: "POST", body: file });
      onImported(result.path);
    } catch (e) { setError(e instanceof Error ? e.message : "Import failed."); } finally { setBusy(false); }
  }
  return <div className="rig-import">
    <label className={`quiet-button${busy ? " disabled" : ""}`}>{busy ? "Importing…" : "Import model or mocap…"}<input type="file" hidden accept=".glb,.vrm,.bvh" disabled={busy} onChange={event => { const file = event.target.files?.[0]; event.target.value = ""; if (file) void importFile(file); }} /></label>
    {error && <p className="object-error" role="alert">{error}</p>}
  </div>;
}

const heat = (w: number, out: THREE.Color) => out.setHSL((1 - w) * 0.66, 0.9, 0.5);

/** Colours each vertex by skin weight: the selected joint as a heat map, otherwise its strongest joint. */
function WeightView({ src, joint }: { src: string; joint: string | null }) {
  const host = useRef<HTMLDivElement>(null), recolor = useRef<(joint: string | null) => void>(() => {}), latest = useRef(joint), [error, setError] = useState("");
  useEffect(() => {
    const element = host.current!;
    let renderer: THREE.WebGLRenderer;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- Report GPU creation failure after the canvas mounts.
    try { renderer = new THREE.WebGLRenderer({ antialias: true }); } catch { setError("WebGL is unavailable."); return; }
    renderer.setPixelRatio(Math.min(devicePixelRatio, 2)); renderer.setClearColor(0x101713); element.prepend(renderer.domElement);
    const scene = new THREE.Scene(), camera = new THREE.PerspectiveCamera(40, 1, 0.01, 1000), orbit = new OrbitControls(camera, renderer.domElement);
    const skinned: THREE.SkinnedMesh[] = [];
    let frame = 0, disposed = false;
    recolor.current = selected => {
      const color = new THREE.Color();
      for (const mesh of skinned) {
        const index = mesh.geometry.getAttribute("skinIndex"), weight = mesh.geometry.getAttribute("skinWeight"), count = index.count;
        const target = selected === null ? -1 : mesh.skeleton.bones.findIndex(bone => bone.name === THREE.PropertyBinding.sanitizeNodeName(selected));
        const colors = new Float32Array(count * 3);
        for (let v = 0; v < count; v++) {
          let strongest = 0, best = -1, picked = 0;
          for (let k = 0; k < 4; k++) {
            const j = index.getComponent(v, k), w = weight.getComponent(v, k);
            if (w > strongest) { strongest = w; best = j; }
            if (j === target) picked += w;
          }
          if (selected === null) color.setHSL(((best * 0.618) % 1 + 1) % 1, 0.65, 0.55); else heat(picked, color);
          color.toArray(colors, v * 3);
        }
        mesh.geometry.setAttribute("color", new THREE.BufferAttribute(colors, 3));
      }
    };
    new GLTFLoader().loadAsync(src).then(gltf => {
      if (disposed) return;
      gltf.scene.traverse(object => {
        if (!(object instanceof THREE.SkinnedMesh)) return;
        object.material = new THREE.MeshBasicMaterial({ vertexColors: true });
        skinned.push(object);
      });
      scene.add(gltf.scene, new THREE.SkeletonHelper(gltf.scene));
      recolor.current(latest.current);
      const box = new THREE.Box3().setFromObject(gltf.scene), center = box.getCenter(new THREE.Vector3()), radius = Math.max(0.1, box.getSize(new THREE.Vector3()).length() / 2);
      orbit.target.copy(center); camera.position.copy(center).add(new THREE.Vector3(0, 0.2, 1).multiplyScalar(radius * 2.4)); orbit.update();
    }).catch(() => setError("The weight view could not load this model."));
    const resize = () => { const width = Math.max(1, element.clientWidth), height = Math.max(1, element.clientHeight); renderer.setSize(width, height, false); camera.aspect = width / height; camera.updateProjectionMatrix(); };
    const observer = new ResizeObserver(resize); observer.observe(element); resize();
    const tick = () => { frame = requestAnimationFrame(tick); orbit.update(); renderer.render(scene, camera); };
    tick();
    return () => { disposed = true; cancelAnimationFrame(frame); observer.disconnect(); orbit.dispose(); renderer.dispose(); renderer.domElement.remove(); };
  }, [src]);
  useEffect(() => { latest.current = joint; recolor.current(joint); }, [joint]);
  return <div className="rig-weights" ref={host}>{error && <p className="object-error" role="alert">{error}</p>}</div>;
}
