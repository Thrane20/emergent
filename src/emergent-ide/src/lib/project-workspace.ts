import { execFile } from "node:child_process";
import { randomUUID } from "node:crypto";
import { access, cp, mkdir, readFile, realpath, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import { parse, stringify } from "smol-toml";
import { inspectionSessions } from "./inspection-session";
import { projectRunManager } from "./project-runner";
import { activeProjectRoot, engineRoot, engineLockfile, pinnedProjectRoot, readWorkspace, writeWorkspace, type RecentProject } from "./workspace";

const execute = promisify(execFile);
const MAX_RECENT = 12;
/** Source of new projects; improvements to new-project defaults belong in this folder. */
const standardTemplate = path.resolve(process.cwd(), "../../templates/standard");
const copyExcluded = new Set(["target", ".git", "node_modules", ".emergent-object-staging"]);

export class ProjectWorkspaceError extends Error {
  constructor(message: string, readonly status = 400) { super(message); }
}

type ProjectSummary = { id: string; name: string; root: string };
const git = (cwd: string, args: string[]) => execute("git", args, { cwd, timeout: 60_000, maxBuffer: 1024 * 1024, shell: false, windowsHide: true });

export async function inspectProject(root: unknown): Promise<ProjectSummary> {
  if (typeof root !== "string" || !path.isAbsolute(root.trim())) throw new ProjectWorkspaceError("Enter the absolute path of a project folder.");
  let resolved: string;
  try { resolved = await realpath(root.trim()); } catch { throw new ProjectWorkspaceError("That folder does not exist.", 404); }
  let manifest: string;
  try { manifest = await readFile(path.join(resolved, "emergent.toml"), "utf8"); } catch { throw new ProjectWorkspaceError("That folder has no emergent.toml, so it is not an Emergent project."); }
  try { await access(path.join(resolved, "Cargo.toml")); } catch { throw new ProjectWorkspaceError("That project has no Cargo.toml game crate."); }
  let document;
  try { document = parse(manifest); } catch { throw new ProjectWorkspaceError("emergent.toml is not valid TOML."); }
  const id = document.id;
  if (typeof id !== "string" || !id) throw new ProjectWorkspaceError("emergent.toml does not declare a project id.");
  return { id, name: typeof document.name === "string" ? document.name : path.basename(resolved), root: resolved };
}

export async function workspaceState() {
  const workspace = await readWorkspace();
  const root = pinnedProjectRoot() ?? workspace.active;
  return { active: root ? await inspectProject(root).catch(() => null) : null, recent: workspace.recent, pinned: !!pinnedProjectRoot(), defaultParent: defaultParent() };
}

export async function openProject(root: unknown): Promise<ProjectSummary> {
  if (pinnedProjectRoot()) throw new ProjectWorkspaceError("This IDE is pinned to one project by EMERGENT_PROJECT_PATH.", 409);
  const project = await inspectProject(root);
  // Sessions belong to the previous project's files; end them before anything reads the new one.
  const run = projectRunManager.status();
  if (run.projectId) projectRunManager.stop(run.projectId);
  await inspectionSessions.stopActive();
  const workspace = await readWorkspace();
  const entry: RecentProject = { ...project, openedAt: new Date().toISOString() };
  await writeWorkspace({ active: project.root, recent: [entry, ...workspace.recent.filter(item => item.root !== project.root)].slice(0, MAX_RECENT) });
  return project;
}

function defaultParent() { return path.resolve(engineRoot, "../emergent-projects"); }

function slugFor(name: unknown) {
  if (typeof name !== "string" || !name.trim() || name.trim().length > 64) throw new ProjectWorkspaceError("Enter a project name of 1–64 characters.");
  const slug = name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 48);
  if (!slug) throw new ProjectWorkspaceError("Use at least one letter or digit in the project name.");
  return { name: name.trim(), slug };
}

async function targetFolder(parent: unknown, slug: string) {
  const base = parent === undefined || parent === "" ? defaultParent() : parent;
  if (typeof base !== "string" || !path.isAbsolute(base)) throw new ProjectWorkspaceError("The parent folder must be an absolute path.");
  const target = path.join(base, slug);
  if (await stat(target).then(() => true, () => false)) throw new ProjectWorkspaceError(`${target} already exists. Choose another name or folder.`, 409);
  await mkdir(base, { recursive: true });
  return target;
}

/** New identity and engine paths for a copied project folder. */
async function rewriteCopy(source: string, target: string, name: string, slug: string) {
  let manifest = await readFile(path.join(target, "emergent.toml"), "utf8");
  const set = (key: string, value: string) => { manifest = manifest.replace(new RegExp(String.raw`^${key}\s*=\s*(?:"(?:[^"\\]|\\.)*"|'[^']*')`, "m"), `${key} = ${JSON.stringify(value)}`); };
  set("id", `project.${slug}-${randomUUID().slice(0, 8)}`);
  set("name", name);
  await writeFile(path.join(target, "emergent.toml"), manifest);
  const cargo = parse(await readFile(path.join(target, "Cargo.toml"), "utf8")) as Record<string, unknown>;
  if (cargo.package && typeof cargo.package === "object") (cargo.package as Record<string, unknown>).name = `emergent-${slug}`;
  // Its own workspace root, so the project builds even inside another Cargo workspace.
  cargo.workspace ??= {};
  // Path dependencies were relative to the source folder; keep them pointing at the same crates.
  for (const table of ["dependencies", "dev-dependencies", "build-dependencies"]) {
    for (const dependency of Object.values((cargo[table] ?? {}) as Record<string, unknown>)) {
      if (dependency && typeof dependency === "object" && typeof (dependency as { path?: unknown }).path === "string") {
        const entry = dependency as { path: string };
        const absolute = path.resolve(source, entry.path), relative = path.relative(target, absolute) || ".";
        // Crates inside the project (plugins) were copied with it; keep pointing at the copy.
        if (!path.isAbsolute(entry.path) && !path.relative(source, absolute).startsWith("..")) continue;
        entry.path = relative.split(path.sep).filter(part => part === "..").length > 3 ? absolute : relative;
      }
    }
  }
  await writeFile(path.join(target, "Cargo.toml"), stringify(cargo) + "\n");
  await writeFile(path.join(target, ".gitignore"), "/target/\n.emergent-object-staging/\n");
  await cp(engineLockfile, path.join(target, "Cargo.lock"), { force: false, errorOnExist: false });
}

async function commitInitial(target: string) {
  // A project created inside another repository is saved by that repository's scoped commits.
  if (await git(target, ["rev-parse", "--show-toplevel"]).then(() => true, () => false)) return undefined;
  try { await git(target, ["init", "-q"]); await git(target, ["add", "-A"]); await git(target, ["commit", "-q", "-m", "Create project"]); return undefined; }
  catch (error) { return `Project created, but the first git commit failed: ${gitMessage(error)}`; }
}

export async function createProject(input: { name?: unknown; parent?: unknown }) {
  const { name, slug } = slugFor(input.name);
  const target = await targetFolder(input.parent, slug);
  await cp(standardTemplate, target, { recursive: true, filter: item => !copyExcluded.has(path.basename(item)) });
  await rewriteCopy(standardTemplate, target, name, slug);
  const warning = await commitInitial(target);
  return { project: await openProject(target), warning };
}

export async function duplicateProject(input: { name?: unknown; parent?: unknown }) {
  const { name, slug } = slugFor(input.name);
  const source = await activeProjectRoot();
  const target = await targetFolder(input.parent, slug);
  await cp(source, target, { recursive: true, filter: item => !copyExcluded.has(path.basename(item)) });
  await rewriteCopy(source, target, name, slug);
  const warning = await commitInitial(target);
  return { project: await openProject(target), warning };
}

function gitMessage(error: unknown) {
  return (error && typeof error === "object" && "stderr" in error ? String(error.stderr).trim() : "") || (error instanceof Error ? error.message : "git failed");
}

/** Commits the project folder only, even when it sits inside a larger repository. */
export async function saveProject(projectId: unknown, message: unknown) {
  const project = await inspectProject(await activeProjectRoot());
  if (projectId !== project.id) throw new ProjectWorkspaceError("The request does not match the open project.", 409);
  if (message !== undefined && (typeof message !== "string" || message.length > 200)) throw new ProjectWorkspaceError("Keep the save message under 200 characters.");
  const summary = (typeof message === "string" && message.trim()) || `Snapshot ${new Date().toISOString().slice(0, 16).replace("T", " ")}`;
  try {
    if (!(await git(project.root, ["rev-parse", "--show-toplevel"]).then(() => true, () => false))) {
      await writeFile(path.join(project.root, ".gitignore"), "/target/\n.emergent-object-staging/\n", { flag: "wx" }).catch(() => undefined);
      await git(project.root, ["init", "-q"]);
    }
    await git(project.root, ["add", "-A", "--", "."]);
    const changed = await git(project.root, ["diff", "--cached", "--quiet", "--", "."]).then(() => false, () => true);
    if (!changed) return { committed: false, message: "Nothing has changed since the last save." };
    await git(project.root, ["commit", "-q", "-m", `${project.name}: ${summary}`, "--", "."]);
    const commit = (await git(project.root, ["rev-parse", "--short", "HEAD"])).stdout.trim();
    return { committed: true, commit, message: `Saved ${commit}` };
  } catch (error) {
    throw new ProjectWorkspaceError(`Save failed: ${gitMessage(error)}`, 500);
  }
}
