import "server-only";
import { responseText } from "./ai-chat";
import { readAIConnectionSettings } from "./ai-settings";
import type { ProjectSnapshot } from "./project";
import { buildStatementEvaluationPrompt, parseStatementEvaluation, statementEvaluationFormat } from "./statement-evaluation";
import { saveStatementEvaluation, statementById } from "./statements";

export async function statementResponse(prompt: string, text: unknown, maximumTokens: number) {
  const settings = await readAIConnectionSettings();
  if (!settings.apiKey) throw new Error("Configure an AI API key in Settings first.");
  const response = await fetch(`${settings.baseUrl}/responses`, {
    method: "POST", headers: { Authorization: `Bearer ${settings.apiKey}`, "Content-Type": "application/json" },
    body: JSON.stringify({ model: settings.model, input: prompt, text, max_output_tokens: maximumTokens }),
    signal: AbortSignal.timeout(120_000), cache: "no-store",
  });
  const payload = await response.json();
  if (!response.ok) throw new Error(`AI provider error: ${payload?.error?.message ?? response.status}`);
  if (payload.status === "incomplete") throw new Error("The AI response was incomplete. No implementation was applied; try a smaller statement.");
  const output = responseText(payload);
  if (!output) throw new Error("The AI provider returned no statement response.");
  return output;
}

export async function evaluateStatement(project: ProjectSnapshot, subject: string, id: string) {
  const statement = await statementById(project, subject, id);
  const output = await statementResponse(buildStatementEvaluationPrompt(project, statement), statementEvaluationFormat, 700);
  return saveStatementEvaluation(project, subject, id, parseStatementEvaluation(output));
}
