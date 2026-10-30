import {objectProject,sameOrigin} from "@/lib/object-http";
import {boundedBody} from "@/lib/object-files";
import {readTomlDocument,saveTomlDocument} from "@/lib/project-toml";
import {noiseSource,replaceNoiseSettings} from "@/lib/noise-field";
import {runNoiseTool} from "@/lib/noise-tool";
export async function GET(request:Request){
 try{const query=new URL(request.url).searchParams,project=await objectProject(query.get("projectId")),path=query.get("path")??"";if(!path.endsWith(".noise.toml"))throw new Error("Choose a noise filter.");const document=await readTomlDocument(project,path);return Response.json({...document,settings:noiseSource(document.content).settings},{headers:{"Cache-Control":"no-store"}});}catch(error){return failure(error);}
}
export async function POST(request:Request){return update(request,false);}
export async function PUT(request:Request){return update(request,true);}
async function update(request:Request,save:boolean){
 try{sameOrigin(request);const body=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),1024*1024)).toString("utf8"));const project=await objectProject(body.projectId);
  if(typeof body.path!=="string"||!body.path.endsWith(".noise.toml"))throw new Error("Choose a noise filter.");
  const before=await readTomlDocument(project,body.path),candidate=replaceNoiseSettings(before.content,body.settings);
  if(save){if(typeof body.revision!=="string")throw new Error("Source revision is required.");const saved=await saveTomlDocument(project,body.path,candidate,body.revision);return Response.json({...saved,settings:noiseSource(saved.content).settings});}
  const bytes=await runNoiseTool(project,body.path,candidate,true);
  return new Response(new Uint8Array(bytes),{headers:{"Content-Type":"application/octet-stream","Cache-Control":"no-store","X-Content-Type-Options":"nosniff"}});
 }catch(error){return failure(error);}
}
function failure(error:unknown){return Response.json({error:error instanceof Error?error.message:"Noise operation failed."},{status:400});}
