import "server-only";
import {readFile} from "node:fs/promises";
import {createHash} from "node:crypto";
import {parse,stringify} from "smol-toml";
import {containedFile} from "./object-files";
import {applyChanges,checkCandidate} from "./statement-files";
import {foundationRoot} from "./project-code-context";
import {projectScenes} from "./project-scenes";
import type {ProjectSnapshot} from "./project";
import {defaultLabels,objectLabels} from "./object-labels";
const revision=(s:string|null)=>createHash("sha256").update(s??"missing").digest("hex");
export async function readObjectLabels(project:ProjectSnapshot){const path=`${project.assetRoots[0]}/object-labels.toml`;let source:string|null=null;try{source=await readFile(await containedFile(project.root,path,16384),"utf8");}catch(e){if(!(e&&typeof e==="object"&&"code" in e&&e.code==="ENOENT"))throw e;}return {path,source,revision:revision(source),labels:source===null?defaultLabels():objectLabels(parse(source))};}
const state=globalThis as typeof globalThis&{emergentScenePlacementActive?:boolean};
export async function saveObjectLabels(project:ProjectSnapshot,expected:unknown,value:unknown){if(state.emergentScenePlacementActive)throw Error("Another edit is saving.");state.emergentScenePlacementActive=true;try{const before=await readObjectLabels(project);if(before.revision!==expected)throw Error("Labels changed externally. Reopen configuration.");const labels=objectLabels(value),content=stringify(labels),changes=[{path:before.path,before:before.source,content}];await checkCandidate(project,changes,{root:await foundationRoot(project)},[...projectScenes(project).map(s=>s.path),...project.files.filter(f=>f.path.endsWith(".prefab.toml")).map(f=>f.path)]);await applyChanges(project,changes);return {...before,source:content,revision:revision(content),labels};}finally{state.emergentScenePlacementActive=false;}}
