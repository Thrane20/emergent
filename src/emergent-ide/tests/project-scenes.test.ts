import assert from "node:assert/strict";
import test from "node:test";
import { projectScenes } from "../src/lib/project-scenes";
import type { ProjectFile } from "../src/lib/project";

function file(path: string, content: string): ProjectFile {
  return { path, name: path.split("/").at(-1)!, content, kind: "scene", bytes: content.length, modifiedAt: "", authored: true, generated: false };
}

test("scene picker lists all scenes using their top-level names, without treating entities as scenes", () => {
  const scenes = projectScenes({ files: [
    file("assets/scenes/main.scene.toml", 'schema_version = 1\nid = "main"\nname = "Main scene"\n[[entities]]\nname = "Not the scene name"'),
    file("assets/scenes/second.scene.toml", 'schema_version = 1\nid = "second"'),
    file("assets/scenes/empty.scene.toml", ''),
    file("assets/entities/cube.entity.toml", 'name = "Cube"'),
  ] });
  assert.deepEqual(scenes, [
    { path: "assets/scenes/main.scene.toml", name: "Main scene" },
    { path: "assets/scenes/second.scene.toml", name: "second" },
    { path: "assets/scenes/empty.scene.toml", name: "empty" },
  ]);
  assert.deepEqual(projectScenes({ files: [] }), []);
});
