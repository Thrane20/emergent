import "server-only";
import path from "node:path";
import {execFile} from "node:child_process";
import {promisify} from "node:util";
import {lstat,realpath,readFile} from "node:fs/promises";
import {containedFile,missing,within,writeJson} from "./object-files";
import {validateTextureImage} from "./texture-assets";
import {imageLibrarySettings,projectImages} from "./image-library";
import {AssetDeletionError,deleteProjectAsset} from "./project-assets";
import type {ProjectSnapshot} from "./project";

export async function deleteLibraryImage(project:ProjectSnapshot,relative:string){
 if(!projectImages(project).some(image=>image.path===relative))throw new AssetDeletionError("Choose an image from this project's library.",404);
 const owners=project.files.filter(file=>path.posix.dirname(file.path)===path.posix.dirname(relative)&&["texture.asset.json","particle-media.asset.json"].includes(file.name)&&file.content).filter(file=>{
  try{const metadata=JSON.parse(file.content!);return metadata.schema_version===1&&(file.name==="texture.asset.json"?metadata.kind==="generated-texture"&&metadata.image===relative:metadata.atlas?.path===relative);}catch{return false;}
 });
 const ownerPaths=new Set(owners.map(file=>file.path));
 const indirect=project.connections.filter(connection=>ownerPaths.has(connection.target)&&!ownerPaths.has(connection.source));
 if(indirect.length)throw new AssetDeletionError(`Image metadata is still referenced by ${indirect.map(connection=>connection.source).join(", ")}.`,409);
 // Keep provenance for re-adding a cached candidate; its own image link is not a runtime use.
 const historicalSources=new Set(project.files.filter(file=>file.name==="particle-media.asset.json"&&file.content).filter(file=>{try{const metadata=JSON.parse(file.content!);return metadata.output==="sprite"&&metadata.source===relative;}catch{return false;}}).map(file=>file.path));
 await deleteProjectAsset({...project,connections:project.connections.filter(connection=>!ownerPaths.has(connection.source)&&!historicalSources.has(connection.source))},relative);
}

export async function configureLibraryImage(project:ProjectSnapshot,relative:string,value:unknown){
 let settings=imageLibrarySettings(value);
 if(!relative||!/[.](png|jpe?g|webp)$/i.test(relative)||relative.includes("\\")||relative.includes(":")||relative.split("/").some(part=>!part||part==="."||part===".."||part.startsWith("."))||!project.assetRoots.some(root=>relative.startsWith(`${root.replace(/\/$/,"")}/`)))throw new AssetDeletionError("Choose a project library image.",403);
 const root=await realpath(project.root);let current=root;
 for(const part of relative.split("/")){current=path.join(current,part);if((await lstat(current)).isSymbolicLink())throw new AssetDeletionError("Cannot configure images through symbolic links.",403);}
 const image=await containedFile(root,relative,16*1024*1024);let allowed=false;
 for(const assetRoot of project.assetRoots){const canonical=await realpath(path.resolve(root,assetRoot));if(within(root,canonical)&&within(canonical,image))allowed=true;}
 if(!allowed)throw new AssetDeletionError("Image is outside project asset roots.",403);
 if(settings.kind==="skybox")await validateTextureImage(project,await readFile(image),true);
 if(settings.kind==="sprite"){
  const {stdout}=await promisify(execFile)("ffprobe",["-v","error","-protocol_whitelist","file,pipe","-select_streams","v:0","-show_entries","stream=width,height","-of","json",image],{timeout:15000,maxBuffer:8192,shell:false});const dimensions=JSON.parse(stdout).streams?.[0];
  if(!dimensions||dimensions.width%settings.sprite.columns||dimensions.height%settings.sprite.rows)throw new Error("Sprite rows and columns must divide the image dimensions evenly.");
 }
 const sidecar=`${image}.library.json`;try{if((await lstat(sidecar)).isSymbolicLink())throw new AssetDeletionError("Cannot write settings through a symbolic link.",403);}catch(error){if(!missing(error))throw error;}
 if(!settings.family)try{const previous=JSON.parse(await readFile(sidecar,"utf8"));if(typeof previous.family==="string"&&/^[a-f0-9-]{36}$/.test(previous.family))settings={...settings,family:previous.family};}catch(error){if(!missing(error)&&!(error instanceof SyntaxError))throw error;}
 await writeJson(sidecar,{schema_version:1,...settings});
 return {path:relative,name:projectImages(project).find(item=>item.path===relative)?.name??path.basename(relative,path.extname(relative)),modifiedAt:new Date().toISOString(),kind:settings.kind,...(settings.family?{family:settings.family}:{}),...(settings.kind==="sprite"?{sprite:settings.sprite}:{})};
}
