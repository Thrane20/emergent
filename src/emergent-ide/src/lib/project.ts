import { readdir, readFile, stat } from "node:fs/promises";
import path from "node:path";

export type ProjectFileKind =
  | "manifest"
  | "scene"
  | "entity"
  | "data"
  | "audio"
  | "model"
  | "texture"
  | "source"
  | "documentation"
  | "file";

export type ProjectFile = {
  path: string;
  name: string;
  kind: ProjectFileKind;
  bytes: number;
  modifiedAt: string;
  content: string | null;
  authored: boolean;
  generated: boolean;
};

export type ProjectConnectionKind =
  | "entry-scene"
  | "asset-reference"
  | "generated-by"
  | "source-module";

export type ProjectConnection = {
  id: string;
  source: string;
  target: string;
  kind: ProjectConnectionKind;
  label: string;
};

export type ProjectOutput = {
  id: string;
  os: string;
  architecture: string;
  profile: string;
  enabled: boolean;
};

export type ProjectEntity = {
  id: string;
  kind: string;
  sourcePath: string;
  definition: "scene-entity" | "entity-definition";
};

export type ProjectSnapshot = {
  name: string;
  id: string;
  entryScene: string;
  root: string;
  assetRoots: string[];
  files: ProjectFile[];
  connections: ProjectConnection[];
  entities: ProjectEntity[];
  outputs: ProjectOutput[];
};

const defaultProjectRoot = path.resolve(process.cwd(), "../../projects/first_project");
const textExtensions = new Set([".md", ".rs", ".toml", ".txt", ".wgsl", ".json", ".ron"]);

export async function readProjectSnapshot(): Promise<ProjectSnapshot> {
  const root = path.resolve(
    /* turbopackIgnore: true */ process.env.EMERGENT_PROJECT_PATH ?? defaultProjectRoot,
  );
  const manifestPath = path.join(root, "emergent.toml");
  const manifest = await readFile(manifestPath, "utf8");
  const assetRoots = arrayFor(manifest, "asset_roots");
  const generatedRoot = valueFor(manifest, "generated_code") ?? "src/generated";
  const roots = [...assetRoots, "src"];
  const discovered = await Promise.all(
    roots.map((relative) => listFiles(path.join(root, relative), root, generatedRoot)),
  );
  const manifestFile = await projectFile(manifestPath, root, generatedRoot);
  const files = [manifestFile, ...discovered.flat()].sort(fileOrder);
  const entryScene = valueFor(manifest, "entry_scene") ?? "No entry scene declared";

  return {
    name: valueFor(manifest, "name") ?? path.basename(root),
    id: valueFor(manifest, "id") ?? "unidentified-project",
    entryScene,
    root,
    assetRoots,
    files,
    connections: connectionsFor(files, entryScene),
    entities: entitiesFor(files),
    outputs: outputsFor(manifest),
  };
}

async function listFiles(directory: string, root: string, generatedRoot: string): Promise<ProjectFile[]> {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = await Promise.all(
    entries.map(async (entry) => {
      const absolute = path.join(directory, entry.name);
      return entry.isDirectory()
        ? listFiles(absolute, root, generatedRoot)
        : [await projectFile(absolute, root, generatedRoot)];
    }),
  );
  return files.flat();
}

async function projectFile(absolute: string, root: string, generatedRoot: string): Promise<ProjectFile> {
  const details = await stat(absolute);
  const relative = portable(path.relative(root, absolute));
  const extension = path.extname(relative).toLowerCase();
  return {
    path: relative,
    name: path.basename(relative),
    kind: fileKind(relative),
    bytes: details.size,
    modifiedAt: details.mtime.toISOString(),
    content: textExtensions.has(extension) ? await readFile(absolute, "utf8") : null,
    authored: relative === "emergent.toml" || relative.startsWith("assets/"),
    generated: relative === generatedRoot || relative.startsWith(`${generatedRoot}/`),
  };
}

function connectionsFor(files: ProjectFile[], entryScene: string): ProjectConnection[] {
  const connections = new Map<string, ProjectConnection>();
  const byPath = new Map(files.map((file) => [file.path, file]));
  const add = (source: string, target: string, kind: ProjectConnectionKind, label: string) => {
    if (source === target || !byPath.has(source) || !byPath.has(target)) return;
    const id = `${source}|${target}`;
    const existing = connections.get(id);
    if (!existing || connectionPriority(kind) > connectionPriority(existing.kind)) {
      connections.set(id, { id, source, target, kind, label });
    }
  };

  add("emergent.toml", entryScene, "entry-scene", "declares entry scene");

  for (const source of files) {
    if (!source.content) continue;
    if (source.name === "object.asset.json") {
      try {
        const descriptor = JSON.parse(source.content);
        if (descriptor.schema_version === 1 && descriptor.kind === "generated-model") {
          if (typeof descriptor.model === "string") add(source.path, descriptor.model, "asset-reference", "contains model");
          if (typeof descriptor.reference === "string") add(source.path, descriptor.reference, "asset-reference", "generated from reference");
        }
      } catch { /* Malformed descriptors remain visible as source for repair. */ }
    }
    for (const target of files) {
      if (source.path !== target.path && source.content.includes(target.path)) {
        add(source.path, target.path, "asset-reference", "references path");
      }
    }

    const builder = valueFor(source.content, "builder");
    if (builder) {
      const moduleName = builder.split(".").at(-1);
      const target = files.find(
        (candidate) => candidate.generated && candidate.name === `${moduleName}.rs`,
      );
      if (target) add(source.path, target.path, "generated-by", "implemented by builder");
    }

    if (source.kind === "source") {
      for (const target of files.filter((candidate) => candidate.kind === "source")) {
        const stem = target.name.replace(/\.rs$/, "");
        const modulePattern = new RegExp(`(?:mod\\s+${escapeRegExp(stem)}\\b|::${escapeRegExp(stem)}(?:::|\\b))`);
        if (modulePattern.test(source.content)) {
          add(source.path, target.path, "source-module", "uses source module");
        }
      }
    }
  }

  return [...connections.values()].sort((left, right) => left.id.localeCompare(right.id));
}

function valueFor(source: string, key: string): string | undefined {
  return source.match(new RegExp(`^${key}\\s*=\\s*"([^"]+)"`, "m"))?.[1];
}

function arrayFor(source: string, key: string): string[] {
  const value = source.match(new RegExp(`^${key}\\s*=\\s*\\[([^\\]]*)\\]`, "m"))?.[1];
  return value ? [...value.matchAll(/"([^"]+)"/g)].map((match) => match[1]) : [];
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

function entitiesFor(files: ProjectFile[]): ProjectEntity[] {
  const entities: ProjectEntity[] = [];
  for (const file of files) {
    if (!file.content) continue;
    if (file.kind === "entity") {
      const id = valueFor(file.content, "id");
      if (id) {
        entities.push({
          id,
          kind: valueFor(file.content, "kind") ?? "entity-definition",
          sourcePath: file.path,
          definition: "entity-definition",
        });
      }
    }
    if (file.kind === "scene") {
      for (const block of file.content.split("[[entities]]").slice(1)) {
        const id = valueFor(block, "id");
        if (id) {
          entities.push({
            id,
            kind: valueFor(block, "kind") ?? "entity",
            sourcePath: file.path,
            definition: "scene-entity",
          });
        }
      }
    }
  }
  return entities.sort((left, right) => left.id.localeCompare(right.id));
}

function fileKind(relative: string): ProjectFileKind {
  if (relative === "emergent.toml") return "manifest";
  if (relative.endsWith(".md")) return "documentation";
  if (relative.toLowerCase().endsWith(".glb")) return "model";
  if (/\.(png|jpe?g|webp)$/i.test(relative)) return "texture";
  if (relative.startsWith("assets/scenes/")) return "scene";
  if (relative.startsWith("assets/entities/")) return "entity";
  if (relative.startsWith("assets/data/")) return "data";
  if (relative.startsWith("assets/audio/")) return "audio";
  if (relative.startsWith("assets/models/")) return "model";
  if (relative.startsWith("assets/textures/")) return "texture";
  if (relative.endsWith(".rs")) return "source";
  return "file";
}

function fileOrder(left: ProjectFile, right: ProjectFile): number {
  if (left.path === "emergent.toml") return -1;
  if (right.path === "emergent.toml") return 1;
  return left.path.localeCompare(right.path);
}

function portable(value: string): string {
  return value.split(path.sep).join("/");
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function connectionPriority(kind: ProjectConnectionKind): number {
  switch (kind) {
    case "entry-scene": return 4;
    case "generated-by": return 3;
    case "source-module": return 2;
    case "asset-reference": return 1;
  }
}
