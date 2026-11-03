import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdir, mkdtemp, readFile, realpath, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import { parse } from "smol-toml";
import { readProjectSnapshot } from "../src/lib/project";
import { createProject, duplicateProject, openProject, saveProject, workspaceState } from "../src/lib/project-workspace";
import { engineRoot, engineLockfile } from "../src/lib/workspace";

const git = (cwd: string, ...args: string[]) => promisify(execFile)("git", args, { cwd }).then(result => result.stdout.trim());

test("projects are created, switched, duplicated and saved without touching other files", async t => {
  const directory = await realpath(await mkdtemp(path.join(os.tmpdir(), "emergent-projects-")));
  const previous = { root: process.env.EMERGENT_PROJECT_PATH, data: process.env.EMERGENT_IDE_DATA_DIR };
  delete process.env.EMERGENT_PROJECT_PATH;
  process.env.EMERGENT_IDE_DATA_DIR = path.join(directory, "data");
  t.after(async () => {
    for (const [key, value] of [["EMERGENT_PROJECT_PATH", previous.root], ["EMERGENT_IDE_DATA_DIR", previous.data]] as const) {
      if (value === undefined) delete process.env[key]; else process.env[key] = value;
    }
    await rm(directory, { recursive: true, force: true, maxRetries: 5 });
  });
  assert.equal((await workspaceState()).active, null, "no project is selected implicitly");
  await assert.rejects(readProjectSnapshot(), /Open or create a project/);
  await assert.rejects(duplicateProject({ name: "No source", parent: directory }), /Open or create a project/);

  const parent = path.join(directory, "projects");
  const created = await createProject({ name: "Forest Game", parent });
  assert.equal(created.warning, undefined);
  const root = created.project.root;
  assert.equal(root, path.join(parent, "forest-game"));
  assert.match(created.project.id, /^project\.forest-game-[0-9a-f]{8}$/);
  const snapshot = await readProjectSnapshot();
  assert.equal(snapshot.id, created.project.id, "readProjectSnapshot follows the active project");
  assert.equal(snapshot.name, "Forest Game");
  assert.equal(snapshot.entryScene, "assets/scenes/main.scene.toml");
  assert.deepEqual(snapshot.files.filter(file => file.kind === "scene").map(file => file.path), ["assets/scenes/first-person.scene.toml", "assets/scenes/main.scene.toml", "assets/scenes/shader-showcase.scene.toml"]);
  const cargo = parse(await readFile(path.join(root, "Cargo.toml"), "utf8")) as { package: { name: string }; dependencies: { emergent: { path: string } }; workspace?: object };
  assert.equal(cargo.package.name, "emergent-forest-game");
  assert.equal(await readFile(path.join(root, "Cargo.lock"), "utf8"), await readFile(engineLockfile, "utf8"), "starts from the engine's resolved versions");
  assert.equal(Object.keys(cargo.workspace ?? { missing: true }).length, 0, "its own Cargo workspace root");
  assert.equal(await readFile(path.join(root, "src/main.rs"), "utf8"), await readFile(path.join(engineRoot, "templates/standard/src/main.rs"), "utf8"), "new projects get current template code");
  // Only the plugin registration list (empty until a plugin is added) is generated up front.
  assert.ok(!snapshot.files.some(file => file.path.startsWith("src/generated/") && file.path.endsWith(".rs") && !["src/generated/mod.rs", "src/generated/plugins.rs"].includes(file.path)), "no generated game code");
  assert.equal(path.resolve(root, cargo.dependencies.emergent.path), await realpath(engineRoot), "engine path resolves from the new folder");
  assert.equal(await git(root, "rev-list", "--count", "HEAD"), "1", "own repository with a first commit");

  const manifestPath = path.join(root, "emergent.toml");
  await writeFile(manifestPath, (await readFile(manifestPath, "utf8")).replaceAll('"', "'"));
  assert.equal((await readProjectSnapshot()).id, created.project.id, "literal-quoted manifest IDs and roots are supported");
  assert.ok((await readProjectSnapshot()).files.some(file => file.kind === "scene"));

  await mkdir(path.join(root, "assets/data"), { recursive: true });
  await writeFile(path.join(root, "assets/data/note.txt"), "changed\n");
  const saved = await saveProject(created.project.id, "Add note");
  assert.equal(saved.committed, true);
  assert.match(await git(root, "log", "-1", "--format=%s"), /^Forest Game: Add note$/);
  assert.equal((await saveProject(created.project.id, undefined)).committed, false, "nothing to save");
  await assert.rejects(saveProject("another-project", undefined), /does not match the open project/);

  const copy = await duplicateProject({ name: "Forest Copy", parent });
  assert.notEqual(copy.project.id, created.project.id);
  assert.equal((await readProjectSnapshot()).id, copy.project.id);
  assert.equal(await readFile(path.join(copy.project.root, "assets/data/note.txt"), "utf8"), "changed\n");

  await openProject(root);
  assert.equal((await readProjectSnapshot()).id, created.project.id);
  assert.deepEqual((await workspaceState()).recent.map(item => item.name), ["Forest Game", "Forest Copy"]);
  await assert.rejects(openProject(path.join(directory, "data")), /no emergent.toml/);

  // Inside a larger repository, Save commits the project folder only.
  const outer = path.join(directory, "outer");
  await mkdir(outer);
  await git(outer, "init", "-q");
  await writeFile(path.join(outer, "unrelated.txt"), "keep me out\n");
  const nested = await createProject({ name: "Nested", parent: outer });
  assert.equal(await git(outer, "rev-list", "--all", "--count"), "0", "no nested repository or commit on create");
  await saveProject(nested.project.id, "First save");
  const committed = (await git(outer, "show", "--name-only", "--format=", "HEAD")).split("\n");
  assert.ok(committed.length > 0 && committed.every(file => file.startsWith("nested/")), committed.join(", "));
  assert.equal(await git(outer, "status", "--porcelain", "--", "unrelated.txt"), "?? unrelated.txt");

  process.env.EMERGENT_PROJECT_PATH = root;
  await assert.rejects(openProject(root), /pinned/);
});
