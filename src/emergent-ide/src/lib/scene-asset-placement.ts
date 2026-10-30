import "server-only";
import {createHash,randomUUID} from "node:crypto";
import {readFile} from "node:fs/promises";
import {parse} from "smol-toml";
import type {ProjectSnapshot} from "./project";
import {projectScenes} from "./project-scenes";
import {containedFile} from "./object-files";
export async function prepareSceneAssetPlacement(project:ProjectSnapshot,input:{assetPath:string;scenePath:unknown;sceneRevision:unknown}) {
 if(typeof input.scenePath!=="string"||!projectScenes(project).some(s=>s.path===input.scenePath))throw new Error("Choose a project scene.");
 if(!project.assetRoots.some(root=>(input.scenePath as string).startsWith(`${root}/`)))throw new Error("Choose a project asset scene.");
 const before=await readFile(await containedFile(project.root,input.scenePath,256*1024),"utf8");if(createHash("sha256").update(before).digest("hex")!==input.sceneRevision)throw new Error("Destination scene changed. Refresh before placing the asset.");
 const scene=parse(before);if(scene.schema_version!==1||!Array.isArray(scene.entities)||scene.entities.length>=1024)throw new Error("Scene is full or unsupported.");
 const instanceId=randomUUID(),first=before.search(/^\[/m),header=first<0?before:before.slice(0,first),tail=first<0?"":before.slice(first);
 const content=header.replace(/^entities\s*=\s*\[\s*\][^\S\r\n]*(?:#[^\r\n]*)?$/m,"")+tail+`\n[[entities]]\nid = ${JSON.stringify(instanceId)}\nasset = ${JSON.stringify(input.assetPath)}\n`;
 parse(content);if(Buffer.byteLength(content)>256*1024)throw new Error("Destination scene exceeds 256 KiB.");
 return {change:{path:input.scenePath,before,content},instanceId};
}
