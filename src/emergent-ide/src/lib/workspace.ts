import { readFile } from "node:fs/promises";
import path from "node:path";
import { ideDataDirectory, writeJson } from "./object-files";

export type RecentProject = { id: string; name: string; root: string; openedAt: string };
export type Workspace = { active?: string; recent: RecentProject[] };

export const engineRoot = path.resolve(process.cwd(), "../..");
/** Each project builds in its own target dir: binaries embed their manifest dir, so projects sharing a package name must never share one. */
export const projectTargetDir = (root: string) => path.join(root, "target");
/** New projects start from the engine's resolved versions. */
export const engineLockfile = path.join(engineRoot, "Cargo.lock");
const workspaceFile = () => path.join(ideDataDirectory(), "workspace.json");

/** The IDE is pinned to one project when EMERGENT_PROJECT_PATH is set. */
export const pinnedProjectRoot = () => process.env.EMERGENT_PROJECT_PATH?.trim() || undefined;

export async function readWorkspace(): Promise<Workspace> {
  try {
    const value = JSON.parse(await readFile(workspaceFile(), "utf8")) as Partial<Workspace>;
    const recent = Array.isArray(value.recent) ? value.recent.filter((item): item is RecentProject =>
      !!item && typeof item.id === "string" && typeof item.name === "string" && typeof item.root === "string" && path.isAbsolute(item.root) && typeof item.openedAt === "string") : [];
    return { active: typeof value.active === "string" && path.isAbsolute(value.active) ? value.active : undefined, recent };
  } catch {
    // A missing or damaged workspace opens the project chooser.
    return { recent: [] };
  }
}

export const writeWorkspace = (workspace: Workspace) => writeJson(workspaceFile(), workspace);

export class NoActiveProjectError extends Error {
  readonly status = 409;
  constructor() { super("Open or create a project before using project tools."); }
}

export async function activeProjectRoot(): Promise<string> {
  const root = pinnedProjectRoot() ?? (await readWorkspace()).active;
  if (!root) throw new NoActiveProjectError();
  return path.resolve(/* turbopackIgnore: true */ root);
}
