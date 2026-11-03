import {FalBalance} from "@/components/fal-balance";
import Link from "next/link";
import { PlayerSettingsEditor } from "@/components/player-settings-editor";
import { readPlayerSettings } from "@/lib/player-settings";
import { readProjectForPage } from "@/lib/project-page";

export const instant = false;

export default async function PlayerPage() {
  const project = await readProjectForPage();
  const document = await readPlayerSettings(project);
  return (
    <main className="player-shell">
      <header className="settings-topbar">
        <Link className="brand" href="/"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></Link>
        <span className="settings-breadcrumb">{project.name} / <b>Player settings</b></span>
        <Link className="settings-back" href="/">← Project map</Link>
      <FalBalance/></header>
      <section className="player-page">
        <div className="player-intro">
          <span className="eyebrow">PROJECT CONFIGURATION</span>
          <h1>Player settings</h1>
          <p>Set the display and rendering goals for players of this project.</p>
        </div>
        <PlayerSettingsEditor projectId={project.id} initialDocument={document} />
      </section>
    </main>
  );
}
