import { randomUUID } from "node:crypto";
import { lstat, mkdir, open, readFile, realpath, unlink } from "node:fs/promises";
import path from "node:path";
import { parse } from "smol-toml";
import { containedFile, missing, within } from "./object-files";
import { applyChanges } from "./statement-files";
import {
  assetContents,
  assetPath,
  assetTemplate,
  sceneCompatible,
} from "@/lib/asset-catalog";
import type { ProjectSnapshot } from "@/lib/project";

export class AssetCreationError extends Error {
  constructor(message: string, readonly status = 400) {
    super(message);
  }
}

export class AssetDeletionError extends Error {
  constructor(message: string, readonly status = 400) {
    super(message);
  }
}

export async function deleteProjectAsset(project:ProjectSnapshot,relative:string):Promise<void> {
  if(!relative||relative.includes("\\")||relative.includes(":")||path.isAbsolute(relative)||relative.split("/").some(part=>!part||part==="."||part===".."||part.startsWith(".")))throw new AssetDeletionError("Invalid project asset path.");
  if(!project.files.some(file=>file.path===relative))throw new AssetDeletionError("Asset does not exist in this project.",404);
  if(!project.assetRoots.some(root=>relative.startsWith(`${root}/`)))throw new AssetDeletionError("Only files under declared project asset roots can be deleted.",403);
  if(relative===project.entryScene)throw new AssetDeletionError("The project's entry scene cannot be deleted.",409);
  const references=project.connections.filter(connection=>connection.target===relative);
  if(references.length)throw new AssetDeletionError(`Asset is still referenced by ${references.map(reference=>reference.source).join(", ")}.`,409);

  const root=await realpath(project.root);
  let current=root;
  for(const segment of relative.split("/")){
    current=path.join(current,segment);
    try{if((await lstat(current)).isSymbolicLink())throw new AssetDeletionError("Cannot delete assets through symbolic links.",403);}
    catch(error){if(missing(error))throw new AssetDeletionError("Asset no longer exists.",404);throw error;}
  }
  const target=await realpath(current);
  if(!within(root,target))throw new AssetDeletionError("Asset path resolves outside the project.",403);
  const details=await lstat(target);
  if(!details.isFile())throw new AssetDeletionError("Only project files can be deleted.",400);
  await unlink(target);
}

export async function createProjectAsset(
  project: ProjectSnapshot,
  templateId: string,
  requestedName: string,
  scenePath?: string,
): Promise<{ path: string; id: string; sceneError?: string }> {
  const template = assetTemplate(templateId);
  if (!template) throw new AssetCreationError("Unknown Emergent asset template.");
  if(template.id==="effects.particle-emitter")throw new AssetCreationError("Use the Particle Generator to create a complete effect and entity asset.");

  const name = requestedName.trim();
  if (!name) throw new AssetCreationError("Asset name is required.");
  if (name.length > 64) throw new AssetCreationError("Asset name must be 64 characters or fewer.");
  if (!/^[\p{L}\p{N}][\p{L}\p{N} _-]*$/u.test(name)) {
    throw new AssetCreationError("Use letters, numbers, spaces, hyphens, or underscores for the asset name.");
  }

  const id = randomUUID();

  const relative = assetPath(template, id);
  const absolute = path.resolve(project.root, relative);
  const allowedRoots = project.assetRoots.map((root) => path.resolve(project.root, root));
  const insideAssetRoot = allowedRoots.some(
    (root) => absolute === root || absolute.startsWith(`${root}${path.sep}`),
  );
  if (!insideAssetRoot) throw new AssetCreationError("The selected template is outside the project's declared asset roots.");

  let current = await realpath(project.root);
  for (const segment of relative.split("/")) {
    current = path.join(current, segment);
    try { if ((await lstat(current)).isSymbolicLink()) throw new AssetCreationError("Cannot create assets through symbolic links."); }
    catch (error) { if (!missing(error)) throw error; }
  }
  await mkdir(path.dirname(absolute), { recursive: true });
  if (!within(await realpath(project.root), await realpath(path.dirname(absolute)))) {
    throw new AssetCreationError("Asset directory resolves outside the project.");
  }
  let file;
  try {
    file = await open(absolute, "wx");
    await file.writeFile(assetContents(template, name, id), "utf8");
  } catch (error) {
    if (isAlreadyExists(error)) {
      throw new AssetCreationError(`An asset already exists at ${relative}.`, 409);
    }
    throw error;
  } finally {
    await file?.close();
  }

  if (scenePath) {
    try {
      if (!sceneCompatible(template)) throw new AssetCreationError("This asset type is not supported by the scene loader.");
      if (!scenePath.endsWith(".scene.toml") || !project.assetRoots.some((root) => scenePath.startsWith(`${root}/`))) throw new AssetCreationError("Choose a scene under a declared asset root.");
      const target = await containedFile(project.root, scenePath, 256 * 1024);
      const before = await readFile(target, "utf8");
      const scene = parse(before);
      if (scene.schema_version !== 1 || !Array.isArray(scene.entities) || scene.entities.length >= 1024) throw new AssetCreationError("Scene must use schema 1 with fewer than 1024 instances.");
      if (scene.entities.some((entity) => !entity || typeof entity !== "object" || Array.isArray(entity) || !("id" in entity) || typeof entity.id !== "string" || !("asset" in entity) || typeof entity.asset !== "string")) throw new AssetCreationError("Scene entries must reference entity assets. Migrate legacy inline entries first.");
      if (scene.entities.some((entity) => (entity as { id: string }).id === id)) throw new AssetCreationError(`Scene already contains instance '${id}'.`);
      // Preserve authored comments; remove only an empty top-level entities list before appending tables.
      const headerEnd = before.search(/^\[/m);
      const header = headerEnd < 0 ? before : before.slice(0, headerEnd);
      const rest = headerEnd < 0 ? "" : before.slice(headerEnd);
      const cameraSelection = template.kind === "camera" && typeof scene.active_camera !== "string"
        ? `active_camera = ${JSON.stringify(id)}\n`
        : "";
      const content = header.replace(/^entities\s*=\s*\[\s*\]\s*$/m, "") + cameraSelection + rest + `\n[[entities]]\nid = ${JSON.stringify(id)}\nasset = ${JSON.stringify(relative)}\n`;
      parse(content);
      if (Buffer.byteLength(content) > 256 * 1024) throw new AssetCreationError("Updated scene exceeds 256 KiB.");
      await applyChanges(project, [{ path: scenePath, before, content }]);
    } catch (error) {
      return { path: relative, id, sceneError: `Asset saved, but not included in scene: ${error instanceof Error ? error.message : "inclusion failed"}` };
    }
  }
  return { path: relative, id };
}

function isAlreadyExists(error: unknown): error is NodeJS.ErrnoException {
  return error instanceof Error && "code" in error && error.code === "EEXIST";
}
