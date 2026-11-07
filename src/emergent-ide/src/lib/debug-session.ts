import "server-only";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { randomUUID } from "node:crypto";
import path from "node:path";
import { parse } from "smol-toml";
import type { ProjectSnapshot } from "./project";
import { projectScenes } from "./project-scenes";
import { projectTargetDir } from "./workspace";

type Binding = Pick<ProjectSnapshot, "id" | "root">;
export type DebugFrame = { sequence: number; tick: number; png: Buffer };
/** Live state from the game (`--debug-scene`): tick, pause, speed and the selected object. */
export type DebugState = { tick: number; paused: boolean; speed: number; selected?: Record<string, unknown>; spawned?: { id: string; name: string; parent?: string }[] };
export type DebugStatus = { id: string; scene: string; state: Session["state"]; error: string | null; log: string; live: DebugState | null };
type Session = {
  id: string; projectId: string; root: string; scene: string; state: "building" | "running" | "stopping" | "stopped" | "failed";
  child: ChildProcessWithoutNullStreams; frame: DebugFrame | null; live: DebugState | null; log: string; error: string | null;
  seq: number; used: number; waiters: Set<() => void>; buffer: Buffer;
  pending: Map<number, { resolve: () => void; reject: (error: Error) => void }>; lease: NodeJS.Timeout;
};
const MAX_PACKET = 4 * 1024 * 1024;
/** The page polls frames continuously; a session nobody watches for this long is stopped. */
const IDLE_MS = 30_000;
const KEYS = new Set(["w", "a", "s", "d", "q", "e", "space", "shift", "up", "down", "left", "right"]);

/** One TOML command line for the game, validated here so the game only sees well-formed input. */
export function debugCommand(value: unknown, seq: number): string {
  if (!value || typeof value !== "object") throw new Error("A debug command is required.");
  const input = value as Record<string, unknown>;
  let fields: string;
  if (input.op === "pause" || input.op === "resume") fields = `op="${input.op}"`;
  else if (input.op === "step") {
    if (!Number.isInteger(input.ticks) || (input.ticks as number) < 1 || (input.ticks as number) > 600) throw new Error("Step 1-600 ticks.");
    fields = `op="step", ticks=${input.ticks}`;
  } else if (input.op === "speed") {
    if (typeof input.factor !== "number" || !(input.factor >= 0.1 && input.factor <= 8)) throw new Error("Speed must be 0.1-8.");
    fields = `op="speed", factor=${input.factor}`;
  } else if (input.op === "select") {
    if (input.id !== null && (typeof input.id !== "string" || !/^[a-z0-9_.-]{1,128}$/i.test(input.id))) throw new Error("Invalid scene instance ID.");
    fields = `op="select"${input.id === null ? "" : `, id=${JSON.stringify(input.id)}`}`;
  } else if (input.op === "keys") {
    if (!Array.isArray(input.held) || input.held.length > 16 || !input.held.every(key => typeof key === "string" && KEYS.has(key))) throw new Error("Unsupported keys.");
    fields = `op="keys", held=${JSON.stringify(input.held)}`;
  } else if (input.op === "mouse") {
    const valid = (n: unknown) => typeof n === "number" && Number.isFinite(n) && Math.abs(n) <= 10_000;
    if (!valid(input.dx) || !valid(input.dy)) throw new Error("Mouse motion must be finite pixels.");
    fields = `op="mouse", dx=${input.dx}, dy=${input.dy}`;
  } else throw new Error("Unsupported debug command.");
  return `request={seq=${seq}, ${fields}}\n`;
}

export class DebugSessions {
  private active: Session | null = null;

  async start(project: ProjectSnapshot, scene: string): Promise<DebugStatus> {
    if (!projectScenes(project).some(item => item.path === scene)) throw new Error("Choose a scene from the linked project.");
    if (this.active) await this.terminate(this.active);
    // Optimised like inspection (shares its build): frame capture and encoding are too slow unoptimised.
    const child = spawn("cargo", ["run", "--release", "--quiet", "--manifest-path", path.join(project.root, "Cargo.toml"), "--target-dir", projectTargetDir(project.root), "--", "--debug-scene", scene], { cwd: project.root, shell: false, detached: process.platform !== "win32", stdio: ["pipe", "pipe", "pipe"] });
    const s: Session = {
      id: randomUUID(), projectId: project.id, root: project.root, scene, state: "building", child, frame: null, live: null, log: "", error: null,
      seq: 0, used: Date.now(), waiters: new Set(), buffer: Buffer.alloc(0), pending: new Map(),
      lease: setInterval(() => { if (Date.now() - s.used > IDLE_MS) void this.terminate(s); }, 5000),
    };
    s.lease.unref();
    this.active = s;
    child.stdout.on("data", (chunk: Buffer) => { try { this.decode(s, chunk); } catch (error) { this.fail(s, error instanceof Error ? error.message : "Invalid game output."); } });
    child.stderr.on("data", (chunk: Buffer) => { s.log = (s.log + chunk.toString()).slice(-16000); });
    child.stdin.on("error", error => { if (!["stopping", "stopped"].includes(s.state)) this.fail(s, error.message); });
    child.on("error", error => this.fail(s, error.message));
    child.on("close", () => {
      clearInterval(s.lease);
      if (s.state === "stopping") s.state = "stopped";
      else if (s.state !== "failed" && s.state !== "stopped") { s.state = "failed"; s.error = s.log.trim().split("\n").slice(-12).join("\n") || "The game stopped."; }
      this.settle(s);
    });
    return this.status(project, s.id);
  }

  status(project: Binding, id: string): DebugStatus {
    const s = this.get(project, id);
    return { id: s.id, scene: s.scene, state: s.state, error: s.error, log: s.log.slice(-4000), live: s.live };
  }

  async command(project: Binding, id: string, value: unknown) {
    const s = this.get(project, id);
    if (s.state !== "running") throw new Error(s.error || "The debug run is not running yet.");
    if (s.pending.size >= 16) throw new Error("Debug command queue is full.");
    const seq = ++s.seq, line = debugCommand(value, seq);
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => { s.pending.delete(seq); reject(new Error("The game did not acknowledge the command.")); }, 10_000);
      s.pending.set(seq, { resolve: () => { clearTimeout(timer); resolve(); }, reject: error => { clearTimeout(timer); reject(error); } });
      s.child.stdin.write(line);
    });
    return this.status(project, id);
  }

  async frame(project: Binding, id: string, after: number, signal: AbortSignal): Promise<DebugFrame | null> {
    const s = this.get(project, id);
    if (s.frame && s.frame.sequence > after) return s.frame;
    if (!["building", "running"].includes(s.state)) throw new Error(s.error || "The debug run stopped.");
    if (s.waiters.size >= 4) throw new Error("Too many debug frame subscribers.");
    await new Promise<void>(resolve => {
      const done = () => { clearTimeout(timer); signal.removeEventListener("abort", done); s.waiters.delete(done); resolve(); };
      const timer = setTimeout(done, 10_000); s.waiters.add(done); signal.addEventListener("abort", done, { once: true }); if (signal.aborted) done();
    });
    if (s.state === "failed") throw new Error(s.error || "The debug run failed.");
    return s.frame && s.frame.sequence > after ? s.frame : null;
  }

  /** Stopping a run that has already ended (or was replaced by a newer one) succeeds quietly. */
  async stop(project: Binding, id: string) {
    const s = this.active;
    if (!s || s.id !== id || s.projectId !== project.id || s.root !== project.root) return { id, stopped: true };
    await this.terminate(s);
    return this.status(project, id);
  }
  async stopActive() { if (this.active) await this.terminate(this.active); }

  /** `EMI1` packets: 0 frame (36-byte header + PNG), 1 acknowledgement, 2 live state TOML. */
  private decode(s: Session, chunk: Buffer) {
    if (s.buffer.length + chunk.length > MAX_PACKET * 2) throw new Error("Debug output exceeded its bounded buffer.");
    s.buffer = Buffer.concat([s.buffer, chunk]);
    while (s.buffer.length >= 12) {
      if (s.buffer.toString("ascii", 0, 4) !== "EMI1") throw new Error("This game build does not support Debug (--debug-scene). Rebuild it with the current engine.");
      const kind = s.buffer.readUInt32LE(4), size = s.buffer.readUInt32LE(8);
      if (size > MAX_PACKET || kind > 2) throw new Error("Invalid debug packet.");
      if (s.buffer.length < size + 12) return;
      const bytes = s.buffer.subarray(12, size + 12);
      s.buffer = s.buffer.subarray(size + 12);
      if (kind === 0) {
        if (bytes.length < 44) throw new Error("Invalid debug frame.");
        s.frame = { sequence: bytes.readUInt32LE(0), tick: Number(bytes.readBigUInt64LE(8)), png: Buffer.from(bytes.subarray(36)) };
        s.state = s.state === "building" ? "running" : s.state;
        for (const notify of [...s.waiters]) notify();
      } else if (kind === 1) {
        const seq = bytes.readUInt32LE(0), failed = bytes.readUInt32LE(4) !== 0, pending = s.pending.get(seq);
        if (pending) { s.pending.delete(seq); if (failed) pending.reject(new Error(bytes.toString("utf8", 8))); else pending.resolve(); }
      } else s.live = parse(bytes.toString("utf8")) as unknown as DebugState;
    }
  }

  private get(project: Binding, id: string) {
    const s = this.active;
    if (!s || s.id !== id || s.projectId !== project.id || s.root !== project.root) throw new Error("This debug run has ended or belongs to another project.");
    s.used = Date.now();
    return s;
  }
  private settle(s: Session) {
    for (const item of s.pending.values()) item.reject(new Error(s.error || "The debug run stopped."));
    s.pending.clear();
    for (const notify of [...s.waiters]) notify();
  }
  private kill(s: Session, signal: NodeJS.Signals = "SIGTERM") {
    if (s.child.exitCode !== null || s.child.signalCode !== null) return;
    try { if (s.child.pid && process.platform !== "win32") process.kill(-s.child.pid, signal); else s.child.kill(signal); } catch { /* Already exited. */ }
  }
  private fail(s: Session, message: string) {
    if (["stopped", "stopping", "failed"].includes(s.state)) return;
    s.error = message; s.state = "failed"; clearInterval(s.lease); this.kill(s);
    setTimeout(() => this.kill(s, "SIGKILL"), 2000).unref();
    this.settle(s);
  }
  private async terminate(s: Session) {
    clearInterval(s.lease);
    if (["stopped", "failed"].includes(s.state)) { this.kill(s); return; }
    s.state = "stopping";
    this.settle(s);
    await new Promise<void>(resolve => {
      const done = () => { clearTimeout(timer); s.child.removeListener("close", done); s.state = "stopped"; this.settle(s); resolve(); };
      const timer = setTimeout(() => { this.kill(s, "SIGKILL"); done(); }, 3000);
      s.child.once("close", done);
      // A game still building has no command reader yet; stop the whole process group.
      if (s.frame && !s.child.stdin.destroyed) s.child.stdin.end(`request={seq=${++s.seq}, op="stop"}\n`); else this.kill(s);
    });
  }
}

const state = globalThis as typeof globalThis & { emergentDebugSessions?: DebugSessions };
export const debugSessions = (state.emergentDebugSessions ??= new DebugSessions());
// Keep owned processes across dev reloads while updating the manager's methods.
Object.setPrototypeOf(debugSessions, DebugSessions.prototype);
