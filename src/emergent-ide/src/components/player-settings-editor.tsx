"use client";

import { useState, type FormEvent } from "react";
import type { PlayerSettings, PlayerSettingsDocument } from "@/lib/player-settings";

const resolutions: { value: PlayerSettings["output"]["resolution"]; label: string }[] = [
  { value: "native", label: "Native display resolution" },
  { value: "1280x720", label: "1280 × 720 (720p)" },
  { value: "1920x1080", label: "1920 × 1080 (1080p)" },
  { value: "2560x1440", label: "2560 × 1440 (1440p)" },
  { value: "3840x2160", label: "3840 × 2160 (4K)" },
];
const targetRates: PlayerSettings["output"]["target_fps"][] = [30, 60, 90, 120, 144];
const minimumRates: PlayerSettings["output"]["minimum_fps"][] = [24, 30, 45, 60];
const antialiasingOptions: { value: PlayerSettings["rendering"]["antialiasing"]; label: string }[] = [
  { value: "off", label: "Off" },
  { value: "msaa-2x", label: "MSAA · 2×" },
  { value: "msaa-4x", label: "MSAA · 4×" },
  { value: "msaa-8x", label: "MSAA · 8×" },
  { value: "fxaa", label: "FXAA" },
  { value: "taa", label: "TAA" },
];

export function PlayerSettingsEditor({ projectId, initialDocument }: { projectId: string; initialDocument: PlayerSettingsDocument }) {
  const [document, setDocument] = useState(initialDocument);
  const [draft, setDraft] = useState(initialDocument.settings);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [activeSection, setActiveSection] = useState("game-output");
  const dirty = JSON.stringify(draft) !== JSON.stringify(document.settings);
  const valid = draft.output.minimum_fps <= draft.output.target_fps;

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!dirty || !valid || busy) return;
    setBusy(true);
    setError("");
    setMessage("");
    try {
      const response = await fetch("/api/player-settings", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ projectId, settings: draft, revision: document.revision }),
      });
      const result = await response.json();
      if (!response.ok) throw new Error(result.error ?? "Player settings could not be saved.");
      setDocument(result);
      setDraft(result.settings);
      setMessage("Player targets saved to this project.");
    } catch (error) {
      setError(error instanceof Error ? error.message : "Player settings could not be saved.");
    } finally {
      setBusy(false);
    }
  }

  function reset() {
    setDraft(document.settings);
    setError("");
    setMessage("");
  }

  return (
    <div className="player-layout">
      <nav className="player-nav" aria-label="Player settings sections">
        <a className={activeSection === "game-output" ? "selected" : ""} href="#game-output" onClick={() => setActiveSection("game-output")}>▣ <span>Game output</span></a>
        <a className={activeSection === "rendering-quality" ? "selected" : ""} href="#rendering-quality" onClick={() => setActiveSection("rendering-quality")}>◈ <span>Rendering quality</span></a>
      </nav>
      <form className="player-settings-stack" onSubmit={event => void save(event)}>
        <section className="settings-panel player-settings-panel" id="game-output">
          <div className="settings-panel-head">
            <div><span className="eyebrow">DISPLAY &amp; PERFORMANCE</span><h2>Game output quality</h2></div>
            <span className="local-only">PROJECT TARGET</span>
          </div>
          <div className="player-settings-body">
            <label className="settings-field">
              <span>Output resolution</span>
              <select aria-label="Output resolution" value={draft.output.resolution} disabled={busy} onChange={event => setDraft({ ...draft, output: { ...draft.output, resolution: event.target.value as PlayerSettings["output"]["resolution"] } })}>
                {resolutions.map(option => <option value={option.value} key={option.value}>{option.label}</option>)}
              </select>
            </label>
            <div className="player-field-grid">
              <label className="settings-field">
                <span>Target frame rate</span>
                <select aria-label="Target frame rate" value={draft.output.target_fps} disabled={busy} onChange={event => setDraft({ ...draft, output: { ...draft.output, target_fps: Number(event.target.value) as PlayerSettings["output"]["target_fps"] } })}>
                  {targetRates.map(rate => <option value={rate} key={rate}>{rate} FPS</option>)}
                </select>
                <small>{(1000 / draft.output.target_fps).toFixed(1)} ms per frame</small>
              </label>
              <label className="settings-field">
                <span>Minimum expected frame rate</span>
                <select aria-label="Minimum expected frame rate" value={draft.output.minimum_fps} disabled={busy} onChange={event => setDraft({ ...draft, output: { ...draft.output, minimum_fps: Number(event.target.value) as PlayerSettings["output"]["minimum_fps"] } })}>
                  {minimumRates.map(rate => <option value={rate} key={rate}>{rate} FPS</option>)}
                </select>
              </label>
            </div>
            {!valid && <p className="player-error" role="alert">Minimum expected FPS cannot exceed the target frame rate.</p>}
          </div>
        </section>

        <section className="settings-panel player-settings-panel" id="rendering-quality">
          <div className="settings-panel-head">
            <div><span className="eyebrow">EDGE SMOOTHING</span><h2>Rendering quality</h2></div>
            <span className="local-only">PROJECT TARGET</span>
          </div>
          <div className="player-settings-body">
            <label className="settings-field">
              <span>Anti-aliasing</span>
              <select aria-label="Anti-aliasing" value={draft.rendering.antialiasing} disabled={busy} onChange={event => setDraft({ ...draft, rendering: { antialiasing: event.target.value as PlayerSettings["rendering"]["antialiasing"] } })}>
                {antialiasingOptions.map(option => <option value={option.value} key={option.value}>{option.label}</option>)}
              </select>
            </label>
          </div>
        </section>

        {(error || message) && <p className={`player-save-message${error ? " error" : ""}`} role={error ? "alert" : "status"}>{error || message}</p>}
        <div className="player-actions">
          <button type="button" className="settings-clear" disabled={!dirty || busy} onClick={reset}>Discard changes</button>
          <button type="submit" className="settings-save" disabled={!dirty || !valid || busy}>{busy ? "Saving…" : "Save player settings"}</button>
        </div>
      </form>
    </div>
  );
}
