// MCP-2 smoke: real edits through /api/mcp against a disposable standard-template project with engine-owned fixtures, with native Rust validation.
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
  assert.match(dry.diff, /- translation = \[0, 0, -2\.5\]/);
  assert.match(dry.diff, /\+ translation = \[1,2,3\]/);
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

  // Behaviour authoring: catalogues, brains, locomotion, previews, components by ref, undo.
  const catalogue = await ok("list_brain_nodes", {});
  assert.ok(catalogue.nodes.some((node: { type: string; category: string }) => node.type === "look_at" && node.category === "Look & aim"));
  assert.ok((await ok("list_locomotion_types", {})).types.some((type: { type: string; status: string }) => type.type === "drone" && type.status === "available"));
  const prey = (await ok("find", { query: "Prey", type: "asset" })).matches.find((match: { kind: string }) => match.kind === "brain");
  assert.ok(prey?.ref.startsWith("asset:"), "brains are addressable assets");
  const preyDetail = await ok("describe", { ref: prey.ref });
  assert.ok(preyDetail.brain.nodes.some((node: { type: string }) => node.type === "evade") && /^[0-9a-f]{64}$/.test(preyDetail.revision));
  const hunterNodes = [
    { id: "root", type: "priority" },
    { id: "watch", parent: "root", type: "look_at", fields: { group: "prey" } },
    { id: "chase", parent: "root", type: "seek", fields: { group: "prey" } },
  ];
  const dryBrain = await ok("create_brain", { name: "Smoke hunter", nodes: hunterNodes, dryRun: true });
  assert.equal(dryBrain.applied, false);
  assert.match(dryBrain.diff, /type = "look_at"/);
  const brokenBrain = await tool("create_brain", { name: "Broken", nodes: [{ type: "seek" }] });
  assert.ok(brokenBrain.isError && /needs a target/.test(String(brokenBrain.value)), "the engine rejects an invalid brain");
  const hunter = await ok("create_brain", { name: "Smoke hunter", description: "Chases prey while watching it.", nodes: hunterNodes });
  assert.equal(hunter.applied, true);
  const hunterFile = path.join(project, hunter.path);
  const created = await readFile(hunterFile, "utf8");
  let detail = await ok("describe", { ref: hunter.ref });
  assert.deepEqual(detail.brain.nodes.map((node: { id: string }) => node.id), ["root", "watch", "chase"]);
  assert.ok((await tool("edit_brain", { ref: hunter.ref, revision: "0".repeat(64), operations: [{ op: "remove", id: "watch" }] })).isError, "stale brain revisions are refused");
  const edited = await ok("edit_brain", { ref: hunter.ref, revision: detail.revision, operations: [
    { op: "add", type: "wander", parent: "root", id: "roam", fields: { weight: 0.5 } },
    { op: "update", id: "chase", fields: { weight: 2 } },
    { op: "remove", id: "watch" },
  ] });
  assert.deepEqual(edited.nodes, ["roam"]);
  detail = await ok("describe", { ref: hunter.ref });
  assert.deepEqual(detail.brain.nodes.map((node: { id: string }) => node.id), ["root", "chase", "roam"]);
  assert.equal(detail.brain.nodes.find((node: { id: string }) => node.id === "chase").fields.weight, 2);
  const preview = await ok("preview_brain", { ref: prey.ref, agents: 10, targets: 1, target_group: "predators", agent_group: "prey", seconds: 5 });
  assert.equal(preview.samples.length, 5);
  assert.ok(preview.samples.every((sample: { meanSpeed: number }) => Number.isFinite(sample.meanSpeed)) && preview.finalAgents.length === 10);

  const drone = await ok("write_locomotion", { name: "Smoke drone", type: "drone", fields: { max_tilt: 20, max_speed: 8 } });
  assert.equal(drone.applied, true);
  assert.ok((await tool("write_locomotion", { name: "Nope", type: "biped_ik" })).isError, "planned types are refused");
  const droneDetail = await ok("describe", { ref: drone.ref });
  assert.equal(droneDetail.locomotion.fields.max_tilt, 20);
  await ok("write_locomotion", { ref: drone.ref, revision: droneDetail.revision, fields: { max_speed: 6, max_tilt: null } });
  const flown = await ok("preview_locomotion", { ref: drone.ref, course: "stop", seconds: 15 });
  // max_speed caps horizontal speed (6); climbing adds up to max_climb (4): at most √(6²+4²) ≈ 7.2 m/s overall.
  assert.ok(flown.maxSpeed <= 7.22 && flown.final.speed < 0.3 && flown.height.max > 5, JSON.stringify(flown));
  assert.equal(flown.maxPitchDegrees, 30, "max_tilt null restored the 30° default");

  // Attach by ref; the scene stores paths, describe shows refs.
  const cube = pyramid;
  let cubeRevision = (await ok("describe", { ref: cube })).revision as string;
  const withBrain = await ok("set_component", { ref: cube, revision: cubeRevision, type: "emergent.brain", fields: { brain: hunter.ref, stack: [prey.ref], max_speed: 6 } });
  cubeRevision = withBrain.revision;
  await ok("set_component", { ref: cube, revision: cubeRevision, type: "emergent.locomotion", fields: { profile: drone.ref } });
  const cubeDetail = await ok("describe", { ref: cube });
  const brainComponent = cubeDetail.components.find((component: { type: string }) => component.type === "emergent.brain");
  assert.equal(brainComponent.fields.brain, hunter.ref);
  assert.deepEqual(brainComponent.fields.stack, [prey.ref]);
  assert.match(await readFile(scenePath, "utf8"), new RegExp(hunter.path.replace(/[.]/g, "\\.")));
  assert.ok((await ok("describe", { ref: drone.ref })).usedBy.some((user: { instance: string }) => user.instance === cube), "profiles list their users");
  const behaviourHistory = (await ok("history", { limit: 7 })).changes.map((item: { tool: string }) => item.tool);
  assert.deepEqual(behaviourHistory.slice(0, 6), ["set_component", "set_component", "update_locomotion", "create_locomotion", "edit_brain", "create_brain"]);
  // Undo newest first: both components, the profile update and creation, then the brain edit.
  for (let step = 0; step < 5; step++) await ok("undo", {});
  assert.doesNotMatch(await readFile(scenePath, "utf8"), /emergent\.brain/, "the components are gone");
  assert.equal(await readFile(hunterFile, "utf8"), created, "undo restores the brain before its edit");
  await ok("undo", {});
  await assert.rejects(readFile(hunterFile, "utf8"), "undoing create_brain removes the file");
  console.log("MCP edit smoke passed.");
} finally {
  unsubscribe();
}
