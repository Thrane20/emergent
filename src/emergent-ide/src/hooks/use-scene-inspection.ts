"use client";
import { useCallback, useEffect, useRef, useState } from "react";
import type { InspectionStatus } from "@/lib/inspection-session";

/** Serialize starts and cleanup so rapid tab changes cannot stop a newer session. */
export function useSceneInspection(projectId: string, path: string | undefined, enabled: boolean) {
  const [epoch, setEpoch] = useState(0);
  const [state, setState] = useState<{ key: string; inspection: InspectionStatus | null; busy: boolean; error: string } | null>(null);
  const queue = useRef<Promise<void>>(Promise.resolve());
  const key = JSON.stringify([projectId, path, enabled, epoch]);
  useEffect(() => {
    if (!enabled || !path) return;
    let active = true;
    let id: string | undefined;
    queue.current = queue.current.catch(() => {}).then(async () => {
      if (!active) return;
      setState({ key, inspection: null, busy: true, error: "" });
      try {
        const response = await fetch("/api/inspection", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ projectId, path }) });
        const data = await response.json();
        if (!response.ok) throw new Error(data.error || "Inspection could not start.");
        id = data.id;
        if (active) setState({ key, inspection: data, busy: false, error: "" });
      } catch (error) {
        if (active) setState({ key, inspection: null, busy: false, error: error instanceof Error ? error.message : "Inspection failed." });
      }
    });
    return () => {
      active = false;
      queue.current = queue.current.catch(() => {}).then(async () => {
        if (!id) return;
        const response = await fetch("/api/inspection", { method: "DELETE", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ projectId, id }), keepalive: true });
        if (!response.ok) { const data = await response.json(); throw new Error(data.error || "Inspection could not stop."); }
      }).catch(error => { console.error("Inspection cleanup failed; the server idle lease will stop the worker.", error); });
    };
  }, [projectId, path, enabled, key]);
  const reload = useCallback(() => setEpoch(value => value + 1), []);
  const current = state?.key === key ? state : null;
  return { inspection: enabled ? current?.inspection ?? null : null, busy: Boolean(enabled && path && (!current || current.busy)), error: current?.error ?? "", reload };
}
