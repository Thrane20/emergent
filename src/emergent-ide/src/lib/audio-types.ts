// Audio clip assets (`*.audio.toml` beside their audio file), shared by the editor and server.
export const AUDIO_DIRECTORY = "assets/audio";
export const MAX_AUDIO_BYTES = 50 * 1024 * 1024;
export const AUDIO_CATEGORIES = ["sfx", "music", "voice", "ui"] as const;
export type AudioCategory = (typeof AUDIO_CATEGORIES)[number];
export type AudioClipSettings = { name: string; description: string; category: AudioCategory; gain_db: number; trim: [number, number] | null; loop: [number, number] | null; fade_in: number; fade_out: number; /** null: music streams, other categories load whole. */ stream: boolean | null };
export type AudioInfo = { channels: number; sample_rate: number; frames: number; duration_seconds: number; peak: number; /** Peak per slice, 0..1, as the engine plays it. */ waveform: number[] };
export type AudioClipSummary = { path: string; id: string; /** Changes whenever the clip file does. */ revision: string; source: string; sourcePath: string; settings: AudioClipSettings; generation?: Record<string, unknown>; usedBy: string[]; error?: string };
export type AudioClip = AudioClipSummary & { info?: AudioInfo };

/** Sniffs the container from its first bytes; the extension is never trusted. */
export function audioExtension(bytes: Uint8Array): "wav" | "mp3" | "ogg" | "flac" | null {
  const ascii = (start: number, end: number) => String.fromCharCode(...bytes.subarray(start, end));
  if (ascii(0, 4) === "RIFF" && ascii(8, 12) === "WAVE") return "wav";
  if (ascii(0, 4) === "OggS") return "ogg";
  if (ascii(0, 4) === "fLaC") return "flac";
  if (ascii(0, 3) === "ID3" || (bytes[0] === 0xff && (bytes[1] & 0xe0) === 0xe0)) return "mp3";
  return null;
}

const finite = (value: unknown, fallback: number) => typeof value === "number" && Number.isFinite(value) ? value : fallback;
function pair(value: unknown, label: string, strict: boolean): [number, number] | null {
  if (value === undefined || value === null) return null;
  if (Array.isArray(value) && value.length === 2 && value.every(n => typeof n === "number" && Number.isFinite(n)) && value[0] >= 0 && value[0] < value[1]) return [value[0], value[1]];
  if (strict) throw new Error(`${label} must be [start, end] seconds with 0 <= start < end.`);
  return null;
}

/** Reads clip settings from a parsed file or a request. `strict` rejects what the engine would reject. */
export function audioSettings(value: unknown, strict = false): AudioClipSettings {
  const v = (value && typeof value === "object" ? value : {}) as Record<string, unknown>;
  const name = typeof v.name === "string" ? v.name.trim() : "";
  const description = typeof v.description === "string" ? v.description : "";
  const category = AUDIO_CATEGORIES.includes(v.category as AudioCategory) ? v.category as AudioCategory : "sfx";
  const settings: AudioClipSettings = { name, description, category, gain_db: finite(v.gain_db, 0), trim: pair(v.trim, "Trim", strict), loop: pair(v.loop, "Loop", strict), fade_in: finite(v.fade_in, 0), fade_out: finite(v.fade_out, 0), stream: typeof v.stream === "boolean" ? v.stream : null };
  if (strict) {
    if (!name || name.length > 64 || /[\u0000-\u001f]/.test(name)) throw new Error("Name the clip (1-64 characters).");
    if (description.length > 500) throw new Error("Keep the description under 500 characters.");
    if (v.category !== undefined && settings.category !== v.category) throw new Error("Choose sfx, music, voice or ui.");
    if (settings.gain_db < -60 || settings.gain_db > 24) throw new Error("Gain must be within -60..24 dB.");
    if ([settings.fade_in, settings.fade_out].some(fade => fade < 0 || fade > 60)) throw new Error("Fades must be within 0..60 seconds.");
    if (settings.trim && settings.loop && settings.loop[1] > settings.trim[1] - settings.trim[0] + 1e-6) throw new Error("The loop must end inside the trimmed clip.");
  }
  return settings;
}

export type AudioMode = "sound" | "music" | "speech";
export const AUDIO_MODE_CATEGORY: Record<AudioMode, AudioCategory> = { sound: "sfx", music: "music", speech: "voice" };
/** Editor duration bounds per mode, in seconds; a chosen model's own limits apply on top. */
export const AUDIO_MODE_DURATION: Record<Exclude<AudioMode, "speech">, { min: number; max: number; default: number }> = { sound: { min: 0.5, max: 22, default: 3 }, music: { min: 5, max: 300, default: 30 } };
export type AudioJobRequest = { mode: AudioMode; text: string; seed: number; duration?: number; voice?: string; loop?: boolean; instrumental?: boolean; model?: string };
export type AudioJob = AudioJobRequest & { schema_version: 1; id: string; projectId: string; endpoint: string; settings: Record<string, unknown>; requestId: string | null; state: "submitting" | "uncertain" | "queued" | "generating" | "ready" | "failed"; error: string | null; createdAt: string; file?: string; savedPath?: string; deleted?: boolean };

export function audioJobRequest(value: unknown, modelId: (value: unknown) => string): AudioJobRequest {
  const v = (value && typeof value === "object" ? value : {}) as Record<string, unknown>;
  if (v.mode !== "sound" && v.mode !== "music" && v.mode !== "speech") throw new Error("Choose sound effect, music or speech.");
  const mode = v.mode, text = typeof v.text === "string" ? v.text.trim() : "";
  if (text.length < 3 || text.length > 2000 || text.includes("\0")) throw new Error(mode === "speech" ? "Write the line to speak (3-2000 characters)." : "Describe the sound (3-2000 characters).");
  if (typeof v.seed !== "number" || !Number.isInteger(v.seed) || v.seed < 0 || v.seed > 2147483647) throw new Error("Use an integer seed.");
  const request: AudioJobRequest = { mode, text, seed: v.seed };
  if (mode !== "speech") {
    const bounds = AUDIO_MODE_DURATION[mode];
    if (typeof v.duration !== "number" || !Number.isFinite(v.duration) || v.duration < bounds.min || v.duration > bounds.max) throw new Error(`Choose a duration of ${bounds.min}-${bounds.max} seconds.`);
    request.duration = v.duration;
  }
  if (mode === "speech" && v.voice !== undefined) {
    if (typeof v.voice !== "string" || !/^[\w .'-]{1,64}$/.test(v.voice)) throw new Error("Choose a voice name.");
    request.voice = v.voice.trim();
  }
  if (mode === "sound" && v.loop === true) request.loop = true;
  if (mode === "music" && v.instrumental === true) request.instrumental = true;
  if (v.model !== undefined) request.model = modelId(v.model);
  return request;
}
