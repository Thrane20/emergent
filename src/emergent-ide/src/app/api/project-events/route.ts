import { inspectionProject } from "@/lib/object-http";
import { subscribeProjectEvents } from "@/lib/project-events";

// Server-sent events: one line per external change, plus a heartbeat so idle proxies keep the stream open.
export async function GET(request: Request) {
  try {
    const project = await inspectionProject(new URL(request.url).searchParams.get("projectId"));
    const encoder = new TextEncoder();
    let cleanup = () => {};
    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        const send = (text: string) => { try { controller.enqueue(encoder.encode(text)); } catch { cleanup(); } };
        const unsubscribe = subscribeProjectEvents(event => { if (event.projectId === project.id) send(`event: changed\ndata: ${JSON.stringify(event)}\n\n`); });
        const heartbeat = setInterval(() => send(": heartbeat\n\n"), 25_000);
        cleanup = () => { clearInterval(heartbeat); unsubscribe(); };
        request.signal.addEventListener("abort", () => { cleanup(); try { controller.close(); } catch { /* Already closed. */ } }, { once: true });
        send(": connected\n\n");
      },
      cancel() { cleanup(); },
    });
    return new Response(stream, { headers: { "Content-Type": "text/event-stream", "Cache-Control": "no-store, no-transform", Connection: "keep-alive" } });
  } catch (error) {
    return Response.json({ error: error instanceof Error ? error.message : "Project events unavailable." }, { status: 400 });
  }
}
