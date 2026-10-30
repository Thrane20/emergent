import {objectProject,sameOrigin} from "@/lib/object-http";
import {boundedBody} from "@/lib/object-files";
import {captureParticles} from "@/lib/particle-assets";
export async function POST(request:Request){try{sameOrigin(request);const b=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),128*1024)).toString("utf8"));return new Response(new Uint8Array(await captureParticles(await objectProject(b.projectId),b.definition,b.seconds)),{headers:{"Content-Type":"image/png","Cache-Control":"no-store"}});}catch(e){return Response.json({error:e instanceof Error?e.message:"Capture failed."},{status:400});}}
