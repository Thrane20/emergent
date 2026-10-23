import "server-only";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { ideDataDirectory, missing, writeJson } from "./object-files";
import { OBJECT_ENDPOINT, ObjectCreatorError, type ObjectSettings } from "./object-types";

export async function objectApiKey() {
  if (process.env.FAL_KEY?.trim()) return process.env.FAL_KEY.trim();
  try { return (JSON.parse(await readFile(path.join(ideDataDirectory(), "object-provider.json"), "utf8")) as { apiKey: string }).apiKey || ""; }
  catch (error) { if (missing(error)) return ""; throw error; }
}

export async function publicObjectSettings(): Promise<ObjectSettings> {
  const key = await objectApiKey();
  return { configured: !!key, keyHint: key ? `…${key.slice(-4)}` : null, source: process.env.FAL_KEY?.trim() ? "environment" : "settings", endpoint: OBJECT_ENDPOINT };
}

export async function updateObjectSettings(input: { apiKey?: unknown; clearApiKey?: unknown }) {
  if (process.env.FAL_KEY?.trim()) throw new ObjectCreatorError("FAL_KEY is set by the server environment. Update it there.");
  const key = input.clearApiKey === true ? "" : typeof input.apiKey === "string" && input.apiKey.trim() ? input.apiKey.trim() : await objectApiKey();
  if (key.length > 512 || /\s/.test(key)) throw new ObjectCreatorError("API key is invalid.");
  await writeJson(path.join(ideDataDirectory(), "object-provider.json"), { apiKey: key });
  return publicObjectSettings();
}
