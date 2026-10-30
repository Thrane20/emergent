import { objectProject, sameOrigin } from "@/lib/object-http";
import { readTomlDocument, saveTomlDocument } from "@/lib/project-toml";
import { boundedBody } from "@/lib/object-files";
export async function GET(request:Request) {
  try{const query=new URL(request.url).searchParams,project=await objectProject(query.get("projectId"));return Response.json(await readTomlDocument(project,query.get("path")??""),{headers:{"Cache-Control":"no-store"}});}catch(error){return failure(error);}
}
export async function PUT(request:Request) {
  try{sameOrigin(request);const body=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),1024*1024)).toString("utf8")),project=await objectProject(body.projectId);if(typeof body.path!=="string"||typeof body.content!=="string"||typeof body.revision!=="string")throw new Error("Path, source and revision are required.");return Response.json(await saveTomlDocument(project,body.path,body.content,body.revision));}catch(error){return failure(error);}
}
function failure(error:unknown){return Response.json({error:error instanceof Error?error.message:"TOML operation failed."},{status:400});}
