"use client";

import { FormEvent, useState } from "react";
import type { AIProvider, PublicAISettings } from "@/lib/ai-settings";

type Props = { initialSettings: PublicAISettings };

const ORDER: AIProvider[] = ["anthropic", "openai-codex"];

export function AISettingsForm({ initialSettings }: Props) {
  const [settings, setSettings] = useState(initialSettings);
  const [provider, setProvider] = useState<AIProvider>(initialSettings.provider);
  const [baseUrl, setBaseUrl] = useState(initialSettings.baseUrl);
  const [model, setModel] = useState(initialSettings.model);
  const [apiKey, setApiKey] = useState("");
  const [status, setStatus] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [message, setMessage] = useState("");
  const selected = settings.providers[provider];

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
      setApiKey("");
      setStatus("saved");
      setMessage(result.configured ? `${result.label} is now the active provider.` : `${result.label} saved without an API key.`);
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
        <span className={selected.configured ? "settings-state connected" : "settings-state"}><i />{selected.configured ? "Configured" : "Key required"}</span>
      </div>

      <label className="settings-field"><span>Model</span><input value={model} list={`ai-models-${provider}`} onChange={(event) => setModel(event.target.value)} required /><datalist id={`ai-models-${provider}`}>{selected.models.map(item => <option key={item} value={item} />)}</datalist></label>
      <label className="settings-field"><span>Base URL</span><input type="url" value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} required /></label>
      <label className="settings-field"><span>API key</span><input type="password" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder={selected.keyHint ? `Saved: ${selected.keyHint}` : provider === "anthropic" ? "sk-ant-…" : "sk-…"} autoComplete="new-password" /></label>

      <div className="settings-actions">
        <button className="settings-save" type="submit" disabled={status === "saving"}>{status === "saving" ? "Saving…" : settings.provider === provider ? "Save provider" : `Save and use ${selected.label}`}</button>
        {selected.configured && <button className="settings-clear" type="button" onClick={clearKey} disabled={status === "saving"}>Remove key</button>}
        {message && <span className={`settings-message ${status}`}>{message}</span>}
      </div>
    </form>
  );
}
