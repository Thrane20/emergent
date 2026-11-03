"use client";

import { FormEvent, useEffect, useState } from "react";
import type { AIProvider, PublicAISettings } from "@/lib/ai-settings";

type Props = { initialSettings: PublicAISettings };

const ORDER: AIProvider[] = ["anthropic", "openai-codex", "vercel", "ollama"];

export function AISettingsForm({ initialSettings }: Props) {
  const [settings, setSettings] = useState(initialSettings);
  const [provider, setProvider] = useState<AIProvider>(initialSettings.provider);
  const [baseUrl, setBaseUrl] = useState(initialSettings.baseUrl);
  const [model, setModel] = useState(initialSettings.model);
  const [apiKey, setApiKey] = useState("");
  const [status, setStatus] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [message, setMessage] = useState("");
  const selected = settings.providers[provider];
  const ollama = provider === "ollama";

  const choose = (next: AIProvider) => {
    setProvider(next);
    setBaseUrl(settings.providers[next].baseUrl);
    setModel(settings.providers[next].model);
    setApiKey("");
    setMessage("");
  };

  const save = async (event: FormEvent) => {
    event.preventDefault();
    await submit({ provider, baseUrl, model, apiKey });
  };

  const clearKey = async () => {
    await submit({ provider, baseUrl, model, clearApiKey: true });
  };

  const submit = async (body: Record<string, unknown>) => {
    setStatus("saving");
    setMessage("");
    try {
      const response = await fetch("/api/settings/ai", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      const result = await response.json() as PublicAISettings & { error?: string };
      if (!response.ok) throw new Error(result.error ?? "Unable to save settings.");
      setSettings(result);
      window.dispatchEvent(new Event("ai-settings-changed"));
      setApiKey("");
      setStatus("saved");
      setMessage(result.configured ? `${result.label} is now the active provider.` : `${result.label} saved ${result.provider === "ollama" ? "and deactivated" : "without an API key"}.`);
    } catch (error) {
      setStatus("error");
      setMessage(error instanceof Error ? error.message : "Unable to save settings.");
    }
  };

  return (
    <form className="settings-form" onSubmit={save}>
      <div className="view-switch" role="group" aria-label="AI provider" style={{ marginBottom: 14 }}>
        {ORDER.map(id => <button key={id} type="button" className={provider === id ? "selected" : undefined} aria-pressed={provider === id} onClick={() => choose(id)}>{settings.providers[id].label}{settings.provider === id ? " · active" : ""}</button>)}
      </div>
      <div className="provider-card">
        <div className="provider-identity"><span>✦</span><div><b>{selected.label}</b><small>{selected.api}</small></div></div>
        <span className={selected.configured ? "settings-state connected" : "settings-state"}><i />{selected.configured ? ollama ? "Activated" : "Configured" : ollama ? "Not activated" : "Key required"}</span>
      </div>

      {provider === "vercel" && <VercelModelPicker model={model} onChange={setModel} />}
      {ollama && <OllamaConnection baseUrl={baseUrl} model={model} onBaseUrl={setBaseUrl} onModel={setModel} />}
      <label className="settings-field"><span>{provider === "vercel" ? "Model ID (editable)" : "Model"}</span><input value={model} list={`ai-models-${provider}`} onChange={(event) => setModel(event.target.value)} required /><datalist id={`ai-models-${provider}`}>{selected.models.map(item => <option key={item} value={item} />)}</datalist></label>
      {!ollama && <><label className="settings-field"><span>Base URL</span><input type="url" value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} required /></label>
      <label className="settings-field"><span>API key</span><input type="password" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder={selected.keyHint ? `Saved: ${selected.keyHint}` : provider === "anthropic" ? "sk-ant-…" : provider === "vercel" ? "Vercel AI Gateway key" : "sk-…"} autoComplete="new-password" /></label></>}

      <div className="settings-actions">
        <button className="settings-save" type="submit" disabled={status === "saving"}>{status === "saving" ? "Saving…" : ollama && !(selected.configured && settings.provider === provider) ? "Activate Ollama" : settings.provider === provider ? "Save provider" : `Save and use ${selected.label}`}</button>
        {selected.configured && <button className="settings-clear" type="button" onClick={clearKey} disabled={status === "saving"}>{ollama ? "Deactivate" : "Remove key"}</button>}
        {message && <span className={`settings-message ${status}`}>{message}</span>}
      </div>
    </form>
  );
}

function VercelModelPicker({ model, onChange }: { model: string; onChange: (model: string) => void }) {
  const [models, setModels] = useState<{ id: string; name: string }[] | null>(null);
  const [error, setError] = useState("");
  const [refresh, setRefresh] = useState(0);
  const choices = models ?? [];
  useEffect(() => {
    const controller = new AbortController();
    fetch("/api/settings/ai/models", { signal: controller.signal }).then(async response => {
      const result = await response.json() as { models: { id: string; name: string }[]; error?: string };
      if (!response.ok) throw new Error(result.error ?? "Unable to load Vercel models.");
      if (!controller.signal.aborted) setModels(result.models);
    }).catch(error => {
      if (!controller.signal.aborted) setError(error instanceof Error ? error.message : "Unable to load Vercel models.");
    });
    return () => controller.abort();
  }, [refresh]);

  return <>
    <label className="settings-field"><span>Vercel model catalogue</span>
      <select value={choices.some(item => item.id === model) ? model : ""} onChange={event => onChange(event.target.value)}>
        <option value="" disabled>{error ? "Catalogue unavailable" : models === null ? "Loading models…" : choices.length ? "Choose a model, or enter an ID below" : "No matching models; enter an ID below"}</option>
        {choices.map(item => <option key={item.id} value={item.id}>{item.id} — {item.name}</option>)}
      </select>
      <small>Lists language models advertising tools and structured output. Save to use the selected model for subsequent AI requests. Model access depends on your Vercel account.</small>
    </label>
    <button type="button" className="settings-clear" onClick={() => { setModels(null); setError(""); setRefresh(value => value + 1); }}>Refresh models</button>
    {error && <p role="status">{error}</p>}
  </>;
}

function OllamaConnection({ baseUrl, model, onBaseUrl, onModel }: { baseUrl: string; model: string; onBaseUrl: (url: string) => void; onModel: (model: string) => void }) {
  const [port, setPort] = useState(() => new URL(baseUrl).port || "80");
  const [result, setProbe] = useState<{ port: string; running: boolean; models: string[]; message: string } | null>(null);
  const probe = result?.port === port ? result : null;
  const [refresh, setRefresh] = useState(0);
  useEffect(() => {
    const controller = new AbortController();
    fetch(`/api/settings/ai/ollama?port=${encodeURIComponent(port)}`, { signal: controller.signal })
      .then(async response => { const result = await response.json(); if (!controller.signal.aborted) setProbe({ port, ...(response.ok ? result : { running: false, models: [], message: result.error }) }); })
      .catch(() => { if (!controller.signal.aborted) setProbe({ port, running: false, models: [], message: "Unable to check Ollama." }); });
    return () => controller.abort();
  }, [port, refresh]);

  return <>
    <label className="settings-field"><span>Port</span>
      <input type="number" min={1} max={65535} value={port} onChange={event => { setPort(event.target.value); onBaseUrl(`http://localhost:${event.target.value}/v1`); }} required />
      <small role="status"><span className={probe?.running ? "settings-state connected" : "settings-state"}><i />{probe ? probe.message : `Checking port ${port}…`}</span></small>
    </label>
    {probe?.running && probe.models.length > 0 && <label className="settings-field"><span>Installed models</span>
      <select value={probe.models.includes(model) ? model : ""} onChange={event => onModel(event.target.value)}>
        <option value="" disabled>Choose a model, or enter a name below</option>
        {probe.models.map(name => <option key={name} value={name}>{name}</option>)}
      </select>
    </label>}
    <button type="button" className="settings-clear" onClick={() => setRefresh(value => value + 1)}>Check again</button>
  </>;
}
