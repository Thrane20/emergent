import "server-only";
import { execFile } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { cp, lstat, mkdir, mkdtemp, readFile, realpath, rename, rm, unlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
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

export async function checkCandidate(project: ProjectSnapshot, changes: FileChange[], context: Pick<CodeContext,"root">, scenes:string[] = []) {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-statement-check-"));
  const stage = path.join(directory, "project");
  try {
    const excluded = new Set([".git", "target", "node_modules", ".data", ".next", ".emergent-object-staging"]);
    await cp(project.root, stage, { recursive: true, filter: async (source) => !excluded.has(path.basename(source)) && !(await lstat(source)).isSymbolicLink() });
    const manifest = parse(await readFile(path.join(stage,"Cargo.toml"), "utf8"));
    if (JSON.stringify(manifest).includes('"workspace":true')) throw new Error("Workspace-inherited Cargo settings are not yet supported by isolated statement checks.");
    // Relocate only dependency paths, never source/bin paths, in the temporary manifest.
    function dependencies(value: unknown) {
      if (!value || typeof value !== "object" || Array.isArray(value)) return;
      for (const [key, child] of Object.entries(value)) {
        if (["dependencies", "dev-dependencies", "build-dependencies"].includes(key) && child && typeof child === "object") {
          for (const dependency of Object.values(child)) if (dependency && typeof dependency === "object" && "path" in dependency && typeof dependency.path === "string") dependency.path = path.resolve(project.root, dependency.path);
        } else dependencies(child);
      }
    }
    dependencies(manifest);
    await writeFile(path.join(stage, "Cargo.toml"), stringify(manifest));
    try { await cp(path.join(context.root, "Cargo.lock"), path.join(stage, "Cargo.lock")); } catch (error) { if (!missing(error)) throw error; }
    for (const file of changes) {
      const target = path.join(stage, file.path);
      await mkdir(path.dirname(target), { recursive: true }); await writeFile(target, file.content);
    }
    try {
      await execute("cargo", ["check", "--offline", "--all-targets", "--manifest-path", path.join(stage, "Cargo.toml"), "--target-dir", path.join(context.root, "target")], { cwd: stage, timeout: 120_000, maxBuffer: 1024 * 1024, shell: false, windowsHide: true });
      if(scenes.length){
        const dependency=(manifest.dependencies as Record<string,{features?:string[]}>|undefined)?.emergent;
        // Keep staged validation from overwriting the live game's named Cargo executable.
        await execute("cargo",["run","--offline","--quiet","--manifest-path",path.join(context.root,"Cargo.toml"),...(dependency?.features?.includes("physics")?["--features","physics"]:[]),"--example","scene_validate","--","--project",stage,"--validate-scenes",...scenes],{cwd:context.root,timeout:120000,maxBuffer:1024*1024,shell:false,windowsHide:true});
      }
    } catch (error) {
      const diagnostics = error && typeof error === "object" && "stderr" in error ? String(error.stderr).slice(-7000) : "Cargo could not run.";
      throw new Error(`Candidate failed ${scenes.length?"validation":"cargo check"}; project files were not changed.\n${diagnostics}`);
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
