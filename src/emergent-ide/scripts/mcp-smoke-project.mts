// Prepares the disposable game used by MCP smokes at a fixed path with its own Cargo target directory.
// The game binary embeds CARGO_MANIFEST_DIR, so a copy must never share build output with the real project or with
// copies at other paths: Cargo would reuse a binary that points at the wrong (or a deleted) project.
// Usage as a script prints the env for starting an isolated IDE against the copy.
import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

export const repo = path.resolve(import.meta.dirname, "../../..");
export const smokeRoot = path.join(repo, "target", "mcp-smoke");

export async function prepareSmokeProject() {
  const project = path.join(smokeRoot, "game"), data = path.join(smokeRoot, "data"), cargoTarget = path.join(smokeRoot, "target");
  await rm(project, { recursive: true, force: true });
  await rm(data, { recursive: true, force: true });
  await mkdir(data, { recursive: true, mode: 0o700 });
  await cp(path.join(repo, "projects/first_project"), project, { recursive: true, filter: source => !source.includes(`${path.sep}target`) });
  const manifest = path.join(project, "Cargo.toml");
  await writeFile(manifest, (await readFile(manifest, "utf8")).replace('path = "../.."', `path = ${JSON.stringify(repo)}`) + "\n[workspace]\n");
  return { project, data, cargoTarget, env: { EMERGENT_PROJECT_PATH: project, EMERGENT_IDE_DATA_DIR: data, CARGO_TARGET_DIR: cargoTarget } };
}

if (import.meta.filename === process.argv[1]) {
  const { env } = await prepareSmokeProject();
  for (const [key, value] of Object.entries(env)) console.log(`export ${key}=${JSON.stringify(value)}`);
}
