import assert from "node:assert/strict";
import test from "node:test";
import { cp, mkdtemp, readdir, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { parse, stringify } from "smol-toml";
import { POST } from "../src/app/api/mcp/route";
import { mcpToken } from "../src/lib/mcp-auth";

test("MCP marks a car's wheels by instance ref and previews a physical aircraft's circuit, validated by the engine", { timeout: 300000 }, async t => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-mcp-vehicles-"));
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
  (cargo.package as Record<string, unknown>).name = "emergent-mcp-vehicles-fixture";
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

  const profile = await tool("write_locomotion", { name: "Car", type: "wheeled" });
  let revision = (await tool("describe", { ref: "scene:main" })).revision;
  const wheels: string[] = [];
  for (const [name, x, z] of [["Front left", -0.6, -0.8], ["Front right", 0.6, -0.8], ["Rear left", -0.6, 0.8], ["Rear right", 0.6, 0.8]] as const) {
    const placed = await tool("place_asset", { asset: "asset:cube", scene: "scene:main", revision, parent: "instance:main/cube", name, translation: [x, -0.4, z], scale: [0.2, 0.4, 0.4] });
    wheels.push(placed.ref); revision = placed.revision;
  }
  // A wheel that is not part of the car is refused by the engine and nothing changes.
  const before = await readFile(path.join(root, "assets/scenes/main.scene.toml"), "utf8");
  const refused = await tool("set_component", { ref: "instance:main/cube", revision, type: "emergent.locomotion", fields: { profile: profile.ref, steering_wheels: ["instance:main/camera"] } }, true);
  assert.match(refused, /must be a child/);
  assert.equal(await readFile(path.join(root, "assets/scenes/main.scene.toml"), "utf8"), before);
  const set = await tool("set_component", { ref: "instance:main/cube", revision, type: "emergent.locomotion", fields: { profile: profile.ref, steering_wheels: wheels.slice(0, 2), rolling_wheels: wheels.slice(2), wheel_radius: 0.4 } });
  assert.equal(set.applied, true);
  const car = await tool("describe", { ref: "instance:main/cube" });
  const locomotion = car.components.find((component: { type: string }) => component.type === "emergent.locomotion");
  const ids = wheels.map(ref => ref.split("/").at(-1));
  assert.deepEqual([locomotion.fields.steering_wheels, locomotion.fields.rolling_wheels, locomotion.fields.wheel_radius], [ids.slice(0, 2), ids.slice(2), 0.4]);

  // A physical aircraft: the catalogue lists it, and its preview flies a circuit and reports phases and touchdown.
  const types = (await tool("list_locomotion_types")).types as { type: string; status: string }[];
  assert.equal(types.find(type => type.type === "fixed_wing_physics")?.status, "available");
  const nodes = (await tool("list_brain_nodes")).nodes as { type: string }[];
  for (const node of ["take_off", "land"]) assert.ok(nodes.some(item => item.type === node), node);
  const plane = await tool("write_locomotion", { name: "Trainer", type: "fixed_wing_physics" });
  const preview = await tool("preview_locomotion", { ref: plane.ref, course: "circuit", seconds: 240 });
  for (const phase of ["Take-off roll", "Climb", "Downwind", "Base", "Final", "Flare", "Rollout", "Parked"]) assert.ok(preview.phases.includes(phase), `${phase} in ${preview.phases}`);
  assert.ok(preview.touchdown && Math.abs(preview.touchdown.position[0]) < 15, JSON.stringify(preview.touchdown));

  // The car's profile becomes physical underneath it: the scene is now invalid (no dynamic body). Edits name the
  // object at fault, and deleting is still allowed (it only reports the remaining problem).
  const locomotionDirectory = path.join(root, "assets/locomotion");
  const carFile = (await readdir(locomotionDirectory)).map(name => path.join(locomotionDirectory, name)).find(file => file.endsWith(`${profile.ref.slice("asset:".length)}.locomotion.toml`))!;
  await writeFile(carFile, (await readFile(carFile, "utf8")).replace(/type = "wheeled"[\s\S]*$/, 'type = "fixed_wing_physics"\n'));
  revision = (await tool("describe", { ref: "scene:main" })).revision;
  const blocked = await tool("set_transform", { ref: wheels[3], revision, translation: [0.6, -0.4, 0.9] }, true);
  assert.match(blocked, /object cube: physical locomotion .* requires an enabled dynamic PhysicsBody/);
  const removed = await tool("delete", { ref: wheels[3], revision });
  assert.equal(removed.applied, true);
  assert.match(removed.summary, /the scene still has a problem: .*object cube: physical locomotion/);
  assert.ok(!(await readFile(path.join(root, "assets/scenes/main.scene.toml"), "utf8")).includes(`id = "${wheels[3].split("/").at(-1)}"`));
});
