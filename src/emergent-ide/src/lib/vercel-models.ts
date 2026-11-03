import "server-only";
import { z } from "zod";

const catalogue = z.object({ data: z.array(z.object({
  id: z.string().min(1).max(120), name: z.string(), type: z.string(),
  tags: z.array(z.string()).default([]),
})) });

/** Public catalogue only: discovery never sends a saved key or follows a custom provider URL. */
export async function vercelModels(fetcher = fetch): Promise<{ id: string; name: string }[]> {
  const response = await fetcher("https://ai-gateway.vercel.sh/v1/models", {
    signal: AbortSignal.timeout(15_000), cache: "no-store",
  });
  if (!response.ok) throw new Error(`Vercel model catalogue unavailable (${response.status}). Try again or enter a model ID.`);
  return catalogue.parse(await response.json()).data
    .filter(model => model.type === "language" && model.tags.includes("tool-use") && model.tags.includes("structured-output"))
    .map(({ id, name }) => ({ id, name }))
    .sort((a, b) => a.id.localeCompare(b.id));
}
