# Shader editor validation — SHADER-1

2026-11-02, macOS / Apple M3 Pro / Metal; Chrome for browser checks. This is a bounded surface shader preview capability, not a production performance gate.

## Delivered

- Project-owned `.shader.toml` graphs with up to 32 ordered scalar nodes and 16 exposed parameters. Native validation checks operation arity, references/order, unique IDs, finite ranges, outputs and view-independent vertex displacement. Intermediate arithmetic is bounded. Authored IDs become compiled expressions, never runtime ownership keys.
- Material shader references and named parameter overrides, including project-path containment and native validation. Existing materials remain compatible. Custom shader shadows are explicitly disabled and terrain blend layers reject custom shaders.
- Native WGSL compilation at resource preparation; time comes from fixed simulation in the shared host. Colour, emission, opacity and displacement render in native preview/Run. Transparent meshes draw after opaque meshes with object-origin sorting and depth writes disabled.
- Shader Editor in the shared navigation rail: project library, ordered node cards and input/output selectors, parameters, undo, shape/orbit preview, play/pause/restart/scrub and last-valid-preview retention for invalid drafts. Parameter edits retain the preview camera. AI can start before saving and repair incomplete drafts; proposals remain unsaved.
- Create material from a saved shader, then select it in the existing object Material inspector. Material Generator exposes shader/parameter controls and previews the graph. Applying a material updates the retained native inspection resource without replacing the worker or entity/geometry identity.
- Force-field, water and dissolve shaders/materials plus an optional `shader-showcase.scene.toml` in the standard template and reference project. Entry scenes are unchanged.

## Checks

- `cargo check --workspace --all-features`: passed.
- `cargo test --workspace --all-features`: 258 passed. The project-isolation failure initially exposed a sample-project dependency in the new smoke; the smoke now uses the existing disposable-project helper and the rerun passed.
- `cargo test --all-features --test shaders`: 2 passed, including native pipelines, time-driven changes, translated-camera Fresnel, full dissolve visibility, material replacement and stale handles.
- `cargo test --release --all-features --test shaders`: 2 passed on Metal.
- `cargo clippy --workspace --all-features --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo doc --no-deps --all-features`: completed; existing `map` → private `refresh` intra-doc-link warning remains unrelated.
- IDE full unit suite: 188 passed after updating the new-project scene-list expectation for the showcase. An additional targeted parameter-prototype regression passes separately.
- IDE TypeScript and focused ESLint: passed.
- Production Next build: passed. Existing shared inspection fingerprint traversal emits dynamic-filesystem tracing warnings.
- `test:shaders` Chrome smoke on a disposable template-derived project: passed. Covers animation, water shape, parameter editing/undo, invalid-draft preview retention, AI repair before first save, native-validated save, material creation, revision conflicts and material parameter controls. AI responses were mocked; no paid provider requests were made.
- Native showcase offscreen preview: passed and visually inspected at 1280×720 on Apple M3 Pro/Metal (`/tmp/emergent-shader-native-final.png`). Shield rim/transparent centre, displaced water and dissolve mask are visible. A translated-object view-direction defect found in inspection was fixed and regression-tested.
- Direct retained inspection smoke: material change, invalid reference rejection and restoration change/restore PNG output while preserving worker PID, camera and tick zero. The process and temporary scene were cleaned up.

Test port 3001 was occupied, so this run used its own isolated build directory and port 3002. The user's port 3000 was left alone. Test server and temporary assets were closed/cleaned up.

## Limits

Connections use ordered node cards, not a freeform wire canvas. This first graph has scalar parameters and smooth 1D noise; texture/normal inputs, vector parameters, scene depth/colour sampling, refraction, shoreline foam and impact/gameplay parameter bindings remain open. Displacement needs subdivided geometry and does not update collision or CPU picking bounds. Transparent intersections/backfaces are not solved by object-origin sorting. Matching custom shadow passes remain open. Each shader-bearing geometry currently owns a compiled pipeline; compilation happens on preparation/edit, not draws, but sharing/budgets have not been benchmarked. Browser lighting/noise precision approximates native rendering; native validation and native output are authoritative. Separate running game processes need restarting to consume saved asset changes. Existing Rust save validation may compile pending engine source changes.

## Force-field pulse correction

The new-template action now resets time and resumes playback, including after pausing or scrubbing a previous shader. The starter force field pulses opacity and glow together with a 0.25–1.0 envelope; default glow strength is reduced to keep the bright phase from hiding the modulation. Only the unmodified engine-owned template/reference assets were refreshed.

Regression validation: Chrome smoke pauses, chooses New force field, verifies automatic playback, then compares actual preview pixels at the bright and dim phases (mean blue-channel difference must exceed 10 code points in the central preview region). The full shader browser smoke passed on a disposable project at port 3001. Five graph unit tests, two native shader tests, TypeScript and focused ESLint passed. AI remained mocked. The test server and temporary project were cleaned up.

## Library and viewport layout

The Shader Editor now reuses AssetCarousel and shared generated material swatches, including its sticky + New tile. New creates a three-node blank surface; examples load from an explicitly labelled picker. Desktop preview and graph cards are bounded to the available viewport; controls scroll beneath the stationary preview. Top-left graph −/+, zoom readout and Fit all and center only change view state. Fit includes the output card and no longer clamps large graphs to a scale that clips nodes.

Chrome smoke on a disposable template-derived project at port 3001 covers carousel presence, removal of the library dropdown, blank creation, examples and pulse animation, zoom in/out, fit bounds, stationary preview while controls scroll, full graph viewport containment and reachable AI controls at 1440×800, plus no horizontal overflow at 820px width. The existing wiring/unplug/undo/loop-refusal, AI draft, native-validated save, material creation and conflict checks also pass. Screenshots were inspected. Six shader unit tests, TypeScript and focused ESLint pass. No engine/runtime contract changed; native checks were not repeated for this UI-only update.

Production IDE build also passes for the layout update; the existing dynamic-filesystem tracing warnings from shared inspection fingerprinting remain unrelated.
