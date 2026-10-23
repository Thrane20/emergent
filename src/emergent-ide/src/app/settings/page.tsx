import Link from "next/link";
import { AISettingsForm } from "@/components/ai-settings-form";
import { readPublicAISettings } from "@/lib/ai-settings";
import { ObjectSettingsForm } from "@/components/object-settings-form";
import { publicObjectSettings } from "@/lib/object-settings";

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
      </header>
      <section className="settings-page">
        <div className="settings-intro"><span className="eyebrow">AI PROVIDERS</span><h1>Connect the project builder</h1><p>Configure the provider the IDE will use for scoped project, file, and entity conversations. Provider credentials remain IDE-local and outside the game project graph.</p></div>
        <div className="settings-layout">
          <aside className="settings-nav"><span className="selected">✦ AI &amp; Codex</span><a href="#object-generation">◇ Object generation</a><span>Build outputs <i>soon</i></span></aside>
          <div className="settings-stack"><section className="settings-panel"><div className="settings-panel-head"><div><span className="eyebrow">PRIMARY PROVIDER</span><h2>Codex connection</h2></div><span className="local-only">LOCAL ONLY</span></div><AISettingsForm initialSettings={settings} /></section><section className="settings-panel" id="object-generation"><div className="settings-panel-head"><div><span className="eyebrow">OBJECT GENERATION</span><h2>Connect TRELLIS</h2></div><span className="local-only">LOCAL KEY</span></div><ObjectSettingsForm initialSettings={objectSettings} /></section></div>
        </div>
      </section>
    </main>
  );
}
