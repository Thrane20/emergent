import "server-only";
import { randomUUID } from "node:crypto";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { parse } from "smol-toml";
import type { ProjectSnapshot } from "./project";
import { publishProjectEvent } from "./project-events";
import { validateGlb } from "./object-files";
import { gameTool } from "./scene-preview";
import { ObjectCreatorError } from "./object-types";

/** What the engine sees in a model or motion-capture file (`--inspect-model`). */
export type RigReport = {
  vertices?: number; triangles?: number; skinned?: boolean;
  joints?: { name: string; parent?: string }[];
  humanoid?: Record<string, string>; humanoid_missing?: string[];
  clips: { name: string; duration_seconds: number; channels: number }[];
};

type Scope = Pick<ProjectSnapshot, "id" | "root" | "assetRoots">;
const RIGGABLE = /\.(glb|vrm|bvh)$/i;
const slug = (name: string) => name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40) || "model";

function inRoots(project: Scope, relative: string) {
  return !relative.split("/").includes("..") && project.assetRoots.some(root => relative.startsWith(`${root}/`));
}

/** Runs the engine's model inspector; `tool` is replaceable for tests. */
export async function inspectRig(project: ProjectSnapshot, relative: string, tool: typeof gameTool = gameTool): Promise<RigReport> {
  if (!RIGGABLE.test(relative) || !inRoots(project, relative)) throw new ObjectCreatorError("Choose a .glb, .vrm or .bvh file in this project's assets.");
  let output: string;
  // The engine's loader message says what is wrong with the file; show it, trimmed.
  try { output = await tool(project, ["--inspect-model", relative]); }
  catch (error) { throw new ObjectCreatorError(`The engine could not load ${relative}: ${(error instanceof Error ? error.message : String(error)).split("\n").filter(Boolean).at(-1)?.slice(0, 400) ?? "unknown error"}`, 422); }
  return parse(output) as unknown as RigReport;
}

/**
 * Imports a rigged model (GLB/VRM → assets/models + a model entity) or a BVH take
 * (→ assets/mocap). The engine inspects it before it is kept; a file the engine
 * cannot load is removed again. Returns the asset to select and its report.
 */
export async function importRigged(project: ProjectSnapshot, input: { name: string; extension: string; bytes: Buffer }, tool: typeof gameTool = gameTool) {
  const extension = input.extension.toLowerCase().replace(/^\./, "");
  if (!["glb", "vrm", "bvh"].includes(extension)) throw new ObjectCreatorError("Import a .glb, .vrm or .bvh file.");
  if (extension === "bvh") {
    if (input.bytes.length > 16 * 1024 * 1024 || !input.bytes.subarray(0, 64).toString("utf8").trimStart().startsWith("HIERARCHY")) throw new ObjectCreatorError("This is not a BVH file (it must start with HIERARCHY and be 16 MiB or smaller).");
  } else validateGlb(input.bytes);
  const base = slug(input.name), folder = extension === "bvh" ? "assets/mocap" : "assets/models";
  if (!inRoots(project, `${folder}/x`)) throw new ObjectCreatorError(`This project's asset roots do not include ${folder}.`);
  const file = `${folder}/${base}.${extension}`, entity = `assets/entities/${base}.entity.toml`;
  const written: string[] = [];
  try {
    await mkdir(path.join(project.root, folder), { recursive: true });
    await writeFile(path.join(project.root, file), input.bytes, { flag: "wx" });
    written.push(file);
    const report = await inspectRig(project, file, tool);
    if (extension !== "bvh") {
      const title = input.name.replace(/[\u0000-\u001f"\\]/g, "").trim().slice(0, 64) || base;
      await mkdir(path.join(project.root, "assets/entities"), { recursive: true });
      await writeFile(path.join(project.root, entity), `schema_version = 1\nid = "${base}"\nname = "${title}"\nkind = "model"\nmodel = "${file}"\n`, { flag: "wx" });
      written.push(entity);
    }
    publishProjectEvent({ type: "changed", projectId: project.id, changeId: randomUUID(), source: "ide", summary: `imported ${file}`, paths: written, scenes: [], live: false });
    return { path: written.at(-1)!, file, report };
  } catch (error) {
    await Promise.all(written.map(item => rm(path.join(project.root, item), { force: true })));
    if (error instanceof Error && /EEXIST/.test(error.message)) throw new ObjectCreatorError(`${extension === "bvh" ? file : `${file} or ${entity}`} already exists. Choose another name.`, 409);
    throw error;
  }
}

/** Copies a ready auto-rig result into the project as a new rigged model. */
export async function saveRigResult(project: ProjectSnapshot, artifact: string, name: string, tool: typeof gameTool = gameTool) {
  return importRigged(project, { name, extension: "glb", bytes: await readFile(artifact) }, tool);
}
