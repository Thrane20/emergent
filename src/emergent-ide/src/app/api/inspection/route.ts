import { objectProject,inspectionProject, sameOrigin } from "@/lib/object-http";
import { inspectionSessions } from "@/lib/inspection-session";

export async function GET(request:Request) {
  try {const query=new URL(request.url).searchParams;const project=await inspectionProject(query.get("projectId"));return Response.json(inspectionSessions.status(project,query.get("id")??""),{headers:{"Cache-Control":"no-store"}});}catch(error){return failure(error);}
}
export async function POST(request:Request) {
  try {sameOrigin(request);const body=await request.json();const project=await objectProject(body.projectId);if(body.terrain!==undefined)return Response.json(await inspectionSessions.startTerrain(project,body.terrain));if(typeof body.path!=="string")throw new Error("Scene path is required.");return Response.json(await inspectionSessions.start(project,body.path));}catch(error){return failure(error);}
}
export async function PUT(request:Request) {
  try {sameOrigin(request);const body=await request.json();const project=await inspectionProject(body.projectId);if(typeof body.id!=="string")throw new Error("Session ID is required.");return Response.json(await inspectionSessions.command(project,body.id,body.command));}catch(error){return failure(error);}
}
export async function DELETE(request:Request) {
  try {sameOrigin(request);const body=await request.json();const project=await inspectionProject(body.projectId);if(typeof body.id!=="string")throw new Error("Session ID is required.");return Response.json(await inspectionSessions.stop(project,body.id));}catch(error){return failure(error);}
}
function failure(error:unknown){return Response.json({error:error instanceof Error?error.message:"Inspection failed."},{status:400});}
