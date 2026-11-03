import "server-only";
import { SCENE_MAX_BYTES } from "./scene-limits";
import { createHash, randomUUID } from "node:crypto";
import {createReadStream} from "node:fs";
import { readFile } from "node:fs/promises";
import { runNoiseTool } from "./noise-tool";
import { runTweenTool } from "./tween-assets";
import { parse } from "smol-toml";
import { containedFile } from "./object-files";
import { foundationRoot } from "./project-code-context";
import { readProjectSnapshot, type ProjectSnapshot } from "./project";
import { applyChanges, checkCandidate, type FileChange } from "./statement-files";

export type TomlDocument = {path:string;content:string;revision:string};
function editable(project:ProjectSnapshot,path:string) {
  if(!/\.(scene|entity|noise|tween)\.toml$/.test(path)||!project.files.some(file=>file.path===path)||!project.assetRoots.some(root=>path.startsWith(`${root}/`)))throw new Error("Choose an authored scene, entity, noise or tween TOML asset.");
}
async function source(project:ProjectSnapshot,path:string) {return readFile(await containedFile(project.root,path,path.endsWith(".scene.toml")?SCENE_MAX_BYTES:256*1024),"utf8");}
async function revision(project:ProjectSnapshot) {
  const hash=createHash("sha256");
  for(const file of project.files.filter(file=>file.path.endsWith(".toml")||file.path.endsWith(".rs"))){
    // Revision checks hash full dependencies; preview limits apply only to the edited document.
    const target=await containedFile(project.root,file.path,Number.MAX_SAFE_INTEGER),contents=createHash("sha256");
    for await(const chunk of createReadStream(target))contents.update(chunk);
    hash.update(JSON.stringify([file.path,contents.digest("hex")]));
  }
  return hash.digest("hex");
}
export async function readTomlDocument(project:ProjectSnapshot,path:string):Promise<TomlDocument> {
  editable(project,path);const stamp=await revision(project);const content=await source(project,path);
  if(await revision(project)!==stamp)throw new Error("Project files changed while reading. Reload this TOML document.");
  return {path,content,revision:stamp};
}
const state=globalThis as typeof globalThis & {emergentScenePlacementActive?:boolean};
export async function saveTomlDocument(project:ProjectSnapshot,path:string,content:string,expected:string) {
  if(state.emergentScenePlacementActive)throw new Error("Another scene edit is being saved.");state.emergentScenePlacementActive=true;
  try{
    const before=await readTomlDocument(project,path);
    if(before.revision!==expected)throw new Error("Project files changed externally. Reload this TOML document before saving.");
    if(Buffer.byteLength(content)>(path.endsWith(".scene.toml")?SCENE_MAX_BYTES:256*1024))throw new Error(path.endsWith(".scene.toml")?"Scene source exceeds 2 MiB.":"TOML source exceeds 256 KiB.");parse(content);
    const scenes=project.files.filter(file=>file.path.endsWith(".scene.toml")).map(file=>file.path);
    if(!/\.(noise|tween)\.toml$/.test(path)&&scenes.length>127)throw new Error("TOML candidate validation supports at most 127 authored scenes in this first editor.");
    const changes:FileChange[]=[{path,before:before.content,content}];
    const candidate=[...changes];
    if(path.endsWith(".entity.toml")){
      const synthetic=`${project.assetRoots[0]}/__validation-${randomUUID()}.scene.toml`;
      candidate.push({path:synthetic,before:null,content:`schema_version=1\nid="entity-validation"\n[[entities]]\nid="candidate"\nasset=${JSON.stringify(path)}\n`});scenes.push(synthetic);
    }
    if(path.endsWith(".noise.toml"))await runNoiseTool(project,path,content);
    else if(path.endsWith(".tween.toml"))await runTweenTool(project,content);
    else await checkCandidate(project,candidate,{root:await foundationRoot(project)},scenes);
    const current=await readProjectSnapshot();
    if(current.id!==project.id||current.root!==project.root||await revision(current)!==expected)throw new Error("Project files changed during validation. Reload this TOML document before saving.");
    await applyChanges(project,changes);
    return readTomlDocument(await readProjectSnapshot(),path);
  }finally{state.emergentScenePlacementActive=false;}
}
