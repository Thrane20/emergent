// MCP-2 smoke: real edits through /api/mcp against a temporary copy of first_project, with native Rust validation.
// The linked project, IDE data, journal and Cargo output live under target/mcp-smoke; the user's project is never touched.
import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { prepareSmokeProject } from "./mcp-smoke-project.mjs";

const { project, env } = await prepareSmokeProject();
Object.assign(process.env, env);

const { POST } = await import("../src/app/api/mcp/route");
const { mcpToken } = await import("../src/lib/mcp-auth");
const { subscribeProjectEvents } = await import("../src/lib/project-events");
const events: { summary: string; live: boolean }[] = [];
const unsubscribe = subscribeProjectEvents(event => events.push(event));
const scenePath = path.join(project, "assets/scenes/main.scene.toml");
const original = await readFile(scenePath, "utf8");

let id = 0;
async function tool(name: string, args: Record<string, unknown>) {
  const started = Date.now();
  const response = await POST(new Request("http://127.0.0.1:3001/api/mcp", { method: "POST", headers: { "Content-Type": "application/json", Accept: "application/json, text/event-stream", host: "127.0.0.1:3001", Authorization: `Bearer ${await mcpToken()}` }, body: JSON.stringify({ jsonrpc: "2.0", id: ++id, method: "tools/call", params: { name, arguments: args } }) }));
  const result = (await response.json()).result as { isError?: boolean; content: { type: string; text?: string }[] };
  const text = result.content.find(item => item.type === "text")?.text ?? "";
  console.log(`${name.padEnd(15)} ${String(Date.now() - started).padStart(6)} ms ${result.isError ? "ERROR " + text.slice(0, 140) : ""}`);
  return { isError: result.isError === true, value: text.startsWith("{") ? JSON.parse(text) : text, content: result.content };
}
const ok = async (name: string, args: Record<string, unknown>) => { const result = await tool(name, args); assert.equal(result.isError, false, String(result.value)); return result.value; };

try {
  const pyramid = "instance:main/pyramid";
  let revision = (await ok("describe", { ref: pyramid })).revision as string;

  const stale = await tool("set_transform", { ref: pyramid, revision: "0".repeat(64), translation: [1, 2, 3] });
  assert.ok(stale.isError && /changed externally/.test(String(stale.value)), "stale revisions are refused");

  const dry = await ok("set_transform", { ref: pyramid, revision, translation: [1, 2, 3], dryRun: true });
  assert.equal(dry.applied, false);
  assert.match(dry.diff, /- translation = \[0\.07295987,-0\.20303124,-2\.5\]\n\+ translation = \[1,2,3\]/);
  assert.equal(await readFile(scenePath, "utf8"), original, "dry runs write nothing");

  let result = await ok("set_transform", { ref: pyramid, revision, translation: [1, 2, 3] });
  assert.equal(result.applied, true); revision = result.revision;
  const moved = await ok("describe", { ref: pyramid });
  assert.deepEqual(moved.transform.local.translation, [1, 2, 3]);
  assert.deepEqual(moved.transform.local.rotation_degrees, (await ok("describe", { ref: pyramid })).transform.local.rotation_degrees, "omitted parts are kept");

  result = await ok("set_material", { ref: pyramid, revision, settings: { albedo: [1, 0, 0] } }); revision = result.revision;
  const red = await ok("describe", { ref: pyramid });
  assert.deepEqual(red.material.settings.albedo, [1, 0, 0]);
  assert.equal(red.material.overridden, true);

  result = await ok("rename", { ref: pyramid, revision, name: "Red pyramid" }); revision = result.revision;
  assert.equal((await ok("find", { query: "red pyramid" })).matches[0].ref, pyramid, "renaming keeps the ref");

  result = await ok("place_asset", { asset: "asset:cube", scene: "scene:main", revision, parent: pyramid, name: "Topper", translation: [0, 2, 0] });
  revision = result.revision; const topper = result.ref as string;
  const placed = await ok("describe", { ref: topper });
  assert.equal(placed.parent, pyramid); assert.equal(placed.name, "Topper"); assert.deepEqual(placed.transform.local.translation, [0, 2, 0]);

  result = await ok("reparent", { ref: topper, revision, parent: null }); revision = result.revision;
  assert.equal((await ok("describe", { ref: topper })).parent, null);

  result = await ok("set_physics", { ref: topper, revision, settings: { enabled: true, body: "dynamic", mass: 2 } }); revision = result.revision;
  assert.equal((await ok("describe", { ref: topper })).physics.settings.mass, 2);

  result = await ok("set_environment", { scene: "scene:main", revision, settings: { ambient_intensity: 0.4 } }); revision = result.revision;
  assert.match(await readFile(scenePath, "utf8"), /ambient_intensity = 0\.4/);

  result = await ok("delete", { ref: topper, revision }); revision = result.revision;
  assert.equal((await tool("describe", { ref: topper })).isError, true, "deleted refs no longer resolve");

  const invalid = await tool("set_material", { ref: pyramid, revision, settings: { shading: "glossy" } });
  assert.equal(invalid.isError, true, "invalid settings are rejected before writing");

  const view = await tool("view", { scene: "scene:main" });
  assert.ok(view.content.some(item => item.type === "image"), "view renders the edited scene");

  const history = (await ok("history", {})).changes as { tool: string }[];
  assert.deepEqual(history.map(item => item.tool), ["delete", "set_environment", "set_physics", "reparent", "place_asset", "rename", "set_material", "set_transform"]);
  assert.equal(events.length, 8, "each applied edit notifies open IDE pages");

  for (let step = 0; step < 8; step++) await ok("undo", {});
  assert.equal(await readFile(scenePath, "utf8"), original, "undoing every change restores the original bytes");
  assert.equal((await tool("undo", {})).isError, true, "nothing left to undo");

  revision = (await ok("describe", { ref: pyramid })).revision;
  await ok("set_transform", { ref: pyramid, revision, scale: [2, 2, 2] });
  await writeFile(scenePath, (await readFile(scenePath, "utf8")) + "\n# edited by hand\n");
  const conflict = await tool("undo", {});
  assert.ok(conflict.isError && /undo refused/.test(String(conflict.value)), "undo never overwrites newer edits");

  const ids = await ok("assign_ids", { dryRun: true });
  assert.match(ids.summary, /explosion_atlas\.png/);
  await ok("assign_ids", {});
  const overview = await ok("project_overview", {});
  assert.equal(overview.diagnostics.filter((item: { code: string }) => item.code === "missing-id").length, 0, "every file now has a durable id");
  assert.ok((await ok("find", { query: "explosion atlas" })).matches[0].ref.startsWith("asset:"));
  await ok("undo", {});
  assert.equal((await ok("project_overview", {})).diagnostics.filter((item: { code: string }) => item.code === "missing-id").length, 1, "undo removes the created descriptor");
  console.log("MCP edit smoke passed.");
} finally {
  unsubscribe();
}
