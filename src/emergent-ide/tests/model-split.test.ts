import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { modelParts } from "../src/lib/model-parts";
import { SplitJobs, type SplitProvider } from "../src/lib/model-split";

const ARM = path.resolve("../../tests/fixtures/models/robot-arm.glb");
const uuid = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, "0")}`;

test("a split uploads once, polls, downloads a validated GLB and never resubmits", async t => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-split-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const root = path.join(directory, "game");
  await mkdir(path.join(root, "assets/models/arm"), { recursive: true });
  await writeFile(path.join(root, "assets/models/arm/model.glb"), await readFile(ARM));
  const project = { id: "game", root, assetRoots: ["assets"] };
  const calls: string[] = [];
  let state: "queued" | "splitting" | "completed" = "queued";
  const provider: SplitProvider = {
    async upload(bytes) { calls.push(`upload:${bytes.length}`); return "https://fal.media/in.glb"; },
    async submit(endpoint, input) { calls.push(`submit:${endpoint}:${JSON.stringify(input)}`); return "request-1"; },
    async status() { return state; },
    async result() { calls.push("result"); return readFile(ARM); },
  };
  const jobs = new SplitJobs(path.join(directory, "data"), async () => provider);
  const input = { id: uuid(1), model: "assets/models/arm/model.glb", endpoint: "hitem3d/hi3d/split", level: "high" };
  const job = await jobs.create(project, input);
  assert.equal(job.state, "queued");
  assert.deepEqual(calls.map(call => call.split(":")[0]), ["upload", "submit"]);
  assert.match(calls[1], /"model":"general","level":"high","export_format":"glb"/);
  assert.equal((await jobs.create(project, input)).id, job.id, "the same request ID never submits twice");
  assert.equal(calls.filter(call => call.startsWith("submit")).length, 1);
  state = "splitting";
  assert.equal((await jobs.refresh(project, job.id)).state, "splitting");
  await assert.rejects(jobs.artifact(project, job.id), /not ready/);
  state = "completed";
  assert.equal((await jobs.refresh(project, job.id)).state, "ready");
  assert.equal((await readFile(await jobs.artifact(project, job.id))).length, (await readFile(ARM)).length);
  assert.deepEqual((await jobs.list(project, "assets/models/arm/model.glb")).map(item => item.id), [job.id]);

  const failing: SplitProvider = { ...provider, async submit() { throw new Error("network"); } };
  const uncertain = await new SplitJobs(path.join(directory, "data"), async () => failing).create(project, { ...input, id: uuid(2) });
  assert.equal(uncertain.state, "uncertain", "an unconfirmed paid submission is not retried");
  await assert.rejects(jobs.create(project, { ...input, id: uuid(3), model: "../outside.glb" }), /saved .glb model/);
  await assert.rejects(jobs.create(project, { ...input, id: uuid(4), endpoint: "someone/else" }), /supported part splitter/);
});

test("part lists are bounded, uniquely identified and acyclic", () => {
  const part = (n: number, parent: number | null = null) => ({ id: uuid(n), name: `Part ${n}`, parent: parent === null ? null : uuid(parent), pivot: [0, 1, 0] });
  assert.equal(modelParts([part(1), part(2, 1)]).length, 2);
  assert.throws(() => modelParts([]), /1–64 parts/);
  assert.throws(() => modelParts([part(1), part(1)]), /unique/);
  assert.throws(() => modelParts([part(1, 2), part(2, 1)]), /cycle/);
  assert.throws(() => modelParts([part(1, 9)]), /missing parent/);
  assert.throws(() => modelParts([{ ...part(1), pivot: [0, Number.NaN, 0] }]), /pivots/);
  assert.throws(() => modelParts([{ ...part(1), name: "../rotor" }]), /Name parts/);
});
