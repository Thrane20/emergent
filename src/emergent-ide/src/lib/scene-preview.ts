import "server-only";
import { execFile } from "node:child_process";
import { mkdtemp, readdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { parse, stringify } from "smol-toml";
import { componentTypes, type ComponentType } from "./components";
import { projectScenes } from "./project-scenes";
import type { ProjectSnapshot } from "./project";

const execute = promisify(execFile);
const state = globalThis as typeof globalThis & { emergentScenePreviewActive?: boolean };

/** Runs one of the linked game binary's tool commands and returns its stdout. */
export async function gameTool(project:ProjectSnapshot,args:string[],maxBuffer=1024*1024) {
  try {return (await execute("cargo",["run","--quiet","--manifest-path",path.join(project.root,"Cargo.toml"),"--",...args],{cwd:project.root,shell:false,timeout:120000,maxBuffer,windowsHide:true})).stdout;}
  catch(error){const message=error&&typeof error==="object"&&"stderr"in error?String(error.stderr).trim().slice(-6000):"Rust scene tool failed.";throw new Error(message);}
}
const registries=new Map<string,{stamp:string;types:Promise<ComponentType[]>}>();
/** Newest Rust source change in the game crate; project component types are declared in Rust. */
async function sourceStamp(directory:string,depth=0):Promise<number>{
  if(depth>8)return 0;
  let newest=0;
  for(const entry of await readdir(directory,{withFileTypes:true}).catch(()=>[])){
    const target=path.join(directory,entry.name);
    if(entry.isDirectory())newest=Math.max(newest,await sourceStamp(target,depth+1));
    else if(entry.name.endsWith(".rs"))newest=Math.max(newest,(await stat(target)).mtimeMs);
  }
  return newest;
}
/** Component types exported by the linked game binary, re-read when its Rust source or manifest changes. */
export async function componentRegistry(project:ProjectSnapshot) {
  const stamp=`${await sourceStamp(path.join(project.root,"src"))}:${(await stat(path.join(project.root,"Cargo.toml"))).mtimeMs}`;
  let cached=registries.get(project.root);
  if(!cached||cached.stamp!==stamp){
    const types=gameTool(project,["--export-components"]).then(text=>componentTypes(parse(text)));
    cached={stamp,types};registries.set(project.root,cached);types.catch(()=>{if(registries.get(project.root)===cached)registries.delete(project.root);});
  }
  return cached.types;
}
export async function sceneWorldTransforms(project:ProjectSnapshot,scene:string) {
  return parse(await gameTool(project,["--scene-transforms",scene]));
}
export async function localFromGlobal(project:ProjectSnapshot,scene:string,id:string,values:{translation:number[];rotation_degrees:number[];scale:number[]},positionOnly:boolean) {
  const directory=await mkdtemp(path.join(os.tmpdir(),"emergent-global-transform-"));
  try{const file=path.join(directory,"transform.toml");await writeFile(file,stringify(values));return parse(await gameTool(project,["--convert-global",scene,id,file,...(positionOnly?["--position-only"]:[])]));}
  finally{await rm(directory,{recursive:true,force:true});}
}

export async function validateProjectScene(project: ProjectSnapshot, scenePath: string, source: string) {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-scene-validation-"));
  try {
    const candidate = path.join(directory, "candidate.scene.toml");
    await writeFile(candidate, source);
    try {
      await execute("cargo", ["run", "--quiet", "--manifest-path", path.join(project.root, "Cargo.toml"), "--", "--validate-scene", scenePath, "--candidate", candidate], {
        cwd: project.root, shell: false, timeout: 120_000, maxBuffer: 1024 * 1024, killSignal: "SIGTERM", windowsHide: true,
      });
    } catch (error) {
      const detail = error && typeof error === "object" && "stderr" in error ? String(error.stderr).trim().slice(-6000) : "";
      throw new Error(detail || "Scene validation failed. This game must support --validate-scene / --candidate.");
    }
  } finally { await rm(directory, { recursive: true, force: true }); }
}

export async function renderProjectScene(project: ProjectSnapshot, scenePath: string) {
  if (!projectScenes(project).some((scene) => scene.path === scenePath)) throw new Error("Choose a scene from the linked project.");
  if (state.emergentScenePreviewActive) throw new Error("A Rust scene preview is already building. Wait for it to finish.");
  state.emergentScenePreviewActive = true;
  let directory: string | undefined;
  try {
    directory = await mkdtemp(path.join(os.tmpdir(), "emergent-scene-preview-"));
    const output = path.join(directory, "frame.png");
    try {
      await execute("cargo", ["run", "--quiet", "--manifest-path", path.join(project.root, "Cargo.toml"), "--", "--preview-scene", scenePath, "--preview-output", output], {
        cwd: project.root, shell: false, timeout: 120_000, maxBuffer: 1024 * 1024,
        killSignal: "SIGTERM", windowsHide: true,
      });
    } catch (error) {
      const detail = error && typeof error === "object" && "stderr" in error ? String(error.stderr).trim().slice(-6000) : "";
      throw new Error(detail || "Rust scene preview could not run. Check that Rust is installed and this game supports --preview-scene / --preview-output.");
    }
    if ((await stat(output)).size > 16 * 1024 * 1024) throw new Error("Rendered preview exceeds 16 MiB.");
    const bytes = await readFile(output);
    if (!bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10]))) throw new Error("Rust preview did not produce a PNG frame.");
    return bytes;
  } finally {
    if (directory) await rm(directory, { recursive: true, force: true });
    state.emergentScenePreviewActive = false;
  }
}
