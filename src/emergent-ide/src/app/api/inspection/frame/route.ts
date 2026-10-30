import { inspectionProject } from "@/lib/object-http";
import { inspectionSessions } from "@/lib/inspection-session";

export async function GET(request:Request) {
  try {
    const query=new URL(request.url).searchParams,after=Number(query.get("after")??0);
    if(!Number.isSafeInteger(after)||after<0)throw new Error("Invalid frame sequence.");
    const project=await inspectionProject(query.get("projectId"));
    const frame=await inspectionSessions.frame(project,query.get("id")??"",after,request.signal);
    if(!frame)return new Response(null,{status:204,headers:{"Cache-Control":"no-store"}});
    return new Response(new Uint8Array(frame.png),{headers:{"Content-Type":"image/png","Cache-Control":"no-store","X-Content-Type-Options":"nosniff","X-Inspection-Frame":String(frame.sequence),"X-Inspection-Command":String(frame.command),"X-Inspection-Tick":"0","X-Inspection-Worker":String(frame.workerPid),"X-Inspection-Render-Us":String(frame.renderUs),"X-Inspection-Encode-Us":String(frame.encodeUs)}});
  }catch(error){return Response.json({error:error instanceof Error?error.message:"Inspection frame failed."},{status:400});}
}
