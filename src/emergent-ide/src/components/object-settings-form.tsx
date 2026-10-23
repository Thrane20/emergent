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
      setSettings(result); setKey(""); setMessage("Object generation settings saved.");
    } catch (error) { setMessage(error instanceof Error ? error.message : "Unable to save settings."); }
    finally { setBusy(false); }
  }
  return <form className="settings-form" onSubmit={(event) => { event.preventDefault(); void save(); }}>
    <div className="provider-card"><div className="provider-identity"><span>◇</span><div><b>fal / TRELLIS</b><small>Image → 3D object</small></div></div><span className={settings.configured ? "settings-state connected" : "settings-state"}>{settings.configured ? "Key configured" : "Key required"}</span></div>
    <p className="object-muted">Generate objects from reference images using your fal account. Each generation is a paid provider request. Successful generation verifies access; saving a key does not test it.</p>
    <label className="settings-field"><span>fal API key</span><input type="password" value={key} onChange={(event) => setKey(event.target.value)} disabled={settings.source === "environment"} placeholder={settings.keyHint ? `Saved: ${settings.keyHint}` : "Enter your fal key"} autoComplete="new-password" /><small>{settings.source === "environment" ? "Configured through the server's FAL_KEY environment variable." : "Leave blank to keep the saved key. Credentials stay on the local IDE server."}</small></label>
    <div className="settings-actions"><button className="settings-save" disabled={busy || settings.source === "environment"}>Save provider</button>{settings.configured && settings.source !== "environment" && <button type="button" className="settings-clear" disabled={busy} onClick={() => void save(true)}>Remove key</button>}</div>
    {message && <p className="object-message" role="status">{message}</p>}
  </form>;
}
