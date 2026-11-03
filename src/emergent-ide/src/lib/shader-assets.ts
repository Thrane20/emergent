import "server-only";
import {createHash,randomUUID} from "node:crypto";
import {readFile} from "node:fs/promises";
import {parse,stringify} from "smol-toml";
import {containedFile} from "./object-files";
import {applyChanges,checkCandidate,type FileChange} from "./statement-files";
import {foundationRoot} from "./project-code-context";
import {projectScenes} from "./project-scenes";
import {readProjectBinding,type ProjectSnapshot} from "./project";
import {shaderGraph,type ShaderGraph} from "./shader-graph";
import {journal,textDiff} from "./project-ops";
import {publishProjectEvent} from "./project-events";
const revision=(source:string)=>createHash("sha256").update(source).digest("hex");
export type ShaderAsset={path:string;revision:string;graph:ShaderGraph};
export async function readShader(project:ProjectSnapshot,path:string):Promise<ShaderAsset> {
 if(!path.endsWith(".shader.toml")||!project.assetRoots.some(root=>path.startsWith(`${root}/`)))throw new Error("Choose a project shader asset.");
 const source=await readFile(await containedFile(project.root,path,128*1024),"utf8");
 return {path,revision:revision(source),graph:shaderGraph(parse(source))};
}
export async function shaderLibrary(project:ProjectSnapshot){return Promise.all(project.files.filter(f=>f.path.endsWith(".shader.toml")).map(f=>readShader(project,f.path)));}
const state=globalThis as typeof globalThis&{emergentScenePlacementActive?:boolean};
/** MCP saves journal the change (history/undo) or stop after validation on a dry run. */
export type ShaderSaveOptions={tool:string;summary:string;dryRun?:boolean};
export async function saveShader(project:ProjectSnapshot,input:{path?:unknown;revision?:unknown;graph:unknown},mcp?:ShaderSaveOptions):Promise<ShaderAsset&{changeId?:string|null;diff?:string;applied?:boolean}>{
 if(state.emergentScenePlacementActive)throw new Error("Another asset edit is being saved.");state.emergentScenePlacementActive=true;
 try{
  const graph=shaderGraph(input.graph);
  if(input.path!==undefined&&typeof input.path!=="string")throw new Error("Invalid shader path.");
  const current=typeof input.path==="string"?await readShader(project,input.path):undefined;
  if(current&&current.revision!==input.revision)throw new Error("Shader changed externally. Reopen before saving.");
  const root=project.assetRoots[0];if(!root)throw new Error("Project has no asset root.");
  graph.id=current?.graph.id??randomUUID();
  const path=current?.path??`${root}/shaders/${randomUUID()}.shader.toml`;
  const source=current?await readFile(await containedFile(project.root,path,128*1024),"utf8"):null;
  if(source!==null&&revision(source)!==current?.revision)throw new Error("Shader changed during save.");
  const content=stringify(graph),changes:FileChange[]=[{path,before:source,content}];
  const token=randomUUID(),entity=`${root}/shaders/${token}.entity.toml`,scene=`${root}/shaders/${token}.scene.toml`;
  const probes:FileChange[]=[{path:entity,before:null,content:stringify({schema_version:1,id:token,kind:"primitive",geometry:{primitive:"cube",half_extent:0.5,color:[1,1,1,1]},material:{shader:path,cast_shadows:false}})},{path:scene,before:null,content:stringify({schema_version:1,id:token,entities:[{id:token,asset:entity}]})}];
  const scenes=projectScenes(project).map(s=>s.path);if(scenes.length>126)throw new Error("Shader validation supports at most 126 scenes.");
  await checkCandidate(project,[...changes,...probes],{root:await foundationRoot(project)},[...scenes,scene]);
  const binding=await readProjectBinding();if(binding.id!==project.id||binding.root!==project.root)throw new Error("Project changed during validation.");
  const result={path,revision:revision(content),graph};
  if(!mcp){await applyChanges(project,changes);return result;}
  const diff=textDiff(source??"",content);
  if(mcp.dryRun||source===content)return {...result,diff,applied:false,changeId:null};
  await applyChanges(project,changes);
  const changeId=await journal(project,mcp.tool,mcp.summary,changes);
  publishProjectEvent({type:"changed",projectId:project.id,changeId,source:"mcp",summary:mcp.summary,paths:[path],scenes:[],live:false});
  return {...result,diff,applied:true,changeId};
 }finally{state.emergentScenePlacementActive=false;}
}
