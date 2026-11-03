import {objectProject,sameOrigin} from "@/lib/object-http";
import {boundedBody} from "@/lib/object-files";
import {shaderLibrary,saveShader,readShader} from "@/lib/shader-assets";
export async function GET(request:Request){try{const query=new URL(request.url).searchParams,project=await objectProject(query.get("projectId")),path=query.get("path");return Response.json(path?await readShader(project,path):{shaders:await shaderLibrary(project)},{headers:{"Cache-Control":"no-store"}});}catch(error){return failure(error);}}
export async function POST(request:Request){try{sameOrigin(request);const body=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),128*1024)).toString("utf8"));return Response.json(await saveShader(await objectProject(body.projectId),body),{status:201});}catch(error){return failure(error);}}
function failure(error:unknown){return Response.json({error:error instanceof Error?error.message:"Shader request failed."},{status:400});}
