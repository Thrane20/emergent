import { spawn, type ChildProcess } from "node:child_process";
import { access } from "node:fs/promises";
import path from "node:path";
import type { ProjectSnapshot } from "@/lib/project";

const MAX_OUTPUT_CHARACTERS = 24 * 1024;

export type ProjectRunState =
  | "idle"
  | "building"
  | "running"
  | "stopping"
  | "succeeded"
  | "failed"
  | "stopped";

export type ProjectRunSnapshot = {
  state: ProjectRunState;
  projectId: string | null;
  startedAt: string | null;
  finishedAt: string | null;
  exitCode: number | null;
  output: string;
};

export class ProjectRunError extends Error {
  constructor(message: string, readonly status = 400) {
    super(message);
  }
}

class ProjectRunManager {
  private child: ChildProcess | null = null;
  private stopRequested = false;
  private snapshot: ProjectRunSnapshot = idleSnapshot();

  status(): ProjectRunSnapshot {
    return { ...this.snapshot };
  }

  async start(project: ProjectSnapshot): Promise<ProjectRunSnapshot> {
    if (isActive(this.snapshot.state)) {
      throw new ProjectRunError("The linked project is already running.", 409);
    }

    const manifest = path.join(project.root, "Cargo.toml");
    try {
      await access(manifest);
    } catch {
      throw new ProjectRunError("The linked project does not contain a Cargo.toml manifest.");
    }

    this.stopRequested = false;
    this.snapshot = {
      state: "building",
      projectId: project.id,
      startedAt: new Date().toISOString(),
      finishedAt: null,
      exitCode: null,
      output: `$ cargo run --manifest-path ${manifest}\n`,
    };

    const child = spawn("cargo", ["run", "--manifest-path", manifest], {
      cwd: project.root,
      env: process.env,
      shell: false,
      detached: process.platform !== "win32",
      stdio: ["ignore", "pipe", "pipe"],
    });
    this.child = child;
    child.stdout?.on("data", (chunk: Buffer) => this.append(chunk.toString()));
    child.stderr?.on("data", (chunk: Buffer) => this.append(chunk.toString()));
    child.on("spawn", () => this.append("Starting local development run…\n"));
    child.on("error", (error) => {
      this.append(`${error.message}\n`);
      this.finish("failed", null);
    });
    child.on("close", (code) => {
      this.finish(this.stopRequested ? "stopped" : code === 0 ? "succeeded" : "failed", code);
    });

    return this.status();
  }

  stop(projectId: string): ProjectRunSnapshot {
    if (this.snapshot.projectId !== projectId) {
      throw new ProjectRunError("The active run does not belong to the linked project.", 409);
    }
    if (!this.child || !isActive(this.snapshot.state)) return this.status();

    this.stopRequested = true;
    this.snapshot = { ...this.snapshot, state: "stopping" };
    try {
      if (this.child.pid && process.platform !== "win32") {
        process.kill(-this.child.pid, "SIGTERM");
      } else {
        this.child.kill("SIGTERM");
      }
    } catch (error) {
      this.append(`Unable to stop run: ${error instanceof Error ? error.message : "unknown error"}\n`);
      this.snapshot = { ...this.snapshot, state: "failed", finishedAt: new Date().toISOString() };
    }
    return this.status();
  }

  private append(text: string) {
    const state = /\bRunning\s+/.test(text) ? "running" : this.snapshot.state;
    const output = `${this.snapshot.output}${text}`;
    this.snapshot = {
      ...this.snapshot,
      state,
      output: output.length > MAX_OUTPUT_CHARACTERS ? output.slice(-MAX_OUTPUT_CHARACTERS) : output,
    };
  }

  private finish(state: "succeeded" | "failed" | "stopped", exitCode: number | null) {
    if (!this.child && this.snapshot.finishedAt) return;
    this.child = null;
    this.snapshot = {
      ...this.snapshot,
      state,
      exitCode,
      finishedAt: new Date().toISOString(),
    };
  }
}

const runnerGlobal = globalThis as typeof globalThis & {
  emergentProjectRunManager?: ProjectRunManager;
};

export const projectRunManager = runnerGlobal.emergentProjectRunManager ??= new ProjectRunManager();

function idleSnapshot(): ProjectRunSnapshot {
  return {
    state: "idle",
    projectId: null,
    startedAt: null,
    finishedAt: null,
    exitCode: null,
    output: "",
  };
}

function isActive(state: ProjectRunState): boolean {
  return state === "building" || state === "running" || state === "stopping";
}
