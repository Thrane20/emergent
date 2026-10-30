import "server-only";
import type { CodeContext } from "./project-code-context";
import { implementationFormat, parseImplementation, ReuseEvidenceError, reuseEvidenceGuide } from "./statement-proposal";
import { responseText } from "./ai-chat";
import Anthropic from "@anthropic-ai/sdk";
import { readAIConnectionSettings, type AIConnectionSettings } from "./ai-settings";
import type { ProjectSnapshot } from "./project";
import { buildStatementEvaluationPrompt, parseStatementEvaluation, statementEvaluationFormat } from "./statement-evaluation";
import { saveStatementEvaluation, statementById } from "./statements";

/** Sends one structured-output request to the active provider. `text` uses the Responses `text.format` JSON-schema shape. */
export async function statementResponse(prompt: string, text: unknown, maximumTokens: number) {
  const settings = await readAIConnectionSettings();
  if (!settings.apiKey) throw new Error("Configure an AI API key in Settings first.");
  if (settings.provider === "anthropic") return claudeResponse(settings, prompt, text);
  const response = await fetch(`${settings.baseUrl}/responses`, {
    method: "POST", headers: { Authorization: `Bearer ${settings.apiKey}`, "Content-Type": "application/json" },
    body: JSON.stringify({ model: settings.model, input: prompt, text, max_output_tokens: maximumTokens }),
    signal: AbortSignal.timeout(120_000), cache: "no-store",
  });
  const payload = await response.json();
  if (!response.ok) throw new Error(`AI provider error: ${payload?.error?.message ?? response.status}`);
  if (payload.status === "incomplete") throw new Error("The AI response was incomplete. No implementation was applied; try a smaller request.");
  const output = responseText(payload);
  if (!output) throw new Error("The AI provider returned no statement response.");
  return output;
}

async function claudeResponse(settings: AIConnectionSettings, prompt: string, text: unknown) {
  const schema = (text as { format?: { type?: string; schema?: Record<string, unknown> } } | undefined)?.format;
  if (schema?.type !== "json_schema" || !schema.schema) throw new Error("Claude requests need a JSON schema output format.");
  const client = new Anthropic({ apiKey: settings.apiKey, baseURL: settings.baseUrl, timeout: 600_000 });
  try {
    // Thinking is adaptive on current Claude models, so output headroom exceeds the Responses token caps;
    // streaming keeps long implementation proposals clear of HTTP timeouts.
    const message = await client.beta.messages.stream({
      model: settings.model,
      max_tokens: 64_000,
      betas: ["server-side-fallback-2026-07-01"],
      fallbacks: "default",
      output_config: { format: { type: "json_schema", schema: schema.schema } },
      messages: [{ role: "user", content: prompt }],
    }).finalMessage();
    if (message.stop_reason === "refusal") throw new Error(`Claude declined this request${message.stop_details?.category ? ` (${message.stop_details.category})` : ""}. No changes were applied.`);
    if (message.stop_reason === "max_tokens") throw new Error("The AI response was incomplete. No implementation was applied; try a smaller request.");
    const output = message.content.flatMap(block => block.type === "text" ? [block.text] : []).join("");
    if (!output) throw new Error("Claude returned no statement response.");
    return output;
  } catch (error) {
    if (error instanceof Anthropic.AuthenticationError) throw new Error("Claude rejected the API key. Update it in Settings.");
    if (error instanceof Anthropic.RateLimitError) throw new Error("Claude rate limit reached. Try again shortly.");
    if (error instanceof Anthropic.APIError) throw new Error(`AI provider error: ${error.message}`);
    throw error;
  }
}

export async function implementationResponse(prompt: string, context: CodeContext, respond = statementResponse) {
  const inspectedPrompt = `${prompt}\n\n${reuseEvidenceGuide(context)}`;
  try { return parseImplementation(await respond(inspectedPrompt, implementationFormat, 14_000), context); }
  catch (error) {
    if (!(error instanceof ReuseEvidenceError)) throw error;
    // Repair citation format once; publication still requires the same parser and native checks.
    return parseImplementation(await respond(`${inspectedPrompt}\n\nREUSE CORRECTION\nThe previous proposal was rejected before any writes: ${error.message}\nRegenerate the full JSON proposal for the original request with corrected inspected citations.`, implementationFormat, 14_000), context);
  }
}

export async function evaluateStatement(project: ProjectSnapshot, subject: string, id: string) {
  const statement = await statementById(project, subject, id);
  const output = await statementResponse(buildStatementEvaluationPrompt(project, statement), statementEvaluationFormat, 700);
  return saveStatementEvaluation(project, subject, id, parseStatementEvaluation(output));
}
