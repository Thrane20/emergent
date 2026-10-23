import { readdir, readFile, stat } from "node:fs/promises";
import path from "node:path";

export type AssetKind = "scene" | "entity" | "data" | "audio" | "model" | "texture" | "file";

export type ProjectAsset = {
  path: string;
  name: string;
  kind: AssetKind;
  bytes: number;
};

export type ProjectOutput = {
  id: string;
  os: string;
  architecture: string;
  profile: string;
  enabled: boolean;
};

export type ProjectSnapshot = {
  name: string;
  id: string;
  entryScene: string;
  root: string;
  assets: ProjectAsset[];
  outputs: ProjectOutput[];
};

const defaultProjectRoot = path.resolve(process.cwd(), "../../projects/first_project");

export async function readProjectSnapshot(): Promise<ProjectSnapshot> {
  const root = path.resolve(
    /* turbopackIgnore: true */ process.env.EMERGENT_PROJECT_PATH ?? defaultProjectRoot,
  );
  const manifest = await readFile(path.join(root, "emergent.toml"), "utf8");
  const assets = await listAssets(path.join(root, "assets"), root);

  return {
    name: valueFor(manifest, "name") ?? path.basename(root),
    id: valueFor(manifest, "id") ?? "unidentified-project",
    entryScene: valueFor(manifest, "entry_scene") ?? "No entry scene declared",
    root,
    assets,
    outputs: outputsFor(manifest),
  };
}

async function listAssets(directory: string, root: string): Promise<ProjectAsset[]> {
  const entries = await readdir(directory, { withFileTypes: true });
  const assets = await Promise.all(
    entries.map(async (entry) => {
      const absolute = path.join(directory, entry.name);
      if (entry.isDirectory()) return listAssets(absolute, root);
      const details = await stat(absolute);
      const relative = path.relative(root, absolute).split(path.sep).join("/");
      return [{ path: relative, name: entry.name, kind: assetKind(relative), bytes: details.size }];
    }),
  );
  return assets.flat().sort((left, right) => left.path.localeCompare(right.path));
}

function valueFor(manifest: string, key: string): string | undefined {
  return manifest.match(new RegExp(`^${key}\\s*=\\s*"([^"]+)"`, "m"))?.[1];
}

function outputsFor(manifest: string): ProjectOutput[] {
  return manifest.split("[[outputs]]").slice(1).map((block) => ({
    id: valueFor(block, "id") ?? "unnamed-output",
    os: valueFor(block, "os") ?? "unknown",
    architecture: valueFor(block, "architecture") ?? "unknown",
    profile: valueFor(block, "profile") ?? "development",
    enabled: /enabled\s*=\s*true/.test(block),
  }));
}

function assetKind(relativePath: string): AssetKind {
  if (relativePath.startsWith("assets/scenes/")) return "scene";
  if (relativePath.startsWith("assets/entities/")) return "entity";
  if (relativePath.startsWith("assets/data/")) return "data";
  if (relativePath.startsWith("assets/audio/")) return "audio";
  if (relativePath.startsWith("assets/models/")) return "model";
  if (relativePath.startsWith("assets/textures/")) return "texture";
  return "file";
}
