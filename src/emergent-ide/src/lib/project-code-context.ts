import "server-only";
import { readFile, realpath } from "node:fs/promises";
import path from "node:path";
import { parse } from "smol-toml";
import type { ProjectSnapshot } from "./project";
import type { ProjectStatement } from "./statements";

export type CodeContext = { root: string; files: Map<string, string>; search: string[] };

export async function foundationRoot(project:ProjectSnapshot) {
  const cargo = parse(await readFile(path.join(project.root, "Cargo.toml"), "utf8"));
  const dependency = (cargo.dependencies as Record<string, unknown> | undefined)?.emergent;
  if (!dependency || typeof dependency !== "object" || !("path" in dependency) || typeof dependency.path !== "string") throw new Error("This implementation preview needs a local path dependency on Emergent to inspect the actual public contracts.");
  return realpath(path.resolve(project.root, dependency.path));
}

export async function projectCodeContext(project: ProjectSnapshot, statement: Pick<ProjectStatement,"subject"|"text">): Promise<CodeContext> {
  const root = await foundationRoot(project);
  const files = new Map<string, string>();
  const subjectPath = statement.subject.slice("file:".length);
  let budget = 180_000;
  const ordered = [...project.files].sort((a,b) => Number(b.path === subjectPath) - Number(a.path === subjectPath));
  for (const file of ordered) {
    if (!file.content || !/\.(rs|toml|txt|json)$/.test(file.path) || budget < file.content.length) continue;
    files.set(file.path, file.content); budget -= file.content.length;
  }
  for (const relative of ["AGENTS.md", "src/lib.rs", "src/behavior.rs", "src/component.rs", "src/motion.rs", "src/brain.rs", "src/ui.rs", "src/locomotion.rs", "src/engine.rs", "src/world.rs", "src/geometry.rs", "src/primitives.rs", "src/light.rs", "docs/capabilities/engine-capabilities.toml"]) {
    const source = await readFile(/* turbopackIgnore: true */ path.join(/* turbopackIgnore: true */ root, relative), "utf8");
    files.set(`foundation/${relative}`, source.split("#[cfg(test)]")[0].slice(0,40_000));
  }
  try { files.set("project/AGENTS.md", await readFile(path.join(project.root, "AGENTS.md"), "utf8")); } catch { /* Project guidance is optional. */ }
  const words = statement.text.toLowerCase().split(/[^a-z0-9]+/).filter((word) => word.length > 3);
  if (/spin|rotat|angular/i.test(statement.text)) words.push("rotate", "rotation", "angular", "quat", "spin");
  const search: string[] = [];
  for (const [file, content] of files) {
    let hits = 0;
    for (const [index,line] of content.split("\n").entries()) {
      if (words.some((word) => line.toLowerCase().includes(word))) { search.push(`${file}:${index + 1}: ${line.slice(0,200)}`); if (++hits === 4) break; }
    }
  }
  return { root, files, search };
}
