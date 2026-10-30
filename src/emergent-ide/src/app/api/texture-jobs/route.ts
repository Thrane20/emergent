import {boundedBody} from "@/lib/object-files";
import {objectProject,sameOrigin} from "@/lib/object-http";
import {textureJobs} from "@/lib/texture-jobs";
import {ObjectCreatorError} from "@/lib/object-types";
export async function GET(request:Request){try{const project=await objectProject(new URL(request.url).searchParams.get("projectId"));return Response.json(await textureJobs.list(project),{headers:{"Cache-Control":"no-store"}});}catch(error){return failure(error);}}
export async function POST(request:Request){try{sameOrigin(request);const body=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),16384)).toString("utf8")),project=await objectProject(body.projectId);if(typeof body.id!=="string")throw new ObjectCreatorError("A texture generation ID is required.");return Response.json(await textureJobs.create(project,body.id,body.input),{status:201});}catch(error){return failure(error);}}
function failure(error:unknown){return Response.json({error:error instanceof Error?error.message:"Texture generation failed."},{status:error instanceof ObjectCreatorError?error.status:400});}
