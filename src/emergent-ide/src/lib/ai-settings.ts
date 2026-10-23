import { chmod, mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import "server-only";

export const DEFAULT_AI_SETTINGS = {
  provider: "openai-codex",
  baseUrl: "https://api.openai.com/v1",
  model: "gpt-6.1-sol",
} as const;

export type AIConnectionSettings = {
  provider: string;
  baseUrl: string;
  model: string;
  apiKey: string;
};

export type PublicAISettings = Omit<StoredAISettings, "apiKey"> & {
  configured: boolean;
  keyHint: string | null;
};

type StoredAISettings = AIConnectionSettings;

export type AISettingsUpdate = {
  baseUrl?: unknown;
  model?: unknown;
  apiKey?: unknown;
  clearApiKey?: unknown;
};

export async function readPublicAISettings(): Promise<PublicAISettings> {
  return toPublic(await readStoredAISettings());
}

export async function readAIConnectionSettings(): Promise<AIConnectionSettings> {
  return readStoredAISettings();
}

export async function updateAISettings(input: AISettingsUpdate): Promise<PublicAISettings> {
  const current = await readStoredAISettings();
  const baseUrl = validateBaseUrl(input.baseUrl);
  const model = validateModel(input.model);
  const suppliedKey = typeof input.apiKey === "string" ? input.apiKey.trim() : "";
  const apiKey = input.clearApiKey === true ? "" : suppliedKey || current.apiKey;
  const next: StoredAISettings = {
    provider: DEFAULT_AI_SETTINGS.provider,
    baseUrl,
    model,
    apiKey,
  };

  const directory = settingsDirectory();
  const target = path.join(directory, "ai-provider.json");
  const temporary = path.join(directory, `ai-provider.${process.pid}.tmp`);
  await mkdir(directory, { recursive: true, mode: 0o700 });
  await writeFile(temporary, `${JSON.stringify(next, null, 2)}\n`, { mode: 0o600 });
  await rename(temporary, target);
  await chmod(target, 0o600);
  return toPublic(next);
}

async function readStoredAISettings(): Promise<StoredAISettings> {
  try {
    const raw = await readFile(path.join(settingsDirectory(), "ai-provider.json"), "utf8");
    const parsed = JSON.parse(raw) as Partial<StoredAISettings>;
    return {
      provider: DEFAULT_AI_SETTINGS.provider,
      baseUrl: validateBaseUrl(parsed.baseUrl),
      model: validateModel(parsed.model),
      apiKey: typeof parsed.apiKey === "string" ? parsed.apiKey : "",
    };
  } catch (error) {
    if (isMissingFile(error)) {
      return { ...DEFAULT_AI_SETTINGS, apiKey: "" };
    }
    throw error;
  }
}

function settingsDirectory(): string {
  const configured = process.env.EMERGENT_IDE_DATA_DIR?.trim();
  return configured ? path.resolve(configured) : path.join(process.cwd(), ".data");
}

function validateBaseUrl(value: unknown): string {
  const candidate = typeof value === "string" && value.trim()
    ? value.trim().replace(/\/+$/, "")
    : DEFAULT_AI_SETTINGS.baseUrl;
  let url: URL;
  try {
    url = new URL(candidate);
  } catch {
    throw new Error("Base URL must be a valid absolute URL.");
  }
  const localHttp = url.protocol === "http:" && ["localhost", "127.0.0.1", "::1"].includes(url.hostname);
  if (url.protocol !== "https:" && !localHttp) {
    throw new Error("Use HTTPS, or HTTP only for a local provider.");
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new Error("Base URL cannot contain credentials, query parameters, or a fragment.");
  }
  return candidate;
}

function validateModel(value: unknown): string {
  const model = typeof value === "string" && value.trim() ? value.trim() : DEFAULT_AI_SETTINGS.model;
  if (model.length > 120 || !/^[a-zA-Z0-9._:/-]+$/.test(model)) {
    throw new Error("Model contains unsupported characters or is too long.");
  }
  return model;
}

function toPublic(settings: StoredAISettings): PublicAISettings {
  return {
    provider: settings.provider,
    baseUrl: settings.baseUrl,
    model: settings.model,
    configured: settings.apiKey.length > 0,
    keyHint: settings.apiKey ? maskKey(settings.apiKey) : null,
  };
}

function maskKey(key: string): string {
  return key.length > 8 ? `${key.slice(0, 3)}…${key.slice(-4)}` : "saved securely";
}

function isMissingFile(error: unknown): boolean {
  return error instanceof Error && "code" in error && error.code === "ENOENT";
}
