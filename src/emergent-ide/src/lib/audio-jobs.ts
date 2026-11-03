import "server-only";
import { createHash, randomUUID } from "node:crypto";
import { mkdir, readFile, readdir, realpath, unlink, writeFile } from "node:fs/promises";
import path from "node:path";
import { createFalClient } from "@fal-ai/client";
import { importClip } from "./audio-assets";
import { AUDIO_MODE_CATEGORY, MAX_AUDIO_BYTES, audioExtension, audioJobRequest, type AudioJob } from "./audio-types";
import { DEFAULT_FAL_MODELS, falModelId } from "./fal-model-types";
import { audioModelAdapter, audioModelInput, falModels } from "./fal-models";
import { boundedBody, containedFile, ideDataDirectory, missing, writeJson } from "./object-files";
import { singleSubmissionFetch } from "./object-provider";
import { objectApiKey } from "./object-settings";
import type { ProjectSnapshot } from "./project";

const uuid = /^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/;
export interface AudioProvider { submit(endpoint: string, input: Record<string, unknown>): Promise<string>; status(endpoint: string, id: string): Promise<"queued" | "generating" | "ready">; result(endpoint: string, id: string, field: string): Promise<Buffer> }

export async function falAudioProvider(): Promise<AudioProvider> { const key = await objectApiKey(); if (!key) throw new Error("Configure the saved fal key in Settings."); return createAudioProvider(key); }
export function createAudioProvider(key: string, transport: typeof fetch = fetch): AudioProvider {
  const client = createFalClient({ credentials: key, retry: { maxRetries: 0 }, fetch: singleSubmissionFetch(transport) });
  return {
    async submit(endpoint, input) { const r = await client.queue.submit(endpoint, { input, abortSignal: AbortSignal.timeout(30000) }); if (!r.request_id) throw new Error("fal returned no request ID."); return r.request_id; },
    async status(endpoint, requestId) { const r = await client.queue.status(endpoint, { requestId, logs: false, abortSignal: AbortSignal.timeout(15000) }); return r.status === "COMPLETED" ? "ready" : r.status === "IN_QUEUE" ? "queued" : "generating"; },
    async result(endpoint, requestId, field) {
      const r = await client.queue.result(endpoint, { requestId, abortSignal: AbortSignal.timeout(15000) }), data = r.data as Record<string, { url?: string } | undefined>;
      let url: URL; try { url = new URL(data[field]?.url ?? ""); } catch { throw new Error("fal returned no audio."); }
      // Only fal's own media hosts are fetched; catalogue data never chooses a network destination.
      if (url.protocol !== "https:" || url.username || url.password || (url.port && url.port !== "443") || !(url.hostname === "fal.media" || url.hostname.endsWith(".fal.media"))) throw new Error("Unsupported fal download host.");
      const response = await transport(url, { redirect: "error", signal: AbortSignal.timeout(90000) });
      if (!response.ok) throw new Error("Audio download failed. Refresh to resume.");
      return boundedBody(response, MAX_AUDIO_BYTES);
    },
  };
}

type Adapter = ReturnType<typeof audioModelAdapter>;
/** The defaults are known shapes, so they work without a catalogue lookup. */
const DEFAULT_ADAPTERS: Record<AudioJob["mode"], Adapter> = {
  sound: { endpoint: DEFAULT_FAL_MODELS.sound.id, input: { required: ["text"] }, properties: {}, textField: "text", audioField: "audio", duration: { field: "duration_seconds", scale: 1, min: 0.5, max: 22, integer: false }, outputFormat: undefined, seed: false, voice: undefined, voices: [], loop: true, instrumental: false },
  music: { endpoint: DEFAULT_FAL_MODELS.music.id, input: {}, properties: {}, textField: "prompt", audioField: "audio", duration: { field: "music_length_ms", scale: 1000, min: 5000, max: 300000, integer: true }, outputFormat: undefined, seed: false, voice: undefined, voices: [], loop: false, instrumental: true },
  speech: { endpoint: DEFAULT_FAL_MODELS.speech.id, input: { required: ["text"] }, properties: {}, textField: "text", audioField: "audio", duration: undefined, outputFormat: undefined, seed: false, voice: "voice", voices: [], loop: false, instrumental: false },
};

export class AudioJobs {
  private locks = new Map<string, Promise<unknown>>();
  constructor(private data = ideDataDirectory(), private provider: () => Promise<AudioProvider> = falAudioProvider, private adapter: (mode: AudioJob["mode"], id: string) => Promise<Adapter> = (mode, id) => falModels.audio(mode, id)) {}
  private async folder(p: ProjectSnapshot, id?: string) { if (id !== undefined && !uuid.test(id)) throw new Error("Invalid audio job ID."); const scope = createHash("sha256").update(`${p.id}\0${await realpath(p.root)}`).digest("hex"); return path.join(this.data, "audio-jobs", scope, ...(id ? [id] : [])); }
  private async serial<T>(key: string, fn: () => Promise<T>) { const previous = this.locks.get(key) ?? Promise.resolve(), next = previous.catch(() => {}).then(fn); this.locks.set(key, next); try { return await next; } finally { if (this.locks.get(key) === next) this.locks.delete(key); } }
  async get(p: ProjectSnapshot, id: string): Promise<AudioJob> { const j = JSON.parse(await readFile(path.join(await this.folder(p, id), "job.json"), "utf8")); if (j.id !== id || j.projectId !== p.id || j.schema_version !== 1) throw new Error("Invalid audio job journal."); return j; }
  private async persist(p: ProjectSnapshot, j: AudioJob) { await writeJson(path.join(await this.folder(p, j.id), "job.json"), j); return j; }
  async list(p: ProjectSnapshot) {
    let ids: string[]; try { ids = await readdir(await this.folder(p)); } catch (e) { if (missing(e)) return []; throw e; }
    const jobs = await Promise.all(ids.filter(id => uuid.test(id)).map(id => this.get(p, id).catch(e => { if (missing(e)) return null; throw e; })));
    return jobs.filter((job): job is AudioJob => job !== null && !job.deleted).sort((a, b) => b.createdAt.localeCompare(a.createdAt)).slice(0, 50);
  }
  /** Submits once per job ID: repeating the same request returns the existing job, so a retry never pays twice. */
  async create(p: ProjectSnapshot, id: string, value: unknown) {
    const request = audioJobRequest(value, falModelId), folder = await this.folder(p, id), endpoint = request.model ?? DEFAULT_FAL_MODELS[request.mode].id;
    return this.serial(folder, async () => {
      try { const j = await this.get(p, id); if (j.deleted) throw new Error("This result was deleted. Start a new generation."); if (j.text !== request.text || j.mode !== request.mode || j.seed !== request.seed || j.endpoint !== endpoint) throw new Error("This job ID belongs to another request."); return j; } catch (e) { if (!missing(e)) throw e; }
      // Check the model can be driven before claiming a paid submission.
      const adapter = endpoint === DEFAULT_FAL_MODELS[request.mode].id ? DEFAULT_ADAPTERS[request.mode] : await this.adapter(request.mode, endpoint);
      const provider = await this.provider();
      const settings = audioModelInput(adapter, { text: request.text, seed: request.seed, duration: request.duration, voice: request.voice, loop: request.loop, instrumental: request.instrumental });
      await mkdir(folder, { recursive: true }); await writeFile(path.join(folder, "submission.claim"), "claimed", { flag: "wx" });
      const job: AudioJob = { schema_version: 1, id, projectId: p.id, ...request, endpoint, settings: { ...settings, audio_field: adapter.audioField }, requestId: null, state: "submitting", error: null, createdAt: new Date().toISOString() };
      await this.persist(p, job);
      try { job.requestId = await provider.submit(endpoint, settings); job.state = "queued"; }
      catch { job.state = "uncertain"; job.error = "Submission could not be confirmed. Check fal before starting a new paid attempt. This job will not be resubmitted."; }
      return this.persist(p, job);
    });
  }
  async refresh(p: ProjectSnapshot, id: string) {
    const folder = await this.folder(p, id);
    return this.serial(folder, async () => {
      const j = await this.get(p, id);
      if (j.deleted) throw new Error("This result was deleted.");
      if (j.state === "submitting") { j.state = "uncertain"; j.error = "Restart interrupted submission. Check fal; no automatic resubmission."; }
      else if (j.requestId && j.state !== "ready") try {
        const provider = await this.provider(), state = await provider.status(j.endpoint, j.requestId);
        j.state = state; j.error = null;
        if (state === "ready") {
          const bytes = await provider.result(j.endpoint, j.requestId, String(j.settings.audio_field ?? "audio")), extension = audioExtension(bytes);
          if (!extension) throw new Error("fal returned audio in a format the engine cannot play.");
          j.file = `candidate.${extension}`; await writeFile(path.join(folder, j.file), bytes);
        }
      } catch (e) { j.state = "failed"; j.error = e instanceof Error ? e.message : "Audio generation failed. Refresh to resume."; }
      return this.persist(p, j);
    });
  }
  async artifact(p: ProjectSnapshot, id: string) { const j = await this.get(p, id); if (j.deleted || j.state !== "ready" || !j.file || !/^candidate\.(wav|mp3|ogg|flac)$/.test(j.file)) throw new Error("Audio is not ready."); return containedFile(await this.folder(p, id), j.file, MAX_AUDIO_BYTES); }
  /** Saves a ready result as a project clip, with its prompt, model and seed as provenance. */
  async keep(p: ProjectSnapshot, id: string, name: unknown) {
    const folder = await this.folder(p, id);
    return this.serial(folder, async () => {
      const j = await this.get(p, id);
      if (j.savedPath) throw new Error(`Already saved as ${j.savedPath}.`);
      if (typeof name !== "string" || !name.trim()) throw new Error("Name the clip.");
      const bytes = await readFile(await this.artifact(p, id));
      const clip = await importClip(p, { name: name.trim().slice(0, 64), category: AUDIO_MODE_CATEGORY[j.mode], bytes, description: j.text.slice(0, 500), generation: { model: j.endpoint, mode: j.mode, prompt: j.text.slice(0, 2000), ...(j.settings.seed !== undefined ? { seed: j.seed } : {}), ...(j.voice ? { voice: j.voice } : {}), ...(j.duration !== undefined ? { duration_seconds: j.duration } : {}), ...(j.loop ? { loop: true } : {}), ...(j.instrumental ? { instrumental: true } : {}), job: j.id, created_at: j.createdAt } });
      j.savedPath = clip.path; await this.persist(p, j);
      return { job: j, clip };
    });
  }
  async discard(p: ProjectSnapshot, id: string) {
    const folder = await this.folder(p, id);
    return this.serial(folder, async () => {
      const j = await this.get(p, id);
      if (j.state !== "ready" && j.state !== "failed" && j.state !== "uncertain") throw new Error("Wait for the result before deleting it.");
      j.deleted = true; await this.persist(p, j);
      if (j.file) await unlink(path.join(folder, j.file)).catch(e => { if (!missing(e)) throw e; });
      return { id };
    });
  }
}
const state = globalThis as typeof globalThis & { emergentAudioJobs?: AudioJobs };
export const audioJobs = state.emergentAudioJobs instanceof AudioJobs ? state.emergentAudioJobs : (state.emergentAudioJobs = new AudioJobs());

/** MCP `generate_audio`: one paid request, waited for (bounded), then kept as a clip. */
export async function generateAudioClip(project: ProjectSnapshot, input: { mode: unknown; text: unknown; name: unknown; duration?: unknown; voice?: unknown; loop?: unknown; instrumental?: unknown; model?: unknown; seed?: unknown; confirm_paid?: unknown }, options: { jobs?: AudioJobs; timeoutMs?: number; pollMs?: number } = {}) {
  if (input.confirm_paid !== true) throw new Error("generate_audio makes a paid fal request. Ask the user first, then pass confirm_paid: true.");
  if (typeof input.name !== "string" || !input.name.trim() || input.name.length > 64) throw new Error("Name the clip (1-64 characters).");
  const jobs = options.jobs ?? audioJobs, id = randomUUID(), deadline = Date.now() + (options.timeoutMs ?? 150000), poll = options.pollMs ?? 3000;
  const request = { mode: input.mode, text: input.text, seed: input.seed ?? Math.floor(Math.random() * 2147483000), duration: input.duration, voice: input.voice, loop: input.loop, instrumental: input.instrumental, model: input.model };
  let job = await jobs.create(project, id, Object.fromEntries(Object.entries(request).filter(([, value]) => value !== undefined)));
  while ((job.state === "queued" || job.state === "generating") && Date.now() < deadline) {
    await new Promise(resolve => setTimeout(resolve, poll));
    job = await jobs.refresh(project, id);
  }
  if (job.state !== "ready") return { generated: false, job: id, state: job.state, error: job.error, note: job.state === "failed" || job.state === "uncertain" ? "Nothing was saved." : "Still generating. It appears in the Audio editor's Generate tab to keep later." };
  const { clip } = await jobs.keep(project, id, input.name);
  return { generated: true, ref: `asset:${clip.id}`, path: clip.path, name: clip.settings.name, category: clip.settings.category, duration_seconds: clip.info?.duration_seconds, model: job.endpoint, usage: "Play it from an object with set_component type emergent.audio_source, fields { clip: this ref }." };
}
