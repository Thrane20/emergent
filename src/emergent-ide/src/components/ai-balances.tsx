"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { AIBalance } from "@/lib/ai-balance-types";

export function AIBalances() {
  const [balances, setBalances] = useState<AIBalance[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const pending = useRef<AbortController | null>(null);
  const load = useCallback(async (refresh: boolean) => {
    pending.current?.abort();
    const controller = new AbortController();
    pending.current = controller;
    setBusy(true);
    setError("");
    try {
      const response = await fetch("/api/ai-balances", { method: refresh ? "POST" : "GET", cache: "no-store", signal: controller.signal });
      const data = await response.json() as { balances: AIBalance[]; error?: string };
      if (!response.ok) throw new Error(data.error ?? "Unable to check AI balances.");
      if (!controller.signal.aborted) setBalances(data.balances);
    } catch {
      if (!controller.signal.aborted) { setBalances([]); setError("AI balances unavailable. Refresh to retry."); }
    } finally {
      if (!controller.signal.aborted) setBusy(false);
    }
  }, []);

  useEffect(() => {
    const changed = () => void load(false);
    const initial = setTimeout(changed, 0);
    window.addEventListener("ai-settings-changed", changed);
    return () => { clearTimeout(initial); pending.current?.abort(); window.removeEventListener("ai-settings-changed", changed); };
  }, [load]);

  if (!balances.length && !error) return null;
  return <div className="ai-balances" role="group" aria-label="AI balances" aria-busy={busy}>
    <div className="ai-balance-items">
    {balances.map(item => <a key={item.provider} className="ai-balance" href={item.billingUrl} target="_blank" rel="noreferrer" aria-label={`${item.label} balance`} title={`${item.message}${item.checkedAt ? ` Checked ${new Date(item.checkedAt).toLocaleString()}.` : ""}`}>
      <span>{item.label}</span><strong aria-live="polite">{item.state === "ready" ? new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" }).format(item.balance!) : item.state === "unsupported" ? "Billing ↗" : item.state === "unchecked" ? "—" : "Unavailable"}</strong>
    </a>)}
    {error && <span role="status" className="ai-balance" title={error}>AI unavailable</span>}
    </div>
    <button type="button" className="ai-balance-refresh" aria-label="Refresh AI balances" title="Refresh configured AI balances" disabled={busy} onClick={() => void load(true)}>
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><path d="M20 7v5h-5M4 17v-5h5"/><path d="M6.1 7a7 7 0 0 1 11.6-1L20 9M4 15l2.3 3A7 7 0 0 0 17.9 17"/></svg>
    </button>
  </div>;
}
