import type { ProjectSnapshot } from "./project";

export type ScenePreview = { path: string; name: string; frame: string };

export function projectScenes(project: Pick<ProjectSnapshot, "files">) {
  return project.files.filter((file) => file.path.endsWith(".scene.toml")).map((file) => {
    const header = (file.content ?? "").split(/^\[/m)[0];
    const field = (key: string) => header.match(new RegExp(`^${key}\\s*=\\s*"([^"]+)"`, "m"))?.[1];
    return { path: file.path, name: field("name") ?? field("id") ?? file.name.replace(/\.scene\.toml$/, "") };
  });
}
