import { readdir, readFile, stat } from "node:fs/promises";
import path from "node:path";
import { parse } from "smol-toml";
import { readProjectPlugins } from "./project-plugins";
import { activeProjectRoot } from "./workspace";

export type ProjectFileKind =
  | "tween"
  | "filter"
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
  displayName?: string;
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

const textExtensions = new Set([".md", ".rs", ".toml", ".txt", ".wgsl", ".json", ".ron"]);

export async function readProjectBinding():Promise<Pick<ProjectSnapshot,"id"|"root">> {
  const root=await activeProjectRoot();
  const manifest=await readFile(path.join(root,"emergent.toml"),"utf8");
  return {root,id:valueFor(manifest,"id")??"unidentified-project"};
}

export async function readProjectSnapshot(): Promise<ProjectSnapshot> {
  const root = await activeProjectRoot();
  const manifestPath = path.join(root, "emergent.toml");
  const manifest = await readFile(manifestPath, "utf8");
  const plugins = (await readProjectPlugins(root)).filter(plugin => !plugin.error);
  const projectRoots = arrayFor(manifest, "asset_roots");
  const pluginRoots = plugins.flatMap(plugin => plugin.assetRoots.map(assetRoot => `${plugin.path}/${assetRoot}`));
  const assetRoots = [...projectRoots, ...pluginRoots];
  const generatedRoot = valueFor(manifest, "generated_code") ?? "src/generated";
  const pluginSources = plugins.filter(plugin => plugin.crate).map(plugin => `${plugin.path}/src`);
  const roots = [...projectRoots, "src", ...pluginRoots, ...pluginSources];
  const discovered = await Promise.all(
    // A missing plugin folder is reported on the Plugins page, not by failing the whole project.
    roots.map((relative, index) => {
      const listed = listFiles(path.join(root, relative), root, generatedRoot);
      return index <= projectRoots.length ? listed : listed.catch(() => []);
    }),
  );
  const manifestFile = await projectFile(manifestPath, root, generatedRoot);
  const files = [manifestFile, ...discovered.flat()].sort(fileOrder);
  for(const descriptor of files.filter(file=>file.name==="texture.asset.json"&&file.content)){
    try{const asset=JSON.parse(descriptor.content!);if(asset.schema_version===1&&asset.kind==="generated-texture"&&typeof asset.name==="string"){const image=files.find(file=>file.path===asset.image);if(image)image.displayName=asset.name;}}catch{/* Invalid sidecars remain visible for repair. */}
  }
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
  const content = textExtensions.has(extension) ? await readFile(absolute, "utf8") : null;
  let displayName = relative.endsWith(".toml") && content ? valueFor(content.split(/^\[/m)[0], "name") : undefined;
  if (path.basename(relative) === "object.asset.json" && content) {
    try { const descriptor = JSON.parse(content); if (typeof descriptor.name === "string") displayName = descriptor.name; }
    catch { /* Invalid descriptors remain available in source view. */ }
  }
  return {
    path: relative,
    name: path.basename(relative),
    displayName,
    kind: fileKind(relative),
    bytes: details.size,
    modifiedAt: details.mtime.toISOString(),
    content,
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
  try { const value = parse(source)[key]; return typeof value === "string" ? value : undefined; }
  catch { return undefined; }
}

function arrayFor(source: string, key: string): string[] {
  try { const value = parse(source)[key]; return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : []; }
  catch { return []; }
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
  if (relative.endsWith(".tween.toml")) return "tween";
  if (relative === "emergent.toml") return "manifest";
  if (relative.endsWith(".noise.toml")) return "filter";
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
