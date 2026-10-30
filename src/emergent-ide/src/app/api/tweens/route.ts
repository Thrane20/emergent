import {objectProject,sameOrigin} from "@/lib/object-http";
import {boundedBody} from "@/lib/object-files";
import {saveTween} from "@/lib/tween-assets";
export async function POST(request:Request){
  try{
    sameOrigin(request);
    const body=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),8192)).toString("utf8"));
    const project=await objectProject(body.projectId);
    return Response.json(await saveTween(project,body),{status:201,headers:{"Cache-Control":"no-store"}});
  }catch(error){return Response.json({error:error instanceof Error?error.message:"Tween could not be saved."},{status:400});}
}
