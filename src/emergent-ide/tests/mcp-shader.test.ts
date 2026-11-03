import assert from "node:assert/strict";
import test from "node:test";
import { cp, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { parse, stringify } from "smol-toml";
import { POST } from "../src/app/api/mcp/route";
import { mcpToken } from "../src/lib/mcp-auth";

test("MCP clients discover, create, edit, apply and undo shader node graphs with native validation", { timeout: 300000 }, async t => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-mcp-shader-"));
  const root = path.join(directory, "game"), foundation = path.resolve("../..");
  const oldRoot = process.env.EMERGENT_PROJECT_PATH, oldData = process.env.EMERGENT_IDE_DATA_DIR;
  t.after(async () => {
    if (oldRoot === undefined) delete process.env.EMERGENT_PROJECT_PATH; else process.env.EMERGENT_PROJECT_PATH = oldRoot;
    if (oldData === undefined) delete process.env.EMERGENT_IDE_DATA_DIR; else process.env.EMERGENT_IDE_DATA_DIR = oldData;
    await rm(directory, { recursive: true, force: true });
  });
  await cp(path.join(foundation, "templates/standard"), root, { recursive: true });
  const cargo = parse(await readFile(path.join(root, "Cargo.toml"), "utf8"));
  (cargo.dependencies as Record<string, { path: string }>).emergent.path = foundation; cargo.workspace = {};
  // Its own package name gives this fixture its own candidate binary in the shared target directory.
  (cargo.package as Record<string, unknown>).name = "emergent-mcp-shader-fixture";
  await writeFile(path.join(root, "Cargo.toml"), stringify(cargo));
  await cp(path.join(foundation, "Cargo.lock"), path.join(root, "Cargo.lock"));
  process.env.EMERGENT_PROJECT_PATH = root; process.env.EMERGENT_IDE_DATA_DIR = path.join(directory, "data");
  let id = 0;
  const tool = async (name: string, args: unknown = {}, error = false) => {
    const response = await POST(new Request("http://127.0.0.1:3001/api/mcp", {
      method: "POST", headers: { "Content-Type": "application/json", Accept: "application/json, text/event-stream", host: "127.0.0.1:3001", Authorization: `Bearer ${await mcpToken()}` },
      body: JSON.stringify({ jsonrpc: "2.0", id: ++id, method: "tools/call", params: { name, arguments: args } }),
    }));
    const result = (await response.json()).result, text = result.content[0].text;
    assert.equal(result.isError === true, error, text);
    return text.startsWith("{") ? JSON.parse(text) : text;
  };

  // Discovery: the catalogue, the template shaders and a described graph.
  const catalogue = await tool("list_shader_nodes");
  assert.deepEqual(catalogue.nodes.find((node: { type: string }) => node.type === "smoothstep").inputs, ["Low", "High", "X"]);
  assert.deepEqual(catalogue.outputs.map((output: { name: string }) => output.name), ["red", "green", "blue", "emission", "opacity", "displacement"]);
  const listed = await tool("list", { type: "assets", kind: "shader" }); assert.equal(listed.total, 3);
  const water = await tool("describe", { ref: "asset:water" });
  assert.equal(water.outputs.displacement, "displace");
  assert.deepEqual(water.nodes.find((node: { id: string }) => node.id === "displace").inputs, { A: "waves", B: "height" });
  assert.ok(water.usedBy.some((file: string) => file.endsWith(".material.toml")), "materials using the shader are listed");

  // Create: dry run writes nothing; then a white graph with a pulsing glow wired by node ID.
  const glow = [
    { op: "set_parameter", id: "speed", name: "Pulse speed", default: 2, min: 0, max: 10 },
    { op: "add_node", id: "t", type: "time" },
    { op: "add_node", id: "rate", type: "parameter", parameter: "speed" },
    { op: "add_node", id: "phase", type: "multiply", inputs: ["t", "rate"] },
    { op: "add_node", id: "wave", type: "sine" },
    { op: "connect", from: "phase", to: "wave" },
    { op: "connect", from: "wave", output: "emission" },
  ];
  const dry = await tool("create_shader", { name: "Pulse", operations: glow, dryRun: true });
  assert.equal(dry.applied, false); await assert.rejects(stat(path.join(root, dry.path)), { code: "ENOENT" });
  const created = await tool("create_shader", { name: "Pulse", operations: glow });
  assert.equal(created.applied, true); assert.match(created.ref, /^asset:/); assert.deepEqual(created.nodes, ["t", "rate", "phase", "wave"]);
  const saved = parse(await readFile(path.join(root, created.path), "utf8")) as { nodes: { id: string; position?: number[] }[] };
  assert.ok(saved.nodes.every(node => node.position?.length === 2), "MCP-made nodes get editor positions");
  let described = await tool("describe", { ref: created.ref });
  assert.equal(described.outputs.emission, "wave");

  // Edit: wiring a new node into an earlier one is re-ordered for the file contract; bad edits change nothing.
  const good = await readFile(path.join(root, created.path), "utf8");
  for (const operations of [
    [{ op: "connect", from: "wave", to: "phase", input: "A" }],
    [{ op: "remove_node", id: "wave" }],
    [{ op: "connect", from: "t", to: "phase", input: "C" }],
    [{ op: "remove_parameter", id: "speed" }],
    [{ op: "add_node", type: "fresnel", id: "rim" }, { op: "connect", from: "rim", output: "displacement" }],
  ]) {
    await tool("edit_shader", { ref: created.ref, revision: described.revision, operations }, true);
    assert.equal(await readFile(path.join(root, created.path), "utf8"), good);
  }
  const edited = await tool("edit_shader", { ref: created.ref, revision: described.revision, transparent: true, operations: [
    { op: "add_node", id: "rim", type: "fresnel" },
    { op: "connect", from: "rim", output: "opacity" },
    { op: "update_node", id: "t", type: "uv_y" },
  ] });
  assert.equal(edited.applied, true);
  await tool("edit_shader", { ref: created.ref, revision: described.revision, name: "Stale" }, true);
  described = await tool("describe", { ref: created.ref });
  assert.equal(described.transparent, true); assert.equal(described.outputs.opacity, "rim");
  assert.equal(described.nodes.find((node: { id: string }) => node.id === "t").type, "uv_y");

  // Apply through a material by shader ref, then undo the shader edit.
  const cube = await tool("describe", { ref: "instance:main/cube" });
  const applied = await tool("set_material", { ref: "instance:main/cube", revision: cube.revision, settings: { shader: created.ref, cast_shadows: false, shader_parameters: { speed: 4 } } });
  assert.equal(applied.applied, true);
  assert.match(await readFile(path.join(root, "assets/scenes/main.scene.toml"), "utf8"), new RegExp(created.path.replace(/[.]/g, "\\.")));
  await tool("undo", { changeId: edited.changeId });
  assert.equal(await readFile(path.join(root, created.path), "utf8"), good);
});
