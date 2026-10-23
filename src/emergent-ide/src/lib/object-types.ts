export const OBJECT_ENDPOINT = "fal-ai/trellis";
export const GENERATION_SETTINGS = {
  texture_size: 1024, ss_guidance_strength: 7.5, ss_sampling_steps: 12,
  slat_guidance_strength: 3, slat_sampling_steps: 12, mesh_simplify: 0.95,
} as const;
export const MAX_IMAGE_BYTES = 10 * 1024 * 1024;
export const MAX_MODEL_BYTES = 100 * 1024 * 1024;

export type ObjectJobState = "uploading" | "submitting" | "uncertain" | "queued" | "generating" | "downloading" | "ready" | "failed";
export type ObjectJob = {
  schema_version: 1;
  id: string;
  projectId: string;
  name: string;
  state: ObjectJobState;
  seed: number;
  endpoint: string;
  settings: typeof GENERATION_SETTINGS;
  requestId: string | null;
  reference: string;
  createdAt: string;
  updatedAt: string;
  error: string | null;
  savedPath: string | null;
};

export type ObjectAsset = {
  schema_version: 1;
  kind: "generated-model";
  id: string;
  jobId: string;
  name: string;
  model: string;
  reference: string;
  provider: "fal";
  endpoint: string;
  requestId: string;
  modelRevision: string | null;
  seed: number;
  settings: typeof GENERATION_SETTINGS;
  createdAt: string;
  savedAt: string;
};
export type ObjectSettings = { configured: boolean; keyHint: string | null; source: "environment" | "settings"; endpoint: string };

export class ObjectCreatorError extends Error {
  constructor(message: string, readonly status = 400) { super(message); }
}
