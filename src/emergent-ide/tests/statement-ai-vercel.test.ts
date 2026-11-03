import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { createServer } from "node:http";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { updateAISettings } from "../src/lib/ai-settings";
import { statementResponse } from "../src/lib/statement-ai";

test("Vercel structured requests use the saved gateway key/model and reject incomplete responses and provider errors", async t => {
  const previous = process.env.EMERGENT_IDE_DATA_DIR;
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-vercel-"));
  process.env.EMERGENT_IDE_DATA_DIR = directory;
  t.after(async () => {
    if (previous === undefined) delete process.env.EMERGENT_IDE_DATA_DIR;
    else process.env.EMERGENT_IDE_DATA_DIR = previous;
    await rm(directory, { recursive: true, force: true });
  });
  const requests: { path?: string; authorization?: string; body: Record<string, unknown> }[] = [];
  let status = "completed";
  const server = createServer((req, res) => {
    let raw = "";
    req.on("data", chunk => { raw += chunk; });
    req.on("end", () => {
      requests.push({ path: req.url, authorization: req.headers.authorization, body: JSON.parse(raw) });
      res.setHeader("Content-Type", "application/json");
      if (status === "error") { res.writeHead(401); res.end(JSON.stringify({ error: { message: "Invalid gateway key" } })); return; }
      res.end(JSON.stringify({ status, output_text: '{"answer":"ok"}' }));
    });
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  t.after(() => new Promise<void>(resolve => server.close(() => resolve())));
  const address = server.address(); assert.ok(address && typeof address === "object");
  const baseUrl = `http://127.0.0.1:${address.port}`;
  await updateAISettings({ provider: "vercel", baseUrl, model: "anthropic/claude-sonnet-4.6", apiKey: "test-gateway-key" });
  const format = { format: { type: "json_schema", name: "answer", strict: true, schema: { type: "object", properties: { answer: { type: "string" } }, required: ["answer"], additionalProperties: false } } };
  assert.equal(await statementResponse("Say ok", format, 700), '{"answer":"ok"}');
  assert.deepEqual(requests[0], { path: "/responses", authorization: "Bearer test-gateway-key", body: {
    model: "anthropic/claude-sonnet-4.6", input: "Say ok", text: format, max_output_tokens: 700,
  } });
  await updateAISettings({ provider: "vercel", baseUrl, model: "openai/gpt-6.1-sol" });
  await statementResponse("Say ok", format, 700);
  assert.equal(requests[1].body.model, "openai/gpt-6.1-sol");
  assert.equal(requests[1].authorization, "Bearer test-gateway-key");
  status = "incomplete";
  await assert.rejects(statementResponse("Say ok", format, 700), /incomplete/);
  status = "error";
  await assert.rejects(statementResponse("Say ok", format, 700), /Invalid gateway key/);
});
