import "server-only";
import { createFalClient } from "@fal-ai/client";
import { boundedBody } from "./object-files";
import { objectApiKey } from "./object-settings";
import { GENERATION_SETTINGS, MAX_MODEL_BYTES, OBJECT_ENDPOINT, ObjectCreatorError } from "./object-types";

export interface ObjectProvider {
  upload(bytes: Buffer, extension: string): Promise<string>;
  submit(imageUrl: string, seed: number): Promise<string>;
  status(requestId: string): Promise<"queued" | "generating" | "completed">;
  result(requestId: string): Promise<Buffer>;
}

export class ProviderFailure extends Error {
  constructor(message: string, readonly terminal: boolean) { super(message); }
}

export async function falProvider(): Promise<ObjectProvider> {
  const key = await objectApiKey();
  if (!key) throw new ObjectCreatorError("Configure a fal API key in Settings → Object generation.", 503);
  return createTrellisProvider(key);
}

// The SDK applies its own queue retry policy even when config.retry is disabled.
// Guard paid POSTs at the transport boundary to allow at most one network attempt.
export function singleSubmissionFetch(transport: typeof fetch): typeof fetch {
  let submitted = false;
  return async (input, init) => {
    const url = new URL(typeof input === "string" ? input : input instanceof URL ? input.href : input.url);
    if (url.hostname === "queue.fal.run" && (init?.method ?? "GET").toUpperCase() === "POST") {
      if (submitted) throw new Error("Submission outcome uncertain; automatic resubmission disabled.");
      submitted = true;
    }
    return transport(input, { ...init, signal: init?.signal ?? AbortSignal.timeout(30_000) });
  };
}

function providerFailure(error: unknown): ProviderFailure {
  const status = error && typeof error === "object" && "status" in error ? Number(error.status) : 0;
  if (status === 401 || status === 403) return new ProviderFailure("fal rejected the credentials. Check the API key and account access in Settings.", false);
  if (status === 402) return new ProviderFailure("fal requires account credit before generation can continue.", false);
  if (status === 422 || status === 400) return new ProviderFailure("fal could not generate this object. Try a different reference image.", true);
  return new ProviderFailure("The provider could not be reached. The existing request can be checked again.", false);
}

export function createTrellisProvider(key: string, transport: typeof fetch = fetch): ObjectProvider {
  const client = createFalClient({ credentials: key, retry: { maxRetries: 0 }, fetch: singleSubmissionFetch(transport) });
  return {
    async upload(bytes, extension) {
      try { return await client.storage.upload(new File([new Uint8Array(bytes)], `reference.${extension}`, { type: `image/${extension === "jpg" ? "jpeg" : extension}` })); }
      catch (error) { throw providerFailure(error); }
    },
    async submit(imageUrl, seed) {
      try {
        const result = await client.queue.submit(OBJECT_ENDPOINT, { input: { ...GENERATION_SETTINGS, texture_size: "1024", image_url: imageUrl, seed }, abortSignal: AbortSignal.timeout(30_000) });
        if (typeof result.request_id !== "string" || !result.request_id) throw new Error("Provider returned no request ID.");
        return result.request_id;
      } catch (error) { throw providerFailure(error); }
    },
    async status(requestId) {
      try {
        const result = await client.queue.status(OBJECT_ENDPOINT, { requestId, logs: false, abortSignal: AbortSignal.timeout(15_000) });
        return result.status === "COMPLETED" ? "completed" : result.status === "IN_QUEUE" ? "queued" : "generating";
      } catch (error) { throw providerFailure(error); }
    },
    async result(requestId) {
      let url: URL;
      try {
        const result = await client.queue.result(OBJECT_ENDPOINT, { requestId, abortSignal: AbortSignal.timeout(15_000) });
        url = new URL(result.data.model_mesh.url);
      } catch (error) { throw providerFailure(error); }
      if (url.protocol !== "https:" || !(url.hostname === "fal.media" || url.hostname.endsWith(".fal.media")) || url.username || url.password || (url.port && url.port !== "443")) {
        throw new ObjectCreatorError("Provider returned an unsupported model download host.");
      }
      const response = await transport(url, { redirect: "error", signal: AbortSignal.timeout(60_000) });
      if (!response.ok) throw new ProviderFailure("Model download failed. Check the existing request again.", false);
      return boundedBody(response, MAX_MODEL_BYTES);
    },
  };
}
