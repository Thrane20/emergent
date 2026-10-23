import { buildProjectPrompt, responseText, validateChatRequest, type ChatRequest } from "@/lib/ai-chat";
import { readAIConnectionSettings } from "@/lib/ai-settings";
import { readProjectSnapshot } from "@/lib/project";
import { statementsFor } from "@/lib/statements";

export async function POST(request: Request) {
  try {
    const input = validateChatRequest(await request.json() as ChatRequest);
    const [settings, project] = await Promise.all([
      readAIConnectionSettings(),
      readProjectSnapshot(),
    ]);
    if (!settings.apiKey) {
      return Response.json({ error: "Configure an AI API key in Settings before sending a message." }, { status: 409 });
    }
    if (input.projectId !== project.id) {
      return Response.json({ error: "The chat request does not match the linked project." }, { status: 409 });
    }

    const statements = input.scope.startsWith("file:") ? await statementsFor(project, input.scope) : [];
    const upstream = await fetch(`${settings.baseUrl}/responses`, {
      method: "POST",
      headers: {
        "Authorization": `Bearer ${settings.apiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        model: settings.model,
        input: buildProjectPrompt(project, input.scope, input.message, input.history, statements),
        max_output_tokens: 1_500,
      }),
      signal: AbortSignal.timeout(120_000),
      cache: "no-store",
    });
    const payload = await upstream.json() as unknown;
    if (!upstream.ok) {
      return Response.json({ error: upstreamError(payload, upstream.status) }, { status: 502 });
    }
    const text = responseText(payload);
    if (!text) {
      return Response.json({ error: "The AI provider returned no text response." }, { status: 502 });
    }
    const responseId = payload && typeof payload === "object" && "id" in payload && typeof payload.id === "string"
      ? payload.id
      : null;
    return Response.json({ text, model: settings.model, responseId });
  } catch (error) {
    const message = error instanceof Error
      ? error.name === "TimeoutError" ? "The AI provider timed out after 120 seconds." : error.message
      : "Unable to send the chat request.";
    const status = error instanceof SyntaxError ? 400 : 500;
    return Response.json({ error: message }, { status });
  }
}

function upstreamError(payload: unknown, status: number): string {
  if (payload && typeof payload === "object" && "error" in payload) {
    const error = payload.error;
    if (error && typeof error === "object" && "message" in error && typeof error.message === "string") {
      return `AI provider error: ${error.message}`;
    }
  }
  return `AI provider request failed with status ${status}.`;
}
