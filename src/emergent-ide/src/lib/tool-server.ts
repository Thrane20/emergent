import "server-only";
import { execFile, spawn, type ChildProcess } from "node:child_process";
import { readdir, stat } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import type { ProjectSnapshot } from "./project";
import { engineRoot, projectTargetDir } from "./workspace";

const execute = promisify(execFile);
const TIMEOUT = 120_000;

/**
 * One resident `--tool-server` process per game project. It answers tool commands without a
 * `cargo run` (about 1 s of cargo checking) per call. It is rebuilt and restarted when Rust
 * source or manifests change, in the game, its plugins or the engine; scene and asset edits
 * need no restart because the server re-reads the project for every request.
 */
type Server = { stamp: string; child: ChildProcess; queue: Promise<unknown>; buffer: Buffer; stderr: string; pending?: { resolve: (text: string) => void; reject: (error: Error) => void; timer: NodeJS.Timeout } };
const state = globalThis as typeof globalThis & { emergentToolServers?: Map<string, Promise<Server>> };
const servers = (state.emergentToolServers ??= new Map<string, Promise<Server>>());

/** Newest change among the inputs a rebuild depends on. */
async function newest(directory: string, depth = 0): Promise<number> {
  if (depth > 8) return 0;
  let latest = 0;
  for (const entry of await readdir(directory, { withFileTypes: true }).catch(() => [])) {
    if (entry.name === "target" || entry.name.startsWith(".")) continue;
    const target = path.join(directory, entry.name);
    if (entry.isDirectory()) latest = Math.max(latest, await newest(target, depth + 1));
    else if (entry.name.endsWith(".rs") || entry.name === "Cargo.toml") latest = Math.max(latest, (await stat(target)).mtimeMs);
  }
  return latest;
}
async function sourceStamp(root: string) {
  const file = (name: string) => stat(name).then(s => s.mtimeMs, () => 0);
  const parts = await Promise.all([newest(path.join(root, "src")), newest(path.join(root, "plugins")), newest(path.join(engineRoot, "src")), file(path.join(root, "Cargo.toml")), file(path.join(root, "Cargo.lock")), file(path.join(root, "emergent.toml")), file(path.join(engineRoot, "Cargo.toml"))]);
  return parts.join(":");
}

async function start(root: string, stamp: string): Promise<Server> {
  // Build once and find the game binary; a compile error reports like `cargo run` did.
  let built: string;
  try {
    built = (await execute("cargo", ["build", "--quiet", "--message-format=json", "--manifest-path", path.join(root, "Cargo.toml"), "--target-dir", projectTargetDir(root)], { cwd: root, shell: false, timeout: 600_000, maxBuffer: 256 * 1024 * 1024, windowsHide: true })).stdout;
  } catch (error) {
    const detail = error && typeof error === "object" && "stderr" in error ? String(error.stderr).trim().slice(-6000) : "";
    throw new Error(detail || "The game could not be built.");
  }
  const manifest = path.join(root, "Cargo.toml");
  const binary = built.split("\n").flatMap(line => {
    try {
      const message = JSON.parse(line) as { reason?: string; executable?: string | null; manifest_path?: string; target?: { kind?: string[] } };
      return message.reason === "compiler-artifact" && message.executable && path.resolve(message.manifest_path ?? "") === manifest && message.target?.kind?.includes("bin") ? [message.executable] : [];
    } catch { return []; }
  }).at(-1);
  if (!binary) throw new Error("The game build produced no binary.");
  const child = spawn(binary, ["--tool-server"], { cwd: root, shell: false, stdio: ["pipe", "pipe", "pipe"], windowsHide: true });
  const server: Server = { stamp, child, queue: Promise.resolve(), buffer: Buffer.alloc(0), stderr: "" };
  const fail = (error: Error) => {
    if (server.pending) { clearTimeout(server.pending.timer); server.pending.reject(error); server.pending = undefined; }
    void servers.get(root)?.then(current => { if (current === server) servers.delete(root); }, () => undefined);
  };
  child.stderr!.on("data", (chunk: Buffer) => { server.stderr = (server.stderr + chunk.toString()).slice(-6000); });
  child.stdout!.on("data", (chunk: Buffer) => {
    server.buffer = Buffer.concat([server.buffer, chunk]);
    const newline = server.buffer.indexOf(10);
    if (newline < 0 || !server.pending) return;
    const [status, size] = server.buffer.subarray(0, newline).toString().split(" ");
    const end = newline + 1 + Number(size);
    if (server.buffer.length < end) return;
    const body = server.buffer.subarray(newline + 1, end).toString();
    server.buffer = server.buffer.subarray(end);
    const { resolve, reject, timer } = server.pending;
    server.pending = undefined;
    clearTimeout(timer);
    if (status === "ok") resolve(body); else reject(new Error(body));
  });
  child.on("error", error => fail(error));
  child.on("exit", () => fail(new Error(server.stderr.trim() || "The game tool server stopped.")));
  // An idle server must not keep Node running (a script or test would never exit). A request in
  // flight is kept alive by its timeout timer; when Node exits, the closed pipe stops the server.
  child.unref();
  for (const stream of [child.stdin, child.stdout, child.stderr]) (stream as unknown as { unref?: () => void } | null)?.unref?.();
  return server;
}

async function server(root: string) {
  const stamp = await sourceStamp(root);
  const current = servers.get(root);
  const running = await current?.catch(() => undefined);
  if (current && running?.stamp === stamp) return running;
  if (current && !running) servers.delete(root);
  // Rust changed: retire the old server once it has answered what it was asked.
  if (running) void running.queue.finally(() => running.child.stdin?.end());
  const next = start(root, stamp);
  servers.set(root, next);
  next.catch(() => { if (servers.get(root) === next) servers.delete(root); });
  return next;
}

/** Runs a tool command on the project's resident server and returns its output. */
export async function serveTool(project: ProjectSnapshot, args: string[]): Promise<string> {
  if (args.some(arg => /[\t\n\r]/.test(arg))) throw new Error("Tool arguments cannot contain tabs or newlines.");
  const running = await server(project.root);
  const reply = running.queue.then(() => new Promise<string>((resolve, reject) => {
    const timer = setTimeout(() => { reject(new Error("The game tool timed out.")); running.child.kill(); }, TIMEOUT);
    running.pending = { resolve, reject, timer };
    running.child.stdin!.write(`${args.join("\t")}\n`);
  }));
  running.queue = reply.catch(() => undefined);
  return reply;
}
