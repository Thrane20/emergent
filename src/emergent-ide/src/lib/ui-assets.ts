import "server-only";
import { createHash, randomUUID } from "node:crypto";
import { mkdtemp, rm, writeFile, readFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { parse, stringify } from "smol-toml";
import type { ProjectSnapshot } from "./project";
import { gameTool } from "./scene-preview";
import { applyChanges } from "./statement-files";
import { withSceneLock } from "./project-ops";
import { parseUi, serializeUi, uiLayout, widgetCatalogue, type UiDocument, type UiSummary, type WidgetCatalogue } from "./ui";

export const UI_DIRECTORY = "assets/ui";
const catalogues = new Map<string, Promise<WidgetCatalogue>>();

/** Widget catalogue from the linked game binary (the engine's single source). */
export function uiCatalogue(project: ProjectSnapshot) {
  // shortcut: cached for the IDE process; the catalogue only changes with the engine, which needs an IDE restart to rebuild anyway.
  let cached = catalogues.get(project.root);
  if (!cached) { cached = gameTool(project, ["--export-ui-widgets"]).then(text => widgetCatalogue(parse(text))); catalogues.set(project.root, cached); cached.catch(() => catalogues.delete(project.root)); }
  return cached;
}

export const uiRevision = (source: string) => createHash("sha256").update(source).digest("hex");

export function listUis(project: ProjectSnapshot): UiSummary[] {
  return project.files.filter(file => file.path.endsWith(".ui.toml") && project.assetRoots.some(root => file.path.startsWith(`${root}/`))).map(file => {
    try {
      const ui = parseUi(file.content ?? ""), rects = uiLayout(ui);
      return { path: file.path, id: ui.id, name: ui.name, description: ui.description, nodes: ui.nodes.length, canvas: ui.canvas, boxes: ui.nodes.flatMap(node => { const rect = rects.get(node.id); return rect ? [{ type: node.type, rect }] : []; }) };
    } catch (error) { return { path: file.path, id: "", name: file.name, description: "", nodes: 0, error: error instanceof Error ? error.message : "Invalid UI." }; }
  }).sort((a, b) => a.name.localeCompare(b.name));
}

export function readUi(project: ProjectSnapshot, uiPath: string) {
  const file = project.files.find(item => item.path === uiPath && item.path.endsWith(".ui.toml") && project.assetRoots.some(root => item.path.startsWith(`${root}/`)));
  if (!file?.content) throw new Error("Choose a UI in this project.");
  return { path: file.path, revision: uiRevision(file.content), ui: parseUi(file.content) };
}

function checkedDocument(value: unknown): UiDocument {
  const ui = value as UiDocument;
  if (!ui || typeof ui.name !== "string" || !ui.name.trim() || ui.name.length > 64 || typeof ui.description !== "string" || ui.description.length > 500 || !Array.isArray(ui.nodes) || ui.nodes.length > 512 || !Array.isArray(ui.canvas)) throw new Error("A UI needs a name (1-64 characters), a description up to 500 characters, a canvas and at most 512 widgets.");
  // Round-trip so only well-formed TOML values reach Rust validation.
  return parseUi(serializeUi({ ...ui, id: typeof ui.id === "string" && /^[\w.-]{1,64}$/.test(ui.id) ? ui.id : randomUUID() }));
}

/** Validates with Rust, then publishes atomically; `revision` guards against overwriting newer edits. */
export async function saveUi(project: ProjectSnapshot, input: { path?: string; revision?: string; ui: unknown }) {
  const ui = checkedDocument(input.ui);
  const target = input.path ?? `${UI_DIRECTORY}/${ui.id}.ui.toml`;
  if (!target.endsWith(".ui.toml") || !project.assetRoots.some(root => target.startsWith(`${root}/`)) || target.split("/").some(part => part === ".." || !part)) throw new Error("UIs live under a project asset root.");
  const existing = project.files.find(file => file.path === target);
  if (input.path && (!existing?.content || uiRevision(existing.content) !== input.revision)) throw new Error("This UI changed on disk since you opened it. Reload it before saving.");
  if (!input.path && existing) throw new Error("A UI with this ID already exists.");
  const content = serializeUi(ui);
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-ui-"));
  try { const file = path.join(directory, "candidate.ui.toml"); await writeFile(file, content); await gameTool(project, ["--validate-ui", target, "--candidate", file]); }
  finally { await rm(directory, { recursive: true, force: true }); }
  await withSceneLock(() => applyChanges(project, [{ path: target, before: existing?.content ?? null, content }]));
  return { path: target, revision: uiRevision(content), ui };
}

/** Place a rendered UI entity using the existing staged native validation path. */
export async function placeUi(project:ProjectSnapshot,input:{path:unknown;revision:unknown;scenePath:unknown;space:unknown}){
 return withSceneLock(async()=>{
  if(typeof input.path!=="string"||typeof input.scenePath!=="string"||!["screen","world"].includes(input.space as string))throw new Error("Choose a saved UI, scene and screen/world space.");
  const ui=readUi(project,input.path);if(ui.revision!==input.revision)throw new Error("UI changed. Reopen before placing.");
  const {projectScenes}=await import("./project-scenes");if(!projectScenes(project).some(scene=>scene.path===input.scenePath))throw new Error("Choose a project scene.");
  const {containedFile}=await import("./object-files"),{foundationRoot}=await import("./project-code-context"),{checkCandidate}=await import("./statement-files"),{journal}=await import("./project-ops"),{publishProjectEvent}=await import("./project-events");
  const before=await readFile(await containedFile(project.root,input.scenePath,256*1024),"utf8"),document=parse(before);
  if(!Array.isArray(document.entities)||document.entities.length>=1024)throw new Error("Scene is full or unsupported.");
  const id=randomUUID(),asset=`${project.assetRoots[0]}/entities/${id}.entity.toml`;
  const content=before.replace(/^entities\s*=\s*\[\s*\][^\S\r\n]*(?:#[^\r\n]*)?$/m,"").trimEnd()+"\n\n"+stringify({entities:[{id,asset,name:ui.ui.name,components:[{type:"emergent.ui",ui:input.path,space:input.space}]}]});
  if(Buffer.byteLength(content)>256*1024)throw new Error("Scene exceeds 256 KiB.");
  const changes=[{path:asset,before:null,content:stringify({schema_version:1,id,name:ui.ui.name,kind:"entity"})},{path:input.scenePath,before,content}];
  await checkCandidate(project,changes,{root:await foundationRoot(project)},[input.scenePath]);
  if(uiRevision(await readFile(await containedFile(project.root,input.path,256*1024),"utf8"))!==ui.revision)throw new Error("UI changed during validation.");
  const {readProjectBinding}=await import("./project");const binding=await readProjectBinding();if(binding.id!==project.id||binding.root!==project.root)throw new Error("Project changed during validation.");
  await applyChanges(project,changes);const summary=`Added ${input.space} UI ${ui.ui.name}`,changeId=await journal(project,"place_ui",summary,changes);
  publishProjectEvent({type:"changed",projectId:project.id,changeId,summary,source:"ide",paths:changes.map(c=>c.path),scenes:[input.scenePath],live:false});return {instanceId:id,asset};
 });
}
