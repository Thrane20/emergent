import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { importRigged, inspectRig } from "../src/lib/model-rig";
import { SplitJobs, type SplitProvider } from "../src/lib/model-split";
import type { ProjectSnapshot } from "../src/lib/project";

const MANNEQUIN = path.resolve("../../templates/standard/assets/models/mannequin.glb");
const WALK = path.resolve("../../templates/standard/assets/mocap/walk.bvh");
const exists = (file: string) => stat(file).then(() => true, () => false);

test("rigged models and BVH takes import through the engine inspector and roll back on failure", async t => {
  const root = await mkdtemp(path.join(os.tmpdir(), "emergent-rig-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const project = { id: "game", root, assetRoots: ["assets"] } as unknown as ProjectSnapshot;
  const calls: string[][] = [];
  const tool = async (_: ProjectSnapshot, args: string[]) => { calls.push(args); return 'skinned = true\nhumanoid_missing = []\nclips = []\n[[joints]]\nname = "hips"\n'; };

  const model = await importRigged(project, { name: "Hero Knight", extension: "glb", bytes: await readFile(MANNEQUIN) }, tool);
  assert.equal(model.path, "assets/entities/hero-knight.entity.toml");
  assert.equal(model.report.skinned, true);
  assert.deepEqual(calls[0], ["--inspect-model", "assets/models/hero-knight.glb"]);
  assert.match(await readFile(path.join(root, model.path), "utf8"), /kind = "model"\nmodel = "assets\/models\/hero-knight\.glb"/);
  await assert.rejects(importRigged(project, { name: "Hero Knight", extension: "glb", bytes: await readFile(MANNEQUIN) }, tool), /already exists/);
  assert.ok(await exists(path.join(root, "assets/models/hero-knight.glb")), "a name clash keeps the existing model");

  const take = await importRigged(project, { name: "walk", extension: "bvh", bytes: await readFile(WALK) }, tool);
  assert.equal(take.path, "assets/mocap/walk.bvh");
  await assert.rejects(importRigged(project, { name: "page", extension: "bvh", bytes: Buffer.from("<html>") }, tool), /not a BVH/);
  await assert.rejects(importRigged(project, { name: "page", extension: "glb", bytes: Buffer.from("<html>") }, tool), /GLB/);
  await assert.rejects(importRigged(project, { name: "x", extension: "fbx", bytes: Buffer.alloc(1) }, tool), /\.glb, \.vrm or \.bvh/);

  const broken = async () => { throw new Error("compiling…\nThe skin names a joint that is not a node."); };
  await assert.rejects(importRigged(project, { name: "Broken", extension: "vrm", bytes: await readFile(MANNEQUIN) }, broken), /could not load assets\/models\/broken\.vrm: The skin names a joint/);
  assert.equal(await exists(path.join(root, "assets/models/broken.vrm")), false, "a file the engine rejects is removed");
  await assert.rejects(inspectRig(project, "../outside.glb", tool), /in this project's assets/);
});

test("auto-rig jobs submit Meshy's input once and stay out of the part-split list", async t => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-autorig-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const root = path.join(directory, "game");
  await mkdir(path.join(root, "assets/models"), { recursive: true });
  await import("node:fs/promises").then(fs => fs.copyFile(MANNEQUIN, path.join(root, "assets/models/statue.glb")));
  const project = { id: "game", root, assetRoots: ["assets"] };
  const submitted: string[] = [];
  const provider: SplitProvider = {
    async upload() { return "https://fal.media/in.glb"; },
    async submit(endpoint, input) { submitted.push(`${endpoint} ${JSON.stringify(input)}`); return "request-1"; },
    async status() { return "completed"; },
    async result() { return readFile(MANNEQUIN); },
  };
  const jobs = new SplitJobs(path.join(directory, "data"), async () => provider);
  const job = await jobs.create(project, { id: "00000000-0000-4000-8000-000000000001", model: "assets/models/statue.glb", endpoint: "fal-ai/meshy/rigging" });
  assert.deepEqual(submitted, ['fal-ai/meshy/rigging {"model_url":"https://fal.media/in.glb","height_meters":1.7}']);
  assert.equal((await jobs.refresh(project, job.id)).state, "ready");
  assert.deepEqual((await jobs.list(project, "assets/models/statue.glb", true)).map(item => item.id), [job.id]);
  assert.deepEqual(await jobs.list(project, "assets/models/statue.glb"), [], "Model Parts lists splits only");
});
