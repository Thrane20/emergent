/** Probes the same localhost the chat uses (so a tunnel on ::1 is seen); only the port is caller-controlled. */
export async function GET(request: Request) {
  const port = Number(new URL(request.url).searchParams.get("port") ?? 11434);
  if (!Number.isInteger(port) || port < 1 || port > 65535) return Response.json({ error: "Port must be 1–65535." }, { status: 400 });
  try {
    const response = await fetch(`http://localhost:${port}/api/tags`, { cache: "no-store", redirect: "error", signal: AbortSignal.timeout(3_000) });
    if (!response.ok) return Response.json({ running: false, models: [], message: `Port ${port} answered ${response.status}; is it Ollama?` });
    const data = await response.json() as { models?: { name?: unknown }[] };
    const models = (data.models ?? []).flatMap(model => typeof model.name === "string" ? [model.name] : []).sort();
    return Response.json({ running: true, models, message: `Ollama running on port ${port} · ${models.length} model${models.length === 1 ? "" : "s"}` });
  } catch {
    return Response.json({ running: false, models: [], message: `Nothing answering on port ${port}.` });
  }
}
