import Link from "next/link";
import { AISettingsForm } from "@/components/ai-settings-form";
import { readPublicAISettings } from "@/lib/ai-settings";

export const instant = false;

export default async function SettingsPage() {
  const settings = await readPublicAISettings();
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
          <aside className="settings-nav"><span className="selected">✦ AI &amp; Codex</span><span>Build outputs <i>soon</i></span><span>Project defaults <i>soon</i></span></aside>
          <section className="settings-panel"><div className="settings-panel-head"><div><span className="eyebrow">PRIMARY PROVIDER</span><h2>Codex connection</h2></div><span className="local-only">LOCAL ONLY</span></div><AISettingsForm initialSettings={settings} /><div className="settings-note"><b>Current boundary</b><p>This configures the provider securely. Sending chat requests and applying generated file changes remain a separate, reviewable execution step.</p></div></section>
        </div>
      </section>
    </main>
  );
}
