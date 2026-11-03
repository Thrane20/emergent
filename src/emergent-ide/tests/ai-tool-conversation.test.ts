import assert from "node:assert/strict";
import { createServer } from "node:http";
import test from "node:test";
import { recoverTextToolCalls, toolConversation, type ToolOutput } from "../src/lib/ai-tool-conversation";
import type { AIProvider } from "../src/lib/ai-settings";

for (const provider of ["anthropic", "openai-codex", "vercel"] as AIProvider[]) {
  test(`${provider} sends tools, continues with results and preserves assistant blocks`, async t => {
    const requests: Record<string, unknown>[] = [];
    const paths: (string | undefined)[] = [];
    const authorizations: (string | undefined)[] = [];
    let mode = "call";
    const server = createServer((req, res) => {
      let raw = "";
      req.on("data", chunk => { raw += chunk; });
      req.on("end", () => {
        paths.push(req.url);
        authorizations.push(req.headers.authorization);
        requests.push(JSON.parse(raw));
        if (mode === "error") { res.writeHead(429); res.end(JSON.stringify({ error: { message: "slow down" } })); return; }
        if (provider !== "anthropic") {
          res.setHeader("Content-Type", "application/json");
          res.end(JSON.stringify({ status: mode === "incomplete" ? "incomplete" : "completed", output: mode === "call" ? [
            { type: "reasoning", id: "reason_1", summary: [], encrypted_content: "opaque" },
            { type: "function_call", id: "fc_1", call_id: "call_1", name: "find", arguments: '{"query":"rover"}' },
          ] : [{ type: "message", role: "assistant", content: [{ type: "output_text", text: "Found the rover." }] }] }));
          return;
        }
        const blocks = mode === "call" ? [{ type: "tool_use", id: "call_1", name: "find", input: { query: "rover" } }] : [{ type: "text", text: "Found the rover." }];
        const events: [string, unknown][] = [["message_start", { type: "message_start", message: { id: "msg_1", type: "message", role: "assistant", model: "test", content: [], stop_reason: null, stop_sequence: null, usage: { input_tokens: 1, output_tokens: 0 } } }]];
        for (const [index, content_block] of blocks.entries()) {
          events.push(["content_block_start", { type: "content_block_start", index, content_block }], ["content_block_stop", { type: "content_block_stop", index }]);
        }
        events.push(["message_delta", { type: "message_delta", delta: { stop_reason: mode === "incomplete" ? "max_tokens" : mode === "call" ? "tool_use" : "end_turn", stop_sequence: null }, usage: { output_tokens: 5 } }], ["message_stop", { type: "message_stop" }]);
        res.writeHead(200, { "Content-Type": "text/event-stream" });
        res.end(events.map(([event, value]) => `event: ${event}\ndata: ${JSON.stringify(value)}\n\n`).join(""));
      });
    });
    await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
    t.after(() => new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve())));
    const address = server.address();
    assert.ok(address && typeof address === "object");
    const next = toolConversation({ provider, baseUrl: `http://127.0.0.1:${address.port}`, model: provider === "vercel" ? "anthropic/claude-sonnet-4.6" : "test", apiKey: "test-key" }, "Use durable refs", "Find the rover", [{ name: "find", description: "Find an object", inputSchema: { type: "object", properties: { query: { type: "string" } } } }]);
    assert.deepEqual((await next([])).calls, [{ id: "call_1", name: "find", arguments: { query: "rover" } }]);
    const results: ToolOutput[] = [{ id: "call_1", isError: true, content: [{ type: "text", text: "not found" }, { type: "image", mimeType: "image/png", data: "aW1hZ2U=" }] }];
    mode = "answer";
    assert.equal((await next(results)).text, "Found the rover.");
    const wire = JSON.stringify(requests[1]);
    assert.match(wire, /call_1/); assert.match(wire, /aW1hZ2U=/); assert.match(wire, /not found/);
    if (provider !== "anthropic") {
      assert.equal(paths[0], "/responses");
      assert.equal(authorizations[0], "Bearer test-key");
    }
    const tool = (requests[0].tools as Record<string, unknown>[])[0];
    assert.equal(tool.name, "find");
    assert.equal(requests[0].model, provider === "vercel" ? "anthropic/claude-sonnet-4.6" : "test");
    if (provider !== "anthropic") {
      assert.equal(requests[0].instructions, "Use durable refs");
      assert.equal(requests[0].store, false);
      assert.match(wire, /encrypted_content.*opaque/);
      assert.match(wire, /function_call_output/); assert.match(wire, /Tool error/);
    } else {
      assert.equal(requests[0].system, "Use durable refs");
      assert.match(wire, /tool_use/); assert.match(wire, /tool_result/); assert.match(wire, /is_error.*true/);
    }
    mode = "incomplete";
    await assert.rejects(next([]), /complete|completing/);
    mode = "error";
    const before = requests.length;
    await assert.rejects(next([]));
    assert.equal(requests.length, before + 1, "provider failures are not automatically retried");
  });
}

test("recovers Qwen text tool calls that Ollama left unparsed", () => {
  const tools = [{ name: "find", inputSchema: { type: "object" as const, properties: { query: { type: "string" }, limit: { type: "number" } } } }];
  const text = "Let me look.\n\n<function=find>\n<parameter=query>\n42\n</parameter>\n<parameter=limit>\n5\n</parameter>\n</function>\n</tool_call>";
  const output = recoverTextToolCalls([{ type: "message", role: "assistant", content: [{ type: "output_text", text }] }], tools);
  assert.equal(output.length, 2);
  assert.deepEqual((output[0].content as { text: string }[])[0].text, "Let me look.");
  assert.equal(output[1].name, "find");
  assert.deepEqual(JSON.parse(output[1].arguments as string), { query: "42", limit: 5 });
  // Real structured calls and unknown tools are left alone.
  const real = [{ type: "function_call", call_id: "c", name: "find", arguments: "{}" }];
  assert.equal(recoverTextToolCalls(real, tools), real);
  const unknown = [{ type: "message", content: [{ type: "output_text", text: "<function=nope></function>" }] }];
  assert.equal(recoverTextToolCalls(unknown, tools), unknown);
});
