import "server-only";

/** Published after an external (MCP) change lands, so open IDE pages can refresh. */
export type ProjectEvent = {
  type: "changed";
  projectId: string;
  changeId: string;
  source: "mcp" | "ide";
  summary: string;
  paths: string[];
  scenes: string[];
  // True when the running inspection world already received the edit; otherwise pages restart it.
  live: boolean;
};

type Listener = (event: ProjectEvent) => void;
const state = globalThis as typeof globalThis & { emergentProjectListeners?: Set<Listener> };
const listeners = state.emergentProjectListeners ??= new Set<Listener>();
const MAX_LISTENERS = 32;

export function publishProjectEvent(event: ProjectEvent) {
  for (const listener of [...listeners]) {
    try { listener(event); } catch { listeners.delete(listener); }
  }
}

export function subscribeProjectEvents(listener: Listener): () => void {
  if (listeners.size >= MAX_LISTENERS) throw new Error("Too many project event subscribers.");
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}
