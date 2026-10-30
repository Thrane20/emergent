import "server-only";
import { createHash, randomInt, randomUUID } from "node:crypto";
import { copyFile, mkdir, readFile, readdir, realpath, rename, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import type { ProjectSnapshot } from "./project";
import { containedFile, ideDataDirectory, imageExtension, missing, readModel, validateGlb, within, writeJson } from "./object-files";
import { falProvider, ProviderFailure } from "./object-provider";
import { GENERATION_SETTINGS, MAX_IMAGE_BYTES, OBJECT_ENDPOINT, ObjectCreatorError, type ObjectAsset, type ObjectJob } from "./object-types";

type ProjectScope = Pick<ProjectSnapshot, "id" | "root" | "assetRoots">;
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const now = () => new Date().toISOString();

export class ObjectJobs {
  private locks = new Map<string, Promise<unknown>>();
  constructor(private dataDirectory = ideDataDirectory(), private provider = falProvider) {}

  private async directory(project: ProjectScope) {
    const root = await realpath(project.root);
    const identity = createHash("sha256").update(`${project.id}\0${root}`).digest("hex");
    return path.join(this.dataDirectory, "object-jobs", identity);
  }

  private async folder(project: ProjectScope, id: string) {
    if (!uuid.test(id)) throw new ObjectCreatorError("Invalid object job ID.");
    return path.join(await this.directory(project), id);
  }

  private async serial<T>(key: string, action: () => Promise<T>): Promise<T> {
    const previous = this.locks.get(key) ?? Promise.resolve();
    const operation = previous.catch(() => undefined).then(action);
    this.locks.set(key, operation);
    try { return await operation; }
    finally { if (this.locks.get(key) === operation) this.locks.delete(key); }
  }

  async list(project: ProjectScope) {
    let ids: string[];
    try { ids = (await readdir(await this.directory(project))).filter((id) => uuid.test(id)); }
    catch (error) { if (missing(error)) return []; throw error; }
    const jobs = await Promise.all(ids.map(async (id) => {
      try { return await this.get(project, id); }
      catch (error) { if (error instanceof ObjectCreatorError && error.status === 404) return null; throw error; }
    }));
    return jobs.filter((job): job is ObjectJob => job !== null).sort((a, b) => b.createdAt.localeCompare(a.createdAt)).slice(0, 100);
  }

  async get(project: ProjectScope, id: string): Promise<ObjectJob> {
    try {
      const job = JSON.parse(await readFile(path.join(await this.folder(project, id), "job.json"), "utf8")) as ObjectJob;
      if (job.schema_version !== 1 || job.id !== id || job.projectId !== project.id) throw new ObjectCreatorError("Invalid object job journal.");
      return job;
    } catch (error) { if (missing(error)) throw new ObjectCreatorError("Object job was not found.", 404); throw error; }
  }

  private async persist(project: ProjectScope, job: ObjectJob) {
    job.updatedAt = now();
    await writeJson(path.join(await this.folder(project, job.id), "job.json"), job);
    return job;
  }

  async create(project: ProjectScope, id: string, name: string, bytes: Buffer) {
    const folder = await this.folder(project, id);
    return this.serial(folder, async () => {
      try { return await this.get(project, id); }
      catch (error) { if (!(error instanceof ObjectCreatorError && error.status === 404)) throw error; }
      const normalized = validateObjectName(name);
      const extension = imageExtension(bytes);
      const provider = await this.provider(); // Fail missing credentials before making a job.
      await mkdir(folder, { recursive: true, mode: 0o700 });
      // A durable exclusive claim prevents a duplicate paid call across module reloads.
      try { await writeFile(path.join(folder, "submission.claim"), "claimed\n", { flag: "wx", mode: 0o600 }); }
      catch { throw new ObjectCreatorError("This submission is already being recorded. Refresh its history.", 409); }
      const job: ObjectJob = {
        schema_version: 1, id, projectId: project.id, name: normalized, state: "uploading",
        seed: randomInt(0, 2 ** 31), endpoint: OBJECT_ENDPOINT, settings: GENERATION_SETTINGS,
        requestId: null, reference: `reference.${extension}`, createdAt: now(), updatedAt: now(), error: null, savedPath: null,
      };
      await writeFile(path.join(folder, job.reference), bytes, { mode: 0o600 });
      await this.persist(project, job);
      let imageUrl: string;
      try { imageUrl = await provider.upload(bytes, extension); }
      catch { job.state = "failed"; job.error = "Reference upload failed. Check your fal key, account credit, and image, then start a new attempt."; return this.persist(project, job); }
      job.state = "submitting";
      await this.persist(project, job);
      try {
        job.requestId = await provider.submit(imageUrl, job.seed);
        job.state = "queued";
      } catch {
        job.state = "uncertain";
        job.error = "Submission could not be confirmed. Check your fal dashboard before starting another paid attempt; this job will not be resubmitted.";
      }
      return this.persist(project, job);
    });
  }

  async refresh(project: ProjectScope, id: string) {
    const folder = await this.folder(project, id);
    return this.serial(folder, async () => {
      const job = await this.get(project, id);
      if (job.state === "uploading") {
        job.state = "failed"; job.error = "The IDE restarted during image upload. Start a new attempt.";
      } else if (job.state === "submitting") {
        job.state = "uncertain"; job.error = "The IDE restarted before confirming submission. Check the fal dashboard before a new attempt.";
      } else if (job.requestId && ["queued", "generating", "downloading"].includes(job.state)) {
        try {
          const provider = await this.provider();
          const status = await provider.status(job.requestId);
          job.error = null;
          if (status === "completed") {
            job.state = "downloading";
            await this.persist(project, job);
            const bytes = await provider.result(job.requestId);
            validateGlb(bytes);
            await writeFile(path.join(folder, "model.glb.tmp"), bytes, { mode: 0o600 });
            await rename(path.join(folder, "model.glb.tmp"), path.join(folder, "model.glb"));
            job.state = "ready";
          } else { job.state = status; }
        } catch (error) {
          if (error instanceof ObjectCreatorError && error.status !== 503 || error instanceof ProviderFailure && error.terminal) {
            job.state = "failed";
          }
          job.error = error instanceof ObjectCreatorError || error instanceof ProviderFailure ? error.message : "Unable to check or download this request. Check again to resume.";
        }
      }
      return this.persist(project, job);
    });
  }

  async artifact(project: ProjectScope, id: string, kind: string) {
    const job = await this.get(project, id);
    if (kind !== "reference" && kind !== "model") throw new ObjectCreatorError("Unknown artifact.");
    if (kind === "model" && job.state !== "ready") throw new ObjectCreatorError("Model is not ready.", 409);
    const relative = kind === "model" ? "model.glb" : job.reference;
    return containedFile(await this.folder(project, id), relative, kind === "model" ? 100 * 1024 * 1024 : MAX_IMAGE_BYTES);
  }

  async save(project: ProjectScope, id: string, requestedName: string) {
    const key = await this.directory(project);
    return this.serial(`${key}/save`, async () => {
      const job = await this.get(project, id);
      if (job.state !== "ready" || !job.requestId) throw new ObjectCreatorError("Generate a valid model before saving.", 409);
      if (job.savedPath) return job;
      const name = validateObjectName(requestedName);
      const root = await realpath(project.root);
      const declaredRoot = project.assetRoots[0];
      if (!declaredRoot || path.isAbsolute(declaredRoot) || !within(root, path.resolve(root, declaredRoot))) throw new ObjectCreatorError("Project has no safe asset root.");
      const assets = await realpath(path.resolve(root, declaredRoot));
      if (!within(root, assets)) throw new ObjectCreatorError("Asset root escapes the project.", 403);
      const models = path.join(assets, "models");
      await mkdir(models, { recursive: true });
      if (!within(assets, await realpath(models))) throw new ObjectCreatorError("Models folder escapes the asset root.", 403);
      const target = path.join(models, id);
      const lock = path.join(models, ".object-save-lock");
      try { await mkdir(lock); }
      catch { throw new ObjectCreatorError("Another object save is active. If the IDE crashed, remove assets/models/.object-save-lock before retrying.", 409); }
      let staging: string | undefined;
      try {
        try {
          const existing = JSON.parse(await readFile(path.join(target, "object.asset.json"), "utf8")) as ObjectAsset;
          if (existing.jobId !== id) throw new ObjectCreatorError("An object with this ID already exists.", 409);
          job.savedPath = path.relative(root, path.join(target, "model.glb")).split(path.sep).join("/");
          return this.persist(project, job);
        } catch (error) { if (!missing(error)) throw error; }
        // Any existing directory, including one without a descriptor, is a collision.
        try { await realpath(target); throw new ObjectCreatorError("An object with this ID already exists.", 409); }
        catch (error) { if (!missing(error)) throw error; }
        const source = await this.artifact(project, id, "model");
        await readModel(source);
        const reference = await this.artifact(project, id, "reference");
        const stagingRoot = path.join(root, ".emergent-object-staging");
        await mkdir(stagingRoot, { recursive: true });
        if (!within(root, await realpath(stagingRoot))) throw new ObjectCreatorError("Staging folder escapes the project.", 403);
        staging = path.join(stagingRoot, randomUUID());
        await mkdir(staging);
        const relative = (file: string) => path.relative(root, path.join(target, file)).split(path.sep).join("/");
        const descriptor: ObjectAsset = {
          schema_version: 1, kind: "generated-model", id, jobId: id, name,
          model: relative("model.glb"), reference: relative(job.reference), provider: "fal",
          endpoint: job.endpoint, requestId: job.requestId, modelRevision: null, seed: job.seed,
          settings: job.settings, createdAt: job.createdAt, savedAt: now(),
        };
        await copyFile(source, path.join(staging, "model.glb"));
        await copyFile(reference, path.join(/* turbopackIgnore: true */ staging, job.reference));
        await writeFile(path.join(staging, "object.asset.json"), JSON.stringify(descriptor, null, 2) + "\n");
        await rename(staging, target);
        staging = undefined;
        job.savedPath = descriptor.model;
        return this.persist(project, job);
      } finally {
        if (staging) await rm(staging, { recursive: true, force: true });
        await rm(lock, { recursive: true, force: true });
      }
    });
  }
}

export function validateObjectName(name: string) {
  const normalized = name.trim();
  if (!/^[a-zA-Z0-9][a-zA-Z0-9 _-]{0,63}$/.test(normalized)) throw new ObjectCreatorError("Use 1–64 letters, numbers, spaces, underscores, or hyphens for the object name.");
  const slug = normalized.toLowerCase().replace(/[ _]+/g, "-");
  if (/^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/.test(slug)) throw new ObjectCreatorError("Choose a name that is portable across operating systems.");
  return normalized;
}

const globalJobs = globalThis as typeof globalThis & { emergentObjectJobs?: ObjectJobs };
export const objectJobs = globalJobs.emergentObjectJobs ??= new ObjectJobs();
