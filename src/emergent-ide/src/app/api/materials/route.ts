import {objectProject,sameOrigin} from "@/lib/object-http";
import {boundedBody} from "@/lib/object-files";
import {materialLibrary,saveMaterial} from "@/lib/material-assets";
export async function GET(request:Request){try{return Response.json({materials:await materialLibrary(await objectProject(new URL(request.url).searchParams.get("projectId")))},{headers:{"Cache-Control":"no-store"}});}catch(error){return failure(error);}}
export async function POST(request:Request){try{sameOrigin(request);const body=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),256*1024)).toString("utf8"));return Response.json(await saveMaterial(await objectProject(body.projectId),body),{status:201});}catch(error){return failure(error);}}
function failure(error:unknown){return Response.json({error:error instanceof Error?error.message:"Material operation failed."},{status:400});}
