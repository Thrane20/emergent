import "server-only";
import Anthropic from "@anthropic-ai/sdk";
import type { BetaMessageParam, BetaTool } from "@anthropic-ai/sdk/resources/beta/messages/messages";
import { z } from "zod";
import type { AIConnectionSettings } from "./ai-settings";
import { responseText } from "./ai-chat";

export type ToolDefinition = { name: string; description?: string; inputSchema: { type: "object"; [key: string]: unknown } };
export type ToolContent = { type: "text"; text: string } | { type: "image"; data: string; mimeType: string };
export type ToolOutput = { id: string; content: ToolContent[]; isError?: boolean };
export type ToolTurn = { text: string; calls: { id: string; name: string; arguments: unknown }[] };
export type ToolConversation = (results: ToolOutput[]) => Promise<ToolTurn>;

const responseSchema = z.object({
  status: z.string(), error: z.object({ message: z.string() }).nullish(),
  output: z.array(z.record(z.string(), z.unknown())).default([]),
});
const callSchema = z.object({ call_id: z.string().min(1), name: z.string().min(1), arguments: z.string() });

/** Provider-specific envelopes only; MCP owns tool definitions, validation and execution. */
export function toolConversation(settings: AIConnectionSettings, instructions: string, prompt: string, tools: ToolDefinition[]): ToolConversation {
  if (!settings.apiKey) throw new Error("Configure an AI API key in Settings first.");
  if (settings.provider === "anthropic") {
    const client = new Anthropic({ apiKey: settings.apiKey, baseURL: settings.baseUrl, timeout: 120_000, maxRetries: 0 });
    const messages: BetaMessageParam[] = [{ role: "user", content: prompt }];
    return async results => {
      if (results.length) messages.push({ role: "user", content: results.map(result => ({
        type: "tool_result", tool_use_id: result.id, is_error: result.isError,
        content: result.content.map(block => block.type === "text" ? block : {
          type: "image", source: { type: "base64", data: block.data, media_type: block.mimeType as "image/png" },
        }),
      })) });
      const message = await client.beta.messages.stream({
        model: settings.model, max_tokens: 16_000, system: instructions,
        betas: ["server-side-fallback-2026-07-01"], fallbacks: "default",
        tools: tools.map(tool => ({ name: tool.name, description: tool.description, input_schema: tool.inputSchema } as BetaTool)),
        messages,
      }).finalMessage();
      if (message.stop_reason !== "end_turn" && message.stop_reason !== "tool_use") {
        throw new Error(`Claude stopped before completing the request (${message.stop_reason}).`);
      }
      messages.push({ role: "assistant", content: message.content });
      return {
        text: message.content.flatMap(block => block.type === "text" ? [block.text] : []).join("\n"),
        calls: message.content.flatMap(block => block.type === "tool_use" ? [{ id: block.id, name: block.name, arguments: block.input }] : []),
      };
    };
  }

  const input: unknown[] = [{ role: "user", content: prompt }];
  return async results => {
    for (const result of results) input.push({
      type: "function_call_output", call_id: result.id,
      output: result.content.map(block => block.type === "text"
        ? { type: "input_text", text: result.isError ? `Tool error: ${block.text}` : block.text }
        : { type: "input_image", image_url: `data:${block.mimeType};base64,${block.data}` }),
    });
    const response = await fetch(`${settings.baseUrl}/responses`, {
      method: "POST", headers: { Authorization: `Bearer ${settings.apiKey}`, "Content-Type": "application/json" },
      body: JSON.stringify({
        model: settings.model, instructions, input, max_output_tokens: 16_000, store: false,
        parallel_tool_calls: false,
        include: ["reasoning.encrypted_content"],
        tools: tools.map(tool => ({ type: "function", name: tool.name, description: tool.description, parameters: tool.inputSchema, strict: false })),
      }), signal: AbortSignal.timeout(120_000), cache: "no-store",
    });
    const payload = await response.json();
    if (!response.ok) throw new Error(`AI provider error: ${payload?.error?.message ?? response.status}`);
    const parsed = responseSchema.parse(payload);
    if (parsed.status !== "completed") throw new Error(`The AI response did not complete (${parsed.status}).`);
    const output = settings.provider === "ollama" ? recoverTextToolCalls(parsed.output, tools) : parsed.output;
    // Preserve reasoning items and every call ID when continuing a stateless Responses conversation.
    input.push(...output);
    return {
      text: responseText(payload) ?? "",
      calls: output.filter(item => item.type === "function_call").map(item => {
        const call = callSchema.parse(item);
        let args: unknown;
        try { args = JSON.parse(call.arguments); } catch { args = null; }
        return { id: call.call_id, name: call.name, arguments: args };
      }),
    };
  };
}

type OutputItem = Record<string, unknown>;

/**
 * Qwen3-Coder on Ollama often writes prose then a tool call without its opening `<tool_call>` tag, which
 * Ollama's parser misses; the call arrives as text. Rebuild those calls so the turn continues.
 */
export function recoverTextToolCalls(output: OutputItem[], tools: ToolDefinition[]): OutputItem[] {
  if (output.some(item => item.type === "function_call")) return output;
  const text = output.flatMap(item => item.type === "message" && Array.isArray(item.content)
    ? item.content.flatMap(part => part?.type === "output_text" && typeof part.text === "string" ? [part.text] : []) : []).join("\n");
  const calls = [...text.matchAll(/<function=([\w.-]+)>([\s\S]*?)<\/function>/g)].flatMap((match, index) => {
    const tool = tools.find(candidate => candidate.name === match[1]);
    if (!tool) return [];
    const properties = (tool.inputSchema.properties ?? {}) as Record<string, { type?: unknown }>;
    const args: Record<string, unknown> = {};
    for (const [, key, raw] of match[2].matchAll(/<parameter=([\w.-]+)>\n?([\s\S]*?)\n?<\/parameter>/g)) {
      if (properties[key]?.type === "string") { args[key] = raw; continue; }
      try { args[key] = JSON.parse(raw); } catch { args[key] = raw; }
    }
    return [{ type: "function_call", call_id: `text_call_${Date.now()}_${index}`, name: tool.name, arguments: JSON.stringify(args) }];
  });
  if (!calls.length) return output;
  const prose = text.slice(0, text.indexOf("<function=")).replace(/<tool_call>\s*$/, "").trim();
  return [...(prose ? [{ type: "message", role: "assistant", content: [{ type: "output_text", text: prose }] }] : []), ...calls];
}
