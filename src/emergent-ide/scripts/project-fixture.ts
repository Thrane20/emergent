// Disposable validation projects use the public standard template plus engine-owned test assets.
import { cp, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { parse, stringify } from "smol-toml";

export const engineRoot = path.resolve(import.meta.dirname, "../../..");

export function smokeProjectRoot() {
  if (!process.env.EMERGENT_PROJECT_PATH) throw new Error("Set EMERGENT_PROJECT_PATH to a disposable template-derived project and run its IDE on port 3001.");
  return path.resolve(process.env.EMERGENT_PROJECT_PATH);
}

export async function prepareTestProject(target: string, name = "emergent-editor-qa") {
  await cp(path.join(engineRoot, "templates/standard"), target, {
    recursive: true,
    filter: source => !["target", ".git", "Cargo.lock"].includes(path.basename(source)),
  });
  await cp(path.join(engineRoot, "tests/fixtures/editor-project/assets"), path.join(target, "assets"), { recursive: true });
  const cargo = parse(await readFile(path.join(target, "Cargo.toml"), "utf8")) as { package: { name: string }; dependencies: { emergent: { path: string } }; workspace?: object };
  cargo.package.name = name;
  cargo.dependencies.emergent.path = engineRoot;
  cargo.workspace = {};
  await writeFile(path.join(target, "Cargo.toml"), stringify(cargo));
  await cp(path.join(engineRoot, "Cargo.lock"), path.join(target, "Cargo.lock"));
}
