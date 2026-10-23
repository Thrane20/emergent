import { mkdir, open } from "node:fs/promises";
import path from "node:path";
import {
  assetContents,
  assetPath,
  assetSlug,
  assetTemplate,
} from "@/lib/asset-catalog";
import type { ProjectSnapshot } from "@/lib/project";

export class AssetCreationError extends Error {
  constructor(message: string, readonly status = 400) {
    super(message);
  }
}

export async function createProjectAsset(
  project: ProjectSnapshot,
  templateId: string,
  requestedName: string,
): Promise<{ path: string; id: string }> {
  const template = assetTemplate(templateId);
  if (!template) throw new AssetCreationError("Unknown Emergent asset template.");

  const name = requestedName.trim();
  if (!name) throw new AssetCreationError("Asset name is required.");
  if (name.length > 64) throw new AssetCreationError("Asset name must be 64 characters or fewer.");
  if (!/^[\p{L}\p{N}][\p{L}\p{N} _-]*$/u.test(name)) {
    throw new AssetCreationError("Use letters, numbers, spaces, hyphens, or underscores for the asset name.");
  }

  const id = assetSlug(name);
  if (!id) throw new AssetCreationError("Asset name must contain at least one ASCII letter or number.");

  const relative = assetPath(template, name);
  const absolute = path.resolve(project.root, relative);
  const allowedRoots = project.assetRoots.map((root) => path.resolve(project.root, root));
  const insideAssetRoot = allowedRoots.some(
    (root) => absolute === root || absolute.startsWith(`${root}${path.sep}`),
  );
  if (!insideAssetRoot) throw new AssetCreationError("The selected template is outside the project's declared asset roots.");

  await mkdir(path.dirname(absolute), { recursive: true });
  let file;
  try {
    file = await open(absolute, "wx");
    await file.writeFile(assetContents(template, name), "utf8");
  } catch (error) {
    if (isAlreadyExists(error)) {
      throw new AssetCreationError(`An asset already exists at ${relative}.`, 409);
    }
    throw error;
  } finally {
    await file?.close();
  }

  return { path: relative, id };
}

function isAlreadyExists(error: unknown): error is NodeJS.ErrnoException {
  return error instanceof Error && "code" in error && error.code === "EEXIST";
}
