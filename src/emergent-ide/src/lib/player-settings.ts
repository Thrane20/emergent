import "server-only";
import { createHash, randomUUID } from "node:crypto";
import { lstat, readFile, realpath, rename, unlink, writeFile } from "node:fs/promises";
import path from "node:path";
import { parse, stringify } from "smol-toml";
import { missing, within } from "./object-files";
import type { ProjectSnapshot } from "./project";

export type PlayerSettings = {
  output: {
    resolution: "native" | "1280x720" | "1920x1080" | "2560x1440" | "3840x2160";
    target_fps: 30 | 60 | 90 | 120 | 144;
    minimum_fps: 24 | 30 | 45 | 60;
  };
  rendering: {
    antialiasing: "off" | "msaa-2x" | "msaa-4x" | "msaa-8x" | "fxaa" | "taa";
  };
};

export type PlayerSettingsDocument = {
  settings: PlayerSettings;
  revision: string;
};

export const DEFAULT_PLAYER_SETTINGS: PlayerSettings = {
  output: { resolution: "native", target_fps: 60, minimum_fps: 30 },
  rendering: { antialiasing: "msaa-4x" },
};

const fileName = "player.settings.toml";
const state = globalThis as typeof globalThis & { emergentPlayerSettingsSaving?: boolean };

function digest(source: string | null) {
  return createHash("sha256").update(source ?? "missing-player-settings").digest("hex");
}

function validateSettings(value: unknown): PlayerSettings {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Player settings must be a table.");
  const input = value as Record<string, unknown>;
  const output = input.output as Record<string, unknown> | undefined;
  const rendering = input.rendering as Record<string, unknown> | undefined;
  const resolutions = ["native", "1280x720", "1920x1080", "2560x1440", "3840x2160"];
  const frameRates = [30, 60, 90, 120, 144];
  const minimumRates = [24, 30, 45, 60];
  const antialiasing = ["off", "msaa-2x", "msaa-4x", "msaa-8x", "fxaa", "taa"];
  if (!output || typeof output !== "object" || Array.isArray(output) ||
    !rendering || typeof rendering !== "object" || Array.isArray(rendering) ||
    Object.keys(input).some(key => !["output", "rendering"].includes(key)) ||
    Object.keys(output).some(key => !["resolution", "target_fps", "minimum_fps"].includes(key)) ||
    Object.keys(rendering).some(key => key !== "antialiasing") ||
    typeof output.resolution !== "string" || !resolutions.includes(output.resolution) ||
    typeof output.target_fps !== "number" || !frameRates.includes(output.target_fps) ||
    typeof output.minimum_fps !== "number" || !minimumRates.includes(output.minimum_fps) ||
    typeof rendering.antialiasing !== "string" || !antialiasing.includes(rendering.antialiasing)) {
    throw new Error("Player settings contain an unsupported option.");
  }
  if (output.minimum_fps > output.target_fps) {
    throw new Error("Minimum expected FPS cannot exceed target FPS.");
  }
  return {
    output: {
      resolution: output.resolution as PlayerSettings["output"]["resolution"],
      target_fps: output.target_fps as PlayerSettings["output"]["target_fps"],
      minimum_fps: output.minimum_fps as PlayerSettings["output"]["minimum_fps"],
    },
    rendering: { antialiasing: rendering.antialiasing as PlayerSettings["rendering"]["antialiasing"] },
  };
}

async function readSource(project: ProjectSnapshot): Promise<string | null> {
  const root = await realpath(project.root);
  const target = path.join(root, fileName);
  try {
    const info = await lstat(target);
    if (info.isSymbolicLink() || !info.isFile()) throw new Error("Player settings must be a regular project file.");
    if (!within(root, await realpath(target))) throw new Error("Player settings path resolves outside the project.");
    return await readFile(target, "utf8");
  } catch (error) {
    if (missing(error)) return null;
    throw error;
  }
}

export async function readPlayerSettings(project: ProjectSnapshot): Promise<PlayerSettingsDocument> {
  const source = await readSource(project);
  if (source === null) return { settings: DEFAULT_PLAYER_SETTINGS, revision: digest(null) };
  const parsed = parse(source);
  const settings = validateSettings(parsed);
  return { settings, revision: digest(source) };
}

export async function savePlayerSettings(project: ProjectSnapshot, value: unknown, revision: unknown): Promise<PlayerSettingsDocument> {
  if (state.emergentPlayerSettingsSaving) throw new Error("Another player settings save is in progress.");
  if (typeof revision !== "string") throw new Error("Player settings revision is required.");
  const settings = validateSettings(value);
  state.emergentPlayerSettingsSaving = true;
  let temporary: string | undefined;
  try {
    if (digest(await readSource(project)) !== revision) throw new Error("Player settings changed externally. Reload before saving.");
    const content = stringify(settings);
    const root = await realpath(project.root);
    const target = path.join(root, fileName);
    try {
      if ((await lstat(target)).isSymbolicLink()) throw new Error("Cannot save player settings through a symbolic link.");
    } catch (error) {
      if (!missing(error)) throw error;
    }
    temporary = path.join(root, `.${fileName}.${randomUUID()}.tmp`);
    await writeFile(temporary, content, { flag: "wx", mode: 0o600 });
    if (digest(await readSource(project)) !== revision) throw new Error("Player settings changed during save. Reload before saving.");
    await rename(temporary, target);
    temporary = undefined;
    return readPlayerSettings(project);
  } finally {
    if (temporary) await unlink(temporary).catch(error => { if (!missing(error)) throw error; });
    state.emergentPlayerSettingsSaving = false;
  }
}
