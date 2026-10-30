import {objectProject,sameOrigin} from "@/lib/object-http";
import {boundedBody} from "@/lib/object-files";
import {particleMedia} from "@/lib/particle-media";
export async function GET(request:Request){try{return Response.json(await particleMedia.list(await objectProject(new URL(request.url).searchParams.get("projectId"))),{headers:{"Cache-Control":"no-store"}});}catch(e){return fail(e);}}
export async function POST(request:Request){try{sameOrigin(request);const q=new URL(request.url).searchParams;if(q.get("import")==="video"){const p=await objectProject(q.get("projectId")),id=q.get("id");if(!id)throw new Error("Import ID required.");return Response.json(await particleMedia.import(p,id,await boundedBody(new Response(request.body,{headers:request.headers}),64*1024*1024)));}const b=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),8192)).toString("utf8"));return Response.json(await particleMedia.create(await objectProject(b.projectId),b.id,b.input));}catch(e){return fail(e);}}
function fail(e:unknown){return Response.json({error:e instanceof Error?e.message:"Media operation failed."},{status:400});}
