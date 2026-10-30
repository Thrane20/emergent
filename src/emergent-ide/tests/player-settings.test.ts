import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, mkdir, readFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { DEFAULT_PLAYER_SETTINGS, readPlayerSettings, savePlayerSettings } from "../src/lib/player-settings";
import type { ProjectSnapshot } from "../src/lib/project";

test("player settings default cleanly, persist valid targets, and reject stale or invalid saves", async t => {
  const root = await mkdtemp(path.join(os.tmpdir(), "emergent-player-settings-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  await mkdir(path.join(root, "assets"));
  const project: ProjectSnapshot = {
    root,
    id: "player-settings-test",
    name: "Player Settings Test",
    assetRoots: ["assets"],
    entryScene: "assets/main.scene.toml",
    files: [],
    connections: [],
    entities: [],
    outputs: [],
  };

  const initial = await readPlayerSettings(project);
  assert.deepEqual(initial.settings, DEFAULT_PLAYER_SETTINGS);
  const updated = await savePlayerSettings(project, {
    output: { resolution: "1920x1080", target_fps: 120, minimum_fps: 60 },
    rendering: { antialiasing: "fxaa" },
  }, initial.revision);
  assert.equal(updated.settings.output.resolution, "1920x1080");
  assert.equal(updated.settings.output.target_fps, 120);
  assert.equal(updated.settings.rendering.antialiasing, "fxaa");
  assert.match(await readFile(path.join(root, "player.settings.toml"), "utf8"), /target_fps = 120/);
  assert.notEqual(updated.revision, initial.revision);
  await assert.rejects(
    savePlayerSettings(project, updated.settings, initial.revision),
    /changed externally/,
  );
  await assert.rejects(
    savePlayerSettings(project, {
      ...updated.settings,
      output: { ...updated.settings.output, target_fps: 30, minimum_fps: 45 },
    }, updated.revision),
    /Minimum expected FPS/,
  );
});
