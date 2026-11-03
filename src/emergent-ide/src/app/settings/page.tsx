import {FalBalance} from "@/components/fal-balance";
import Link from "next/link";
import { AISettingsForm } from "@/components/ai-settings-form";
import { readPublicAISettings } from "@/lib/ai-settings";
import { ObjectSettingsForm } from "@/components/object-settings-form";
import { publicObjectSettings } from "@/lib/object-settings";
import { ThemeSettings } from "@/components/theme-settings";

export const instant = false;

export default async function SettingsPage() {
  const settings = await readPublicAISettings();
  const objectSettings = await publicObjectSettings();
  return (
    <main className="settings-shell">
      <header className="settings-topbar">
        <Link className="brand" href="/"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></Link>
        <span className="settings-breadcrumb">Workspace / <b>Settings</b></span>
        <Link className="settings-back" href="/">← Project map</Link>
      <FalBalance/></header>
      <section className="settings-page">
        <div className="settings-intro"><span className="eyebrow">WORKSPACE SETTINGS</span><h1>Make Emergent your own</h1><p>Choose your workspace appearance and connect your AI providers. Appearance stays in this browser; provider credentials remain IDE-local and outside the game project graph.</p></div>
        <div className="settings-layout">
          <aside className="settings-nav"><a href="#appearance">◐ Appearance</a><a href="#codex-connection">✦ AI providers</a><a href="#object-generation">◇ Objects &amp; textures</a><span>Build outputs <i>soon</i></span></aside>
          <div className="settings-stack"><ThemeSettings/><section className="settings-panel" id="codex-connection"><div className="settings-panel-head"><div><span className="eyebrow">PRIMARY PROVIDER</span><h2>AI provider — Claude, Codex, Vercel or Ollama</h2></div><span className="local-only">LOCAL ONLY</span></div><AISettingsForm initialSettings={settings} /></section><section className="settings-panel" id="object-generation"><div className="settings-panel-head"><div><span className="eyebrow">GENERATED ASSETS</span><h2>Connect fal generation</h2></div><span className="local-only">LOCAL KEY</span></div><ObjectSettingsForm initialSettings={objectSettings} /></section></div>
        </div>
      </section>
    </main>
  );
}
