import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

let root = "", project = "", data = "";
let POST: typeof import("../src/app/api/mcp/route").POST;
let mcpToken: typeof import("../src/lib/mcp-auth").mcpToken;
test.before(async () => {
  root = await mkdtemp(path.join(os.tmpdir(), "emergent-mcp-"));
  project = path.join(root, "game"); data = path.join(root, "data");
  process.env.EMERGENT_PROJECT_PATH = project;
  process.env.EMERGENT_IDE_DATA_DIR = data;
  await mkdir(path.join(project, "assets/entities"), { recursive: true });
  await mkdir(path.join(project, "assets/scenes"), { recursive: true });
  await mkdir(path.join(project, "src"), { recursive: true });
  await writeFile(path.join(project, "emergent.toml"), 'schema_version = 1\nid = "mcp.fixture"\nname = "MCP fixture"\nentry_scene = "assets/scenes/main.scene.toml"\nasset_roots = ["assets"]\n');
  await writeFile(path.join(project, "assets/entities/pyramid.entity.toml"), 'schema_version = 1\nid = "pyramid"\nname = "Pyramid"\nkind = "primitive"\n[transform]\ntranslation = [0, 0, 0]\nrotation_degrees = [0, 0, 0]\nscale = [1, 1, 1]\n[geometry]\nprimitive = "pyramid"\ncolor = [1, 0, 0, 1]\n');
  await writeFile(path.join(project, "assets/scenes/main.scene.toml"), 'schema_version = 1\nid = "main"\n[[entities]]\nid = "pyramid"\nasset = "assets/entities/pyramid.entity.toml"\n');
  await writeFile(path.join(project, "src/main.rs"), "fn main() {}\n");
  ({ POST } = await import("../src/app/api/mcp/route"));
  ({ mcpToken } = await import("../src/lib/mcp-auth"));
});
test.after(() => rm(root, { recursive: true, force: true }));

let id = 0;
async function call(body: unknown, headers: Record<string, string> = {}) {
  return POST(new Request("http://127.0.0.1:3001/api/mcp", { method: "POST", headers: { "Content-Type": "application/json", Accept: "application/json, text/event-stream", host: "127.0.0.1:3001", ...headers }, body: JSON.stringify(body) }));
}
async function rpc(method: string, params: unknown = {}) {
  const response = await call({ jsonrpc: "2.0", id: ++id, method, params }, { Authorization: `Bearer ${await mcpToken()}` });
  assert.equal(response.status, 200);
  return (await response.json()) as { result?: Record<string, unknown>; error?: { message: string } };
}
async function tool(name: string, args: unknown) {
  const { result } = await rpc("tools/call", { name, arguments: args });
  const content = result!.content as { type: string; text: string }[];
  return { isError: result!.isError === true, value: content[0].text.startsWith("{") ? JSON.parse(content[0].text) : content[0].text };
}

test("the endpoint rejects missing tokens, foreign hosts and cross-origin browsers", async () => {
  const init = { jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "t", version: "1" } } };
  assert.equal((await call(init)).status, 401);
  assert.equal((await call(init, { Authorization: "Bearer wrong" })).status, 401);
  const token = await mcpToken();
  assert.equal((await call(init, { Authorization: `Bearer ${token}`, host: "attacker.example" })).status, 403);
  assert.equal((await call(init, { Authorization: `Bearer ${token}`, Origin: "http://evil.example" })).status, 403);
  assert.equal(((await stat(path.join(data, "mcp-token"))).mode & 0o777), 0o600);
  assert.equal((await readFile(path.join(data, "mcp-token"), "utf8")).trim(), token);
});

test("an authorised client initialises, lists tools and resolves a name to a durable ref", async () => {
  const init = await rpc("initialize", { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "test", version: "1" } });
  assert.match(String((init.result as { instructions?: string }).instructions), /Names are NOT unique/);
  const tools = ((await rpc("tools/list")).result!.tools as { name: string; annotations?: { readOnlyHint?: boolean } }[]);
  const readOnly = ["describe", "find", "history", "list", "list_brain_nodes", "list_component_types", "list_locomotion_types", "list_shader_nodes", "list_ui_widgets", "preview_brain", "preview_locomotion", "preview_ui", "project_overview", "view"];
  const writes = ["assign_ids", "create_brain", "create_shader", "create_ui", "delete", "edit_brain", "edit_shader", "edit_ui", "generate_audio", "place_asset", "rename", "reparent", "set_camera", "set_component", "set_environment", "set_material", "set_physics", "set_transform", "set_wind", "undo", "write_locomotion"];
  assert.deepEqual(tools.map(item => item.name).sort(), [...readOnly, ...writes].sort());
  for (const item of tools) assert.equal(item.annotations?.readOnlyHint, readOnly.includes(item.name), `${item.name} read-only annotation`);
  const edit = await tool("set_transform", { ref: "instance:main/pyramid", revision: "nope", translation: [1, 2, 3] });
  assert.equal(edit.isError, true, "edits validate their revision argument");

  const found = await tool("find", { query: "pyramid" });
  assert.equal(found.value.matches[0].ref, "instance:main/pyramid");
  const described = await tool("describe", { ref: "instance:main/pyramid" });
  assert.equal(described.value.asset, "asset:pyramid");
  assert.deepEqual(described.value.transform.local.translation, [0, 0, 0]);
  assert.match(described.value.revision, /^[0-9a-f]{64}$/);
  const asset = await tool("describe", { ref: "asset:pyramid" });
  assert.deepEqual(asset.value.usedBy.map((item: { ref: string }) => item.ref), ["instance:main/pyramid"]);

  const stale = await tool("describe", { ref: "instance:main/gone" });
  assert.equal(stale.isError, true);
  assert.match(stale.value, /does not exist/);
  const overview = await tool("project_overview", {});
  assert.equal(overview.value.project.entryScene, "scene:main");
  const scene = (await rpc("resources/read", { uri: "emergent://scene/main" })).result!.contents as { text: string }[];
  assert.equal(JSON.parse(scene[0].text).roots[0].ref, "instance:main/pyramid");
});
