import { inspectionProject, objectProject, sameOrigin } from "@/lib/object-http";
import { debugSessions } from "@/lib/debug-session";

/** Debug runs: start (POST), live status and state (GET), commands (PUT), stop (DELETE). */
export async function GET(request: Request) {
  try { const query = new URL(request.url).searchParams; const project = await inspectionProject(query.get("projectId")); return Response.json(debugSessions.status(project, query.get("id") ?? ""), { headers: { "Cache-Control": "no-store" } }); } catch (error) { return failure(error); }
}
export async function POST(request: Request) {
  try { sameOrigin(request); const body = await request.json(); const project = await objectProject(body.projectId); if (typeof body.path !== "string") throw new Error("Scene path is required."); return Response.json(await debugSessions.start(project, body.path)); } catch (error) { return failure(error); }
}
export async function PUT(request: Request) {
  try { sameOrigin(request); const body = await request.json(); const project = await inspectionProject(body.projectId); if (typeof body.id !== "string") throw new Error("Session ID is required."); return Response.json(await debugSessions.command(project, body.id, body.command)); } catch (error) { return failure(error); }
}
export async function DELETE(request: Request) {
  try { sameOrigin(request); const body = await request.json(); const project = await inspectionProject(body.projectId); if (typeof body.id !== "string") throw new Error("Session ID is required."); return Response.json(await debugSessions.stop(project, body.id)); } catch (error) { return failure(error); }
}
function failure(error: unknown) { return Response.json({ error: error instanceof Error ? error.message : "Debug run failed." }, { status: 400 }); }
