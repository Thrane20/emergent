"use client";

import { FormEvent, useEffect, useState } from "react";
import type { ProjectStatement } from "@/lib/statements";

type Props = {
  projectId: string;
  subject: string;
  subjectLabel: string;
  aiConfigured: boolean;
};

export function StatementsPanel({ projectId, subject, subjectLabel, aiConfigured }: Props) {
  const [statements, setStatements] = useState<ProjectStatement[]>([]);
  const [draft, setDraft] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [checkingId, setCheckingId] = useState<string | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    const controller = new AbortController();
    fetch(`/api/statements?subject=${encodeURIComponent(subject)}`, { signal: controller.signal })
      .then(async (response) => {
        const result = await response.json() as { statements?: ProjectStatement[]; error?: string };
        if (!response.ok || !result.statements) throw new Error(result.error ?? "Unable to load statements.");
        setStatements(result.statements);
      })
      .catch((reason: unknown) => {
        if (reason instanceof Error && reason.name !== "AbortError") setError(reason.message);
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [subject]);

  const add = async (event: FormEvent) => {
    event.preventDefault();
    const text = draft.trim();
    if (!text || saving) return;
    setSaving(true);
    setError("");
    try {
      setStatements(await mutate("/api/statements", "POST", { projectId, subject, text }));
      setDraft("");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Unable to add statement.");
    } finally {
      setSaving(false);
    }
  };

  const remove = async (id: string) => {
    setError("");
    try {
      setStatements(await mutate("/api/statements", "DELETE", { projectId, subject, id }));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Unable to remove statement.");
    }
  };

  const evaluate = async (id: string) => {
    setCheckingId(id);
    setError("");
    try {
      setStatements(await mutate("/api/statements/evaluate", "POST", { projectId, subject, id }));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Unable to check implementation.");
    } finally {
      setCheckingId(null);
    }
  };

  return <section className="panel statements-panel">
    <header className="statements-head"><div><span className="eyebrow">STATEMENTS</span><h3>{subjectLabel}</h3></div><span className="statement-count">{statements.length}</span></header>
    <div className="statement-list">
      {loading && <p className="statement-empty">Loading intentions…</p>}
      {!loading && statements.length === 0 && <p className="statement-empty">Record what this selection is meant to be or do.</p>}
      {statements.map((statement) => <article className="statement-item" key={statement.id}>
        <input type="checkbox" checked={statement.implemented === true} readOnly aria-label={statement.implemented ? "AI marked implemented" : "Not marked implemented"} />
        <div><p>{statement.text}</p><small className={statement.implemented === true ? "implemented" : statement.implemented === false ? "missing" : "unchecked"}>{statusLabel(statement)}</small>{statement.evidence && <em>{statement.evidence}</em>}</div>
        <div className="statement-actions"><button onClick={() => evaluate(statement.id)} disabled={!aiConfigured || checkingId !== null} title={aiConfigured ? "Ask AI to check the project" : "Configure AI first"}>{checkingId === statement.id ? "…" : "Check"}</button><button onClick={() => remove(statement.id)} title="Remove statement">×</button></div>
      </article>)}
    </div>
    {error && <p className="statement-error">{error}</p>}
    <form className="statement-composer" onSubmit={add}><textarea value={draft} onChange={(event) => setDraft(event.target.value)} placeholder="Add an intention for this selection…" rows={2} maxLength={1_000} /><button disabled={!draft.trim() || saving}>{saving ? "…" : "+"}</button></form>
  </section>;
}

async function mutate(url: string, method: string, body: Record<string, unknown>): Promise<ProjectStatement[]> {
  const response = await fetch(url, { method, headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });
  const result = await response.json() as { statements?: ProjectStatement[]; error?: string };
  if (!response.ok || !result.statements) throw new Error(result.error ?? "Statement request failed.");
  return result.statements;
}

function statusLabel(statement: ProjectStatement): string {
  if (statement.implemented === true) return `AI verified${statement.confidence ? ` · ${statement.confidence}` : ""}`;
  if (statement.implemented === false) return `Not implemented${statement.confidence ? ` · ${statement.confidence}` : ""}`;
  return "Not checked";
}
