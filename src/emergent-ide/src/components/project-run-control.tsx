"use client";

import { useCallback, useEffect, useState } from "react";
import type { ProjectRunSnapshot, ProjectRunState } from "@/lib/project-runner";

type Props = { projectId: string };

export function ProjectRunControl({ projectId }: Props) {
  const [run, setRun] = useState<ProjectRunSnapshot | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");

  const refresh = useCallback(async () => {
    try {
      const response = await fetch("/api/run", { cache: "no-store" });
      const result = await response.json() as ProjectRunSnapshot;
      if (!response.ok) throw new Error("Run status unavailable");
      setRun(result);
      setError("");
    } catch {
      setError("Run status unavailable");
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    void readRunStatus().then((result) => {
      if (!cancelled) {
        setRun(result);
        setError("");
      }
    }).catch(() => {
      if (!cancelled) setError("Run status unavailable");
    });
    return () => { cancelled = true; };
  }, []);
  useEffect(() => {
    const timer = window.setInterval(() => { void refresh(); }, 1_500);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const toggle = async () => {
    const stop = run ? isActive(run.state) : false;
    setPending(true);
    setError("");
    try {
      const response = await fetch("/api/run", {
        method: stop ? "DELETE" : "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ projectId }),
      });
      const result = await response.json() as ProjectRunSnapshot & { error?: string };
      if (!response.ok) throw new Error(result.error ?? "Project run request failed.");
      setRun(result);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Project run request failed.");
    } finally {
      setPending(false);
    }
  };

  const state = run?.state ?? "idle";
  const detail = error || runDetail(run);
  return <div className="run-control">
    <button
      className={`run-button ${state}`}
      disabled={pending || state === "stopping"}
      onClick={toggle}
      title={detail}
    >
      <span>{isActive(state) ? "■" : "▶"}</span>
      {pending ? "Working…" : runLabel(state)}
    </button>
    {(error || isTerminal(state)) && <span className={`run-result ${error ? "failed" : state}`} title={detail}>{error || runResult(state)}</span>}
  </div>;
}

function runLabel(state: ProjectRunState): string {
  if (state === "building") return "Cancel";
  if (state === "running") return "Stop";
  if (state === "stopping") return "Stopping";
  return "Run";
}

function runResult(state: ProjectRunState): string {
  if (state === "succeeded") return "Finished";
  if (state === "failed") return "Failed";
  if (state === "stopped") return "Stopped";
  return "";
}

function runDetail(run: ProjectRunSnapshot | null): string {
  if (!run?.output) return "Run the linked Cargo project on this computer";
  return run.output.trim().split("\n").slice(-4).join("\n");
}

function isActive(state: ProjectRunState): boolean {
  return state === "building" || state === "running" || state === "stopping";
}

function isTerminal(state: ProjectRunState): boolean {
  return state === "succeeded" || state === "failed" || state === "stopped";
}

async function readRunStatus(): Promise<ProjectRunSnapshot> {
  const response = await fetch("/api/run", { cache: "no-store" });
  if (!response.ok) throw new Error("Run status unavailable");
  return response.json() as Promise<ProjectRunSnapshot>;
}
