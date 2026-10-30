import {objectProject,sameOrigin} from "@/lib/object-http";
import {boundedBody} from "@/lib/object-files";
import {readSceneLighting,saveSceneLighting} from "@/lib/scene-lighting";
export async function GET(request:Request){try{const query=new URL(request.url).searchParams,project=await objectProject(query.get("projectId")),scene=await readSceneLighting(project,query.get("path")??"");return Response.json({path:scene.path,settings:scene.settings,revision:scene.revision},{headers:{"Cache-Control":"no-store"}});}catch(error){return failure(error);}}
export async function PUT(request:Request){try{sameOrigin(request);const body=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),8192)).toString()),project=await objectProject(body.projectId);if(typeof body.path!=="string"||typeof body.revision!=="string")throw new Error("Scene and revision are required.");return Response.json(await saveSceneLighting(project,body));}catch(error){return failure(error);}}
function failure(error:unknown){return Response.json({error:error instanceof Error?error.message:"Scene lighting failed."},{status:400});}
