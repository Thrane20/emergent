import {TERRAIN_SAVE_MAX_BYTES} from "@/lib/terrain";
import {objectProject,sameOrigin} from "@/lib/object-http";
import {boundedBody} from "@/lib/object-files";
import {saveTerrain} from "@/lib/terrain-assets";
export async function POST(request:Request){
 try{
  sameOrigin(request);
  const body=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),TERRAIN_SAVE_MAX_BYTES)).toString("utf8"));
  const project=await objectProject(body.projectId);
  return Response.json(await saveTerrain(project,body),{status:201,headers:{"Cache-Control":"no-store"}});
 }catch(error){return Response.json({error:error instanceof Error?error.message:"Terrain could not be saved."},{status:400});}
}
