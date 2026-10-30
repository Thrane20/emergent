import "server-only";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { isDeepStrictEqual } from "node:util";
import { parse, stringify } from "smol-toml";
import { containedFile } from "./object-files";
import type { ProjectSnapshot } from "./project";
import { applyChanges } from "./statement-files";
import { localFromGlobal, sceneWorldTransforms, validateProjectScene } from "./scene-preview";
import { materialSettings, materialChannels,materialTextures,isTexturePath, type MaterialSettings } from "./material-settings";
import {readMaterial} from "./material-assets";
import {TERRAIN_DATA_MAX_BYTES,terrainSettings} from "./terrain";
import { sceneHierarchy } from "./scene-hierarchy";
import {lightingSettings} from "./lighting-settings";
import {cameraSettings,type CameraSettings} from "./camera-settings";
import {raymarchedSettings} from "./raymarched-terrain";

export type TransformValues = { translation: number[]; rotation_degrees: number[]; scale: number[] };
export type TransformSpace = "local"|"global";
export type PhysicsSettings = {enabled:boolean;body:"static"|"dynamic";gravity_scale:number;mass:number;friction:number;restitution:number;collider:{shape:"box"|"terrain";center?:number[];half_extents?:number[]}};
export function physicsSettings(value:unknown):PhysicsSettings {
  const input=value===undefined?{enabled:false}:table(value);
  const collider=input.collider===undefined?{}:table(input.collider);
  const result={enabled:true,body:"dynamic",gravity_scale:1,mass:1,friction:0.5,restitution:0,...input,collider:{shape:"box",...collider}} as PhysicsSettings;
  if(Object.keys(input).some(key=>!["enabled","body","gravity_scale","mass","friction","restitution","collider"].includes(key)) || Object.keys(collider).some(key=>!["shape","center","half_extents"].includes(key)) || typeof result.enabled!=="boolean" || !["static","dynamic"].includes(result.body) || !["box","terrain"].includes(result.collider.shape))throw new Error("Invalid physics settings; choose static/dynamic and a box/terrain collider.");
  if(result.collider.shape==="terrain"&&(result.body!=="static"||collider.center!==undefined||collider.half_extents!==undefined))throw new Error("Terrain colliders are static and use terrain heights, without box dimensions.");
  for(const [key,min,max] of [["gravity_scale",0,100],["mass",0.0001,1000000],["friction",0,10],["restitution",0,1]] as const){const n=result[key];if(typeof n!=="number"||!Number.isFinite(n)||n<min||n>max)throw new Error(`Invalid physics ${key}.`);}
  for(const key of ["center","half_extents"] as const){const vector=result.collider[key];if(vector!==undefined&&(!Array.isArray(vector)||vector.length!==3||!vector.every(n=>typeof n==="number"&&Number.isFinite(n)&&Math.abs(n)<=1000000&&(key!=="half_extents"||n>=0.0001))))throw new Error(`Invalid collider ${key}.`);}
  return result;
}
export type SceneInstance = { id: string; name?: string; asset: string; parent: string | null; defaults: TransformValues; transform: TransformValues; overridden: boolean; display?:TransformValues; globalTranslationOnly?:boolean; physics?:PhysicsSettings; physicsOverridden?:boolean;terrain?:boolean;raymarched?:boolean;structure?:boolean;particles?:boolean;terrainExtent?:{size:number;height:number};renderable?:boolean;material?:MaterialSettings;materialOverridden?:boolean;geometryColor?:number[];camera?:CameraSettings;cameraOverridden?:boolean };
export type ScenePlacement = { path: string; revision: string; instances: SceneInstance[]; space:TransformSpace;frame?:{target:number[];distance:number} };
const identity: TransformValues = { translation: [0,0,0], rotation_degrees: [0,0,0], scale: [1,1,1] };
const fields = ["translation", "rotation_degrees", "scale"] as const;

function table(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Expected a TOML table.");
  return value as Record<string,unknown>;
}

function transform(value: unknown, defaults = identity, space:TransformSpace="local"): TransformValues {
  const input = value === undefined ? {} : table(value);
  if (Object.keys(input).some(key => !fields.includes(key as typeof fields[number]))) throw new Error("Unknown transform field.");
  const result = {} as TransformValues;
  for (const field of fields) {
    const vector = input[field] === undefined ? defaults[field] : input[field];
    if (!Array.isArray(vector) || vector.length !== 3 || !vector.every(n => typeof n === "number" && Number.isFinite(n) && Math.abs(n) <= 3.4028234e38 && (field !== "scale" || (space==="global"?n!==0:Math.abs(n)>=0.0001)))) throw new Error(`${field} requires three finite values; scale axes must be non-zero.`);
    result[field] = [...vector];
  }
  return result;
}

async function assetSource(project: ProjectSnapshot, relative: string, limit=256*1024) {
  if (!project.assetRoots.some(root => relative.startsWith(`${root}/`)) || relative.split("/").some(part => part === ".." || part === "." || !part) || relative.includes(":")) throw new Error("Choose an asset under a declared project asset root.");
  return readFile(await containedFile(project.root, relative, limit), "utf8");
}

async function snapshot(project: ProjectSnapshot, path: string) {
  if (!path.endsWith(".scene.toml")) throw new Error("Choose a scene file.");
  const source = await assetSource(project, path);
  const document = parse(source);
  if (document.schema_version !== 1 || !Array.isArray(document.entities) || document.entities.length > 1024) throw new Error("Scene must use schema 1 and at most 1024 instances.");
  const hash = createHash("sha256").update(JSON.stringify([path,source]));
  const environment=lightingSettings(document.environment);
  if(environment.skybox){
    if(!project.assetRoots.some(root=>environment.skybox!.startsWith(`${root}/`)))throw new Error("Skybox is outside project asset roots.");
    hash.update(environment.skybox).update(await readFile(await containedFile(project.root,environment.skybox,16*1024*1024)));
  }
  const ids = new Set<string>();
  const assets = new Map<string, string>();
  const instances: SceneInstance[] = [];
  for (const entry of document.entities) {
    const item = table(entry);
    if (typeof item.id !== "string" || !/^[a-z0-9_.-]{1,128}$/i.test(item.id) || ids.has(item.id) || typeof item.asset !== "string" || (item.parent !== undefined && typeof item.parent !== "string")) throw new Error("Scene instances need unique IDs, asset references and valid parent IDs.");
    ids.add(item.id);
    let text = assets.get(item.asset);
    if (text === undefined) { text = await assetSource(project,item.asset); assets.set(item.asset,text); }
    const asset = parse(text);
    if (asset.schema_version !== 1) throw new Error(`${item.asset}: unsupported entity schema.`);
    if(typeof asset.particles==="string"&&!assets.has(asset.particles))assets.set(asset.particles,await assetSource(project,asset.particles,128*1024));
    if(typeof asset.structure==="string"&&!assets.has(asset.structure))assets.set(asset.structure,await assetSource(project,asset.structure,2*1024*1024));
    if(typeof asset.terrain_heights==="string"&&!assets.has(asset.terrain_heights))assets.set(asset.terrain_heights,await assetSource(project,asset.terrain_heights,TERRAIN_DATA_MAX_BYTES));
    const defaults = transform(asset.transform);
    const raymarched = asset.kind === "raymarched-terrain" ? raymarchedSettings(asset.raymarched_terrain) : undefined;
    const material=materialSettings(asset.terrain_paint?{terrain_paint:asset.terrain_paint,shading:"smooth"}:item.material??asset.material);
    const resolved=material.asset?(await readMaterial(project,material.asset)):undefined;
    if(resolved)assets.set(resolved.path,resolved.source);
    const surface=resolved?.material??material;
    if(material.terrain_paint&&!assets.has(material.terrain_paint))assets.set(material.terrain_paint,await assetSource(project,material.terrain_paint,TERRAIN_DATA_MAX_BYTES));
    for(const channel of materialChannels){const map=surface[`${channel}_map`];if(map&&!assets.has(map.asset))assets.set(map.asset,await assetSource(project,map.asset));}
    for(const map of materialTextures(surface)){
      const target=await containedFile(project.root,map.asset,16*1024*1024);
      if(!project.assetRoots.some(root=>map.asset.startsWith(`${root}/`)))throw new Error("Texture is outside project assets.");
      hash.update(map.asset).update(await readFile(target));
    }
    const geometry=asset.geometry as {color?:number[]}|undefined;
    if(raymarched) instances.push({id:item.id as string,asset:item.asset as string,parent:item.parent as string|undefined??null,name:typeof item.name==="string"?item.name:typeof asset.name==="string"?asset.name:item.id as string,defaults,transform:transform(item.transform,defaults),overridden:item.transform!==undefined,terrain:true,raymarched:true,terrainExtent:{size:raymarched.size,height:raymarched.version===1?raymarched.size*0.1875:12+raymarched.relief+raymarched.overhang},renderable:true});
    else
    instances.push({ camera:asset.kind==="camera"?cameraSettings(item.camera??asset.camera):undefined,cameraOverridden:item.camera!==undefined, id: item.id, name: item.name === undefined ? (typeof asset.name === "string" && asset.name.trim() ? asset.name : item.id) : instanceName(item.name), asset: item.asset, parent: item.parent as string | undefined ?? null, defaults, transform: transform(item.transform,defaults), overridden: item.transform !== undefined, physics:physicsSettings(item.physics??asset.physics), physicsOverridden:item.physics!==undefined,particles:asset.kind==="particle-effect",structure:asset.kind==="structure",terrain:asset.kind==="terrain",terrainExtent:asset.terrain?(({size,height})=>({size,height}))(terrainSettings(asset.terrain)):undefined,renderable:Boolean(asset.geometry||asset.terrain||asset.structure),material,materialOverridden:item.material!==undefined,geometryColor:geometry?.color?.slice(0,3) });
  }
  // Include picker candidates too, so attaching a newly selected map cannot race its edits.
  for(const file of project.files.filter(file=>file.path.endsWith(".noise.toml")&&project.assetRoots.some(root=>file.path.startsWith(`${root}/`))))if(!assets.has(file.path))assets.set(file.path,await assetSource(project,file.path));
  for(const file of project.files.filter(file=>file.path.endsWith(".material.toml"))){const library=await readMaterial(project,file.path);assets.set(file.path,library.source);for(const channel of materialChannels){const map=library.material[`${channel}_map`];if(map&&!assets.has(map.asset))assets.set(map.asset,await assetSource(project,map.asset));}for(const map of materialTextures(library.material))hash.update(map.asset).update(await readFile(await containedFile(project.root,map.asset,16*1024*1024)));}
  for (const [path,text] of assets) hash.update(JSON.stringify([path,text]));
  sceneHierarchy(instances);
  return { source, placement: { path, revision:hash.digest("hex"), instances,space:"local" as TransformSpace,frame:undefined as ScenePlacement["frame"] } satisfies ScenePlacement };
}

export async function scenePlacement(project: ProjectSnapshot, path: string, space:TransformSpace="local") {
  const {placement}=await snapshot(project,path);
  if(space==="global"||placement.instances.some(instance=>instance.terrain)){
    const result=await sceneWorldTransforms(project,path);
    const reports=new Map((result.instances as unknown[]).map(value=>{const entry=table(value);return [entry.id,entry];}));
    for(const instance of placement.instances){
      if(space!=="global")break;
      const report=reports.get(instance.id);if(!report)throw new Error("Rust transform report is missing an instance.");
      instance.globalTranslationOnly=!report.global;
      instance.display=report.global?transform(report.global,identity,"global"):{...instance.transform,translation:transform({translation:report.translation}).translation};
    }
    if((await snapshot(project,path)).placement.revision!==placement.revision)throw new Error("Scene changed while reading global transforms. Reload the inspector.");
    placement.space=space;
    placement.frame=terrainSceneFrame(placement.instances,reports);
  }
  return placement;
}

/** Conservative bounds around terrain origins; handles parent rotation and nonuniform scale. */
export function terrainSceneFrame(instances:SceneInstance[],reports:Map<unknown,Record<string,unknown>>){
 const min=[Infinity,Infinity,Infinity],max=[-Infinity,-Infinity,-Infinity];const spheres:{position:number[];radius:number}[]=[];
 const byId=new Map(instances.map(instance=>[instance.id,instance]));
 for(const instance of instances){
  if(!instance.terrainExtent)continue;
  const position=reports.get(instance.id)?.translation as number[]|undefined;if(!position)continue;
  let scale=1,current:SceneInstance|undefined=instance;
  while(current){scale=Math.min(1000000,scale*Math.max(...current.transform.scale.map(Math.abs)));current=current.parent?byId.get(current.parent):undefined;}
  const radius=Math.hypot(instance.terrainExtent.size/Math.sqrt(2),instance.terrainExtent.height)*scale;
  for(let axis=0;axis<3;axis++){min[axis]=Math.min(min[axis],position[axis]-radius);max[axis]=Math.max(max[axis],position[axis]+radius);}spheres.push({position,radius});
 }
 if(!spheres.length)return undefined;
 const target=min.map((value,axis)=>Math.max(-10000,Math.min(10000,(value+max[axis])/2)));
 const radius=Math.max(...spheres.map(sphere=>sphere.radius+Math.hypot(...sphere.position.map((value,axis)=>value-target[axis]))));
 return {target,distance:Math.max(2,Math.min(30000,radius*2.6))};
}

/** Edit only the selected instance's table; reject syntax we cannot preserve safely. */
export function replaceInstanceTransform(source: string, id: string, value: TransformValues | null) {
  const expected = parse(source);
  if (!Array.isArray(expected.entities)) throw new Error("Scene instances are missing.");
  const index = expected.entities.findIndex(entry => table(entry).id === id);
  if (index < 0) throw new Error("Scene instance no longer exists.");
  const starts = [...source.matchAll(/^[\t ]*\[\[entities\]\][\t ]*(?:#.*)?\r?$/gm)];
  if (starts.length !== expected.entities.length) throw new Error("Placement editing requires [[entities]] tables. Reload or edit this scene in source.");
  const start = starts[index].index!;
  const end = starts[index+1]?.index ?? source.length;
  let block = source.slice(start,end);
  const header = /^[\t ]*\[entities\.transform\][\t ]*(?:#.*)?\r?$/m.exec(block);
  if (header) {
    const tail = block.slice(header.index + header[0].length);
    const next = tail.search(/^[\t ]*\[/m);
    const stop = next < 0 ? block.length : header.index + header[0].length + next;
    const comments = block.slice(header.index,stop).split("\n").map(line => line.match(/#.*$/)?.[0]).filter(Boolean).join("\n");
    block = block.slice(0,header.index) + (comments ? `${comments}\n` : "") + block.slice(stop);
  }
  const item = table(expected.entities[index]);
  if (value === null) delete item.transform;
  else {
    const vectors = transform(value);
    const text = fields.map(field => `${field} = ${JSON.stringify(vectors[field])}`).join("\n");
    item.transform = parse(text);
    block = block.trimEnd() + `\n\n[entities.transform]\n${text}\n\n`;
  }
  const content = source.slice(0,start) + block + source.slice(end);
  if (Buffer.byteLength(content) > 256 * 1024 || !isDeepStrictEqual(parse(content), expected)) throw new Error("Could not safely edit this transform. Use table-form transforms or edit the scene in source.");
  return content;
}

function instanceName(value:unknown):string {
  if(typeof value!=="string" || !value.trim() || Array.from(value.trim()).length>64 || /\p{Cc}/u.test(value))throw new Error("Instance name must contain 1–64 printable characters.");
  return value.trim();
}

export function replaceInstanceName(source:string,id:string,value:unknown) {
  const name=instanceName(value),expected=parse(source);
  if(!Array.isArray(expected.entities))throw new Error("Scene instances are missing.");
  const index=expected.entities.findIndex(entry=>table(entry).id===id);
  if(index<0)throw new Error("Scene instance no longer exists.");
  table(expected.entities[index]).name=name;
  const starts=[...source.matchAll(/^[\t ]*\[\[entities\]\][\t ]*(?:#.*)?\r?$/gm)];
  if(starts.length!==expected.entities.length)throw new Error("Renaming requires [[entities]] tables.");
  const start=starts[index].index!,end=starts[index+1]?.index??source.length,block=source.slice(start,end);
  const headerEnd=block.indexOf("\n")+1,subtable=block.slice(headerEnd).search(/^[\t ]*\[/m),topEnd=subtable<0?block.length:headerEnd+subtable;
  const top=block.slice(0,topEnd).replace(/^[\t ]*name\s*=[^\n]*(?:\n|$)/m,line=>{const note=line.match(/#.*/)?.[0];return note?`${note}\n`:"";});
  const content=source.slice(0,start)+top.slice(0,headerEnd)+`name = ${JSON.stringify(name)}\n`+top.slice(headerEnd)+block.slice(topEnd)+source.slice(end);
  if(Buffer.byteLength(content)>256*1024 || !isDeepStrictEqual(parse(content),expected))throw new Error("Could not safely rename this instance. Edit the scene in TOML instead.");
  return content;
}

export async function renameSceneInstance(project:ProjectSnapshot,path:string,id:string,revision:string,name:unknown,space:TransformSpace="local") {
  return commitPlacement(project,path,revision,space,source=>replaceInstanceName(source,id,name));
}

const state = globalThis as typeof globalThis & { emergentScenePlacementActive?: boolean };
export async function saveScenePlacement(project: ProjectSnapshot, path: string, id: string, revision: string, value: unknown, space:TransformSpace="local",positionOnly=false) {
  return commitPlacement(project,path,revision,space,transformEdit(project,path,id,value,space,positionOnly));
}

/** Complete-transform edit; global values convert through the Rust world so parent shear is handled natively. */
export function transformEdit(project:ProjectSnapshot,path:string,id:string,value:unknown,space:TransformSpace="local",positionOnly=false) {
  return async (source:string)=>{
    if(value!==null&&fields.some(field=>!(field in table(value))))throw new Error("A placement edit must supply position, rotation and scale vectors.");
    let next=value===null?null:transform(value,identity,space);
    if(next && space==="global")next=transform(await localFromGlobal(project,path,id,next,positionOnly));
    return replaceInstanceTransform(source,id,next);
  };
}

export function replaceInstanceParent(source:string,id:string,parent:string|null) {
  const expected=parse(source);if(!Array.isArray(expected.entities))throw new Error("Scene instances are missing.");
  const index=expected.entities.findIndex(entry=>table(entry).id===id);if(index<0)throw new Error("Scene instance no longer exists.");
  const item=table(expected.entities[index]);if(parent===null)delete item.parent;else item.parent=parent;
  sceneHierarchy(expected.entities.map(value=>{const node=table(value);return {id:node.id as string,parent:node.parent as string??null};}));
  const starts=[...source.matchAll(/^[\t ]*\[\[entities\]\][\t ]*(?:#.*)?\r?$/gm)];
  if(starts.length!==expected.entities.length)throw new Error("Reparenting requires [[entities]] tables.");
  const start=starts[index].index!,end=starts[index+1]?.index??source.length,block=source.slice(start,end);
  const headerEnd=block.indexOf("\n")+1,subtable=block.slice(headerEnd).search(/^[\t ]*\[/m);
  const topEnd=subtable<0?block.length:headerEnd+subtable;
  let top=block.slice(0,topEnd).replace(/^[\t ]*parent\s*=[^\n]*(?:\n|$)/m,line=>{const note=line.match(/#.*$/)?.[0];return note?`${note}\n`:"";});
  if(parent!==null)top=top.slice(0,headerEnd)+`parent = ${JSON.stringify(parent)}\n`+top.slice(headerEnd);
  const content=source.slice(0,start)+top+block.slice(topEnd)+source.slice(end);
  if(Buffer.byteLength(content)>256*1024 || !isDeepStrictEqual(parse(content),expected))throw new Error("Could not safely edit this parent. Edit the scene in TOML instead.");
  return content;
}

export function replaceSceneSubtree(source:string,id:string) {
  const expected=parse(source);
  if(!Array.isArray(expected.entities))throw new Error("Scene instances are missing.");
  const nodes=expected.entities.map(value=>table(value));
  const target=nodes.find(node=>node.id===id);
  if(!target)throw new Error("Scene instance no longer exists.");
  const children=new Map<string,Set<string>>();
  for(const node of nodes)if(typeof node.parent==="string"){
    const group=children.get(node.parent)??new Set<string>();
    group.add(node.id as string);
    children.set(node.parent,group);
  }
  const removed=new Set<string>([id]);
  const pending=[id];
  while(pending.length){
    for(const child of children.get(pending.pop()!)??[])if(!removed.has(child)){removed.add(child);pending.push(child);}
  }
  const starts=[...source.matchAll(/^[\t ]*\[\[entities\]\][\t ]*(?:#.*)?\r?$/gm)];
  if(starts.length!==expected.entities.length)throw new Error("Deleting instances requires [[entities]] tables.");
  // An instance's text ends at the next [[entities]] or at the next table that is not one of its own subtables
  // (for example a trailing [environment]); that text is kept even when the last instance is deleted.
  const headers=[...source.matchAll(/^[\t ]*\[{1,2}([^\]\n]+)\]{1,2}[\t ]*(?:#.*)?\r?$/gm)];
  const blockEnd=(start:number)=>headers.find(match=>match.index!>start&&(match[0].trim().startsWith("[[entities]]")||!match[1].trim().startsWith("entities.")))?.index??source.length;
  let content=source;
  const remaining=expected.entities.filter(value=>!removed.has(table(value).id as string));
  for(let index=expected.entities.length-1;index>=0;index--){
    if(!removed.has(table(expected.entities[index]).id as string))continue;
    const start=starts[index].index!;
    content=content.slice(0,start)+content.slice(blockEnd(start));
  }
  expected.entities=remaining;
  if(typeof expected.active_camera==="string"&&removed.has(expected.active_camera)){
    delete expected.active_camera;
    const headerEnd=starts[0]?.index??source.length;
    const header=content.slice(0,headerEnd).replace(/^[\t ]*active_camera\s*=[^\n]*(?:\n|$)/m,line=>{
      const note=line.match(/#.*$/)?.[0];
      return note?`${note}\n`:"";
    });
    content=header+content.slice(headerEnd);
  }
  sceneHierarchy(remaining.map(value=>{const node=table(value);return {id:node.id as string,parent:node.parent as string|undefined??null};}));
  if(Buffer.byteLength(content)>256*1024||!isDeepStrictEqual(parse(content),expected))throw new Error("Could not safely delete this instance subtree. Edit the scene in source instead.");
  return content;
}

export async function deleteSceneInstance(project:ProjectSnapshot,path:string,id:string,revision:string,space:TransformSpace="local") {
  return commitPlacement(project,path,revision,space,source=>replaceSceneSubtree(source,id));
}

export async function reparentSceneInstance(project:ProjectSnapshot,path:string,id:string,parent:string|null,revision:string,space:TransformSpace="local") {
  return commitPlacement(project,path,revision,space,source=>replaceInstanceParent(source,id,parent));
}

async function commitPlacement(project:ProjectSnapshot,path:string,revision:string,space:TransformSpace,edit:(source:string)=>string|Promise<string>,check?:()=>Promise<void>) {
  await commitSceneEdit(project,path,revision,edit,{check});
  return scenePlacement(project,path,space);
}

/**
 * The single guarded scene write: revision check, text-preserving edit, native Rust validation,
 * revision recheck, then atomic publication. `dryRun` stops after validation and publishes nothing.
 */
export async function commitSceneEdit(project:ProjectSnapshot,path:string,revision:string,edit:(source:string)=>string|Promise<string>,options:{check?:()=>Promise<void>;dryRun?:boolean}={}) {
  if (state.emergentScenePlacementActive) throw new Error("Another placement is being saved. Wait for it to finish.");
  state.emergentScenePlacementActive = true;
  try {
    const before = await snapshot(project,path);
    if (before.placement.revision !== revision) throw new Error("Scene or asset changed externally. Reload the inspector before saving.");
    const content = await edit(before.source);
    await validateProjectScene(project,path,content);
    if ((await snapshot(project,path)).placement.revision !== revision) throw new Error("Scene or asset changed during validation. Reload the inspector before saving.");
    await options.check?.();
    if (!options.dryRun) await applyChanges(project,[{path,before:before.source,content}]);
    return { before: before.source, content, changed: content !== before.source };
  } finally { state.emergentScenePlacementActive = false; }
}


/** Preserve unrelated scene text and prove that only the selected settings table changed. */
function replaceInstanceSettings(source:string,id:string,section:"physics"|"material"|"camera",settings:PhysicsSettings|MaterialSettings|CameraSettings|null) {
  const expected=parse(source);if(!Array.isArray(expected.entities))throw new Error("Scene instances are missing.");
  const index=expected.entities.findIndex(entry=>table(entry).id===id);if(index<0)throw new Error("Scene instance no longer exists.");
  const item=table(expected.entities[index]);if(settings===null)delete item[section];else item[section]=parse(stringify({[section]:settings}))[section];
  const starts=[...source.matchAll(/^[\t ]*\[\[entities\]\][\t ]*(?:#.*)?\r?$/gm)];
  if(starts.length!==expected.entities.length)throw new Error("Settings editing requires [[entities]] tables.");
  const start=starts[index].index!,end=starts[index+1]?.index??source.length;
  let block=source.slice(start,end);
  const headers=[...block.matchAll(/^[\t ]*\[{1,2}([^\]\n]+)\]{1,2}[\t ]*(?:#.*)?\r?$/gm)];
  for(let i=headers.length-1;i>=0;i--){const match=headers[i];if(match[1]===`entities.${section}`||match[1].startsWith(`entities.${section}.`)){
    const stop=headers[i+1]?.index??block.length;
    const comments=block.slice(match.index,stop).split("\n").map(line=>line.match(/#.*$/)?.[0]).filter(Boolean).join("\n");
    block=block.slice(0,match.index)+(comments?`${comments}\n`:"")+block.slice(stop);
  }}
  if(settings!==null){const addition=stringify({entities:[{[section]:settings}]}).replace(/^\[\[entities\]\]\s*\n/,"");block=block.trimEnd()+"\n"+addition+"\n";}
  const content=source.slice(0,start)+block+source.slice(end);
  if(Buffer.byteLength(content)>256*1024||!isDeepStrictEqual(parse(content),expected))throw new Error("Could not safely edit settings. Edit the scene in TOML instead.");
  return content;
}
export function replaceInstancePhysics(source:string,id:string,value:unknown) {
  return replaceInstanceSettings(source,id,"physics",value===null?null:physicsSettings(value));
}
export function replaceInstanceMaterial(source:string,id:string,value:unknown) {
  return replaceInstanceSettings(source,id,"material",value===null?null:materialSettings(value));
}
export async function saveSceneMaterial(project:ProjectSnapshot,path:string,id:string,revision:string,value:unknown,space:TransformSpace="local") {
  const {edit,check}=await materialEdit(project,id,value);
  return commitPlacement(project,path,revision,space,edit,check);
}

/** Material override edit plus a check that referenced textures did not change during validation. */
export async function materialEdit(project:ProjectSnapshot,id:string,value:unknown) {
  const settings=value===null?null:materialSettings(value);
  for(const channel of materialChannels){const map=settings?.[`${channel}_map`];if(map&&!project.files.some(file=>file.path===map.asset&&file.path.endsWith(".noise.toml")))throw new Error("Choose a noise filter from this project.");}
  for(const map of settings?materialTextures(settings):[])if(!project.files.some(file=>file.path===map.asset&&isTexturePath(file.path)))throw new Error("Choose a PNG or JPEG texture in this project.");
  const stamps=await Promise.all((settings?materialTextures(settings):[]).map(async map=>({path:map.asset,hash:createHash("sha256").update(await readFile(await containedFile(project.root,map.asset,16*1024*1024))).digest("hex")})));
  return {edit:(source:string)=>replaceInstanceMaterial(source,id,settings),check:async()=>{for(const stamp of stamps)if(createHash("sha256").update(await readFile(await containedFile(project.root,stamp.path,16*1024*1024))).digest("hex")!==stamp.hash)throw new Error("Texture changed during validation. Reload before saving.");}};
}
export async function saveScenePhysics(project:ProjectSnapshot,path:string,id:string,revision:string,value:unknown,space:TransformSpace="local") {
  return commitPlacement(project,path,revision,space,source=>replaceInstancePhysics(source,id,value));
}
export function replaceInstanceCamera(source:string,id:string,value:unknown) {
 return replaceInstanceSettings(source,id,"camera",value===null?null:cameraSettings(value));
}
export async function saveSceneCamera(project:ProjectSnapshot,path:string,id:string,revision:string,value:unknown,space:TransformSpace="local") {
 return commitPlacement(project,path,revision,space,source=>replaceInstanceCamera(source,id,value));
}
