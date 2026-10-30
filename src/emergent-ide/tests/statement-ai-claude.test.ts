import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { createServer, type IncomingMessage, type Server } from "node:http";
import os from "node:os";
import path from "node:path";
import test from "node:test";

// A local stand-in for the Anthropic Messages API: no paid request is made.
let server: Server, data = "", baseUrl = "";
let requests: { headers: IncomingMessage["headers"]; body: Record<string, unknown> }[] = [];
let stopReason = "end_turn";
let ai: typeof import("../src/lib/statement-ai");

function sse(events: [string, unknown][]) { return events.map(([event, value]) => `event: ${event}\ndata: ${JSON.stringify(value)}\n\n`).join(""); }

test.before(async () => {
  server = createServer((request, response) => {
    let raw = "";
    request.on("data", chunk => { raw += chunk; });
    request.on("end", () => {
      requests.push({ headers: request.headers, body: JSON.parse(raw) });
      const text = stopReason === "refusal" ? "" : '{"answer":"ok"}';
      response.writeHead(200, { "Content-Type": "text/event-stream" });
      response.end(sse([
        ["message_start", { type: "message_start", message: { id: "msg_1", type: "message", role: "assistant", model: "claude-opus-5-5", content: [], stop_reason: null, stop_sequence: null, usage: { input_tokens: 1, output_tokens: 0 } } }],
        ["content_block_start", { type: "content_block_start", index: 0, content_block: { type: "text", text: "" } }],
        ["content_block_delta", { type: "content_block_delta", index: 0, delta: { type: "text_delta", text } }],
        ["content_block_stop", { type: "content_block_stop", index: 0 }],
        ["message_delta", { type: "message_delta", delta: { stop_reason: stopReason, stop_sequence: null, stop_details: stopReason === "refusal" ? { type: "refusal", category: "cyber", explanation: null } : null }, usage: { output_tokens: 5 } }],
        ["message_stop", { type: "message_stop" }],
      ]));
    });
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  baseUrl = `http://127.0.0.1:${typeof address === "object" && address ? address.port : 0}`;
  data = await mkdtemp(path.join(os.tmpdir(), "emergent-claude-"));
  process.env.EMERGENT_IDE_DATA_DIR = data;
  const settings = await import("../src/lib/ai-settings");
  await settings.updateAISettings({ provider: "anthropic", baseUrl, model: "claude-opus-5-5", apiKey: "sk-ant-test-key" });
  ai = await import("../src/lib/statement-ai");
});
test.after(async () => { server.close(); await rm(data, { recursive: true, force: true }); });

const format = { format: { type: "json_schema", name: "answer", strict: true, schema: { type: "object", properties: { answer: { type: "string" } }, required: ["answer"], additionalProperties: false } } };

test("Claude requests send the JSON schema as output_config with server-side fallback and return the text", async () => {
  requests = []; stopReason = "end_turn";
  assert.equal(await ai.statementResponse("Say ok", format, 700), '{"answer":"ok"}');
  const [request] = requests;
  assert.equal(request.headers.authorization, "Bearer sk-ant-test-key");
  assert.match(String(request.headers["anthropic-beta"]), /server-side-fallback-2026-07-01/);
  assert.equal(request.body.model, "claude-opus-5-5");
  assert.equal(request.body.fallbacks, "default");
  assert.equal(request.body.stream, true);
  assert.deepEqual(request.body.output_config, { format: { type: "json_schema", schema: format.format.schema } });
  assert.deepEqual(request.body.messages, [{ role: "user", content: "Say ok" }]);
});

test("refusals and missing schemas fail without returning partial output", async () => {
  stopReason = "refusal";
  await assert.rejects(ai.statementResponse("Say ok", format, 700), /declined this request \(cyber\)/);
  stopReason = "max_tokens";
  await assert.rejects(ai.statementResponse("Say ok", format, 700), /incomplete/);
  await assert.rejects(ai.statementResponse("Say ok", {}, 700), /JSON schema/);
});
