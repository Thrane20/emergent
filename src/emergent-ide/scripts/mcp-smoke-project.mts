// Disposable engine-owned smoke fixture, assembled from the standard template.
// Its stable path and unique package name reuse the shared engine build safely.
// Usage as a script prints the env for starting an isolated IDE against the copy.
import { mkdir, rm } from "node:fs/promises";
import path from "node:path";
import { prepareTestProject } from "./project-fixture.js";

export const repo = path.resolve(import.meta.dirname, "../../..");
export const smokeRoot = path.join(repo, "target", "mcp-smoke");

export async function prepareSmokeProject() {
  const project = path.join(smokeRoot, "game"), data = path.join(smokeRoot, "data"), cargoTarget = path.join(repo, "target");
  await rm(project, { recursive: true, force: true });
  await rm(data, { recursive: true, force: true });
  await mkdir(data, { recursive: true, mode: 0o700 });
  await prepareTestProject(project, "emergent-mcp-smoke");
  return { project, data, cargoTarget, env: { EMERGENT_PROJECT_PATH: project, EMERGENT_IDE_DATA_DIR: data, CARGO_TARGET_DIR: cargoTarget } };
}

if (import.meta.filename === process.argv[1]) {
  const { env } = await prepareSmokeProject();
  for (const [key, value] of Object.entries(env)) console.log(`export ${key}=${JSON.stringify(value)}`);
}
