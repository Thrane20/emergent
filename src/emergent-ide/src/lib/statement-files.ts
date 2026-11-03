import "server-only";
import { execFile } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { cp, lstat, mkdir, mkdtemp, readFile, realpath, rename, rm, unlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { readProjectPlugins } from "./project-plugins";
import { promisify } from "node:util";
import { parse, stringify } from "smol-toml";
import { ideDataDirectory, missing, within, writeJson } from "./object-files";
import type { ProjectSnapshot } from "./project";
import type { CodeContext } from "./project-code-context";
import type { StatementProposal } from "./statement-proposal";

const execute = promisify(execFile);
export type FileChange = { path: string; content: string; before: string | null };
const hash = (value: string) => createHash("sha256").update(value).digest("hex");

async function targetPath(project: Pick<ProjectSnapshot, "root" | "assetRoots">, relative: string) {
  const segments = relative.split("/");
  const allowed = /^(src|tests)\/.+\.rs$/.test(relative) || relative.endsWith(".toml") && project.assetRoots.some((root) => relative.startsWith(`${root}/`));
  if (!allowed || segments.some((part) => !part || part === "." || part === ".." || part.startsWith(".")) || relative.includes("\\") || relative.includes(":")) throw new Error(`AI cannot modify ${relative}. Only project Rust and TOML assets are allowed.`);
  const root = await realpath(project.root);
  let current = root;
  for (const segment of segments) {
    current = path.join(current, segment);
    try { if ((await lstat(current)).isSymbolicLink()) throw new Error(`AI cannot write through symlink ${relative}.`); }
    catch (error) { if (!missing(error)) throw error; }
  }
  if (!within(root, current)) throw new Error("AI file path escapes the project.");
  return current;
}
async function textOrMissing(target: string) {
  try { return await readFile(target, "utf8"); } catch (error) { if (missing(error)) return null; throw error; }
}

export async function prepareChanges(project: ProjectSnapshot, proposal: StatementProposal, context: CodeContext) {
  const changes: FileChange[] = [];
  for (const file of proposal.files) {
    const target = await targetPath(project, file.path);
    if (file.path.endsWith(".toml")) parse(file.content);
    const before = await textOrMissing(target);
    if (before !== null && (!context.files.has(file.path) || hash(before) !== hash(context.files.get(file.path)!))) throw new Error(`${file.path} changed or was not inspected. Retry implementation with current source.`);
    if (before !== file.content) changes.push({ ...file, before });
  }
  return changes;
}

/** `binaries` are written into the stage only, so native validation sees them; callers publish them. */
export async function checkCandidate(project: ProjectSnapshot, changes: FileChange[], context: Pick<CodeContext,"root">, scenes:string[] = [], binaries: { path: string; bytes: Buffer }[] = []) {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-statement-check-"));
  const stage = path.join(directory, "project");
  try {
    const excluded = new Set([".git", "target", "node_modules", ".data", ".next", ".emergent-object-staging"]);
    await cp(project.root, stage, { recursive: true, filter: async (source) => !excluded.has(path.basename(source)) && !(await lstat(source)).isSymbolicLink() });
    const manifest = parse(await readFile(path.join(stage,"Cargo.toml"), "utf8"));
    if (JSON.stringify(manifest).includes('"workspace":true')) throw new Error("Workspace-inherited Cargo settings are not yet supported by isolated statement checks.");
    // Relocate only dependency paths that leave the project, never source/bin paths; crates
    // inside it (plugins) were staged with it, so candidate edits to them are what gets checked.
    function dependencies(value: unknown, directory: string) {
      if (!value || typeof value !== "object" || Array.isArray(value)) return;
      for (const [key, child] of Object.entries(value)) {
        if (["dependencies", "dev-dependencies", "build-dependencies"].includes(key) && child && typeof child === "object") {
          for (const dependency of Object.values(child)) if (dependency && typeof dependency === "object" && "path" in dependency && typeof dependency.path === "string") {
            const absolute = path.resolve(directory, dependency.path);
            if (!within(project.root, absolute)) dependency.path = absolute;
          }
        } else dependencies(child, directory);
      }
    }
    dependencies(manifest, project.root);
    for (const plugin of (await readProjectPlugins(project.root)).filter(plugin => plugin.crate && !plugin.error)) {
      const staged = path.join(stage, plugin.path, "Cargo.toml");
      const pluginManifest = parse(await readFile(staged, "utf8"));
      dependencies(pluginManifest, path.join(project.root, plugin.path));
      await writeFile(staged, stringify(pluginManifest));
    }
    // Scene validation runs the staged game so project components resolve; a distinct binary name keeps the live game's executable untouched.
    const candidateBin = `${String((manifest.package as Record<string, unknown> | undefined)?.name ?? "game")}-candidate`;
    const brains = changes.filter(file => file.path.endsWith(".brain.toml")).map(file => file.path);
    const profiles = changes.filter(file => file.path.endsWith(".locomotion.toml")).map(file => file.path);
    const uis = changes.filter(file => file.path.endsWith(".ui.toml")).map(file => file.path);
    if (scenes.length || brains.length || profiles.length || uis.length) {
      if (manifest.bin !== undefined) throw new Error("Explicit [[bin]] targets are not yet supported by isolated scene validation.");
      manifest.bin = [{ name: candidateBin, path: "src/main.rs" }];
    }
    await writeFile(path.join(stage, "Cargo.toml"), stringify(manifest));
    try { await cp(path.join(context.root, "Cargo.lock"), path.join(stage, "Cargo.lock")); } catch (error) { if (!missing(error)) throw error; }
    for (const file of [...changes.map(change => ({ path: change.path, bytes: change.content })), ...binaries]) {
      const target = path.join(stage, file.path);
      if (!within(stage, target)) throw new Error(`${file.path} escapes the project.`);
      await mkdir(path.dirname(target), { recursive: true }); await writeFile(target, file.bytes);
    }
    try {
      await execute("cargo", ["check", "--offline", "--all-targets", "--manifest-path", path.join(stage, "Cargo.toml"), "--target-dir", path.join(context.root, "target")], { cwd: stage, timeout: 120_000, maxBuffer: 1024 * 1024, shell: false, windowsHide: true });
      if(scenes.length)await execute("cargo",["run","--offline","--quiet","--manifest-path",path.join(stage,"Cargo.toml"),"--target-dir",path.join(context.root,"target"),"--bin",candidateBin,"--","--validate-scenes",...scenes],{cwd:stage,timeout:120000,maxBuffer:1024*1024,shell:false,windowsHide:true});
      for(const profile of profiles)await execute("cargo",["run","--offline","--quiet","--manifest-path",path.join(stage,"Cargo.toml"),"--target-dir",path.join(context.root,"target"),"--bin",candidateBin,"--","--validate-locomotion",profile],{cwd:stage,timeout:120000,maxBuffer:1024*1024,shell:false,windowsHide:true});
      for(const brain of brains)await execute("cargo",["run","--offline","--quiet","--manifest-path",path.join(stage,"Cargo.toml"),"--target-dir",path.join(context.root,"target"),"--bin",candidateBin,"--","--validate-brain",brain],{cwd:stage,timeout:120000,maxBuffer:1024*1024,shell:false,windowsHide:true});
      for(const ui of uis)await execute("cargo",["run","--offline","--quiet","--manifest-path",path.join(stage,"Cargo.toml"),"--target-dir",path.join(context.root,"target"),"--bin",candidateBin,"--","--validate-ui",ui],{cwd:stage,timeout:120000,maxBuffer:1024*1024,shell:false,windowsHide:true});
    } catch (error) {
      const diagnostics = error && typeof error === "object" && "stderr" in error ? String(error.stderr).slice(-7000) : "Cargo could not run.";
      throw new Error(`Candidate failed ${scenes.length||brains.length||profiles.length||uis.length?"validation":"cargo check"}; project files were not changed.\n${diagnostics}`);
    }
  } finally { await rm(directory, { recursive: true, force: true }); }
}

export async function applyChanges(project: ProjectSnapshot, changes: FileChange[]) {
  // Check the entire batch before publishing any file.
  for (const file of changes) if (await textOrMissing(await targetPath(project, file.path)) !== file.before) throw new Error(`${file.path} changed during generation. No files were applied.`);
  const journal = path.join(ideDataDirectory(), "statement-implementations", randomUUID());
  await mkdir(journal, { recursive: true, mode: 0o700 });
  await writeJson(path.join(journal, "changes.json"), { projectId: project.id, root: project.root, state: "prepared", changes });
  const written: FileChange[] = [];
  try {
    for (const file of changes) {
      const target = await targetPath(project, file.path);
      if (await textOrMissing(target) !== file.before) throw new Error(`${file.path} changed during publication.`);
      await mkdir(path.dirname(target), { recursive: true });
      if (file.before === null) await writeFile(target, file.content, { flag: "wx" });
      else {
        const temporary = `${target}.${randomUUID()}.tmp`;
        await writeFile(temporary, file.content);
        await rename(temporary, target);
      }
      written.push(file);
    }
    await writeJson(path.join(journal, "result.json"), { state: "applied", checkedAt: new Date().toISOString() });
  } catch (error) {
    for (const file of written.reverse()) {
      const target = await targetPath(project, file.path);
      if (await textOrMissing(target) !== file.content) continue;
      if (file.before === null) await unlink(target); else await writeFile(target, file.before);
    }
    throw error;
  }
}
