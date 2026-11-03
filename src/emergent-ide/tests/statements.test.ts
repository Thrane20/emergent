import assert from "node:assert/strict";
import test, { type TestContext } from "node:test";
import { mkdtemp, mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import type { ProjectFile, ProjectSnapshot } from "../src/lib/project";
import { addStatement, saveStatementEvaluation, saveStatementImplementation, statementsFor } from "../src/lib/statements";
import { implementationPrompt, parseImplementation } from "../src/lib/statement-proposal";
import { applyChanges, checkCandidate, prepareChanges } from "../src/lib/statement-files";
import { projectCodeContext } from "../src/lib/project-code-context";

const before = "fn main() {}\n";
const source: ProjectFile = { path: "src/main.rs", name: "main.rs", content: before, kind: "source", bytes: before.length, modifiedAt: "", authored: false, generated: false };
async function fixture(t: TestContext) {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-statement-test-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const root = path.join(directory, "game");
  await mkdir(path.join(root,"src"), { recursive: true }); await mkdir(path.join(root,"assets"));
  await writeFile(path.join(root, "src/main.rs"), before);
  await writeFile(path.join(root,"Cargo.toml"), '[package]\nname = "statement-check-fixture"\nversion = "0.1.0"\nedition = "2021"\n');
  const previousData = process.env.EMERGENT_IDE_DATA_DIR; process.env.EMERGENT_IDE_DATA_DIR = path.join(directory,"data");
  t.after(() => { if (previousData === undefined) delete process.env.EMERGENT_IDE_DATA_DIR; else process.env.EMERGENT_IDE_DATA_DIR = previousData; });
  const project: ProjectSnapshot = { root, id: "test", name: "Test", assetRoots: ["assets"], files: [source], entryScene: "assets/main.scene.toml", connections: [], entities: [], outputs: [] };
  const context = { root: directory, files: new Map([[source.path,before], ["foundation/src/world.rs", "pub fn set_local_transform"]]), search: ["foundation/src/world.rs:1: pub fn set_local_transform"] };
  const proposal = { summary: "Reuse transforms", reuse: [{ path:"foundation/src/world.rs", symbol:"World::set_local_transform", reason:"Use the canonical transform boundary" }], files: [{ path:"src/main.rs",content:"fn main() { let _value = 1; }\n" }] };
  return { project, context, proposal };
}

test("saved statements remain unchecked until actual evidence; implementation resets stale checks and records reuse", async (t) => {
  const { project } = await fixture(t);
  const [statement] = await addStatement(project,"file:src/main.rs","Spin slowly");
  assert.equal(statement.implemented,null);
  await saveStatementEvaluation(project,statement.subject,statement.id,{ implemented:false, confidence:"high", evidence:"No rotation exists" });
  await saveStatementImplementation(project,statement.subject,statement.id,{ summary:"Added rotation",reused:["World::set_local_transform"],files:["src/main.rs"],validatedAt:new Date().toISOString() });
  const [saved] = await statementsFor(project,statement.subject);
  assert.equal(saved.implemented,null); assert.deepEqual(saved.implementation?.reused,["World::set_local_transform"]);
});

test("generation context is reuse-first and rejects invented citations or duplicate files", async (t) => {
  const { project,context,proposal } = await fixture(t);
  const [statement] = await addStatement(project,"file:src/main.rs","Spin slowly on Z");
  const prompt = implementationPrompt(project,statement,context);
  assert.ok(prompt.includes("SEARCH RESULTS")); assert.ok(prompt.includes("Reuse matching behaviours")); assert.ok(prompt.includes("15 degrees per second"));
  assert.deepEqual(parseImplementation(JSON.stringify(proposal),context),proposal);
  assert.throws(() => parseImplementation(JSON.stringify({ ...proposal,reuse:[{...proposal.reuse[0],symbol:"InventedSpin"}] }),context),/reuse evidence/);
  assert.throws(() => parseImplementation(JSON.stringify({ ...proposal,files:[...proposal.files,...proposal.files] }),context),/duplicate/);
});

test("reuse search discovers an existing project spin helper and the linked foundation contracts", async (t) => {
  const { project } = await fixture(t);
  const foundation = path.resolve(process.cwd(), "../..");
  await writeFile(path.join(project.root,"Cargo.toml"), `[package]\nname = "context-test"\nversion = "0.1.0"\n[dependencies]\nemergent = { path = ${JSON.stringify(foundation)} }\n`);
  project.files.push({ ...source, path:"src/spin.rs", name:"spin.rs", content:"pub fn rotate_z() {}" });
  const [statement] = await addStatement(project,"file:src/main.rs","Spin the cube slowly");
  const context = await projectCodeContext(project,statement);
  assert.ok(context.search.some((hit) => hit.includes("src/spin.rs") && hit.includes("rotate_z")));
  assert.ok(context.files.get("foundation/src/world.rs")?.includes("set_local_transform"));
  assert.ok(context.files.get("foundation/src/engine.rs")?.includes("fixed_delta"));
  assert.ok(context.files.get("foundation/src/ui.rs")?.includes("widget_catalogue"));
});

test("candidate compilation happens away from live source, and compilation failure changes nothing", async (t) => {
  const { project,context,proposal } = await fixture(t);
  const changes = await prepareChanges(project,proposal,context);
  await checkCandidate(project,changes,context);
  assert.equal(await readFile(path.join(project.root,"src/main.rs"),"utf8"),before);
  await assert.rejects(checkCandidate(project,[{ ...changes[0],content:"invalid Rust!" }],context),/project files were not changed/);
  assert.equal(await readFile(path.join(project.root,"src/main.rs"),"utf8"),before);
  await applyChanges(project,changes);
  assert.equal(await readFile(path.join(project.root,"src/main.rs"),"utf8"),proposal.files[0].content);
});

test("file scope, symlinks and concurrent edits prevent unrelated or lost changes", async (t) => {
  const { project,context,proposal } = await fixture(t);
  for (const target of ["../outside.rs", "src/../../outside.rs", "Cargo.toml", ".env", "assets/private.json"]) await assert.rejects(prepareChanges(project,{...proposal,files:[{path:target,content:"anything"}]},context),/cannot modify/);
  await assert.rejects(prepareChanges(project,{...proposal,files:[{path:"assets/bad.entity.toml",content:"not valid TOML"}]},context));
  await symlink(os.tmpdir(),path.join(project.root,"src/link"));
  await assert.rejects(prepareChanges(project,{...proposal,files:[{path:"src/link/escape.rs",content:"anything"}]},context),/symlink/);
  const changes = await prepareChanges(project,proposal,context);
  await writeFile(path.join(project.root,"src/main.rs"),"user changed this");
  await assert.rejects(applyChanges(project,changes),/changed during generation/);
  assert.equal(await readFile(path.join(project.root,"src/main.rs"),"utf8"),"user changed this");
});

test("publication failure rolls back only its own writes", async (t) => {
  const { project,context,proposal } = await fixture(t);
  const changes = await prepareChanges(project,proposal,context);
  await assert.rejects(applyChanges(project,[...changes,...changes]),/changed during publication/);
  assert.equal(await readFile(path.join(project.root,"src/main.rs"),"utf8"),before);
});
