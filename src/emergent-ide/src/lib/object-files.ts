import { randomUUID } from "node:crypto";
import { mkdir, readFile, realpath, rename, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { MAX_IMAGE_BYTES, MAX_MODEL_BYTES, ObjectCreatorError } from "./object-types";

export function ideDataDirectory() {
  return path.resolve(/* turbopackIgnore: true */ process.env.EMERGENT_IDE_DATA_DIR?.trim() || path.join(process.cwd(), ".data"));
}

export async function writeJson(target: string, value: unknown) {
  await mkdir(path.dirname(target), { recursive: true, mode: 0o700 });
  const temp = `${target}.${randomUUID()}.tmp`;
  await writeFile(temp, JSON.stringify(value, null, 2) + "\n", { mode: 0o600 });
  await rename(temp, target);
}

export function missing(error: unknown) {
  return error instanceof Error && "code" in error && error.code === "ENOENT";
}

export function within(root: string, target: string) {
  const relative = path.relative(root, target);
  return relative === "" || (!relative.startsWith(`..${path.sep}`) && relative !== ".." && !path.isAbsolute(relative));
}

export async function containedFile(root: string, relative: string, maxBytes: number) {
  if (!relative || path.isAbsolute(relative) || relative.includes("\\")) throw new ObjectCreatorError("Invalid asset path.");
  const canonicalRoot = await realpath(root);
  const target = await realpath(path.resolve(root, relative));
  if (!within(canonicalRoot, target)) throw new ObjectCreatorError("Asset path is outside its owning folder.", 403);
  const info = await stat(target);
  if (!info.isFile() || info.size > maxBytes) throw new ObjectCreatorError("Asset exceeds the preview size limit.", 413);
  return target;
}

export async function boundedBody(response: Response, maxBytes: number) {
  if (Number(response.headers.get("content-length")) > maxBytes) throw new ObjectCreatorError("File exceeds the size limit.", 413);
  const reader = response.body?.getReader();
  if (!reader) throw new ObjectCreatorError("File body is missing.");
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > maxBytes) throw new ObjectCreatorError("File exceeds the size limit.", 413);
      chunks.push(value);
    }
  } finally { await reader.cancel(); }
  return Buffer.concat(chunks, size);
}

export function imageExtension(bytes: Buffer) {
  if (!bytes.length || bytes.length > MAX_IMAGE_BYTES) throw new ObjectCreatorError("Use an image smaller than 10 MiB.", 413);
  if (bytes.subarray(0, 8).equals(Buffer.from([137,80,78,71,13,10,26,10]))) return "png";
  if (bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff) return "jpg";
  if (bytes.toString("ascii", 0, 4) === "RIFF" && bytes.toString("ascii", 8, 12) === "WEBP") return "webp";
  throw new ObjectCreatorError("Choose a PNG, JPEG, or WebP reference image.");
}

export function validateGlb(bytes: Buffer) {
  if (bytes.length < 20 || bytes.length > MAX_MODEL_BYTES || bytes.toString("ascii", 0, 4) !== "glTF" || bytes.readUInt32LE(4) !== 2 || bytes.readUInt32LE(8) !== bytes.length) {
    throw new ObjectCreatorError("The provider did not return a valid GLB 2 model.");
  }
  let offset = 12;
  let document: { asset?: { version?: string }; meshes?: unknown[]; buffers?: { uri?: string }[]; images?: { uri?: string }[] } | undefined;
  while (offset < bytes.length) {
    if (offset + 8 > bytes.length) throw new ObjectCreatorError("GLB chunk is truncated.");
    const size = bytes.readUInt32LE(offset);
    const type = bytes.readUInt32LE(offset + 4);
    if (size % 4 || offset + 8 + size > bytes.length) throw new ObjectCreatorError("GLB chunk has an invalid size.");
    if (offset === 12) {
      if (type !== 0x4e4f534a) throw new ObjectCreatorError("GLB JSON chunk is missing.");
      try { document = JSON.parse(bytes.toString("utf8", offset + 8, offset + 8 + size)); }
      catch { throw new ObjectCreatorError("GLB contains invalid JSON."); }
    }
    offset += 8 + size;
  }
  if (document?.asset?.version !== "2.0" || !Array.isArray(document.meshes) || !document.meshes.length || (document.buffers !== undefined && !Array.isArray(document.buffers)) || (document.images !== undefined && !Array.isArray(document.images))) throw new ObjectCreatorError("GLB contains no supported mesh or resource layout.");
  for (const resource of [...(document.buffers ?? []), ...(document.images ?? [])]) {
    if (!resource || (resource.uri !== undefined && (typeof resource.uri !== "string" || !resource.uri.startsWith("data:")))) throw new ObjectCreatorError("GLB must embed its textures and buffers.");
  }
}

export async function readModel(target: string) {
  if ((await stat(target)).size > MAX_MODEL_BYTES) throw new ObjectCreatorError("Model exceeds 100 MiB.", 413);
  const bytes = await readFile(target);
  validateGlb(bytes);
  return bytes;
}
