import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { mkdtemp, mkdir, readFile, readdir, rm, stat, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test, { type TestContext } from "node:test";
import { ObjectJobs } from "../src/lib/object-jobs";
import { boundedBody, containedFile, imageExtension, validateGlb } from "../src/lib/object-files";
import { createTrellisProvider, ProviderFailure, singleSubmissionFetch, type ObjectProvider } from "../src/lib/object-provider";
import { objectApiKey, publicObjectSettings, updateObjectSettings } from "../src/lib/object-settings";
import { ObjectCreatorError, MAX_IMAGE_BYTES, type ObjectAsset } from "../src/lib/object-types";
import { readProjectSnapshot } from "../src/lib/project";
import { referenceImage as reference, triangleGlb } from "./object-fixture";

async function fixture(t: TestContext) {
  const root = await mkdtemp(path.join(os.tmpdir(), "emergent-object-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  await mkdir(path.join(root, "game", "assets"), { recursive: true });
  const project = { id: "test-project", root: path.join(root, "game"), assetRoots: ["assets"] };
  let submissions = 0;
  const provider: ObjectProvider = {
    upload: async () => "https://fal.media/reference.png",
    submit: async () => { submissions++; return "provider-request-1"; },
    status: async () => "completed",
    result: async () => triangleGlb(),
  };
  const data = path.join(root, "data");
  const jobs = new ObjectJobs(data, async () => provider);
  return { root, project, provider, data, jobs, submissions: () => submissions };
}

test("generation survives restart, duplicate requests submit once, saved bundles reopen with explicit graph edges", async (t) => {
  const f = await fixture(t);
  const id = randomUUID();
  const [a, b] = await Promise.all([f.jobs.create(f.project, id, "Moon Rover", reference), f.jobs.create(f.project, id, "Moon Rover", reference)]);
  assert.equal(a.id, b.id); assert.equal(f.submissions(), 1);
  const restarted = new ObjectJobs(f.data, async () => f.provider);
  assert.equal((await restarted.list(f.project))[0].requestId, "provider-request-1");
  assert.equal((await restarted.refresh(f.project, id)).state, "ready");
  const saved = await restarted.save(f.project, id, "Moon Rover");
  assert.equal(saved.savedPath, `assets/models/${id}/model.glb`);
  assert.equal((await restarted.save(f.project, id, "Ignored duplicate name")).savedPath, saved.savedPath);
  const descriptor = JSON.parse(await readFile(path.join(f.project.root, `assets/models/${id}/object.asset.json`), "utf8")) as ObjectAsset;
  assert.equal(descriptor.jobId, id); assert.equal(descriptor.modelRevision, null); assert.equal(descriptor.settings.texture_size, 1024);
  validateGlb(await readFile(path.join(f.project.root, saved.savedPath!)));
  await writeFile(path.join(f.project.root, "emergent.toml"), 'id = "test-project"\nname = "Test"\nasset_roots = ["assets"]\n');
  await mkdir(path.join(f.project.root, "src"));
  const previous = process.env.EMERGENT_PROJECT_PATH;
  process.env.EMERGENT_PROJECT_PATH = f.project.root;
  t.after(() => { if (previous === undefined) delete process.env.EMERGENT_PROJECT_PATH; else process.env.EMERGENT_PROJECT_PATH = previous; });
  const snapshot = await readProjectSnapshot();
  assert.ok(snapshot.connections.some((edge) => edge.target === descriptor.model && edge.label === "contains model"));
  assert.ok(snapshot.connections.some((edge) => edge.target === descriptor.reference && edge.label === "generated from reference"));
  assert.equal(snapshot.files.find((file) => file.path === descriptor.reference)?.kind, "texture");
  assert.equal(snapshot.files.some((file) => file.path.includes("staging")), false);
  assert.equal((await restarted.list({ ...f.project, id: "different-project" })).length, 0);
});

test("uncertain submission is never repeated; refresh recovers a journaled submission intent", async (t) => {
  const f = await fixture(t);
  f.provider.submit = async () => { throw new Error("network lost after acceptance"); };
  const id = randomUUID();
  const job = await f.jobs.create(f.project, id, "Rover", reference);
  assert.equal(job.state, "uncertain");
  assert.equal((await f.jobs.create(f.project, id, "Rover", reference)).state, "uncertain");
  const directory = path.dirname(await f.jobs.artifact(f.project, id, "reference"));
  await writeFile(path.join(directory, "job.json"), JSON.stringify({ ...job, state: "submitting" }));
  const restarted = new ObjectJobs(f.data, async () => { throw new Error("must not call provider"); });
  assert.equal((await restarted.refresh(f.project, id)).state, "uncertain");
});

test("missing credentials create no job; failed uploads remain reviewable", async (t) => {
  const f = await fixture(t);
  const missing = new ObjectJobs(f.data, async () => { throw new ObjectCreatorError("Key required", 503); });
  await assert.rejects(missing.create(f.project, randomUUID(), "Rover", reference), /Key required/);
  assert.deepEqual(await missing.list(f.project), []);
  f.provider.upload = async () => { throw new ProviderFailure("invalid credentials", false); };
  assert.equal((await f.jobs.create(f.project, randomUUID(), "Rover", reference)).state, "failed");
});

test("transient observation failures resume the same request, terminal errors stop it", async (t) => {
  const f = await fixture(t);
  const job = await f.jobs.create(f.project, randomUUID(), "Rover", reference);
  f.provider.status = async () => { throw new ProviderFailure("Temporary interruption", false); };
  assert.equal((await f.jobs.refresh(f.project, job.id)).state, "queued");
  f.provider.status = async () => "completed";
  assert.equal((await f.jobs.refresh(f.project, job.id)).state, "ready");
  assert.equal(f.submissions(), 1);
  const second = await f.jobs.create(f.project, randomUUID(), "Rover Two", reference);
  f.provider.status = async () => { throw new ProviderFailure("Cannot generate", true); };
  assert.equal((await f.jobs.refresh(f.project, second.id)).state, "failed");
});

test("malformed provider output cannot be saved", async (t) => {
  const f = await fixture(t);
  const job = await f.jobs.create(f.project, randomUUID(), "Rover", reference);
  f.provider.result = async () => Buffer.from("not a model");
  assert.equal((await f.jobs.refresh(f.project, job.id)).state, "failed");
  await assert.rejects(f.jobs.save(f.project, job.id, "Rover"), /valid model/);
  await assert.rejects(f.jobs.artifact(f.project, job.id, "model"), /not ready/);
});

test("duplicate display names save independently and invalid assets never publish", async (t) => {
  const f = await fixture(t);
  const first = await f.jobs.create(f.project, randomUUID(), "Rover", reference);
  await f.jobs.refresh(f.project, first.id);
  const savedFirst = await f.jobs.save(f.project, first.id, "Rover");
  const second = await f.jobs.create(f.project, randomUUID(), "Rover", reference);
  await f.jobs.refresh(f.project, second.id);
  const savedSecond = await f.jobs.save(f.project, second.id, "Rover");
  assert.notEqual(savedFirst.savedPath,savedSecond.savedPath);
  for(const job of [first,second]) {
    const descriptor=JSON.parse(await readFile(path.join(f.project.root,`assets/models/${job.id}/object.asset.json`),"utf8")) as ObjectAsset;
    assert.equal(descriptor.id,job.id); assert.equal(descriptor.name,"Rover");
  }
  const third=await f.jobs.create(f.project,randomUUID(),"Rover",reference);
  await f.jobs.refresh(f.project,third.id);
  const model = await f.jobs.artifact(f.project, third.id, "model");
  await writeFile(model, "broken");
  await assert.rejects(f.jobs.save(f.project, third.id, "Rover"), /valid GLB/);
  assert.deepEqual((await readdir(path.join(f.project.root, "assets/models"))).sort(), [first.id,second.id].sort());
});

test("paths, symlinks, image size and signatures are checked", async (t) => {
  const f = await fixture(t);
  assert.equal(imageExtension(reference), "png");
  assert.throws(() => imageExtension(Buffer.from("<svg/>")), /PNG/);
  assert.throws(() => imageExtension(Buffer.alloc(MAX_IMAGE_BYTES + 1)), /smaller/);
  await writeFile(path.join(f.root, "outside.glb"), triangleGlb());
  await symlink(path.join(f.root, "outside.glb"), path.join(f.project.root, "assets/leak.glb"));
  await assert.rejects(containedFile(path.join(f.project.root, "assets"), "leak.glb", 1024), /outside/);
  await assert.rejects(f.jobs.get(f.project, "../escape"), /Invalid/);
  const job = await f.jobs.create(f.project, randomUUID(), "Rover", reference);
  await f.jobs.refresh(f.project, job.id);
  await assert.rejects(f.jobs.save(f.project, job.id, "../bad"), /object name/);
  await symlink(f.root, path.join(f.project.root, "assets/models"));
  await assert.rejects(f.jobs.save(f.project, job.id, "Rover"), /escapes/);
  await assert.rejects(boundedBody(new Response("12345"), 4), /size limit/);
});

test("GLB validation rejects bad lengths, chunks, and external resources", () => {
  const glb = triangleGlb();
  validateGlb(glb);
  const bad = Buffer.from(glb); bad.writeUInt32LE(999, 8);
  assert.throws(() => validateGlb(bad), /valid GLB/);
  const external = Buffer.from(glb.toString("binary").replace('"buffers":[{"byteLength":36}]', '"buffers":[{"uri":"abc.glb"}]'), "binary");
  assert.equal(external.length, glb.length);
  assert.throws(() => validateGlb(external), /embed/);
});

test("SDK transport guard prevents a second paid POST", async () => {
  let calls = 0;
  const guarded = singleSubmissionFetch(async () => { calls++; return new Response("unavailable", { status: 503 }); });
  await guarded("https://queue.fal.run/fal-ai/trellis", { method: "POST" });
  await assert.rejects(guarded("https://queue.fal.run/fal-ai/trellis", { method: "POST" }), /automatic resubmission disabled/);
  assert.equal(calls, 1);
});

test("provider adapter maps the documented queue/result contract and downloads local GLB bytes", async () => {
  let postCount = 0;
  let submission: Record<string, unknown> | undefined;
  const transport: typeof fetch = async (input, init) => {
    const url = String(input);
    if (url === "https://fal.media/result.glb") return new Response(new Uint8Array(triangleGlb()));
    if (url.includes("/status")) return Response.json({ status: "COMPLETED" });
    if (init?.method?.toUpperCase() === "POST") {
      postCount++;
      submission = JSON.parse(String(init.body)) as Record<string, unknown>;
      return Response.json({ request_id: "request-test", status: "IN_QUEUE" });
    }
    return Response.json({ model_mesh: { url: "https://fal.media/result.glb" } });
  };
  const provider = createTrellisProvider("test-key", transport);
  assert.equal(await provider.submit("https://fal.media/input.png", 12), "request-test");
  assert.equal(submission?.texture_size, 1024);
  assert.equal(typeof submission?.texture_size, "number");
  assert.equal(await provider.status("request-test"), "completed");
  validateGlb(await provider.result("request-test")); assert.equal(postCount, 1);
});

test("settings mask credentials and retain owner-only storage permissions", async (t) => {
  const f = await fixture(t);
  const dataBefore = process.env.EMERGENT_IDE_DATA_DIR, keyBefore = process.env.FAL_KEY;
  process.env.EMERGENT_IDE_DATA_DIR = f.data; delete process.env.FAL_KEY;
  t.after(() => {
    if (dataBefore === undefined) delete process.env.EMERGENT_IDE_DATA_DIR; else process.env.EMERGENT_IDE_DATA_DIR = dataBefore;
    if (keyBefore === undefined) delete process.env.FAL_KEY; else process.env.FAL_KEY = keyBefore;
  });
  assert.equal((await publicObjectSettings()).configured, false);
  const settings = await updateObjectSettings({ apiKey: "test-secret-key-1234" });
  assert.equal(settings.configured, true); assert.equal(settings.keyHint, "…1234");
  assert.equal(JSON.stringify(settings).includes("test-secret"), false);
  assert.equal(await objectApiKey(), "test-secret-key-1234");
  assert.equal((await stat(path.join(f.data, "object-provider.json"))).mode & 0o777, 0o600);
  await updateObjectSettings({ clearApiKey: true }); assert.equal((await publicObjectSettings()).configured, false);
});
