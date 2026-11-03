import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { createServer } from "node:http";
import { mcpChat } from "../src/lib/mcp-chat";
import { readProjectSnapshot } from "../src/lib/project";
import { validateChatRequest } from "../src/lib/ai-chat";
import type { ToolOutput, ToolTurn } from "../src/lib/ai-tool-conversation";

test("embedded chat uses the real MCP server, guards writes and reports partial completion", async t => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-mcp-chat-"));
  const oldRoot = process.env.EMERGENT_PROJECT_PATH, oldData = process.env.EMERGENT_IDE_DATA_DIR;
  const root = path.join(directory, "game"), foundation = path.resolve("../..");
  process.env.EMERGENT_PROJECT_PATH = root; process.env.EMERGENT_IDE_DATA_DIR = path.join(directory, "data");
  t.after(async () => {
    if (oldRoot === undefined) delete process.env.EMERGENT_PROJECT_PATH; else process.env.EMERGENT_PROJECT_PATH = oldRoot;
    if (oldData === undefined) delete process.env.EMERGENT_IDE_DATA_DIR; else process.env.EMERGENT_IDE_DATA_DIR = oldData;
    await rm(directory, { recursive: true, force: true });
  });
  await mkdir(path.join(root, "assets/entities"), { recursive: true });
  await mkdir(path.join(root, "src"));
  await mkdir(path.join(root, "assets/scenes"));
  await writeFile(path.join(root, "emergent.toml"), 'schema_version=1\nid="chat"\nname="Chat"\nasset_roots=["assets"]\nentry_scene="assets/scenes/main.scene.toml"\n');
  await writeFile(path.join(root, "Cargo.toml"), `[dependencies]\nemergent={path=${JSON.stringify(foundation)}}\n`);
  await writeFile(path.join(root, "AGENTS.md"), "Preserve the rover.");
  await writeFile(path.join(root, "assets/scenes/main.scene.toml"), 'schema_version=1\nid="main"\n');
  await writeFile(path.join(root, "assets/entities/rover.entity.toml"), 'schema_version=1\nid="rover"\nname="Rover"\nkind="primitive"\n');
  const project = await readProjectSnapshot();
  const input = validateChatRequest({ projectId: project.id, scope: `project:${project.id}`, message: "Find the rover", history: [] });
  const call = (name: string, args: unknown = {}): ToolTurn => ({ text: "", calls: [{ id: "call_1", name, arguments: args }] });
  let handoffs = 0;
  const code = async () => { handoffs++; return { text: "Checked code applied", applied: true, files: ["src/components.rs"] }; };
  let step = 0;
  const found = await mcpChat(project, input, code, (_settings, instructions, prompt, tools) => {
    assert.match(instructions, /Names are NOT unique/); assert.match(instructions, /ACTIVE SCENE assets\/scenes\/main/);
    assert.match(prompt, /Preserve the rover/); assert.match(prompt, /tools.statement-implementation/);
    assert.ok(tools.some(tool => tool.name === "set_component"));
    return async results => {
      if (step++ === 0) return call("find", { query: "rover" });
      assert.match(JSON.stringify(results), /asset:rover/);
      return { text: "Found Rover", calls: [] };
    };
  });
  assert.equal(found.applied, false); assert.equal(handoffs, 0);
  const run = (next: (results: ToolOutput[]) => Promise<ToolTurn>) => mcpChat(project, input, code, () => next);
  step = 0;
  await run(async results => {
    if (step++ === 0) return call("read_resource", { uri: "emergent://project" });
    assert.match(JSON.stringify(results), /Chat/); return { text: "Read", calls: [] };
  });
  step = 0;
  await run(async results => {
    if (step++ === 0) return call("set_transform", { ref: "instance:main/gone", revision: "stale", translation: [1, 2, 3] });
    assert.equal(results[0].isError, true); return { text: "Edit refused", calls: [] };
  });
  assert.equal((await run(async () => call("implement_project_code"))).applied, true); assert.equal(handoffs, 1);
  step = 0;
  await run(async results => {
    if (step++ === 0) return call("assign_ids", { dryRun: true });
    assert.equal(results[0].isError, false); return { text: "No changes", calls: [] };
  });
  await assert.rejects(run(async () => call("find", { query: "rover" })), /24-turn limit/);
  await assert.rejects(run(async () => ({ text: "", calls: Array.from({ length: 33 }, (_, i) => ({ id: `c${i}`, name: "find", arguments: { query: "rover" } })) })), /32-tool-call limit/);
  await assert.rejects(run(async () => { process.env.EMERGENT_PROJECT_PATH = directory; return call("find", { query: "rover" }); }), /ENOENT|linked project changed/);
  process.env.EMERGENT_PROJECT_PATH = root;
  // A real journalled mutation needs no native renderer: give loose media a durable descriptor.
  await mkdir(path.join(root, "assets/textures"));
  const unnamed = path.join(root, "assets/textures/unnamed.png");
  await writeFile(unnamed, Buffer.from("iVBORw0KGgo=", "base64"));
  step = 0;
  const partial = await run(async results => {
    if (step++ === 0) return call("assign_ids");
    assert.equal(results[0].isError, false, JSON.stringify(results));
    assert.equal(JSON.parse((results[0].content[0] as {text:string}).text).applied, true); throw new Error("Provider disconnected");
  });
  assert.equal(partial.applied, true); assert.match(partial.text, /Provider disconnected/); assert.match(partial.text, /Completed MCP changes/);
  assert.ok(JSON.parse(await readFile(path.join(root, "assets/textures/media.asset.json"), "utf8")).id);
  await mkdir(path.join(root, "assets/other"));
  await writeFile(path.join(root, "assets/other/another.png"), Buffer.from("iVBORw0KGgo=", "base64"));
  step = 0;
  const refused = await run(async () => step++ === 0 ? call("assign_ids") : call("implement_project_code"));
  assert.equal(refused.applied, true); assert.match(refused.text, /MCP edits completed/); assert.equal(handoffs, 1);

  // Exercise the default chat route through the actual provider adapter and MCP client.
  let requests = 0;
  const api = createServer((req, res) => {
    let raw = "";
    req.on("data", chunk => { raw += chunk; });
    req.on("end", () => {
      const body = JSON.parse(raw);
      res.setHeader("Content-Type", "application/json");
      if (++requests === 1) {
        assert.ok(body.tools.some((tool: {name:string}) => tool.name === "find"));
        res.end(JSON.stringify({ status: "completed", output: [{ type: "function_call", call_id: "route_1", name: "find", arguments: '{"query":"rover"}' }] }));
      } else {
        assert.match(JSON.stringify(body.input), /asset:rover/);
        res.end(JSON.stringify({ status: "completed", output: [{ type: "message", content: [{ type: "output_text", text: "Rover is available in this project." }] }] }));
      }
    });
  });
  await new Promise<void>(resolve => api.listen(0, "127.0.0.1", resolve));
  t.after(() => new Promise<void>(resolve => api.close(() => resolve())));
  const address = api.address(); assert.ok(address && typeof address === "object");
  const { updateAISettings } = await import("../src/lib/ai-settings");
  const { POST } = await import("../src/app/api/chat/route");
  for (const provider of ["openai-codex", "vercel"] as const) {
    requests = 0;
    await updateAISettings({ provider, baseUrl: `http://127.0.0.1:${address.port}`, apiKey: "test-key", model: provider === "vercel" ? "anthropic/claude-sonnet-4.6" : "test" });
    const route = await POST(new Request("http://localhost:3001/api/chat", { method: "POST", headers: { "Content-Type": "application/json", Origin: "http://localhost:3001" }, body: JSON.stringify(input) }));
    const answer = await route.json();
    assert.equal(route.status, 200, JSON.stringify(answer));
    assert.equal(answer.applied, false); assert.match(answer.text, /Rover is available/); assert.equal(requests, 2);
  }
});
