import "server-only";
import { readAIProviderConnections } from "./ai-settings";
import { boundedBody } from "./object-files";
import type { AIBalance } from "./ai-balance-types";

const BILLING = {
  anthropic: { label: "Anthropic", url: "https://platform.claude.com/settings/billing" },
  "openai-codex": { label: "OpenAI", url: "https://platform.openai.com/settings/organization/billing/overview" },
  vercel: { label: "Vercel", url: "https://vercel.com/ai-gateway" },
};

/** Reading configuration never calls a provider; only an explicit refresh fetches credits. */
export async function aiBalances(refresh = false, credentials = readAIProviderConnections, transport = fetch): Promise<AIBalance[]> {
  const billed = (await credentials()).flatMap(connection => connection.provider === "ollama" ? [] : [{ ...connection, provider: connection.provider }]); // Local Ollama has no billing.
  return Promise.all(billed.map(async connection => {
    const info = BILLING[connection.provider];
    const result: AIBalance = {
      provider: connection.provider, label: info.label, billingUrl: info.url,
      state: "unsupported", balance: null, checkedAt: null,
      message: `${info.label} remaining credit is not available through the configured API key. Open billing to check it.`,
    };
    if (connection.provider !== "vercel") return result;
    if (connection.baseUrl !== "https://ai-gateway.vercel.sh/v1") {
      return { ...result, message: "Balance checks require the official Vercel AI Gateway URL. Open billing to check this account." };
    }
    if (!refresh) return { ...result, state: "unchecked", message: "Click refresh to check Vercel AI Gateway credit." };
    const unavailable = (message: string): AIBalance => ({ ...result, state: "unavailable", message });
    try {
      const response = await transport("https://ai-gateway.vercel.sh/v1/credits", {
        headers: { Authorization: `Bearer ${connection.apiKey}` },
        cache: "no-store", redirect: "error", signal: AbortSignal.timeout(10_000),
      });
      if (response.status === 401 || response.status === 403) return unavailable("Vercel denied the balance request. Check the saved Gateway key and its permissions.");
      if (!response.ok) return unavailable("Vercel balance is temporarily unavailable. Refresh to retry.");
      const data = JSON.parse((await boundedBody(response, 32_768)).toString("utf8"));
      const value = data?.balance;
      if (!(typeof value === "number" || typeof value === "string" && /^-?\d+(?:\.\d+)?$/.test(value)) || !Number.isFinite(Number(value))) {
        return unavailable("Vercel returned an invalid balance. Refresh to retry.");
      }
      return { ...result, state: "ready", balance: Number(value), checkedAt: new Date().toISOString(), message: "Available Vercel AI Gateway credit in USD; last checked on refresh." };
    } catch {
      return unavailable("Unable to read Vercel balance. Refresh to retry.");
    }
  }));
}
