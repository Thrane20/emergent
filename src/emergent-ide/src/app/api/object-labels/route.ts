import {objectProject,sameOrigin} from "@/lib/object-http";
import {boundedBody} from "@/lib/object-files";
import {readObjectLabels,saveObjectLabels} from "@/lib/object-label-assets";
export async function GET(request:Request){try{return Response.json(await readObjectLabels(await objectProject(new URL(request.url).searchParams.get("projectId"))),{headers:{"Cache-Control":"no-store"}});}catch(e){return failure(e);}}
export async function PUT(request:Request){try{sameOrigin(request);const b=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),16384)).toString());return Response.json(await saveObjectLabels(await objectProject(b.projectId),b.revision,b.labels));}catch(e){return failure(e);}}
function failure(e:unknown){return Response.json({error:e instanceof Error?e.message:"Labels failed"},{status:400});}
