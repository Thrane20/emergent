"use client";

import Link from "next/link";
import { useState, type FormEvent } from "react";
import { FalBalance } from "@/components/fal-balance";
import type { ProjectPluginInfo } from "@/lib/project-plugins";

/** Lists the project's plugins and creates new ones from the plugin template. */
export function PluginManager({ projectName, initialPlugins }: { projectName: string; initialPlugins: ProjectPluginInfo[] }) {
  const [plugins, setPlugins] = useState(initialPlugins);
  const [id, setId] = useState(""), [name, setName] = useState("");
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null), [notice, setNotice] = useState<string | null>(null);

  async function create(event: FormEvent) {
    event.preventDefault();
    setBusy(true); setError(null); setNotice(null);
    try {
      const response = await fetch("/api/plugins", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ action: "new", id, name }) });
      const result = await response.json() as { plugin?: ProjectPluginInfo; warning?: string | null; error?: string };
      if (!response.ok || !result.plugin) throw new Error(result.error ?? "Could not create the plugin.");
      setPlugins(current => [...current, result.plugin!]);
      setNotice(result.warning ?? `Created ${result.plugin.path}. Rebuild the game to load it.`);
      setId(""); setName("");
    } catch (failure) { setError(failure instanceof Error ? failure.message : String(failure)); }
    finally { setBusy(false); }
  }

  return <main className="settings-shell">
    <header className="settings-topbar"><Link className="brand" href="/"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></Link><span className="settings-breadcrumb">{projectName} / <b>Plugins</b></span><Link className="settings-back" href="/">← Project map</Link><FalBalance /></header>
    <section className="settings-page">
      <div className="settings-intro"><span className="eyebrow">PROJECT PLUGINS</span><h1>Reusable behaviour and assets</h1><p>A plugin is a folder under <code>plugins/</code> with a <code>plugin.toml</code>, its own assets and optional Rust code that adds components, brain nodes and locomotion types. Its IDs start with the plugin id.</p></div>
      <div className="settings-stack">
        <section className="settings-panel" aria-label="Installed plugins">
          <div className="settings-panel-head"><div><span className="eyebrow">IN THIS PROJECT</span><h2>{plugins.length ? `${plugins.length} plugin${plugins.length === 1 ? "" : "s"}` : "No plugins yet"}</h2></div></div>
          {plugins.length > 0 && <div className="settings-form">{plugins.map(plugin => <article key={plugin.id || plugin.path} className="settings-field">
            <span><b>{plugin.name}</b> <code>{plugin.id}</code> {plugin.version && <small>v{plugin.version}</small>}</span>
            <small>{plugin.path} · {plugin.crate ? `code: ${plugin.crate}` : "assets only"}{plugin.assetRoots.length ? ` · assets: ${plugin.assetRoots.join(", ")}` : ""}</small>
            {plugin.description && <small>{plugin.description}</small>}
            {plugin.error && <small role="alert">{plugin.error}</small>}
          </article>)}</div>}
        </section>
        <section className="settings-panel" aria-label="New plugin">
          <div className="settings-panel-head"><div><span className="eyebrow">NEW</span><h2>Create a plugin</h2></div></div>
          <form className="settings-form" onSubmit={create}>
            <label className="settings-field"><span>Plugin id</span><input aria-label="Plugin id" value={id} placeholder="publisher.name" maxLength={64} onChange={event => setId(event.target.value)} required /><small>Your publisher name, a dot, then the plugin name, e.g. <code>studio.airports</code>. <code>emergent.</code> and <code>project.</code> are reserved.</small></label>
            <label className="settings-field"><span>Name</span><input aria-label="Plugin name" value={name} maxLength={64} onChange={event => setName(event.target.value)} required /></label>
            <div className="settings-actions">
              <button className="settings-save" type="submit" disabled={busy}>{busy ? "Creating…" : "Create plugin"}</button>
              {error && <span className="settings-message error" role="alert">{error}</span>}
              {notice && <span className="settings-message" role="status">{notice}</span>}
            </div>
          </form>
        </section>
      </div>
    </section>
  </main>;
}
