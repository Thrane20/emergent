import "server-only";
import { randomUUID } from "node:crypto";
import { readFile } from "node:fs/promises";
import { stringify } from "smol-toml";
import type { ProjectSnapshot } from "./project";
import { foundationRoot } from "./project-code-context";
import { containedFile } from "./object-files";
import { capturePrefab } from "./prefabs";
import { componentRegistry } from "./scene-preview";
import { scenePlacement } from "./scene-placement";
import { applyChanges, checkCandidate } from "./statement-files";
import { journal } from "./project-ops";
import { publishProjectEvent } from "./project-events";

export async function savePrefab(project: ProjectSnapshot, input: { path: string; root: string; name?: unknown; revision: string }) {
  const before = await scenePlacement(project, input.path);
  if (before.revision !== input.revision) throw new Error("Scene changed. Reload before saving a prefab.");
  const source = await readFile(await containedFile(project.root, input.path, 256 * 1024), "utf8");
  const id = randomUUID(), path = `${project.assetRoots[0]}/prefabs/${id}.prefab.toml`;
  const prefab = capturePrefab(source, input.root, input.name ?? before.instances.find(instance => instance.id === input.root)?.name ?? input.root, id, await componentRegistry(project));
  const content = stringify(prefab);
  const changes = [{ path, before: null, content }];
  await checkCandidate(project, changes, { root: await foundationRoot(project) }, [path]);
  if ((await scenePlacement(project, input.path)).revision !== input.revision) throw new Error("Scene changed during validation. Reload before saving.");
  await applyChanges(project, changes);
  const summary = `Saved prefab ${prefab.name}`, changeId = await journal(project, "save_prefab", summary, changes);
  publishProjectEvent({ type: "changed", projectId: project.id, changeId, source: "ide", summary, paths: [path], scenes: [], live: false });
  return { path, name: prefab.name };
}
