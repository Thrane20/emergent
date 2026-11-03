import type { ScenePlacement } from "./scene-placement";

export const sceneInstanceMime = "application/x-emergent-instance";
export const prefabMime = "application/x-emergent-prefab";

function dragData(source: string, projectId: string, placement: Pick<ScenePlacement, "path" | "revision">) {
  const value = JSON.parse(source);
  if (!value || value.projectId !== projectId || value.scene !== placement.path || value.revision !== placement.revision) throw new Error("Project or scene changed since dragging began. Try again.");
  return value as Record<string, unknown>;
}
export function sceneInstanceDrop(source: string, projectId: string, placement: ScenePlacement): string {
  const data = dragData(source, projectId, placement);
  if (typeof data.id !== "string" || !placement.instances.some(instance => instance.id === data.id)) throw new Error("The dragged scene object no longer exists.");
  return data.id;
}
export function prefabDrop(source: string, projectId: string, placement: Pick<ScenePlacement, "path" | "revision">): string {
  const data = dragData(source, projectId, placement);
  if (typeof data.asset !== "string" || !data.asset.endsWith(".prefab.toml")) throw new Error("Drag a prefab from this project's library.");
  return data.asset;
}
