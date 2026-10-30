import {artifactResponse,objectProject} from "@/lib/object-http";
import {textureJobs} from "@/lib/texture-jobs";
import {ObjectCreatorError} from "@/lib/object-types";
export async function GET(request:Request,context:{params:Promise<{id:string}>}){try{const params=new URL(request.url).searchParams,project=await objectProject(params.get("projectId")),{id}=await context.params;return artifactResponse(await textureJobs.artifact(project,id,params.get("result")??""));}catch(error){return Response.json({error:error instanceof Error?error.message:"Texture preview unavailable."},{status:error instanceof ObjectCreatorError?error.status:400});}}
