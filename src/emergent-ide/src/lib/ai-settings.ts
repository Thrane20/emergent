import { chmod, mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import "server-only";

export const AI_PROVIDERS = ["anthropic", "openai-codex", "vercel", "ollama"] as const;
/** Ollama ignores keys; storing this placeholder marks it activated so every existing key check works unchanged. */
export const OLLAMA_ACTIVE_KEY = "ollama";
export type AIProvider = (typeof AI_PROVIDERS)[number];

export const AI_PROVIDER_DEFAULTS: Record<AIProvider, { label: string; api: string; baseUrl: string; model: string; models: string[] }> = {
  anthropic: { label: "Claude", api: "Anthropic Messages API", baseUrl: "https://api.anthropic.com", model: "claude-opus-5-5", models: ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-fable-5-1"] },
  "openai-codex": { label: "Codex", api: "OpenAI Responses API", baseUrl: "https://api.openai.com/v1", model: "gpt-6.1-sol", models: ["gpt-6.1-sol"] },
  vercel: { label: "Vercel AI Gateway", api: "Vercel AI Gateway · Responses API", baseUrl: "https://ai-gateway.vercel.sh/v1", model: "anthropic/claude-sonnet-4.6", models: [] },
  ollama: { label: "Ollama", api: "Ollama (local) · Responses API", baseUrl: "http://localhost:11434/v1", model: "llama3.2", models: [] },
};

/** Active provider connection, used by every IDE AI request. */
export type AIConnectionSettings = { provider: AIProvider; baseUrl: string; model: string; apiKey: string };

type ProviderSettings = { baseUrl: string; model: string; apiKey: string };
type StoredAISettings = { provider: AIProvider; providers: Record<AIProvider, ProviderSettings> };

export type PublicProviderSettings = { label: string; api: string; baseUrl: string; model: string; models: string[]; configured: boolean; keyHint: string | null };
/** Top-level fields describe the active provider so existing consumers keep working. */
export type PublicAISettings = {
  provider: AIProvider; label: string; baseUrl: string; model: string; configured: boolean; keyHint: string | null;
  providers: Record<AIProvider, PublicProviderSettings>;
};

export type AISettingsUpdate = {
  provider?: unknown;
  baseUrl?: unknown;
  model?: unknown;
  apiKey?: unknown;
  clearApiKey?: unknown;
};

export async function readPublicAISettings(): Promise<PublicAISettings> {
  return toPublic(await readStoredAISettings());
}

export async function readAIConnectionSettings(): Promise<AIConnectionSettings> {
  const stored = await readStoredAISettings();
  return { provider: stored.provider, ...stored.providers[stored.provider] };
}

/** Server-only configured connections, including providers that are not active. */
export async function readAIProviderConnections(): Promise<AIConnectionSettings[]> {
  const stored = await readStoredAISettings();
  return AI_PROVIDERS.filter(provider => stored.providers[provider].apiKey.length > 0)
    .map(provider => ({ provider, ...stored.providers[provider] }));
}

/** Saves one provider's connection and makes it the active provider. */
export async function updateAISettings(input: AISettingsUpdate): Promise<PublicAISettings> {
  const current = await readStoredAISettings();
  const provider = input.provider === undefined ? current.provider : validateProvider(input.provider);
  const previous = current.providers[provider];
  const suppliedKey = provider === "ollama" ? OLLAMA_ACTIVE_KEY : typeof input.apiKey === "string" ? input.apiKey.trim() : "";
  const next: StoredAISettings = {
    provider,
    providers: {
      ...current.providers,
      [provider]: {
        baseUrl: validateBaseUrl(input.baseUrl, provider),
        model: validateModel(input.model, provider),
        apiKey: input.clearApiKey === true ? "" : suppliedKey || previous.apiKey,
      },
    },
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
  const fallback = (provider: AIProvider): ProviderSettings => ({ baseUrl: AI_PROVIDER_DEFAULTS[provider].baseUrl, model: AI_PROVIDER_DEFAULTS[provider].model, apiKey: "" });
  try {
    const raw = await readFile(path.join(settingsDirectory(), "ai-provider.json"), "utf8");
    const parsed = JSON.parse(raw) as Partial<StoredAISettings> & Partial<ProviderSettings>;
    // Version 1 files stored a single flat OpenAI/Codex connection.
    const saved: Partial<Record<AIProvider, Partial<ProviderSettings>>> = parsed.providers ?? { "openai-codex": parsed };
    const providers = Object.fromEntries(AI_PROVIDERS.map(provider => {
      const entry = saved[provider];
      return [provider, entry ? {
        baseUrl: validateBaseUrl(entry.baseUrl, provider),
        model: validateModel(entry.model, provider),
        apiKey: typeof entry.apiKey === "string" ? entry.apiKey : "",
      } : fallback(provider)];
    })) as Record<AIProvider, ProviderSettings>;
    return { provider: AI_PROVIDERS.includes(parsed.provider as AIProvider) ? parsed.provider as AIProvider : "openai-codex", providers };
  } catch (error) {
    if (isMissingFile(error)) {
      return { provider: "anthropic", providers: Object.fromEntries(AI_PROVIDERS.map(provider => [provider, fallback(provider)])) as Record<AIProvider, ProviderSettings> };
    }
    throw error;
  }
}

export function settingsDirectory(): string {
  const configured = process.env.EMERGENT_IDE_DATA_DIR?.trim();
  return configured ? path.resolve(configured) : path.join(process.cwd(), ".data");
}

function validateProvider(value: unknown): AIProvider {
  if (!AI_PROVIDERS.includes(value as AIProvider)) throw new Error("Choose Claude, Codex, Vercel AI Gateway or Ollama as the AI provider.");
  return value as AIProvider;
}

function validateBaseUrl(value: unknown, provider: AIProvider): string {
  const candidate = typeof value === "string" && value.trim()
    ? value.trim().replace(/\/+$/, "")
    : AI_PROVIDER_DEFAULTS[provider].baseUrl;
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
  if (provider === "ollama" && !localHttp) throw new Error("Ollama must be reached over http://localhost:<port>.");
  if (url.username || url.password || url.search || url.hash) {
    throw new Error("Base URL cannot contain credentials, query parameters, or a fragment.");
  }
  return candidate;
}

function validateModel(value: unknown, provider: AIProvider): string {
  const model = typeof value === "string" && value.trim() ? value.trim() : AI_PROVIDER_DEFAULTS[provider].model;
  if (model.length > 120 || !/^[a-zA-Z0-9._:/-]+$/.test(model)) {
    throw new Error("Model contains unsupported characters or is too long.");
  }
  if (provider === "vercel" && !/^[a-zA-Z0-9._-]+\/[a-zA-Z0-9._:/-]+$/.test(model)) {
    throw new Error("Vercel model must use creator/model format, for example anthropic/claude-sonnet-4.6.");
  }
  return model;
}

function toPublic(settings: StoredAISettings): PublicAISettings {
  const providers = Object.fromEntries(AI_PROVIDERS.map(provider => {
    const entry = settings.providers[provider], defaults = AI_PROVIDER_DEFAULTS[provider];
    return [provider, { label: defaults.label, api: defaults.api, baseUrl: entry.baseUrl, model: entry.model, models: defaults.models, configured: entry.apiKey.length > 0, keyHint: entry.apiKey && provider !== "ollama" ? maskKey(entry.apiKey) : null }];
  })) as Record<AIProvider, PublicProviderSettings>;
  const active = providers[settings.provider];
  return { provider: settings.provider, label: active.label, baseUrl: active.baseUrl, model: active.model, configured: active.configured, keyHint: active.keyHint, providers };
}

function maskKey(key: string): string {
  return key.length > 8 ? `${key.slice(0, 3)}…${key.slice(-4)}` : "saved securely";
}

function isMissingFile(error: unknown): boolean {
  return error instanceof Error && "code" in error && error.code === "ENOENT";
}
