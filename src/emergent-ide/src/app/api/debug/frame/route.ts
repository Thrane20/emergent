import { inspectionProject } from "@/lib/object-http";
import { debugSessions } from "@/lib/debug-session";

/** Long-polls the next game frame after `after` (204 when none arrived in time). */
export async function GET(request: Request) {
  try {
    const query = new URL(request.url).searchParams, after = Number(query.get("after") ?? 0);
    if (!Number.isSafeInteger(after) || after < 0) throw new Error("Invalid frame sequence.");
    const project = await inspectionProject(query.get("projectId"));
    const frame = await debugSessions.frame(project, query.get("id") ?? "", after, request.signal);
    if (!frame) return new Response(null, { status: 204, headers: { "Cache-Control": "no-store" } });
    return new Response(new Uint8Array(frame.png), { headers: { "Content-Type": "image/png", "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff", "X-Debug-Frame": String(frame.sequence), "X-Debug-Tick": String(frame.tick) } });
  } catch (error) { return Response.json({ error: error instanceof Error ? error.message : "Debug frame failed." }, { status: 400 }); }
}
