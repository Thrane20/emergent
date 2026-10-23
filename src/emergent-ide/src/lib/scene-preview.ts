import "server-only";
import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { projectScenes } from "./project-scenes";
import type { ProjectSnapshot } from "./project";

const execute = promisify(execFile);
const state = globalThis as typeof globalThis & { emergentScenePreviewActive?: boolean };

export async function renderProjectScene(project: ProjectSnapshot, scenePath: string) {
  if (!projectScenes(project).some((scene) => scene.path === scenePath)) throw new Error("Choose a scene from the linked project.");
  if (state.emergentScenePreviewActive) throw new Error("A Rust scene preview is already building. Wait for it to finish.");
  state.emergentScenePreviewActive = true;
  let directory: string | undefined;
  try {
    directory = await mkdtemp(path.join(os.tmpdir(), "emergent-scene-preview-"));
    const output = path.join(directory, "frame.png");
    try {
      await execute("cargo", ["run", "--quiet", "--manifest-path", path.join(project.root, "Cargo.toml"), "--", "--preview-scene", scenePath, "--preview-output", output], {
        cwd: project.root, shell: false, timeout: 120_000, maxBuffer: 1024 * 1024,
        killSignal: "SIGTERM", windowsHide: true,
      });
    } catch (error) {
      const detail = error && typeof error === "object" && "stderr" in error ? String(error.stderr).trim().slice(-6000) : "";
      throw new Error(detail || "Rust scene preview could not run. Check that Rust is installed and this game supports --preview-scene / --preview-output.");
    }
    if ((await stat(output)).size > 16 * 1024 * 1024) throw new Error("Rendered preview exceeds 16 MiB.");
    const bytes = await readFile(output);
    if (!bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10]))) throw new Error("Rust preview did not produce a PNG frame.");
    return bytes;
  } finally {
    if (directory) await rm(directory, { recursive: true, force: true });
    state.emergentScenePreviewActive = false;
  }
}
