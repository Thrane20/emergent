import "server-only";
import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, realpath, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import { createFalClient } from "@fal-ai/client";
import type { ProjectSnapshot } from "./project";
import { containedFile, ideDataDirectory, missing, validateGlb, writeJson } from "./object-files";
import { downloadFalModel, providerFailure, ProviderFailure, singleSubmissionFetch } from "./object-provider";
import { objectApiKey } from "./object-settings";
import { MAX_MODEL_BYTES, ObjectCreatorError } from "./object-types";

/** fal part splitters that take a GLB and return a GLB whose meshes are the parts. */
export const SPLIT_ENDPOINTS = {
  "tripo3d/tripo/segment": { label: "Tripo Segment · named parts", input: (url: string) => ({ mesh_url: url }) },
  "hitem3d/hi3d/split": { label: "Hi3D Split · choose granularity", input: (url: string, level: SplitLevel) => ({ mesh_url: url, model: "general", level, export_format: "glb" }) },
} as const;
export type SplitEndpoint = keyof typeof SPLIT_ENDPOINTS;
export type SplitLevel = "low" | "medium" | "high";
export type SplitJob = {
  schema_version: 1; id: string; projectId: string; model: string; endpoint: SplitEndpoint; level: SplitLevel;
  state: "uploading" | "submitting" | "uncertain" | "queued" | "splitting" | "downloading" | "ready" | "failed";
  requestId: string | null; createdAt: string; updatedAt: string; error: string | null;
};

export interface SplitProvider {
  upload(bytes: Buffer): Promise<string>;
  submit(endpoint: SplitEndpoint, input: Record<string, unknown>): Promise<string>;
  status(endpoint: SplitEndpoint, requestId: string): Promise<"queued" | "splitting" | "completed">;
  result(endpoint: SplitEndpoint, requestId: string): Promise<Buffer>;
}

export function createSplitProvider(key: string, transport: typeof fetch = fetch): SplitProvider {
  const client = createFalClient({ credentials: key, retry: { maxRetries: 0 }, fetch: singleSubmissionFetch(transport) });
  return {
    async upload(bytes) {
      try { return await client.storage.upload(new File([new Uint8Array(bytes)], "model.glb", { type: "model/gltf-binary" })); }
      catch (error) { throw providerFailure(error); }
    },
    async submit(endpoint, input) {
      try {
        const result = await client.queue.submit(endpoint, { input, abortSignal: AbortSignal.timeout(30_000) });
        if (typeof result.request_id !== "string" || !result.request_id) throw new Error("Provider returned no request ID.");
        return result.request_id;
      } catch (error) { throw providerFailure(error); }
    },
    async status(endpoint, requestId) {
      try {
        const result = await client.queue.status(endpoint, { requestId, logs: false, abortSignal: AbortSignal.timeout(15_000) });
        return result.status === "COMPLETED" ? "completed" : result.status === "IN_QUEUE" ? "queued" : "splitting";
      } catch (error) { throw providerFailure(error); }
    },
    async result(endpoint, requestId) {
      let url: URL;
      try {
        const { data } = await client.queue.result(endpoint, { requestId, abortSignal: AbortSignal.timeout(15_000) }) as { data: { model_urls?: { glb?: { url?: string } }; model_mesh?: { url?: string } } };
        // Tripo may return FBX as its primary mesh; prefer its explicit GLB variant.
        const href = data.model_urls?.glb?.url ?? data.model_mesh?.url;
        if (!href) throw new ObjectCreatorError("The splitter returned no model.");
        url = new URL(href);
      } catch (error) { if (error instanceof ObjectCreatorError) throw error; throw providerFailure(error); }
      return downloadFalModel(url, transport);
    },
  };
}

async function falSplitProvider(): Promise<SplitProvider> {
  const key = await objectApiKey();
  if (!key) throw new ObjectCreatorError("Configure a fal API key in Settings → Object generation.", 503);
  return createSplitProvider(key);
}

type ProjectScope = Pick<ProjectSnapshot, "id" | "root" | "assetRoots">;
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const now = () => new Date().toISOString();

export class SplitJobs {
  private locks = new Map<string, Promise<unknown>>();
  constructor(private dataDirectory = ideDataDirectory(), private provider: () => Promise<SplitProvider> = falSplitProvider) {}

  private async folder(project: ProjectScope, id?: string) {
    if (id !== undefined && !uuid.test(id)) throw new ObjectCreatorError("Invalid split job ID.");
    const identity = createHash("sha256").update(`${project.id}\0${await realpath(project.root)}`).digest("hex");
    return path.join(this.dataDirectory, "model-split-jobs", identity, ...(id ? [id] : []));
  }

  private async serial<T>(key: string, action: () => Promise<T>): Promise<T> {
    const previous = this.locks.get(key) ?? Promise.resolve();
    const operation = previous.catch(() => undefined).then(action);
    this.locks.set(key, operation);
    try { return await operation; } finally { if (this.locks.get(key) === operation) this.locks.delete(key); }
  }

  async get(project: ProjectScope, id: string): Promise<SplitJob> {
    try {
      const job = JSON.parse(await readFile(path.join(await this.folder(project, id), "job.json"), "utf8")) as SplitJob;
      if (job.schema_version !== 1 || job.id !== id || job.projectId !== project.id) throw new ObjectCreatorError("Invalid split job journal.");
      return job;
    } catch (error) { if (missing(error)) throw new ObjectCreatorError("Split job was not found.", 404); throw error; }
  }

  async list(project: ProjectScope, model?: string) {
    let ids: string[];
    try { ids = (await readdir(await this.folder(project))).filter(id => uuid.test(id)); }
    catch (error) { if (missing(error)) return []; throw error; }
    const jobs = await Promise.all(ids.map(id => this.get(project, id).catch(() => null)));
    return jobs.filter((job): job is SplitJob => !!job && (!model || job.model === model)).sort((a, b) => b.createdAt.localeCompare(a.createdAt)).slice(0, 50);
  }

  private async persist(project: ProjectScope, job: SplitJob) {
    job.updatedAt = now();
    await writeJson(path.join(await this.folder(project, job.id), "job.json"), job);
    return job;
  }

  async create(project: ProjectScope, input: { id?: unknown; model?: unknown; endpoint?: unknown; level?: unknown }) {
    if (typeof input.id !== "string" || !uuid.test(input.id)) throw new ObjectCreatorError("Invalid split job ID.");
    if (typeof input.endpoint !== "string" || !(input.endpoint in SPLIT_ENDPOINTS)) throw new ObjectCreatorError("Choose a supported part splitter.");
    const level: SplitLevel = input.level === "low" || input.level === "high" ? input.level : "medium";
    if (typeof input.model !== "string" || !input.model.endsWith(".glb") || !project.assetRoots.some(root => (input.model as string).startsWith(`${root}/`))) throw new ObjectCreatorError("Choose a saved .glb model in this project.");
    const id = input.id, model = input.model, endpoint = input.endpoint as SplitEndpoint;
    const folder = await this.folder(project, id);
    return this.serial(folder, async () => {
      try { return await this.get(project, id); } catch (error) { if (!(error instanceof ObjectCreatorError && error.status === 404)) throw error; }
      const bytes = await readFile(await containedFile(project.root, model, MAX_MODEL_BYTES));
      validateGlb(bytes);
      const provider = await this.provider();
      await mkdir(folder, { recursive: true, mode: 0o700 });
      // A durable exclusive claim prevents a duplicate paid call across module reloads.
      try { await writeFile(path.join(folder, "submission.claim"), "claimed\n", { flag: "wx", mode: 0o600 }); }
      catch { throw new ObjectCreatorError("This split is already being recorded. Refresh its history.", 409); }
      const job: SplitJob = { schema_version: 1, id, projectId: project.id, model, endpoint, level, state: "uploading", requestId: null, createdAt: now(), updatedAt: now(), error: null };
      await this.persist(project, job);
      let url: string;
      try { url = await provider.upload(bytes); }
      catch { job.state = "failed"; job.error = "Model upload failed. Check your fal key and account credit, then start a new split."; return this.persist(project, job); }
      job.state = "submitting";
      await this.persist(project, job);
      try { job.requestId = await provider.submit(endpoint, SPLIT_ENDPOINTS[endpoint].input(url, level)); job.state = "queued"; }
      catch { job.state = "uncertain"; job.error = "Submission could not be confirmed. Check your fal dashboard before starting another paid split; this one will not be resubmitted."; }
      return this.persist(project, job);
    });
  }

  async refresh(project: ProjectScope, id: string) {
    const folder = await this.folder(project, id);
    return this.serial(folder, async () => {
      const job = await this.get(project, id);
      if (job.state === "uploading") { job.state = "failed"; job.error = "The IDE restarted during upload. Start a new split."; }
      else if (job.state === "submitting") { job.state = "uncertain"; job.error = "The IDE restarted before confirming submission. Check the fal dashboard before a new split."; }
      else if (job.requestId && ["queued", "splitting", "downloading"].includes(job.state)) {
        try {
          const provider = await this.provider();
          const status = await provider.status(job.endpoint, job.requestId);
          job.error = null;
          if (status === "completed") {
            job.state = "downloading";
            await this.persist(project, job);
            const bytes = await provider.result(job.endpoint, job.requestId);
            validateGlb(bytes);
            await writeFile(path.join(folder, "split.glb.tmp"), bytes, { mode: 0o600 });
            await rename(path.join(folder, "split.glb.tmp"), path.join(folder, "split.glb"));
            job.state = "ready";
          } else job.state = status;
        } catch (error) {
          if (error instanceof ObjectCreatorError && error.status !== 503 || error instanceof ProviderFailure && error.terminal) job.state = "failed";
          job.error = error instanceof ObjectCreatorError || error instanceof ProviderFailure ? error.message : "Unable to check or download this split. Check again to resume.";
        }
      }
      return this.persist(project, job);
    });
  }

  async artifact(project: ProjectScope, id: string) {
    const job = await this.get(project, id);
    if (job.state !== "ready") throw new ObjectCreatorError("The split is not ready.", 409);
    return containedFile(await this.folder(project, id), "split.glb", MAX_MODEL_BYTES);
  }
}

const globalJobs = globalThis as typeof globalThis & { emergentSplitJobs?: SplitJobs };
export const splitJobs = globalJobs.emergentSplitJobs ??= new SplitJobs();
