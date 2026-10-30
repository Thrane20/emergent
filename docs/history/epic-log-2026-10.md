# Epic log archive — to 2026-10-30

Verbatim snapshot of the former root `plan.md` (part 1), the epic appendices of
`GOALS.MD` (part 2) and the former narrative `docs/implementation-status.md` (part 3),
taken when the documentation was consolidated on 2026-10-30. Frozen: do not update.
Relative links inside parts 1–2 were written for the repository root.

---

# Part 1 — former plan.md

## Emergent implementation plan

### Epic OC-1 — Object Creator with hosted TRELLIS

Status: **Complete — hosted generation, save and real-result browser reopen verified**

Decision date: 2026-10-23. Acceptance recorded: 2026-10-24.

Progress: **5 of 5 implementation stages complete**. The saved robot-arm model
loaded in the running IDE from both its GLB and descriptor after a page reload.
No new paid provider request was needed for the final compatibility check.

#### Goal and agreed decisions

Let a developer generate a 3D object from a reference image, inspect it in the
Emergent IDE, and save the chosen result into the linked game project's assets.

- Use fal's hosted `fal-ai/trellis` image-to-3D endpoint initially.
- Start with image upload. Add text-to-reference-image creation later.
- Preview and save GLB models in the IDE first. Runtime importing and rendering
  in the game are a separate engine increment.
- Use the locally installed `@google/model-viewer` package for browser previews.
- Keep generation outside the Rust engine and its frame-critical game loop.
- Preserve durable project asset identities; resolve them to fresh SlotMap-backed
  runtime handles when runtime loading is implemented.

This epic changes the IDE authoring workflow and project asset metadata. It does
not make the engine's planned model importer, texture, or material capabilities
available.

#### User workflow

1. Open **Object Creator** from the IDE navigation for the currently linked project.
2. Enter an object name and upload a reference image; inspect that image before
   submitting generation.
3. Generate and follow queued, generating, downloading, ready, or failed state.
4. Orbit, zoom, and reset the generated model in a large preview viewport.
5. Select a candidate from the project's generation history and **Save to project**.
6. Find the saved model and its dependencies in Files & Assets and the Project Map.
   Selecting a saved model reopens its preview.

#### Implementation stages

##### OC-1.1 — Provider feasibility and design brief

- [x] Write a concise implementation design brief covering ownership, durable
  asset identities, data flow, resource limits, and validation before adding the
  substantial job/asset system.
- [x] Use a configured fal account to submit one real image-to-3D request.
- [x] Download the real result and verify its GLB 2 header and declared file length.
- [x] Open that hosted result with the chosen viewer and record compatibility.
- [x] Record the observed output format and integration result. The provider API's
  example file metadata is inconsistent, so actual GLB compatibility is an
  acceptance gate, not an assumption to carry into implementation.

##### OC-1.2 — Provider settings and persistent jobs

- [x] Add **Object generation** settings with a fal API key and connection status.
  Follow the existing server-only IDE settings pattern and expose only a masked
  key hint to the browser.
- [x] Add a small server-only provider adapter using `@fal-ai/client`, supporting
  only `fal-ai/trellis` initially. Upload images and submit requests through the IDE
  server.
- [x] Start with the documented provider defaults: 1,024-pixel textures and default
  sampling/simplification settings. Generate and record an explicit seed.
- [x] Persist local job IDs, provider request IDs, settings, timestamps, errors, and
  artifact locations under the IDE data directory, partitioned by project.
- [x] Poll while Object Creator is open; recover known jobs after page refresh or
  IDE restart without submitting another paid request.
- [x] Download completed artifacts into the local job cache. Preview and save from
  that cache rather than relying on provider URLs remaining available.
- [x] Handle transient status failures as recoverable. Show uncertain submission
  outcomes explicitly; never automatically resubmit a possibly accepted request.
  Retrying a confirmed failure is an explicit user action.

##### OC-1.3 — Object Creator interface and preview

- [x] Add Object Creator to the IDE navigation, bound to the linked project.
- [x] Provide object naming, reference-image upload/preview, Generate, status,
  useful errors, and project-specific generation history.
- [x] Load `@google/model-viewer` only in the browser and provide orbit, zoom, and
  reset-view controls for cached GLB output.
- [x] Keep unsuccessful attempts and unsaved candidates in generation history;
  do not add every attempt to the project's asset tree.
- [x] Validate GLB output before enabling Save to project. A successful provider
  response alone does not establish a valid model.

##### OC-1.4 — Save model bundles and connect the project graph

- [x] Save accepted objects as a bundle under a declared project asset root:

  ```text
  assets/models/<object-slug>/
    model.glb
    reference.<original-image-extension>
    object.asset.json
  ```

- [x] Define a versioned descriptor containing a durable asset ID, display name,
  project-relative model/reference paths, provider endpoint/request ID, seed,
  generation settings, and timestamps. Record the model revision when supplied;
  otherwise mark it unavailable. A recorded seed is provenance, not a guarantee of
  identical future hosted output.
- [x] Publish the complete bundle only after every file is ready. Never overwrite
  an existing object; request another name on collision. Repeated saves of the
  same candidate return the existing saved asset.
- [x] Add explicit descriptor-to-model and descriptor-to-reference graph edges.
  Support preview when selecting the saved model from Files & Assets.
- [x] Retain existing file-scoped chat and Statements support for saved assets.
- [x] Serve cached/project artifacts through scoped routes with validated paths,
  bounded uploads/downloads, and enforcement of the project's declared asset roots.
  Never persist runtime SlotMap or ECS keys.

##### OC-1.5 — Verification and documentation

- [x] Test missing/invalid credentials, failed generation, interrupted polling,
  refresh/restart recovery, and prevention of unintended duplicate submissions.
- [x] Test malformed model output, duplicate saves, naming collisions, incomplete
  bundle failures, and paths outside the linked project or cache.
- [x] Use mocked provider responses for automated tests; keep the real paid
  generation as an explicit integration check.
- [x] Run IDE lint, TypeScript checks, and production build.
- [x] Verify live upload → generate → preview → save → reopen with fal. The saved
  real hosted GLB was reopened and visibly inspected on 2026-10-24. The same
  workflow, including server restart, passes with mocked generation and a real GLB fixture.
- [x] Update IDE/user documentation and capability documentation where applicable,
  distinguishing IDE previews from engine runtime model support.

#### Epic acceptance criteria

- A developer can configure fal, upload an image, generate an object, inspect its
  GLB, and save it into `first_project`.
- Refresh/restart can resume a known generation; reopening the IDE can preview a
  saved model without depending on a temporary provider URL.
- Saved assets and provenance appear with meaningful connections in the Project Map.
- Errors preserve existing valid project assets, and provider credentials remain
  outside the project graph and browser responses.
- Generation and preview introduce no work into the Rust simulation/render loop.
- All verification above has recorded outcomes. No runtime model capability is
  claimed on the basis of browser preview alone.

#### Dependencies and follow-up epics

Implementation needs a funded fal account and key configured through IDE settings
or server configuration. Never store that key in the game project or source control.

Later increments: text-to-reference-image creation; evaluation of TRELLIS.2;
runtime GLB importing and textured materials; model placement in authored scenes;
self-hosted generation workers. None are required to complete OC-1.

#### Research references

- [Microsoft TRELLIS](https://github.com/Microsoft/TRELLIS)
- [TRELLIS text generation and GLB export example](https://github.com/microsoft/TRELLIS/blob/main/example_text.py)
- [fal TRELLIS API and queue contract](https://fal.ai/models/fal-ai/trellis/api)
- [Model Viewer](https://modelviewer.dev/)
- [TRELLIS.2 model card for later evaluation](https://huggingface.co/microsoft/TRELLIS.2-4B)

Research establishes documented feasibility. Real generation, project saving and real-result browser compatibility/reopen are now observed.

#### Implementation evidence — 2026-10-23

- Design: `docs/design/object-creator.md`.
- Implemented `/objects`, object-provider settings, persistent job and artifact
  routes, browser GLB preview, safe bundle saving, and explicit project graph edges.
- Service tests cover journal recovery, duplicate paid-call prevention, failure
  handling, GLB validation, collision safety, path/symlink checks, and key masking.
- `pnpm test`, `pnpm exec tsc --noEmit`, `pnpm lint`, and `pnpm build` pass.
- `pnpm test:ui` passes against the production server using an isolated temporary
  game. Generation is mocked; the real GLB viewer, settings, save routes, server
  restart recovery, and project-tree selection are exercised in headless Chrome.
- Browser screenshot inspection caught a stale Turbopack CSS cache; a fresh build
  and an added computed-layout assertion verify the intended desktop layout.
- A configured fal account generated a real result saved under
  `projects/first_project/assets/models/a-robot-arm/`. The descriptor records request
  `01a168ac-3ac2-7cd0-b232-990d3a39e4c1`, seed `1677536427`, and the 2026-10-23
  save. The provider now sends numeric `texture_size: 1024`; the correction and
  regression test are present in the working tree. On 2026-10-24, Chrome loaded that actual saved GLB, then after reload
  loaded it again through descriptor selection. Visual inspection confirmed the
  textured red robot arm. This used localhost:3000 and submitted no provider call.
- No Rust engine/runtime model-import capability was changed or claimed.

For the cross-project implementation audit and remaining goals, see
[implementation status](docs/implementation-status.md) and [GOALS.MD](GOALS.MD).


### Epic AS-1 — Authored primitive/light scenes

Status: **Complete — first bounded slice, 2026-10-24**

- [x] Write the ownership, data-flow, loading limits and validation brief before implementation.
- [x] Load schema-1 scenes with explicit instance IDs, entity asset paths and optional parent IDs.
- [x] Support eight procedural primitive types, existing light types and empty hierarchy nodes.
- [x] Validate the complete candidate before creating fresh SlotMap entities; roll back failed instantiation.
- [x] Use the same authored construction and dense render extraction for headless, native Run and IDE Load; keep project Rust behaviours as an optional extension.
- [x] Add optional selected-scene inclusion to the IDE asset creator; preserve newly saved assets when inclusion fails and report the failure.
- [x] Prove asset edits change a real Rust-rendered frame and invalid asset values preserve the previous preview.
- [x] Update capability status, public API guide, project guidance and goals.

Design: [authored scene loading](docs/design/authored-scene-loading.md).
Evidence: [AS-1 validation](docs/platform/authored-scene-loading-macos.md).

Limits: 256 KiB per file, 1,024 instances, eight lights, 32 subdivisions per
primitive axis, with primitive dimensions capped at 1,000,000. Instance transforms come from the referenced asset. Nested
compositions, authored behaviour links/type registration,
GLB importing/materials, live inspection camera controls and non-macOS runtime
verification remain later work. Run launches the manifest entry scene; Load can
select other supported scenes.


### Epic AS-2 — Scene-instance Transform inspector

Status: **Complete — 2026-10-24**

- [x] Add optional per-vector instance transform overrides to the engine loader, preserving asset defaults and parent-local semantics.
- [x] List scene instances separately from reusable asset files and show instance/asset/parent context in the inspector.
- [x] Edit numeric position, XYZ degree rotation and scale; provide uniform scaling, Apply & reload and Reset to asset defaults.
- [x] Validate complete candidates through the linked game's headless Rust validation contract before publishing.
- [x] Preserve unrelated scene text, detect changed scene/asset revisions, and retain previous frames on preview failures.
- [x] Verify independent shared-asset instances, persistence after refresh, reset, stale edits and rejected transforms in the running IDE.

Design: [scene transform inspector](docs/design/scene-transform-inspector.md).
Validation: [inspector evidence](docs/platform/scene-transform-inspector-macos.md).
Camera navigation, viewport picking, gizmos and general undo/redo remain open.

### Feasibility SP-1 — Selkies for persistent project mode

Status: **Gate complete — Selkies declined for native Mac development, 2026-10-24**

- [x] Check the local host, available Docker environment and required capture-backend distributions.
- [x] Verify upstream host support and distinguish Mac browser support from Mac server support.
- [x] Stop early when the local native-Metal path requires an unsupported host backend or a platform port.
- [x] Record evidence and preserve the persistent, gameplay-stopped inspection-session direction independently of transport.
- [x] Evaluate a Mac-native persistent Rust inspection process with a local command/frame channel; SP-2 proves the bounded PNG prototype, while production transport/encoding remain open.

No Selkies server or streamed viewport was implemented. Docker Desktop is available,
but running a Linux session there would be a different rendering environment;
no Linux GPU or software-rendered container benchmark is claimed.

Evidence: [Selkies/macOS feasibility](docs/research/selkies-macos-feasibility.md).
Reconsider Selkies for an actual Linux GPU worker or supported upstream Mac host.


### Epic SP-2 — Persistent native inspection: first proof

Status: **Complete — bounded macOS/Metal prototype, 2026-10-24**

- [x] Write the ownership, protocol, bounds, process lifetime and validation design before implementation.
- [x] Retain the native SlotMap world and renderer/geometry resources in a single worker with gameplay fixed at tick zero.
- [x] Deliver latest-frame 960×540 PNGs at up to 10 Hz with bounded buffers and no frame backlog.
- [x] Acknowledge camera and instance-transform commands through a separate control path using authored IDs and session generations.
- [x] Add Inspect, Restart inspection and Stop inspection; keep Load as snapshot fallback and normal Run independent.
- [x] Support pointer orbit, Shift-drag pan, wheel zoom and Reset view.
- [x] Apply saved inspector transforms to the retained world without a renderer/process restart.
- [x] Exercise invalid/stale controls, restart, explicit Stop, browser cleanup and inactivity expiry on the running IDE.
- [x] Record initial release-profile frame/encoding/acknowledgement measurements and limitations.

Design: [persistent inspection](docs/design/persistent-inspection-session.md).
Evidence: [macOS integration and measurements](docs/platform/persistent-inspection-macos.md).

Production video encoding, higher-rate input/render scheduling, viewport picking,
gizmos, gamepad input, transient drafts, general undo/redo and non-macOS runtime
validation remain open. Code/geometry/membership changes require Restart inspection.


### Epic SG-1 — Scene-first hierarchy authoring

Status: **Complete — macOS/Metal reference, 2026-10-24**

- [x] Merge the left navigation around authored scene instances; retain a collapsed project file tree.
- [x] Drag instances onto parents or Scene root, with a keyboard Parent selector and keep-local semantics.
- [x] Validate cycles, missing nodes, revisions and complete Rust scene candidates before publication.
- [x] Apply acknowledged parenting to the retained inspection world without recreating runtime identities or GPU resources.
- [x] Add header Local/Global controls, using Rust World transforms for conversion and position-only editing under shear.
- [x] Add an editable TOML tab for scene overrides and shared entity assets; validate all project scenes in isolation, protect drafts and reject external-edit conflicts.
- [x] Cover inherited transforms, live worker identity, real drag/drop, global edits, root drops, keyboard parenting and invalid-source preservation.

Design: [scene graph authoring](docs/design/scene-graph-authoring.md).
Evidence: [reference validation](docs/platform/scene-graph-authoring-macos.md).

Reparenting retains local TRS and can therefore move the subtree in world space.
Preserve-world reparenting, viewport picking/gizmos and undo/redo remain open.
Whole-file TOML changes require Load or Restart inspection; saved inspector and
parent edits update the retained world directly. Windows/Linux runtime validation
remains open.


### Epic VG-1 — 30 fps inspection, picking and axis gizmos

Status: **Complete — reference macOS/Metal slice, 2026-10-24**

- [x] Record a release reference baseline (9.6 fps) and change pacing to a 30 Hz deadline without accumulating missed frames.
- [x] Build an inspection-only CPU cache from the renderer's tessellation; pick nearest triangles with bounds and clip checks.
- [x] Select light/empty-node origin markers and keep viewport/tree/inspector selection connected.
- [x] Project XYZ translate/rotate/scale handles using the renderer's actual view/projection and Rust parent-aware Local/Global math.
- [x] Preview coalesced pointer drags in the retained world; publish once on release through existing Rust validation and revision guards.
- [x] Restore the starting transform on Escape, pointer cancellation or failed publication; reject wrong selection/space/tool and stale gesture tokens.
- [x] Remove asset-tree rescans from session traffic to avoid racing atomic authoring saves; retain live project-ID/root checks.
- [x] Verify real browser picking, all three tools, cancellation, conflicts, worker identity and target cadence; update guides and capability status.
- [x] Follow-up: correct Global Scale to use world axes, preserve exact no-movement snapshots, handle clipped rotation rings safely by keyboard, install saved revisions before allowing another drag, and cover parented Global drags and pointer cancellation.

Design: [viewport picking and gizmos](docs/design/viewport-picking-gizmos.md).
Evidence: [macOS viewport validation](docs/platform/viewport-picking-gizmos-macos.md).

This supersedes SP-2's original 10 Hz limit and its deferral of picking, axis gizmos
and transient drag previews. The latest-frame PNG transport remains a prototype.
Snapping, multi-select, free/plane handles, undo/redo, hardware video and
Windows/Linux runtime validation remain open. Near-parallel handles are hidden;
Global rotation/scale that require shear are rejected without approximation.


### Epic NF-1 — Filter assets and noise fields

Status: **Complete — reference Rust/IDE slice, 2026-10-24**

- [x] Add Filters / Noise function to asset creation and discover .noise.toml files as filter resources.
- [x] Reuse pinned noise-rs 0.9.0 for Perlin, OpenSimplex and distance-Worley; own a versioned normalized/octave contract in the engine.
- [x] Expose prepared sampling and bounded complete 2D/3D dense field generation without graphics or gameplay.
- [x] Add right-inspector algorithm, seed, dimensions, resolution, frequency, octaves, lacunarity, persistence, extent and offset controls.
- [x] Preview absolute 0..1 grayscale in Overview; inspect a generated 3D volume using selectable Z slices.
- [x] Keep generation separate from Save settings; share Rust validation, source revisions, guarded writes and raw TOML editing.
- [x] Validate golden/repeatable seeded samples, algorithm/seed differences, full volume indexing, bounds, grayscale browser pixels, persistence and invalid/stale-source preservation.

Design: [noise filter assets](docs/design/noise-filter-assets.md).
Evidence: [noise filter validation](docs/platform/noise-filter-assets-macos.md).

Bounds: 512-square 2D, 128-cubed 3D, 1..8 octaves, at most 2,097,152 octave
evaluations. A definition stores function inputs; generated buffers remain
transient. Filter composition graphs, baked exports, terrain/material consumers,
value noise, domain warping and Windows/Linux runtime validation remain open.


### Correction PN-1 — Outward primitive winding

Complete — 2026-10-24. Sphere and capsule body rings were connected in reverse Y
order, while cylinder caps, cones and toruses also had inward triangle winding.
Corrected the generated index order, preserving vertex/index counts and authored
assets. Regression tests reproduce the inward faces before the fix, check outward
faces for all five affected shapes and verify sphere normal direction/unit length
under smooth and flat shading (including minimum sphere subdivisions).

Validation: workspace debug and release/all-feature suites, formatting, Clippy and
rustdoc pass. A native Metal tick-zero reference capture succeeds at 1280×720.
Existing inspection sessions retain uploaded geometry: use Restart inspection or
Load to see the correction; restart native Run as needed. Other-platform runtime
verification remains open.


### Epic PH-1 — First physics foundation

Implemented — 2026-10-24. Rapier 3D 0.36.1 is an optional engine dependency
(`physics` feature); the reference project enables it alongside graphics.

- [x] Record engine choice, ownership, data flow, boundaries and validation in
  GOALS.MD and docs/design/physics-foundation.md.
- [x] Static/dynamic PhysicsBody components and fitted/explicit BoxCollider data;
  bounded gravity scale, mass, friction and restitution, fixed gravity -9.81 Y.
- [x] Retained backend with private handles mapped to canonical EntityId; cached
  ECS query/reused inputs, lifecycle cleanup, fixed stepping and pose writeback.
- [x] Asset defaults, complete instance overrides and explicit disable; validate
  source before construction, root-only dynamic bodies, positive TRS/no shear.
- [x] IDE physics inspector, guarded instance saves and inspection restart;
  yellow box outlines in tick-zero inspection, no simulation in project mode.
- [x] Independent physics demo scene and --run-scene path, without changing the
  authored main scene; existing spin rule yields pose ownership to dynamic physics.
- [x] Regression tests for freefall/contact, paused construction, reset, deletion
  and slot reuse, gravity scale, scaled/offset boxes, rotated bodies, hierarchy,
  invalid data, conflicting writes, feature-off diagnostics and safe TOML edits.

Kinematic bodies, forces/impulses, public scene queries, other collider shapes,
triggers/events/behaviour ports, joints, character controllers, simulation controls
inside inspection, and Windows/Linux runtime evidence remain open. IDE Run now uses the selected dropdown scene via --run-scene; a plain Cargo
launch continues to use the project entry scene.

PH-1 validation evidence: [macOS checks](docs/platform/physics-foundation-macos.md).


#### PH-1 follow-up — Run the selected scene

Complete — IDE Run passes the current scene picker path through /api/run to Cargo
as `-- --run-scene <path>`. Validate that the scene is listed in the linked project,
lives under an asset root and resolves to a contained existing file. Run is disabled
while authored edits are unsaved or being applied; Stop remains available for the
active run after changing scene selection. ProjectRunSnapshot records scenePath.
The manifest and authored scene files are not changed by choosing or running a scene.

Validation: 34 IDE unit tests, lint, TypeScript/production build, browser scene-switch
request checks and a live /api/run launch of the selected physics scene passed.
The verification run was stopped; the existing IDE server was left running.


### Epic M-1 — Renderable material editor

Preview implemented — 2026-10-24.

- [x] Headless MaterialDefinition, MaterialShading and MaterialNoiseMap contracts;
  validated Material components with resolved immutable noise inputs.
- [x] Entity defaults and complete scene overrides; reject non-renderable targets
  and invalid/missing/escaping noise references before constructing the world.
- [x] Renderer-owned per-geometry material resources, bounded packed volume bake,
  flat/smooth/unlit paths, Blinn-Phong highlights, emission and derivative bump.
- [x] Inspector shading/colour/intensity/strength controls; a noise picker for
  each of albedo, specular, emission and bump, with sRGB picker conversion.
- [x] Guarded saves/reset through the existing scene edit path; preserve physics,
  transforms, siblings and shared assets, track noise revisions and restart the
  retained paused renderer after successful saves.
- [x] Independent materials scene, material sphere and reusable grain filter.
- [x] Seed/projection/validation/edit regressions and native render comparisons;
  browser save/restart proves changed pixels at tick zero without shared edits.

Design: docs/design/material-editor.md. Evidence: docs/platform/material-editor-macos.md.
Deferred: image textures, PBR, bloom/emissive lights, UV authoring/displacement,
shared standalone materials, shader graphs, map-resolution controls and other-platform
runtime validation. No performance target claimed for this preview.


### SH-1 — Object cast/receive shadow options

Implemented — 2026-10-25. Add MaterialDefinition.cast_shadows/receive_shadows with
backwards-compatible true defaults, independent depth-pass/shader decisions and
Material inspector checkboxes. Existing guarded defaults/instance saves and paused
inspection restarts handle persistence. Collider overlays are unlit non-casters/
non-receivers. No new identity, GPU map or steady-state allocation is introduced.

A separate Shadows — cast and receive scene includes a tilted shadow-enabled
sun, floating caster and receiving floor. The authored main scene remains unchanged.
Directional-only limits stay explicit; point/spot shadows and cascades are deferred.

Design: docs/design/object-shadows.md. Evidence: docs/platform/object-shadows-macos.md.


### SH-2 — Point-light shadow casting

Implemented — 2026-10-25. Two retained point-shadow slots each render six 512-square
Depth32Float views; light-space matrices and a reusable dynamic depth-pass matrix
buffer support both point lights and the existing directional shadow in one frame.
The shader chooses a point face/layer and compares projected depth with bounded
world-space bias. Existing light flags and object cast/receive settings apply.
Additional shadow-enabled point lights and invalid shadow bounds return errors.

- [x] Backend resources, face projection/selection and updated fragment shading.
- [x] Six-direction, moved/two-light, object-option, range and slot-limit native tests.
- [x] Directional and material rendering regressions, Rust/IDE checks.
- [x] Paused IDE save/restart verification using the new point-shadow demo.
- [x] Public capability/API/guide updates and documented cost/bounds.

No authored main-scene or key-light edits were needed; its point light already
opts in. Spot shadows, more slots/quality controls and culling/caching are deferred.
Design: docs/design/point-light-shadows.md. Evidence: docs/platform/point-light-shadows-macos.md.


### IDE-LIVE-1 — Automatic inspector persistence and noise swatches

Complete — 2026-10-25. A shared client hook coalesces typing for 300 ms, validates
before saving, serializes inspector writes against current revisions and preserves
unfinished/failed drafts. Stable inspector keys preserve focus across saved revisions.
Transform, physics, material and noise settings save automatically. Reset controls
remain explicit; Generate preview still generates a noise field on demand.

Material noise pickers are compact square swatches next to their channel inputs,
including bump strength. Unassigned swatches use the theme; assigned ones display
cached Rust-generated 24-square grayscale previews, serializing thumbnail generation
and taking a middle slice for 3D sources. Clicking opens a choose/clear list.

Validation kept narrow: TypeScript, targeted lint and one temporary-project browser
check for automatic section saves, cross-section ordering, focus and swatch images.
No Rust changes or full test-suite run were needed.


### Surface quality (SURF-1)

- [x] Correct smooth sphere normals analytically and replace triangle-dependent bump derivatives with object-space central gradients.
- [x] Add bounded 32/64/128 material volumes using RGBA16Float, retaining 32 as default, and expose resolution in the live inspector.
- [x] Permit authored spheres up to 128 sectors / 64 stacks; preserve other primitive limits.
- [x] Capture native before/after images and scoped release measurements; add a separate Surface quality comparison scene.

Implemented 2026-10-25. See docs/design/surface-quality-comparison.md and
docs/benchmarks/surface-quality-2026-10-25/README.md for evidence and costs.
Higher geometry/map quality is optional; this does not establish a game frame budget.


### IDE-SCENE-AUTO — Inspection follows the Scene tab

Implemented 2026-10-25. Opening Scene automatically starts native paused inspection
for the selected scene; leaving the tab stops its owned session. Scene changes
replace that session automatically. Load explicitly reloads the current scene and
retries failed starts. Removed Inspect, Restart inspection and Stop inspection
buttons, along with the IDE snapshot-only Load path; the underlying snapshot API
remains available. Serialized startup/cleanup handles rapid navigation and delayed
responses without leaking a replaced session.

Validation: IDE unit tests, lint, production build, native persistent-inspection
smoke (tick zero, camera/edit updates, reload, tab stop/resume and idle expiry),
and automatic Scene-tab/error checks. No engine or asset schema changes.


### IDE-CHAT-ACT — Apply Build with AI requests

Implemented 2026-10-25. Chat now uses structured proposals, foundation/project reuse
context, supplied durable UUIDs, existing isolated Cargo/native scene validation
and journalled conflict-checked publication. It shares the statement-generation
lock. Questions may answer without file changes. Game/Scene/Object scope and the
active scene travel with the request. Successful publication refreshes the graph,
inspectors and paused renderer; failed validation/conflicts do not apply edits.

Validation: temporary-project centre-cube publication through actual Cargo/scene
checks with synthetic provider output; no-change answers, invalid geometry, invalid
new IDs and concurrent edits; IDE unit/type/lint/build and browser refresh checks.
No Rust engine changes or paid AI request are needed for these regression checks.
See docs/design/actionable-project-chat.md.


### IDE-NAME — Editable object Source and local scene names

Implemented 2026-10-25. Entity Source views use the existing guarded TOML editor.
Enter on a selected scene row starts inline renaming; Enter/blur saves and Escape
cancels. Optional scene-instance names override shared asset names, with 1–64
printable characters. Duplicate names remain valid; IDs, relationships and paths
stay unchanged. Existing revision/native validation protects local edits.

Validated through default/release Rust suites, strict Clippy/rustdoc/formatting,
44 IDE tests, lint/types/build and a native browser check for local/shared edits,
keyboard focus/cancel, duplicate names and stable identity/hierarchy. Temporary
fixtures only; no user object names were changed. See docs/design/object-renaming.md.


### IDE-TERRAIN-1 — Terrain Builder preview

- [x] Dedicated /terrain view linked from the project rail and Object Creator.
- [x] Hilly, Mountainous, Rivers and Island presets with seed, width, relief,
  frequency, roughness, octaves and bounded grid resolution controls.
- [x] Native Rust seeded noise, prepared smooth grayscale mesh, orbit/zoom/reset,
  retained last-good preview on failure, and responsive compact controls.
- [x] Preview-only noise candidates without requiring or creating saved assets.
- [x] Settings/mesh/GLB tests and native browser checks for all presets and controls.

Implemented 2026-10-25. No project assets are saved; rivers are carved valleys.
Runtime terrain, saving/scene placement, collision, water/hydraulic erosion, chunks and LOD
remain open. Rust checks and 47 IDE tests/lint/types/build passed; observed browser
rendering is Chrome on macOS. See docs/design/terrain-builder.md and
docs/platform/terrain-builder-macos.md.


#### Terrain relief and mountain density

Implemented 2026-10-25. The terrain preview includes an angled directional sun
and terrain-specific exposure. Mountainous replaces dense ridged noise with
seeded, spaced elliptical peaks: Mountain count 1–12, default 3. Roughness is a
0–1 slider with a live numeric readout. Generation remains explicit; identities
and project files are unchanged. Directional relief lighting does not add terrain
self-shadow maps or erosion.

Validation: 48 IDE tests/type/lint/build, native terrain smoke, exact major-mountain
region counts at zero roughness and a browser render comparison with the sun
disabled/enabled. Other object previews retain their exposure.


#### Visible terrain light controls

Implemented 2026-10-25. The terrain viewer uses dim neutral ambient fill so
directional lighting is clearly visible. The Directional light section exposes
strength, direction and elevation sliders; Generate applies them and the header
shows the preview strength. Preview-only lighting does not alter project scenes.
49 IDE tests/types/lint/build and native browser control/brightness comparisons
passed, including readable ambient fill with the sun off.


### TERRAIN-2 — Reusable preparation and algorithm comparison

Implemented 2026-10-25.

- [x] Versioned, validated Rust `TerrainSettings` and owned dense `TerrainField`;
  deterministic seed streams, 32–256 samples per side, smooth `Mesh` conversion.
- [x] Perlin and diamond-square base generators; diamond-square uses 2^k+1 samples
  internally and resamples onto the existing output grid.
- [x] Reusable preset shaping, controlled mountain/ridge layout and domain warp.
- [x] Optional conservative thermal erosion, capped at 64 passes, prepared outside
  the game/render loop with reused working storage.
- [x] Terrain Builder delegates shaping to Rust and exposes generator, warp,
  ridge strength and erosion controls; GLBs carry the versioned input recipe.
- [x] Matched Original noise / Diamond-square / Enhanced pipeline comparison,
  identical initial camera and lighting, independent orbit controls and stacked
  narrow layout. Per-result Rust/GLB preparation times and field/GLB output sizes.
- [x] Seed/bounds, mountain-count, erosion-conservation, normals/winding, protocol,
  API and native browser coverage; optimized generation measurements recorded in
  docs/benchmarks/terrain-comparison-2026-10-25/README.md.
- [x] Terrain colliders and saving/scene placement delivered in TERRAIN-3.
- [ ] Hydraulic erosion and seamless chunks/LOD remain future slices. Ray marching
  explicitly deferred.

The reusable preparation API does not complete the runtime heightmap/chunking
milestone. Output sizes are not peak RAM/GPU memory; native evidence is macOS,
with Windows/Linux execution still unverified. See
[design](docs/design/terrain-generation-pipeline.md).


#### TERRAIN-2 follow-up — irregular mountain massifs

Implemented 2026-10-25. Algorithm version 2 replaces the enhanced radial cone
with a seeded crest of uneven summits and saddles, broad shoulders and asymmetric
contours per mountain group. Major group centres and inter-group ridge links are
retained; Mountain count now describes groups, not exact summit count. Explicit
version-1 recipes still reproduce their old output and comparison baselines use
version 1. No new sliders, ownership system, dependencies or scene asset writes.

Validation covers multiple substantial summits per group across seeds, isolated
group counts, preserved ridge contributions, legacy height fixtures and existing
bounds/determinism/mesh checks. See docs/design/terrain-generation-pipeline.md.


### TERRAIN-3 — Save generation to scene and terrain collision

Implemented 2026-10-25.

- [x] Save the displayed generated recipe, name and optional terrain collider as a
  UUID terrain entity asset plus distinct root instance in the selected scene.
- [x] Carry the active scene into Terrain Builder and return with the saved object
  selected; explicit destination picker, duplicate display names allowed.
- [x] Native staged validation, bounded inputs, conflict checks, project binding
  recheck and journalled publication; failed inclusion leaves no orphan asset.
- [x] Scene loader builds terrain meshes from versioned recipes for native paused
  inspection and Run, using existing World/renderer SlotMap ownership.
- [x] Typed Box/Terrain collider variants; static Rapier heightfield follows the
  generated mesh, including grid orientation, diagonal, TRS and positive scale.
- [x] Live inspector enable/disable and collider type; dynamic terrain rejected.
- [x] Terrain framing, wider bounded camera orbit and distance-aware far clipping.
- [x] Native mesh/collider agreement, falling-body contacts, deletion/box regression,
  save/conflict tests and a temporary-project browser save/inspection flow.

Recipes regenerate on load; this does not freeze a binary heightfield. Existing
scene lights are retained. Seamless chunks/LOD, height editing, hydraulic erosion
and dynamic terrain bodies remain deferred. See docs/design/saved-scene-terrain.md.


### MATERIALS-IMAGE-1 — Diffuse images and terrain material palette

- [x] PNG/JPEG diffuse texture references with bounded native decoding, EXIF
  orientation, linear-light mip preparation and sRGB GPU colour sampling.
- [x] Triplanar projection and repeat per local metre on objects/terrain, retaining
  existing colour tints, noise/bump, lighting and shadow behaviour.
- [x] Live image swatch/picker and native-validated local image import with unique
  project paths; image references participate in scene revisions and graph links.
- [x] Terrain palettes with stable named layer IDs and one active surface; repeated
  TOML layer edits preserve physics, transforms and other instances.
- [ ] Brush strokes/weight maps, height/slope blending, alpha cutouts, authored UVs
  and shared texture resource caching remain future slices.

This first slice selects a whole-surface terrain layer; it does not yet paint
individual regions. Existing renderer GeometryId SlotMap ownership is retained;
only the active image is decoded/uploaded. See
[design](docs/design/diffuse-image-materials.md).

MATERIALS-IMAGE-1 validation: default/release-all-feature Rust suites, strict
Clippy/rustdoc/formatting, 53 IDE tests, lint/types/build, native pixel regression
and browser import/live-layer smoke passed. See docs/platform/diffuse-images-macos.md.


### IDE-TEXTURES-1 — AI texture creation and project selection

- [x] Dedicated Texture Creator editor with prompt/name/seed, 1–4 candidates and
  optional tile-friendly prompting; linked from project, Object and Terrain views.
- [x] fal FLUX.1 Schnell integration using existing server-held fal credentials.
- [x] Persisted project-scoped job history, queue/check/download states, exclusive
  paid-submission claims and no automatic resubmission after uncertain outcomes.
- [x] Native image validation and bounded fal-media-only downloads before previews.
- [x] Multi-select candidate saves with durable IDs, readable display names,
  provenance metadata and idempotent/recoverable publication without overwrites.
- [x] Saved generated textures appear in object diffuse and terrain layer pickers;
  multiple terrain materials remain available for later painting.
- [x] Provider/schema/job recovery tests and temporary-project browser integration
  with mocked fal responses, real native validation and real project publication.
- [ ] Brush painting, splat maps and height/slope material blending remain separate
  future slices; prompting alone does not establish seamless tiling.

No paid generation is needed for automated verification. See
[design](docs/design/texture-creator.md) for ownership, persistence and scope.

IDE-TEXTURES-1 validation: 59 IDE tests, lint/types/build and the temporary-project
Texture Creator browser flow passed. Provider responses were mocked; PNG native
validation, project saves and material selection were real. See
docs/platform/texture-creator-macos.md. Live fal access/quality remains unverified.


#### TWEEN-1 — Reusable tweens and motion editor

- [x] Headless Rust scalar lerp and Linear, Sine, Quadratic and Cubic In/Out/In-Out easing.
- [x] Validated immutable recipes, explicit simulation-time sampling and Once/Loop/Ping-pong playback.
- [x] Tween Editor at `/tweens`: moving cube, curve comparison, pause/restart and scrubbing.
- [x] Save/reopen selected variants as schema-1 `assets/tweens/<uuid>.tween.toml`; duplicate display names remain independent assets.
- [x] Native validation on save and raw TOML editing; browser/native sampling parity checks.
- [ ] Scene inspector bindings that attach tween assets to object properties automatically.

Recipes are owned values; game behaviours own time and apply sampled values through
existing World/EntityId APIs. No additional runtime identity or scheduler is introduced.
See [design](docs/design/tween-editor.md) and the public tween API in the user guide.

TWEEN-1 validation: workspace/default and release/all-feature tests, Clippy,
rustdoc and formatting passed; 61 IDE tests, lint/types/build and a real browser
save/reopen smoke passed. See docs/platform/tween-editor-macos.md.


#### CAMERA-2 — Camera authoring and foundation search

- [x] Add Asset search filters names/categories/descriptions and expands matching branches.
- [x] Live camera inspector for asset defaults and per-instance overrides: Perspective/Orthographic, FOV/view height, near/far clipping planes.
- [x] Revision-guarded persistence, native validation and Use asset defaults reset.
- [x] Selected-camera frustum in the editor overlay, following hierarchy transforms and live camera edits without advancing gameplay or restarting the inspection session.

Validation: workspace/default and release/all-feature tests, Clippy/rustdoc/fmt,
64 IDE tests and lint/types/build passed. A temporary-project browser test verified
search, persistence, reset and live frustum changes. See
docs/design/camera-inspector-frustum.md and docs/platform/camera-authoring-macos.md.


#### VIEW-NAV-1 — Object facing and editor shortcuts

- [x] Derived normalized world look-at vector (-Z), shown for each selected scene instance.
- [x] F frames selected native bounds using the editor camera, without saving object changes.
- [x] Shift-F saves the selected object's position/facing from the actual editor camera, preserving scale and resolving its parent transform.
- [x] Pure native placement proposals, guarded persistence, shear rejection and typing/edit protection.

Validation: new engine/project tests and all 68 IDE tests passed, including a real
browser check with native world-transform verification. Full Rust suite still has
an unrelated authored offender bounce-height test failure. See docs/platform/editor-focus-align-macos.md.

### Material library and terrain paint (2026-10-28)
- [x] Shared material assets and dedicated authoring UI; object library selection.
- [x] Four-slot manual/procedural terrain blend maps with save/reopen.
- [x] Native material blending and editor preview.
- [x] Focused regression tests, build and native/UI checks.

Validation: 79 IDE tests, production build, targeted ESLint/TypeScript, engine debug/release tests, Clippy and rustdoc pass. Native material/diffuse/four-slot paint smokes pass on Apple M3 Pro / Metal. Full workspace debug/release still fails the existing first-project bounce-height test. UI workflow verified in an isolated project on port 3001; screenshot `/tmp/emergent-terrain-material-editor.png`.

### Object Builder (2026-10-28)
- [x] Define reusable polygon mesh and structure asset contracts.
- [x] Primitive palette and local hierarchy with drag/reparent/reorder.
- [x] 3D object/face/vertex selection and move handles.
- [x] Face extrusion with connecting sides and inset.
- [x] Save, place and reopen structures; tests and native/UI validation.

Validation: 87 IDE tests, production build, targeted ESLint/TypeScript, engine debug/release tests, Clippy and rustdoc pass. The full workspace retains the pre-existing first-project bounce-height failure. Browser checks on an isolated project verified primitive creation, parent/reorder drag-and-drop, direct object/face/point dragging, extrusion/inset, save/place, native rendering and exact editable reopening. A further saved topology edit preserved the scene instance at [0, 50, 0]. Proof: `/tmp/emergent-object-builder-editor.png`, `/tmp/emergent-object-builder-native.png`.

### Particle engine and generator (2026-10-29)

Approved native engine approach; see [design](docs/design/particle-generator.md).
- [x] Bounded seeded CPU pools, typed SlotMap ownership, scene effect components.
- [x] Native instanced billboard pass, alpha/additive blending, flipbooks and soft depth intersections.
- [x] Particle Generator controls, preview, save/update/place/reopen workflows.
- [x] Connected fal image/video/matting jobs and local atlas baking.
- [x] Regression tests, optimized measurements, native/browser validation and public documentation.

Particle validation: 93 IDE tests and production build pass; engine tests pass headlessly and with all features in debug/release, Clippy/fmt/rustdoc pass. Native pixel checks cover alpha, flipbooks, depth sorting/occlusion, soft fading and resize. Real Chrome checks cover the complete save/place/reopen and video-bake flow, Add Asset entry, and native Run through 120 ticks with resize/clean shutdown. No paid fal calls were made. The full workspace retains the pre-existing bounce-height failure. See [macOS evidence](docs/platform/particle-generator-macos.md).

### Particle Factory effect library (2026-10-29)
- [x] Header swatch carousel over saved effect assets, with native cached stills.
- [x] Dirty-recipe discard confirmation before loading a swatch.
- [x] Saved-swatch placement without rewriting recipes or replacing the draft.
- [x] Workshop content sizing and scrolling for long image/video histories.
- [x] Native thumbnail, placement, dialog, carousel and responsive browser validation.

Factory library validation: 94 IDE tests, production build, targeted ESLint/TypeScript, engine release tests, Clippy and formatting pass. Chrome covered 18 swatches, native stills, both discard choices, draft-preserving scene placement, 20 workshop image controls plus video/atlas controls, mobile access and native Run/resize/shutdown. See [evidence](docs/platform/particle-factory-library-macos.md). No paid fal calls were made.

---

# Part 2 — former GOALS.MD epic appendices

### Physics foundation — Rapier 3D

Direction agreed 2026-10-24; implementation tracked in plan.md. Use the standalone
Rust `rapier3d` crate behind an optional `physics` feature. Emergent owns scheduling,
project contracts and rendering; SlotMap EntityId remains canonical runtime identity.
Backend body/collider handles are private, world-scoped implementation details and
must never appear in authored assets. Reference: https://rapier.rs/docs/.

- [x] Provide headless static/dynamic rigid bodies and box colliders, gravity,
  mass, friction, restitution and per-body gravity scale.
- [x] Store defaults on entity assets and complete overrides on scene instances;
  an explicit disabled override removes inherited physics.
- [x] Offer inspector controls and a wireframe collider overlay in paused inspection.
- [x] IDE Run launches the selected scene using --run-scene; plain Cargo retains
  the manifest entry-scene default.
- [x] Step physics at the engine fixed timestep, after gameplay commands and
  before final transform propagation. Rendering frequency is independent.
- [x] Demonstrate a cube falling onto static ground, settling and resetting from
  authored transforms on a fresh run. Inspection remains at gameplay tick zero.
- [x] Validate deletion/stale handles, gravity/contact response, unsupported
  hierarchies, scaling and conflicting gameplay transform writes.

First-slice boundaries: dynamic bodies are top-level scene instances (the loader's
identity scene container is allowed); visual children may follow them. A scene
parent does not create a physical joint. Reject sheared/reflected/singular physics
transforms rather than approximating them. Bake positive scale into collider shape;
keep physics poses rigid. A collider is separate from the visible geometry: fit a
box from local geometry bounds, then allow an authored centre and half-extents.

Inspection never advances physics. Play advances a transient world; restarting
constructs from authored values without saving simulated poses. Kinematic bodies,
sphere/capsule/convex/terrain colliders, triggers and behaviour collision events,
joints, force controls and character controllers follow later. Profile optimized
representative scenes before enabling parallelism or claiming a frame budget.

Research: Rapier is a standalone Rust physics library; Avian integrates with Bevy's
application/plugin ecosystem; Jolt introduces C++ build/binding requirements. Rapier
is selected for the first slice on architectural fit, not a measured speed ranking.
Cross-platform documentation is not Emergent runtime verification on each OS.


### First material editor (M-1)

Preview implemented 2026-10-24. Materials are engine values attached to canonical
entities; renderer-scoped SlotMap GeometryId owns mesh/material GPU resources.
Entity assets provide defaults and scene instances accept complete overrides.

- [x] Flat, smooth and unlit shading; optional linear albedo override, specular
  RGB and shininess, emission RGB/intensity and bump strength.
- [x] Attach a contained existing noise-filter asset independently to albedo,
  specular, emission and bump, with a per-channel mixing strength.
- [x] Bake seeded inputs once on upload into bounded packed 32-cubed RGBA8 noise
  maps, using local object bounds, XZ projection for 2D and XYZ for 3D filters.
- [x] Render diffuse/specular/emission and derivative-based bump normals in the
  native renderer, with an inspector for each renderable scene instance.
- [x] Save guarded scene overrides, reset to asset defaults, refresh paused
  inspection and use the same material definition during Run.
- [x] Keep shared assets unchanged and include noise dependencies in revision
  checks; validate channels, references and renderable-only assignment.

This first surface model uses Blinn-Phong specular. Emission brightens the surface;
bloom, lighting other objects, PBR maps, image textures, UV authoring, displacement,
shared standalone material handles/assets and shader graphs remain future work.
Noise maps use fixed preview resolution independently of noise-editor resolution;
no material performance budget or Windows/Linux native evidence is claimed.
See docs/design/material-editor.md and docs/platform/material-editor-macos.md.


### Object shadow controls (SH-1)

Implemented 2026-10-25 on the existing directional shadow-map foundation.

- [x] Independent cast_shadows and receive_shadows options for renderable objects,
  defaulting to true for existing assets and direct renderer callers.
- [x] Persist defaults on entity materials and overrides on scene instances;
  expose both checkboxes in the Material inspector and refresh paused rendering.
- [x] Exclude non-casters from the depth pass while drawing them normally; bypass
  shadow attenuation on non-receivers without changing other objects' shadows.
- [x] Exclude debug collider outlines from both casting and receiving shadows.
- [x] Provide a shadows demo with a supported tilted directional light, caster
  and receiving floor; preserve existing user-authored main-scene lighting.
- [x] Validate independent combinations using native image comparisons and strict
  authored boolean parsing, plus guarded inspector save/restart checks.

Shadows currently use the first eligible shadow-enabled directional light and the
existing fixed 2048-square, origin-centred map. Point/spot shadows, cascades, soft
shadow/quality controls and Windows/Linux native evidence remain future work.
See docs/design/object-shadows.md and docs/platform/object-shadows-macos.md.


### Point-light shadows (SH-2)

Implemented 2026-10-25, extending SH-1.

- [x] Render omnidirectional point-light shadows using six 90-degree depth views
  per light, following world position and range independently of orientation.
- [x] Support two point-shadow slots alongside the existing directional map and
  eight-light total budget; report excess point-shadow requests explicitly.
- [x] Reuse retained depth arrays, face views and an aligned matrix staging/buffer
  path without new steady-state allocations. Each face is 512-square.
- [x] Preserve object cast/receive options, light casts_shadows flags, paused
  inspection and SlotMap ownership; the existing reference key point light works
  without changing authored settings.
- [x] Validate all six directions, moved/multiple lights, object toggles, light
  range/slot bounds and directional coexistence through native render comparisons.
- [x] Add a Point shadows — lamp and cube demo and update IDE/API capability text.

Point shadow range is 0.01..1000000, with world coordinates within +/-1000000 per
axis; near clip is range*0.001. Two point slots reserve 12 MiB depth storage per
renderer and require up to twelve depth passes; no frame budget claim. Spot shadows,
more slots/configurable resolution, seamless face filtering, culling/caching,
cascades and Windows/Linux runtime evidence remain open.
See docs/design/point-light-shadows.md and docs/platform/point-light-shadows-macos.md.


### Live inspector (IDE-LIVE-1)

- [x] Transform, physics, material and noise settings save automatically after a
  300 ms typing pause, with validation, serialized saves and visible error/status.
- [x] Remove Apply/Save settings actions; keep deliberate reset and preview actions.
- [x] Replace material noise dropdowns with themed square swatches beside colour
  and bump controls, with a compact choose/clear picker and real Rust-generated
  grayscale thumbnails (a middle slice for 3D filters).
- [x] Preserve input focus, protect unfinished drafts and retain scene revisions.

Implemented 2026-10-25. TypeScript/targeted lint and a temporary-project browser
check cover persistence across sections, noise thumbnails and focus retention.


### Surface quality (SURF-1)

- [x] Correct smooth sphere normals analytically and replace triangle-dependent bump derivatives with object-space central gradients.
- [x] Add bounded 32/64/128 material volumes using RGBA16Float, retaining 32 as default, and expose resolution in the live inspector.
- [x] Permit authored spheres up to 128 sectors / 64 stacks; preserve other primitive limits.
- [x] Capture native before/after images and scoped release measurements; add a separate Surface quality comparison scene.

Implemented 2026-10-25. See docs/design/surface-quality-comparison.md and
docs/benchmarks/surface-quality-2026-10-25/README.md for evidence and costs.
Higher geometry/map quality is optional; this does not establish a game frame budget.


### IDE-SCENE-AUTO — Inspection follows the Scene tab

Implemented 2026-10-25. Opening Scene automatically starts native paused inspection
for the selected scene; leaving the tab stops its owned session. Scene changes
replace that session automatically. Load explicitly reloads the current scene and
retries failed starts. Removed Inspect, Restart inspection and Stop inspection
buttons, along with the IDE snapshot-only Load path; the underlying snapshot API
remains available. Serialized startup/cleanup handles rapid navigation and delayed
responses without leaking a replaced session.

Validation: IDE unit tests, lint, production build, native persistent-inspection
smoke (tick zero, camera/edit updates, reload, tab stop/resume and idle expiry),
and automatic Scene-tab/error checks. No engine or asset schema changes.


### IDE-CHAT-ACT — Apply Build with AI requests

Implemented 2026-10-25. Chat now uses structured proposals, foundation/project reuse
context, supplied durable UUIDs, existing isolated Cargo/native scene validation
and journalled conflict-checked publication. It shares the statement-generation
lock. Questions may answer without file changes. Game/Scene/Object scope and the
active scene travel with the request. Successful publication refreshes the graph,
inspectors and paused renderer; failed validation/conflicts do not apply edits.

Validation: temporary-project centre-cube publication through actual Cargo/scene
checks with synthetic provider output; no-change answers, invalid geometry, invalid
new IDs and concurrent edits; IDE unit/type/lint/build and browser refresh checks.
No Rust engine changes or paid AI request are needed for these regression checks.
See docs/design/actionable-project-chat.md.


### IDE-NAME — Editable object Source and local scene names

Implemented 2026-10-25. Entity Source views use the existing guarded TOML editor.
Enter on a selected scene row starts inline renaming; Enter/blur saves and Escape
cancels. Optional scene-instance names override shared asset names, with 1–64
printable characters. Duplicate names remain valid; IDs, relationships and paths
stay unchanged. Existing revision/native validation protects local edits.

Validated through default/release Rust suites, strict Clippy/rustdoc/formatting,
44 IDE tests, lint/types/build and a native browser check for local/shared edits,
keyboard focus/cancel, duplicate names and stable identity/hierarchy. Temporary
fixtures only; no user object names were changed. See docs/design/object-renaming.md.


### IDE-TERRAIN-1 — Terrain Builder preview

- [x] Dedicated /terrain view linked from the project rail and Object Creator.
- [x] Hilly, Mountainous, Rivers and Island presets with seed, width, relief,
  frequency, roughness, octaves and bounded grid resolution controls.
- [x] Native Rust seeded noise, prepared smooth grayscale mesh, orbit/zoom/reset,
  retained last-good preview on failure, and responsive compact controls.
- [x] Preview-only noise candidates without requiring or creating saved assets.
- [x] Settings/mesh/GLB tests and native browser checks for all presets and controls.

Implemented 2026-10-25. No project assets are saved; rivers are carved valleys.
Runtime terrain, saving/scene placement, collision, water/hydraulic erosion, chunks and LOD
remain open. Rust checks and 47 IDE tests/lint/types/build passed; observed browser
rendering is Chrome on macOS. See docs/design/terrain-builder.md and
docs/platform/terrain-builder-macos.md.


#### Terrain relief and mountain density

Implemented 2026-10-25. The terrain preview includes an angled directional sun
and terrain-specific exposure. Mountainous replaces dense ridged noise with
seeded, spaced elliptical peaks: Mountain count 1–12, default 3. Roughness is a
0–1 slider with a live numeric readout. Generation remains explicit; identities
and project files are unchanged. Directional relief lighting does not add terrain
self-shadow maps or erosion.

Validation: 48 IDE tests/type/lint/build, native terrain smoke, exact major-mountain
region counts at zero roughness and a browser render comparison with the sun
disabled/enabled. Other object previews retain their exposure.


#### Visible terrain light controls

Implemented 2026-10-25. The terrain viewer uses dim neutral ambient fill so
directional lighting is clearly visible. The Directional light section exposes
strength, direction and elevation sliders; Generate applies them and the header
shows the preview strength. Preview-only lighting does not alter project scenes.
49 IDE tests/types/lint/build and native browser control/brightness comparisons
passed, including readable ambient fill with the sun off.


### TERRAIN-2 — Reusable preparation and algorithm comparison

Implemented 2026-10-25.

- [x] Versioned, validated Rust `TerrainSettings` and owned dense `TerrainField`;
  deterministic seed streams, 32–256 samples per side, smooth `Mesh` conversion.
- [x] Perlin and diamond-square base generators; diamond-square uses 2^k+1 samples
  internally and resamples onto the existing output grid.
- [x] Reusable preset shaping, controlled mountain/ridge layout and domain warp.
- [x] Optional conservative thermal erosion, capped at 64 passes, prepared outside
  the game/render loop with reused working storage.
- [x] Terrain Builder delegates shaping to Rust and exposes generator, warp,
  ridge strength and erosion controls; GLBs carry the versioned input recipe.
- [x] Matched Original noise / Diamond-square / Enhanced pipeline comparison,
  identical initial camera and lighting, independent orbit controls and stacked
  narrow layout. Per-result Rust/GLB preparation times and field/GLB output sizes.
- [x] Seed/bounds, mountain-count, erosion-conservation, normals/winding, protocol,
  API and native browser coverage; optimized generation measurements recorded in
  docs/benchmarks/terrain-comparison-2026-10-25/README.md.
- [x] Terrain colliders and saving/scene placement delivered in TERRAIN-3.
- [ ] Hydraulic erosion and seamless chunks/LOD remain future slices. Ray marching
  explicitly deferred.

The reusable preparation API does not complete the runtime heightmap/chunking
milestone. Output sizes are not peak RAM/GPU memory; native evidence is macOS,
with Windows/Linux execution still unverified. See
[design](docs/design/terrain-generation-pipeline.md).


#### TERRAIN-2 follow-up — irregular mountain massifs

Implemented 2026-10-25. Algorithm version 2 replaces the enhanced radial cone
with a seeded crest of uneven summits and saddles, broad shoulders and asymmetric
contours per mountain group. Major group centres and inter-group ridge links are
retained; Mountain count now describes groups, not exact summit count. Explicit
version-1 recipes still reproduce their old output and comparison baselines use
version 1. No new sliders, ownership system, dependencies or scene asset writes.

Validation covers multiple substantial summits per group across seeds, isolated
group counts, preserved ridge contributions, legacy height fixtures and existing
bounds/determinism/mesh checks. See docs/design/terrain-generation-pipeline.md.


### TERRAIN-3 — Save generation to scene and terrain collision

Implemented 2026-10-25.

- [x] Save the displayed generated recipe, name and optional terrain collider as a
  UUID terrain entity asset plus distinct root instance in the selected scene.
- [x] Carry the active scene into Terrain Builder and return with the saved object
  selected; explicit destination picker, duplicate display names allowed.
- [x] Native staged validation, bounded inputs, conflict checks, project binding
  recheck and journalled publication; failed inclusion leaves no orphan asset.
- [x] Scene loader builds terrain meshes from versioned recipes for native paused
  inspection and Run, using existing World/renderer SlotMap ownership.
- [x] Typed Box/Terrain collider variants; static Rapier heightfield follows the
  generated mesh, including grid orientation, diagonal, TRS and positive scale.
- [x] Live inspector enable/disable and collider type; dynamic terrain rejected.
- [x] Terrain framing, wider bounded camera orbit and distance-aware far clipping.
- [x] Native mesh/collider agreement, falling-body contacts, deletion/box regression,
  save/conflict tests and a temporary-project browser save/inspection flow.

Recipes regenerate on load; this does not freeze a binary heightfield. Existing
scene lights are retained. Seamless chunks/LOD, height editing, hydraulic erosion
and dynamic terrain bodies remain deferred. See docs/design/saved-scene-terrain.md.


### MATERIALS-IMAGE-1 — Diffuse images and terrain material palette

- [x] PNG/JPEG diffuse texture references with bounded native decoding, EXIF
  orientation, linear-light mip preparation and sRGB GPU colour sampling.
- [x] Triplanar projection and repeat per local metre on objects/terrain, retaining
  existing colour tints, noise/bump, lighting and shadow behaviour.
- [x] Live image swatch/picker and native-validated local image import with unique
  project paths; image references participate in scene revisions and graph links.
- [x] Terrain palettes with stable named layer IDs and one active surface; repeated
  TOML layer edits preserve physics, transforms and other instances.
- [ ] Brush strokes/weight maps, height/slope blending, alpha cutouts, authored UVs
  and shared texture resource caching remain future slices.

This first slice selects a whole-surface terrain layer; it does not yet paint
individual regions. Existing renderer GeometryId SlotMap ownership is retained;
only the active image is decoded/uploaded. See
[design](docs/design/diffuse-image-materials.md).

MATERIALS-IMAGE-1 evidence: native object/terrain image renders, sRGB round trip,
PNG/JPEG import, repeated layer edits and live surface switching verified on
macOS. Full checks are recorded in docs/platform/diffuse-images-macos.md.


### IDE-TEXTURES-1 — AI texture creation and project selection

- [x] Dedicated Texture Creator editor with prompt/name/seed, 1–4 candidates and
  optional tile-friendly prompting; linked from project, Object and Terrain views.
- [x] fal FLUX.1 Schnell integration using existing server-held fal credentials.
- [x] Persisted project-scoped job history, queue/check/download states, exclusive
  paid-submission claims and no automatic resubmission after uncertain outcomes.
- [x] Native image validation and bounded fal-media-only downloads before previews.
- [x] Multi-select candidate saves with durable IDs, readable display names,
  provenance metadata and idempotent/recoverable publication without overwrites.
- [x] Saved generated textures appear in object diffuse and terrain layer pickers;
  multiple terrain materials remain available for later painting.
- [x] Provider/schema/job recovery tests and temporary-project browser integration
  with mocked fal responses, real native validation and real project publication.
- [ ] Brush painting, splat maps and height/slope material blending remain separate
  future slices; prompting alone does not establish seamless tiling.

No paid generation is needed for automated verification. See
[design](docs/design/texture-creator.md) for ownership, persistence and scope.

IDE-TEXTURES-1 validation: 59 IDE tests, lint/types/build and the temporary-project
Texture Creator browser flow passed. Provider responses were mocked; PNG native
validation, project saves and material selection were real. See
docs/platform/texture-creator-macos.md. Live fal access/quality remains unverified.


#### TWEEN-1 — Reusable tweens and motion editor

- [x] Headless Rust scalar lerp and Linear, Sine, Quadratic and Cubic In/Out/In-Out easing.
- [x] Validated immutable recipes, explicit simulation-time sampling and Once/Loop/Ping-pong playback.
- [x] Tween Editor at `/tweens`: moving cube, curve comparison, pause/restart and scrubbing.
- [x] Save/reopen selected variants as schema-1 `assets/tweens/<uuid>.tween.toml`; duplicate display names remain independent assets.
- [x] Native validation on save and raw TOML editing; browser/native sampling parity checks.
- [ ] Scene inspector bindings that attach tween assets to object properties automatically.

Recipes are owned values; game behaviours own time and apply sampled values through
existing World/EntityId APIs. No additional runtime identity or scheduler is introduced.
See [design](docs/design/tween-editor.md) and the public tween API in the user guide.

TWEEN-1 validation: workspace/default and release/all-feature tests, Clippy,
rustdoc and formatting passed; 61 IDE tests, lint/types/build and a real browser
save/reopen smoke passed. See docs/platform/tween-editor-macos.md.


#### CAMERA-2 — Camera authoring and foundation search

- [x] Add Asset search filters names/categories/descriptions and expands matching branches.
- [x] Live camera inspector for asset defaults and per-instance overrides: Perspective/Orthographic, FOV/view height, near/far clipping planes.
- [x] Revision-guarded persistence, native validation and Use asset defaults reset.
- [x] Selected-camera frustum in the editor overlay, following hierarchy transforms and live camera edits without advancing gameplay or restarting the inspection session.

Validation: workspace/default and release/all-feature tests, Clippy/rustdoc/fmt,
64 IDE tests and lint/types/build passed. A temporary-project browser test verified
search, persistence, reset and live frustum changes. See
docs/design/camera-inspector-frustum.md and docs/platform/camera-authoring-macos.md.


#### VIEW-NAV-1 — Object facing and editor shortcuts

- [x] Derived normalized world look-at vector (-Z), shown for each selected scene instance.
- [x] F frames selected native bounds using the editor camera, without saving object changes.
- [x] Shift-F saves the selected object's position/facing from the actual editor camera, preserving scale and resolving its parent transform.
- [x] Pure native placement proposals, guarded persistence, shear rejection and typing/edit protection.

Validation: new engine/project tests and all 68 IDE tests passed, including a real
browser check with native world-transform verification. Full Rust suite still has
an unrelated authored offender bounce-height test failure. See docs/platform/editor-focus-align-macos.md.

---

# Part 3 — former docs/implementation-status.md

## Emergent implementation status

Reviewed 2026-10-24 against the current source, iteration records, ADRs, design
notes, tests, and local project artifacts. This is the delivered-feature ledger
for [GOALS.MD](../GOALS.MD), including features added after the original brief.
The original 2026-10-23 audit was documentation-only. The 2026-10-24 authored-scene
implementation and validation are recorded in [the AS-1 evidence](platform/authored-scene-loading-macos.md). Existing unrelated working-tree edits and assets are preserved.

### Engine foundation

| Delivered | Implementation and evidence | Boundary still open |
| --- | --- | --- |
| Rust engine and fixed game loop | `src/engine.rs`: start/fixed-update/stop lifecycle, configurable fixed step, bounded elapsed-time catch-up, exact headless ticks; foundation tag `v0.1.0-foundation` | Interactive pause/resume/step and a dependency-validated module builder |
| SlotMap world and transforms | `src/world.rs`: typed world-scoped generational identities, parent/child operations, cycle/stale/cross-world rejection, iterative removal and dirty transform propagation; ADR 0001 | General authored-world reconstruction and preserve-world reparenting |
| ECS decision and integration | Standalone `bevy_ecs` for components, queries, resources and schedules; internal mappings preserve canonical SlotMap identity; ADRs 0002–0003; `v0.4.0-ecs-integration` | Broader workload and platform acceptance; `hecs` was evaluated and deferred |
| Behaviour graph | `src/behavior.rs`: typed ports, committed previous-tick snapshots, safe feedback, rewiring and unavailable-input rules; ADR 0004; `v0.5.0-behavior-graph` | Richer behaviours and the public authored-type registry |
| Native graphics | Optional `graphics` feature, wgpu/winit, camera/depth/resize, independent simulation and presentation; ADR 0005; `v0.6.0-first-wgpu-object` | Windows/Linux native evidence and production rendering features |
| First-class geometry | Validated points, solid lines, planar convex polygons and indexed coloured meshes; renderer-owned SlotMap `GeometryId` resources; ADR 0006; `v0.8.0-first-class-geometry` | Concave polygons, richer debug drawing, general materials/textures and culling |
| Procedural primitives | Plane, cube/cuboid, pyramid, sphere, cylinder, cone, capsule and torus generation through the mesh contract; ADR 0007; `v0.9.0-procedural-primitives` | Terrain, noise, particles and other catalogue entries remain planned |
| Light entities | Ambient/directional/point/spot components with validated colour, intensity, range/cones and shadow intent; ADR 0008; `v0.10.0-light-source-entities` | Area/environment lights and broader lighting techniques |
| Lit shading and first shadows | Normals, flat/smooth shading, normal matrices, bounded eight-light Lambert data and one 2048-square directional shadow map; ADR 0009; `v0.11.0-lit-geometry-and-shadows` | Point/spot shadows, cascades, culling, measured quality and frame-time budgets |
| Authored scene loading | `SceneDefinition` validates schema-1 primitive/light/empty-node instances and hierarchy before creating fresh SlotMap entities; shared assets and durable instance IDs | Nested compositions, authored behaviour links and the complete type registry |
| Offscreen scene capture | `GeometryRenderer::new_offscreen` / `render_lit_rgba` use the native shaders and passes, with bounded dimensions and readback waits | Capture is an explicit authoring cost, not live frame streaming |

### Game-project foundation

- `projects/first_project` is an independent Cargo package using public engine
  APIs. `emergent.toml` defines schema version, durable project/output IDs, entry
  scene, validated asset roots and target declarations. Generated Rust is separate
  from authored assets. See `v0.7.0-project-foundation` and
  [the project guide](docs/projects.html).
- The project owns its scene construction and game behaviour. The visible example
  contains a linked moving chain, ground, pyramid, light and spinning cube.
  Default Run opens a native window; `--headless` and `--smoke` are bounded modes.
- macOS is the enabled reference development output; Windows/Linux declarations
  are present. These declarations do not implement multi-output packaging,
  cross-compilation, artifact isolation or target-runner verification.
- Preview and native Run load referenced primitive/light/empty-node entity assets.
  An optional registered Rust builder adds the motion chain; data-only scenes need
  no builder. Preview constructs the world at tick zero and writes a PNG. Add Asset
  can include supported entities in the selected scene, and asset edits change
  the actual Rust-rendered output.
- Live AI implementation produced `src/spin.rs`, wired the cube into the project
  and preserved canonical handles. Tests cover fixed-time rotation and transform
  preservation. Manual review removed an unused generated constant and formatted
  the result; this was not unattended generation. See
  [the recorded reference evidence](design/statement-implementation.md).

Nested composition loading, a general registered-type/schema export, complete
behaviour/asset graph validation and the full multi-instance AI-authored-scene
acceptance proof remain open.

### Scene-instance placement — 2026-10-24

The loader supports per-vector transform overrides on scene instances. The IDE
lists instances and offers numeric position/XYZ-degree rotation/scale controls,
uniform scaling, guarded Apply & reload and Reset to asset defaults. Shared
assets retain their own defaults. Candidate scene source is validated by Rust
before publication, with revision checks for scene and referenced entity files.
See [design](design/scene-transform-inspector.md) and
[validation](platform/scene-transform-inspector-macos.md). Picking, gizmos and general undo/redo remain open; persistent camera navigation is described below.

### Persistent native inspection — 2026-10-24

Inspect now retains the actual Rust world, renderer and geometry in one native
worker at tick zero. Bounded latest-frame PNG delivery and acknowledged controls
support orbit/pan/zoom and saved inspector transforms without restarting that
worker. Restart/Stop, session generations, a disconnect lease and snapshot fallback
are implemented. See [design](design/persistent-inspection-session.md) and
[measurements and validation](platform/persistent-inspection-macos.md). The first
960×540 release-profile sample delivered about 9.4 fps at a 10 Hz target. Hardware
video encoding, picking/gizmos, transient drafts and gamepad input remain open.

### Scene graph authoring — 2026-10-24

The scene graph now leads the left panel, with project files collapsed beneath it.
Drag/drop and a keyboard Parent selector persist keep-local hierarchy edits, which
also update the retained inspection world. Parent translation, rotation and scale
propagate through canonical SlotMap entities. Header Local/Global editing uses
Rust World math; sheared global transforms expose position only, and conversions
requiring local shear fail explicitly. Scene/shared-entity TOML editing validates
an isolated project copy, including other scenes that share the asset, and rejects
stale or invalid writes. See [design](design/scene-graph-authoring.md) and
[validation](platform/scene-graph-authoring-macos.md). Whole-file source changes
require Load/Restart inspection. Preserve-world reparenting, picking/gizmos and
undo/redo remain open.

### Viewport picking and gizmos — 2026-10-24

Persistent inspection now targets 30 Hz (30.1 fps measured through the frame API;
30.0 fps observed in the browser fixture). Rust raycasts the renderer's actual
triangles; authored light/empty nodes expose origin markers. Selection follows
through to the graph and inspector. XYZ translate/rotate/scale gizmos use Rust
parent-aware math and the header Local/Global mode. Drags preview in the retained
world and save once on release; cancellation/conflicts restore the start state.
Gesture tokens protect other clients' drags. Session requests validate project
identity without rescanning files during atomic saves. See [design](design/viewport-picking-gizmos.md)
and [validation](platform/viewport-picking-gizmos-macos.md). This supersedes the
10 Hz and picking/gizmo deferrals in the earlier historical sections. Hardware
video, multi-select, snapping, plane handles, undo/redo and other-platform runtime
validation remain open.

### IDE and developer workflow

| Delivered | Current behaviour and scope |
| --- | --- |
| Local Next.js IDE | pnpm application in `src/emergent-ide`; the linked game lives outside the web application and can be selected through server configuration |
| Real project inspection | Minimal searchable file tree, selected-file details/source, inferred Project Map connections and asset relationships; a placeholder tool area remains below the tree |
| Scoped AI chat/settings | Full-width lower chat follows file selection; server-local URL/key configuration and a working Responses connection; credentials are excluded from project context |
| Persistent Statements | Per-file intentions, immediate check after first save, evidence/confidence and implemented status; save survives unavailable AI checking |
| Reuse-first Implement | Searches project and engine capabilities before generating bounded project Rust/TOML changes; validates reuse citations, stages a Cargo compile, checks source conflicts, publishes with backups/rollback, then independently rechecks |
| Add asset | Categorized foundational templates, safe names/asset-root writes and available/preview/planned indicators; creating a definition does not add a general runtime loader |
| Local Run/Stop | Cargo launch with process status, bounded diagnostics/logs and Stop; native debugger attachment and semantic tick controls remain open |
| Scene selector and Load | Lists authored scenes and displays an actual Rust/wgpu paused capture in the central canvas; unsupported builders fail explicitly and failed reloads retain the previous image |
| Interaction fixes | Chat context keys deduplicated; selection controls chat scope; tree/inspector simplified; asset-modal background contrast and scrolling refined; Object Creator styles load independently of the earlier Tailwind cache issue |

The implementation action writes a restricted set of project files after staged
compilation. Ordinary chat remains advisory. Compilation and an AI check do not
replace runtime tests. The preview is one fixed-camera tick-zero frame per Load;
camera transport, live rendering, picking, runtime inspectors, terrain placement,
pause/step, undo/redo and full project/play isolation remain future work.

Relevant designs: [local run](design/ide-local-run-control.md),
[visual game](design/first-project-visual-run.md),
[asset creation](design/ide-asset-creation.md),
[Statements](design/ide-statements.md),
[statement implementation](design/statement-implementation.md), and
[scene preview](design/ide-scene-viewer.md).

### Object Creator

- `/objects` integrates hosted **Microsoft TRELLIS** through fal's image-to-3D
  endpoint, with server-local credentials, reference-image upload and generation
  settings. Text-to-reference-image generation remains future work.
- Project-scoped job journals preserve provider request IDs, history and cached
  artifacts across refresh/restart. Uncertain submissions are not automatically
  retried as new paid jobs. Transient polling failures remain recoverable.
- Upload/download bounds, scoped paths, structural GLB validation and browser
  Model Viewer provide cached and saved-model inspection with orbit/zoom/reset.
- Save publishes a complete model/reference/descriptor bundle with a durable
  authored ID, provider provenance, collision protection and explicit graph edges.
- A real result is present under
  `projects/first_project/assets/models/a-robot-arm/`. Its descriptor records
  `fal-ai/trellis`, request `01a168ac-3ac2-7cd0-b232-990d3a39e4c1`, seed
  `1677536427` and the 2026-10-23 save. These project artifacts are currently
  untracked. The numeric `texture_size` correction and regression test are also
  present as uncommitted working-tree changes.
- This audit checked the saved model's GLB 2 header and confirmed its declared
  length matches the 1,537,512-byte file. This is container validation, not a
  browser rendering or native engine import check.
- Real hosted submission and bundle saving are evidenced by the persisted local
  job/artifact records. Automated browser viewer/save/restart evidence uses mocked
  provider generation with a real GLB fixture. On 2026-10-24, the actual saved
  robot-arm GLB loaded in Chrome and reopened through descriptor selection after
  a page reload. Visual inspection confirmed the model; no paid request was made.

See [the design](design/object-creator.md) and [OC-1 plan](../plan.md).
Browser GLB preview does not implement Rust model importing, textured materials,
or insertion of that model into a rendered game scene.

### Documentation and coding guidance

- The [HTML guide](docs/index.html) includes introduction, getting started,
  Hello World, projects, capability categories and API navigation/search.
- [The machine-readable catalogue](capabilities/engine-capabilities.toml) names
  graphics, media, simulation and tooling families as available, preview or planned.
  Merely having a catalogue entry does not mean a runtime API exists.
- ADRs, iteration records, design briefs, benchmark reports and platform smoke
  records capture decisions and evidence.
- The locally installed `emergent` skill guides project and engine extension using
  current public contracts. It lives under the agent's skills directory, outside
  the repository, and is not automatically installed for other developers.

### Validation and outstanding acceptance gates

Recorded evidence includes Rust correctness/release suites, formatting, Clippy,
rustdoc, IDE service tests, TypeScript/lint/build, production browser smoke tests,
and a real Rust scene-preview smoke. Read the feature-specific records for scope;
these are not claims that every platform or future capability passed.

- [Iteration 11 native smoke](platform/iteration-11-macos-smoke.md): Apple M3 Pro,
  Metal, 120 fixed ticks, 239 presented frames, resize and clean shutdown without
  wgpu validation errors. Shadow quality and frame timing were not measured.
- [Transform baseline](benchmarks/iteration-2-transform-propagation.md),
  [ECS comparison](research/iteration-3-ecs-comparison.md),
  [runtime integration](benchmarks/iteration-4-runtime-integration.md), and
  [behaviour baseline](benchmarks/iteration-5-behavior-graph.md) provide bounded
  local measurements, not an agreed production frame-time acceptance result.
- Statement implementation evidence records focused Rust tests, staged compile,
  independent evaluation, native smoke and browser failure-path checks, including
  the manual corrections.
- Windows/Linux native verification, minimum platform support policy, broader
  composition/behaviour loading, isolated multi-output builds, complete visual project
  mode and representative production CPU/GPU/memory budgets remain open.

This review reconciles implemented slices with the goals. Broader stage exit gates
remain open wherever these outstanding requirements apply.


### Noise filter assets — 2026-10-24

Filters are now a project asset category. A versioned Noise function definition
stores Perlin/OpenSimplex/Worley selection, seed and bounded octave/domain/field
inputs. Public headless Rust APIs prepare samplers and generate normalized dense
2D squares or full 3D cubes. The IDE inspector generates grayscale previews,
selects volume Z slices and separately validates/saves settings with revision
protection. Raw TOML uses the same Rust validator. This does not yet connect
noise assets to terrain or material consumers. See [design](design/noise-filter-assets.md)
and [validation](platform/noise-filter-assets-macos.md).


### Physics preview (PH-1, 2026-10-24)

Optional Rapier 3D foundation: static/dynamic components, box contacts, gravity,
validated authored defaults/instance overrides, paused collider outlines and IDE
controls. SlotMap identity stays canonical. See [physics foundation](design/physics-foundation.md).
Kinematic motion, other shapes, public queries/events, joints and controllers remain
planned; no physics performance target or Windows/Linux runtime validation claimed.


### Material editor preview (M-1, 2026-10-24)

Renderable Material components and authored defaults/scene overrides; flat/smooth/
unlit shading, linear albedo and specular colour, Blinn-Phong shininess, emission,
noise-driven bump and independent noise pickers/strengths for all four channels.
Maps bake once at upload into renderer-owned fixed 32-cubed RGBA8 volumes under
GeometryId ownership. The inspector saves with revision guards and refreshes paused
inspection. See [material design](design/material-editor.md) and
[macOS evidence](platform/material-editor-macos.md). PBR, image textures, shader graphs,
bloom/emissive lights, displacement and shared material assets remain planned.


### Object shadow controls (SH-1, 2026-10-25)

Material defaults and instance overrides now expose independent cast/receive flags.
Native depth draws omit non-casters and shading ignores shadows on non-receivers.
Collider outlines do neither. Inspector controls reconstruct paused inspection after
save. A separate shadow demo supplies a supported directional sun. The existing
single-map/directional-only limits remain. See [design](design/object-shadows.md).


### Point-light shadows (SH-2, 2026-10-25)

Two point-shadow slots, six 512-square faces each, coexist with the directional
map. Matrix resources are retained; light world position/range and existing object
flags drive the shared depth/shading paths. Native checks cover all faces, moving/
multiple lights, range/slot bounds, object independence and directional coexistence.
See [design](design/point-light-shadows.md). Spot shadows, quality controls, seamless
face filtering and culling/caching remain deferred; other-platform execution is open.
