import assert from "node:assert/strict";
import test from "node:test";
import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { parse, stringify } from "smol-toml";
import { readProjectSnapshot } from "../src/lib/project";
import { addMapItems, deleteMapItem, moveMapItem, sceneMap, setMapComponent, toggleWaypointLink } from "../src/lib/scene-map";
import { fitView, invertAffine, metadataOf, toScreen, toWorld, transformPoint, waypointLinks, zoneShape, zoomAt, type MapInstance } from "../src/lib/scene-map-types";
import { undoChange } from "../src/lib/project-ops";

test("top-down maths: inverse transforms, screen round trips, zoom about the cursor, zone shapes", () => {
  // Yaw 90°, scale 2, at (10, 1, -4).
  const m = [0, 0, -2, 0, 0, 2, 0, 0, 2, 0, 0, 0, 10, 1, -4, 1];
  const local = transformPoint(invertAffine(m), [14, 1, -4]);
  assert.deepEqual(local.map(n => Math.round(n * 1000) / 1000), [0, 0, 2]);
  const view = { x: 5, z: -3, height: 50 }, size = { width: 800, height: 600 };
  const [sx, sy] = toScreen(view, size, 12, 4);
  assert.deepEqual(toWorld(view, size, sx, sy).map(n => Math.round(n * 1e6) / 1e6), [12, 4]);
  assert.ok(toScreen(view, size, 5, -10)[1] < 300, "north (-Z) is up");
  const zoomed = zoomAt(view, size, 100, 80, 0.5), before = toWorld(view, size, 100, 80), after = toWorld(zoomed, size, 100, 80);
  assert.equal(zoomed.height, 25);
  assert.deepEqual(after.map(n => Math.round(n * 1e6)), before.map(n => Math.round(n * 1e6)), "the point under the cursor stays put");
  assert.ok(fitView([[0, 0], [100, 40]], size).height > 0);
  const instance = { id: "z", name: "Z", parent: null, asset: "a", components: [{ type: "emergent.zone", enabled: true, fields: { shape: "rect", width: 4, depth: 2, cost: 3 } }], transform: { translation: [0, 0, 0], rotation_degrees: [0, 90, 0], scale: [2, 1, 2] }, matrix: m, outline: [] } satisfies MapInstance;
  const shape = zoneShape(instance)!;
  assert.equal(shape.kind, "rect");
  assert.deepEqual([shape.kind === "rect" && shape.width, shape.kind === "rect" && shape.depth, Math.round(shape.yaw * 180 / Math.PI), shape.cost], [8, 4, 90, 3]);
});

test("map edits place, link, move, edit, delete and undo through engine validation", { timeout: 300000 }, async t => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-map-test-"));
  const root = path.join(directory, "game"), foundation = path.resolve(process.cwd(), "../..");
  const previousRoot = process.env.EMERGENT_PROJECT_PATH, previousData = process.env.EMERGENT_IDE_DATA_DIR;
  t.after(async () => {
    if (previousRoot === undefined) delete process.env.EMERGENT_PROJECT_PATH; else process.env.EMERGENT_PROJECT_PATH = previousRoot;
    if (previousData === undefined) delete process.env.EMERGENT_IDE_DATA_DIR; else process.env.EMERGENT_IDE_DATA_DIR = previousData;
    await rm(directory, { recursive: true, force: true });
  });
  await cp(path.join(foundation, "templates/standard"), root, { recursive: true });
  const cargo = parse(await readFile(path.join(root, "Cargo.toml"), "utf8"));
  (cargo.dependencies as Record<string, { path: string }>).emergent.path = foundation;
  cargo.workspace = {};
  // Its own package name gives this fixture its own binary in the shared target directory.
  (cargo.package as Record<string, unknown>).name = "emergent-scene-map-fixture";
  await writeFile(path.join(root, "Cargo.toml"), stringify(cargo));
  await cp(path.join(foundation, "Cargo.lock"), path.join(root, "Cargo.lock"));
  process.env.EMERGENT_PROJECT_PATH = root; process.env.EMERGENT_IDE_DATA_DIR = path.join(directory, "data");
  const scene = "assets/scenes/main.scene.toml";
  let project = await readProjectSnapshot(), map = await sceneMap(project, scene);
  const ground = map.instances.find(instance => instance.outline.length >= 3);
  assert.ok(ground, "the template's ground has a top-down outline");

  // A path of three linked waypoints under a new group, and a rotated rectangle zone with metadata.
  const path3 = await addMapItems(project, { scene, revision: map.revision, parent: null, path: true, name: "Patrol", items: [{ kind: "waypoint", x: 0, z: 0 }, { kind: "waypoint", x: 10, z: 0 }, { kind: "waypoint", x: 10, z: 10 }] });
  map = await sceneMap(project, scene);
  const [a, b, c] = path3.ids;
  const byId = (id: string) => map.instances.find(instance => instance.id === id)!;
  assert.equal(byId(a).parent, path3.group);
  assert.deepEqual(waypointLinks(byId(a))?.links, [b]);
  assert.deepEqual(waypointLinks(byId(c))?.links, []);
  assert.deepEqual([byId(c).matrix[12], byId(c).matrix[14]], [10, 10], "world positions survive the group parent");
  const zone = await addMapItems(project, { scene, revision: map.revision, parent: null, items: [{ kind: "zone", x: 30, z: 5, shape: "rect", width: 8, depth: 4, yaw: 45, name: "Base", metadata: { type: "homebase", team: 2 } }] });
  map = await sceneMap(project, scene);
  const base = byId(zone.ids[0]);
  assert.deepEqual(metadataOf(base), { type: "homebase", team: 2 });
  const shape = zoneShape(base)!;
  assert.equal(shape.kind === "rect" && Math.round(shape.width), 8);
  assert.equal(Math.round(shape.yaw * 180 / Math.PI), 45);

  // Link c back to a (closing the loop), then toggling the reverse direction removes it again.
  await toggleWaypointLink(project, { scene, revision: map.revision, from: c, to: a });
  map = await sceneMap(project, scene);
  assert.deepEqual(waypointLinks(byId(c))?.links, [a]);
  await toggleWaypointLink(project, { scene, revision: map.revision, from: a, to: c });
  map = await sceneMap(project, scene);
  assert.deepEqual(waypointLinks(byId(c))?.links, [], "a two-way link is one link whichever end you click");

  await moveMapItem(project, { scene, revision: map.revision, id: b, x: 12, z: -3 });
  map = await sceneMap(project, scene);
  assert.deepEqual([byId(b).matrix[12], byId(b).matrix[14]], [12, -3]);
  await setMapComponent(project, { scene, revision: map.revision, id: zone.ids[0], type: "emergent.zone", fields: { cost: 4, blocked: true, layers: "air" } });
  map = await sceneMap(project, scene);
  assert.equal(zoneShape(byId(zone.ids[0]))?.blocked, true);

  // Deleting b removes a's link to it, so the scene stays valid.
  await deleteMapItem(project, { scene, revision: map.revision, id: b });
  map = await sceneMap(project, scene);
  assert.equal(map.instances.some(instance => instance.id === b), false);
  assert.deepEqual(waypointLinks(byId(a))?.links, []);
  await undoChange(project, {});
  map = await sceneMap(project, scene);
  assert.ok(map.instances.some(instance => instance.id === b), "undo brings it back");
  assert.deepEqual(waypointLinks(byId(a))?.links, [b]);
  await assert.rejects(moveMapItem(project, { scene, revision: "0".repeat(64), id: b, x: 0, z: 0 }), /changed/);
  project = await readProjectSnapshot();
  assert.ok(project.files.some(file => file.path === "assets/entities/map-waypoint.entity.toml"));
});

test("airport map tools place tagged items: runway long side is its landing axis, taxiways link", async () => {
  const { MAP_TOOLS } = await import("../src/lib/map-tools");
  const tool = (id: string) => MAP_TOOLS.find(item => item.id === id)!;
  assert.deepEqual([...new Set(MAP_TOOLS.filter(item => item.group === "Airport").map(item => item.id))], ["aerodrome", "runway", "taxiway", "holding_position", "stand", "tower"]);
  // Dragged east-west: 1000 m long, 40 m wide, so it turns 90° and lands along X.
  const runway = tool("runway").build!({ gesture: "rect", x0: -500, z0: -20, x1: 500, z1: 20 }).items[0];
  assert.deepEqual([runway.depth, runway.width, runway.yaw, runway.metadata?.aeroway], [1000, 40, 90, "runway"]);
  const taxiway = tool("taxiway").build!({ gesture: "polyline", points: [[0, 0], [50, 0], [50, 50]] });
  assert.equal(taxiway.path, true);
  assert.ok(taxiway.items.every(item => item.metadata?.aeroway === "taxiway"));
  assert.equal(tool("stand").build!({ gesture: "point", x: 1, z: 2 }).items[0].metadata?.aeroway, "parking_position");
});

test("scene wind is written as a [wind] table and replaced or removed without touching the rest", async () => {
  const { replaceSceneWind } = await import("../src/lib/scene-lighting");
  const source = "schema_version = 1\nid = \"main\"\n\n[[entities]]\nid = \"a\"\nasset = \"x.entity.toml\"\n";
  const windy = replaceSceneWind(source, { speed: 8, from: 270 });
  assert.deepEqual({ ...(parse(windy) as { wind: object }).wind }, { speed: 8, from: 270 });
  assert.ok(windy.startsWith(source.trimEnd()));
  const changed = replaceSceneWind(windy, { speed: 3, from: 90 });
  assert.deepEqual({ ...(parse(changed) as { wind: object }).wind }, { speed: 3, from: 90 });
  assert.equal((parse(replaceSceneWind(changed, null)) as { wind?: unknown }).wind, undefined);
});

test("the map shows an airport's active runway, circuit and holding fix, and lands into the scene wind", { timeout: 300000 }, async t => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-map-airport-"));
  const root = path.join(directory, "game"), foundation = path.resolve(process.cwd(), "../..");
  const previousRoot = process.env.EMERGENT_PROJECT_PATH, previousData = process.env.EMERGENT_IDE_DATA_DIR;
  t.after(async () => {
    if (previousRoot === undefined) delete process.env.EMERGENT_PROJECT_PATH; else process.env.EMERGENT_PROJECT_PATH = previousRoot;
    if (previousData === undefined) delete process.env.EMERGENT_IDE_DATA_DIR; else process.env.EMERGENT_IDE_DATA_DIR = previousData;
    await rm(directory, { recursive: true, force: true });
  });
  await cp(path.join(foundation, "templates/standard"), root, { recursive: true });
  const cargo = parse(await readFile(path.join(root, "Cargo.toml"), "utf8"));
  (cargo.dependencies as Record<string, { path: string }>).emergent.path = foundation;
  cargo.workspace = {};
  (cargo.package as Record<string, unknown>).name = "emergent-scene-map-fixture";
  await writeFile(path.join(root, "Cargo.toml"), stringify(cargo));
  await cp(path.join(foundation, "Cargo.lock"), path.join(root, "Cargo.lock"));
  process.env.EMERGENT_PROJECT_PATH = root; process.env.EMERGENT_IDE_DATA_DIR = path.join(directory, "data");
  const { MAP_TOOLS } = await import("../src/lib/map-tools");
  const { setMapWind } = await import("../src/lib/scene-map");
  const scene = "assets/scenes/main.scene.toml", project = await readProjectSnapshot();
  let map = await sceneMap(project, scene);
  assert.deepEqual([map.airports, map.wind], [[], null], "no airport or wind yet");
  // A northbound 900 m runway drawn with the tool, plus a tower.
  const runway = MAP_TOOLS.find(tool => tool.id === "runway")!.build!({ gesture: "rect", x0: -15, z0: -750, x1: 15, z1: 150 });
  await addMapItems(project, { scene, revision: map.revision, parent: null, items: runway.items });
  map = await sceneMap(project, scene);
  const tower = MAP_TOOLS.find(tool => tool.id === "tower")!.build!({ gesture: "point", x: 100, z: -300 });
  await addMapItems(project, { scene, revision: map.revision, parent: null, items: tower.items });
  map = await sceneMap(project, scene);
  let active = map.airports[0].runways[0];
  assert.equal(map.airports[0].towered, true);
  assert.deepEqual([active.active, Math.round(active.heading), active.circuit.length], [true, 0, 6], "calm: lands along -Z (north)");
  // A left-hand circuit landing north lies west (-X) of the runway; the holding fix is 3 km south of the threshold.
  assert.ok(active.circuit[3][0] < -300 && map.airports[0].holdingFix![1] > 3000);
  // Wind from the south is a tailwind northbound: the overlay turns the runway round.
  await setMapWind(project, { scene, revision: map.revision, wind: { speed: 6, from: 180 } });
  map = await sceneMap(project, scene);
  active = map.airports[0].runways[0];
  assert.deepEqual(map.wind, { speed: 6, from: 180 });
  assert.equal(Math.round(active.heading), 180);
  assert.ok(active.circuit[3][0] > 300, "left-hand circuit landing south is east of the runway");
  await assert.rejects(setMapWind(project, { scene, revision: map.revision, wind: { speed: 500, from: 0 } }), /0–100/);
});
