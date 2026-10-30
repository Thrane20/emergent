import {boundedBody} from "@/lib/object-files";
import {objectProject,sameOrigin} from "@/lib/object-http";
import {textureJobs} from "@/lib/texture-jobs";
import {ObjectCreatorError} from "@/lib/object-types";
export async function POST(request:Request,context:{params:Promise<{id:string}>}){
 try{sameOrigin(request);const body=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),8192)).toString("utf8")),project=await objectProject(body.projectId),{id}=await context.params;
 if(body.action==="delete"&&typeof body.resultId==="string")return Response.json(await textureJobs.discard(project,id,body.resultId));
 if(body.action==="refresh")return Response.json(await textureJobs.refresh(project,id));
 if(body.action==="save"&&typeof body.resultId==="string"&&typeof body.name==="string")return Response.json(await textureJobs.save(project,id,body.resultId,body.name));
 throw new ObjectCreatorError("Choose refresh, save or delete a texture candidate.");
 }catch(error){return Response.json({error:error instanceof Error?error.message:"Texture operation failed."},{status:error instanceof ObjectCreatorError?error.status:400});}
}
