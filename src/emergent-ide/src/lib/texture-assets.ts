import "server-only";
import {execFile} from "node:child_process";
import {randomUUID} from "node:crypto";
import {lstat,mkdir,mkdtemp,readFile,realpath,rm,writeFile} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {promisify} from "node:util";
import {missing,within} from "./object-files";
import {readProjectBinding,type ProjectSnapshot} from "./project";
import {foundationRoot} from "./project-code-context";
const execute=promisify(execFile);
export async function validateTextureImage(project:ProjectSnapshot,bytes:Buffer,skybox=false){
 const extension=bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10]))?"png":bytes[0]===255&&bytes[1]===216&&bytes[2]===255?"jpg":null;
 if(!extension||bytes.length>16*1024*1024)throw new Error("Choose a PNG/JPEG of at most 16 MiB.");
 const temporary=await mkdtemp(path.join(os.tmpdir(),"emergent-texture-"));
 try{
  const candidate=path.join(temporary,`image.${extension}`);await writeFile(candidate,bytes);const root=await foundationRoot(project);
  try{const result=await execute("cargo",["run","--quiet","--manifest-path",path.join(root,"Cargo.toml"),"--example","texture_validate","--",candidate],{cwd:root,shell:false,timeout:120000,maxBuffer:8192,windowsHide:true});const [width,height]=result.stdout.trim().split(/\s+/).map(Number);if(skybox&&width!==height*2)throw new Error("Skyboxes require a 2:1 equirectangular panorama.");}
  catch(error){throw new Error(error&&typeof error==="object"&&"stderr"in error?String(error.stderr).slice(-3000):error instanceof Error?error.message:"Image validation failed.");}
  const binding=await readProjectBinding();if(binding.id!==project.id||binding.root!==project.root)throw new Error("Linked project changed during image import.");
  return extension;
 }finally{await rm(temporary,{recursive:true,force:true});}
}

export async function importTexture(project:ProjectSnapshot,bytes:Buffer,filename:string,identity?:string){
 if(identity!==undefined&&!/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/.test(identity))throw new Error("Invalid texture identity.");
 const extension=await validateTextureImage(project,bytes);
  const name=path.basename(filename,path.extname(filename)).replace(/[^a-z0-9 _-]/gi,"").trim().slice(0,64)||"texture";
  if(!project.assetRoots[0])throw new Error("Project has no asset root.");
  const relative=`${project.assetRoots[0]}/textures/${identity??randomUUID()}/${name}.${extension}`;
  let current=await realpath(project.root);
  for(const segment of relative.split("/")){if(!segment||segment===".."||segment==="."||segment.includes("\\")||segment.includes(":"))throw new Error("Invalid texture destination.");current=path.join(current,segment);try{if((await lstat(current)).isSymbolicLink())throw new Error("Cannot import through a symbolic link.");}catch(error){if(!missing(error))throw error;}}
  await mkdir(path.dirname(current),{recursive:true});if(!within(await realpath(project.root),await realpath(path.dirname(current))))throw new Error("Texture destination escapes project.");
  try{await writeFile(current,bytes,{flag:"wx"});}catch(error){if(!identity||!error||typeof error!=="object"||!("code"in error)||error.code!=="EEXIST"||!(await readFile(current)).equals(bytes))throw error;}
  return {path:relative,name:`${name}.${extension}`};

}
