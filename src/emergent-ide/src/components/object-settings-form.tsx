"use client";

import { useState } from "react";
import type { ObjectSettings } from "@/lib/object-types";

export function ObjectSettingsForm({ initialSettings }: { initialSettings: ObjectSettings }) {
  const [settings, setSettings] = useState(initialSettings);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  async function save(clear = false) {
    setBusy(true); setMessage("");
    try {
      const response = await fetch("/api/settings/objects", { method: "PUT", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ apiKey: key, clearApiKey: clear }) });
      const result = await response.json();
      if (!response.ok) throw new Error(result.error);
      setSettings(result); window.dispatchEvent(new Event("fal-settings-changed")); setKey(""); setMessage("Object and texture generation settings saved.");
    } catch (error) { setMessage(error instanceof Error ? error.message : "Unable to save settings."); }
    finally { setBusy(false); }
  }
  return <form className="settings-form" onSubmit={(event) => { event.preventDefault(); void save(); }}>
    <div className="provider-card"><div className="provider-identity"><span>◇</span><div><b>fal / Objects + Textures</b><small>TRELLIS objects · FLUX textures</small></div></div><span className={settings.configured ? "settings-state connected" : "settings-state"}>{settings.configured ? "Key configured" : "Key required"}</span></div>
    <label className="settings-field"><span>fal API key</span><input type="password" value={key} onChange={(event) => setKey(event.target.value)} disabled={settings.source === "environment"} placeholder={settings.keyHint ? `Saved: ${settings.keyHint}` : "Enter your fal key"} autoComplete="new-password" />{settings.source === "environment" && <small>Configured through the server&apos;s FAL_KEY environment variable.</small>}</label>
    <div className="settings-actions"><button className="settings-save" disabled={busy || settings.source === "environment"}>Save provider</button>{settings.configured && settings.source !== "environment" && <button type="button" className="settings-clear" disabled={busy} onClick={() => void save(true)}>Remove key</button>}</div>
    {message && <p className="object-message" role="status">{message}</p>}
  </form>;
}
