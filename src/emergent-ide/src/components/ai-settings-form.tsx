"use client";

import { FormEvent, useState } from "react";
import type { PublicAISettings } from "@/lib/ai-settings";

type Props = { initialSettings: PublicAISettings };

export function AISettingsForm({ initialSettings }: Props) {
  const [settings, setSettings] = useState(initialSettings);
  const [baseUrl, setBaseUrl] = useState(initialSettings.baseUrl);
  const [model, setModel] = useState(initialSettings.model);
  const [apiKey, setApiKey] = useState("");
  const [status, setStatus] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [message, setMessage] = useState("");

  const save = async (event: FormEvent) => {
    event.preventDefault();
    await submit({ baseUrl, model, apiKey });
  };

  const clearKey = async () => {
    await submit({ baseUrl, model, clearApiKey: true });
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
      setMessage(result.configured ? "Codex connection settings saved." : "Provider settings saved without an API key.");
    } catch (error) {
      setStatus("error");
      setMessage(error instanceof Error ? error.message : "Unable to save settings.");
    }
  };

  return (
    <form className="settings-form" onSubmit={save}>
      <div className="provider-card">
        <div className="provider-identity"><span>✦</span><div><b>OpenAI / Codex</b><small>Responses API</small></div></div>
        <span className={settings.configured ? "settings-state connected" : "settings-state"}><i />{settings.configured ? "Configured" : "Key required"}</span>
      </div>

      <label className="settings-field"><span>Base URL</span><input type="url" value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} required /><small>OpenAI-compatible API root. HTTPS is required except for localhost.</small></label>
      <label className="settings-field"><span>Model</span><input value={model} onChange={(event) => setModel(event.target.value)} required /><small>The model used when project chat execution is enabled.</small></label>
      <label className="settings-field"><span>API key</span><input type="password" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder={settings.keyHint ? `Saved: ${settings.keyHint}` : "sk-…"} autoComplete="new-password" /><small>{settings.configured ? "Leave blank to keep the saved key." : "Stored only by the local IDE server and never returned to the browser."}</small></label>

      <div className="settings-actions">
        <button className="settings-save" type="submit" disabled={status === "saving"}>{status === "saving" ? "Saving…" : "Save provider"}</button>
        {settings.configured && <button className="settings-clear" type="button" onClick={clearKey} disabled={status === "saving"}>Remove key</button>}
        {message && <span className={`settings-message ${status}`}>{message}</span>}
      </div>
    </form>
  );
}
