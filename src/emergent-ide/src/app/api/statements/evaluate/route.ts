import { responseText } from "@/lib/ai-chat";
import { readAIConnectionSettings } from "@/lib/ai-settings";
import { readProjectSnapshot } from "@/lib/project";
import { buildStatementEvaluationPrompt, parseStatementEvaluation, statementEvaluationFormat } from "@/lib/statement-evaluation";
import { saveStatementEvaluation, statementById } from "@/lib/statements";

export async function POST(request: Request) {
  try {
    const body = await request.json() as { projectId?: unknown; subject?: unknown; id?: unknown };
    const subject = requiredString(body.subject, "Subject");
    const id = requiredString(body.id, "Statement ID");
    const [project, settings] = await Promise.all([readProjectSnapshot(), readAIConnectionSettings()]);
    if (body.projectId !== project.id) throw new Error("The request does not match the linked project.");
    if (!settings.apiKey) return Response.json({ error: "Configure an AI API key before checking statements." }, { status: 409 });
    const statement = await statementById(project, subject, id);
    const upstream = await fetch(`${settings.baseUrl}/responses`, {
      method: "POST",
      headers: { "Authorization": `Bearer ${settings.apiKey}`, "Content-Type": "application/json" },
      body: JSON.stringify({
        model: settings.model,
        input: buildStatementEvaluationPrompt(project, statement),
        text: statementEvaluationFormat,
        max_output_tokens: 500,
      }),
      signal: AbortSignal.timeout(120_000),
      cache: "no-store",
    });
    const payload = await upstream.json() as unknown;
    if (!upstream.ok) return Response.json({ error: upstreamError(payload, upstream.status) }, { status: 502 });
    const text = responseText(payload);
    if (!text) return Response.json({ error: "The AI provider returned no statement evaluation." }, { status: 502 });
    const evaluation = parseStatementEvaluation(text);
    return Response.json({ statements: await saveStatementEvaluation(project, subject, id, evaluation) });
  } catch (error) {
    const message = error instanceof Error
      ? error.name === "TimeoutError" ? "The AI provider timed out after 120 seconds." : error.message
      : "Unable to evaluate the statement.";
    return Response.json({ error: message }, { status: 400 });
  }
}

function requiredString(value: unknown, label: string): string {
  if (typeof value !== "string" || !value.trim()) throw new Error(`${label} is required.`);
  return value.trim();
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
