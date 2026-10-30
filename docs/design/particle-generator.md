# Particle Generator: research and proposed first version

Status: approved 29 October 2026; implemented; validation recorded in [the macOS report](../platform/particle-generator-macos.md). Research checked 28 October 2026;
library assessment was source/API research; no third-party particle dependency was adopted. No paid generation tests were performed.

## Library assessment

Emergent currently uses its own wgpu 30 renderer, standalone bevy_ecs 0.20,
and SlotMap identities. A shared underlying GPU API does not make another
engine's particle renderer directly compatible.

| Candidate | Evidence | Fit |
| --- | --- | --- |
| [Hanabi](https://github.com/djeedai/bevy_hanabi) | GPU particle simulation, textured billboards and modifiers. Published 0.19 uses Bevy 0.19/wgpu 29; current main uses Bevy 0.20/wgpu 30, including Bevy render/core-pipeline/assets. | Strongest game VFX candidate reviewed, but adopting or porting its rendering integration is a separate project. |
| [waterui-particle](https://docs.rs/waterui-particle/latest/waterui_particle/) | GPU compute and instanced particles; published 0.2 depends on WaterUI core/graphics and wgpu 29. | UI-oriented API and framework dependencies; no verified standalone 3D integration. |
| [macroquad-particles](https://docs.rs/macroquad-particles/latest/macroquad_particles/struct.EmitterConfig.html) | Emitters, curves and animated sprite atlases using Macroquad textures and Vec2 controls. | Useful 2D feature reference, but tied to Macroquad rendering. |
| [Fyrox particles](https://docs.rs/fyrox-impl/latest/fyrox_impl/scene/particle_system/index.html) | Particle-system scene node within Fyrox. | Full-engine scene/rendering integration rather than a small renderer-neutral library. |

Recommendation: build the smallest Emergent-owned implementation on the existing
wgpu renderer, beginning with pooled CPU simulation and GPU-instanced billboards.
Evaluate compute simulation only after profiling a representative workload.

## Ownership and data flow

- World entities retain canonical EntityId ownership and hierarchy. A particle
  subsystem owns emitter instances in SlotMap storage with a distinct typed key;
  entity components refer to those keys. Missing/stale handles are rejected.
- Live particles occupy bounded dense pools, not individual World entities.
  Reuse simulation, sort and upload buffers. Simulation uses a fixed step and
  seed; particle counts and catch-up work are bounded.
- Save versioned effect assets with durable asset identifiers, never runtime
  SlotMap keys. One effect can contain multiple emitters, allowing fire, smoke
  and embers to share a transform while using different blend modes.
- Extract visible particle instances into the renderer after simulation.
  Draw camera-facing textured quads with depth testing and depth writes disabled.
  Sort alpha smoke back to front across emitters; use additive blending for
  suitable fire/ember layers. Include depth fading for soft surface intersections.
  Existing geometry texture sampling discards texture alpha, so this requires
  a dedicated particle render path.
- Decode/process media off the frame thread; upload shared sprite/atlas assets
  once. Retire emitter resources coherently when their owning entity is removed.

## First generator scope

Controls on the left, particle preview on the right, matching the terrain editor.
Provide play/pause, restart and fixed-step advance, seeded previews, and smoke,
fire, ember and dust starting presets. Save to an effect library, place effects
in scenes and reopen existing effects from the main editor.

Emitter controls: rate/bursts, loop duration, point/cone/box/sphere spawn shapes,
local/world simulation, lifetime, velocity/spread, gravity, wind, drag, rotation,
size/color/opacity over lifetime and maximum live particles. Support static RGBA
sprites and flipbooks with frame rate and optional randomized starting frames.
Use metres and seconds consistently with the engine.

Defer mesh particles, collision, ribbons/trails, fluid simulation and node graphs.

## Connected fal authoring

Reuse the existing server-side fal credentials, durable media-job queue,
submission protection, result validation and image library. Add a particle asset
workflow with effect-specific prompts rather than seamless surface-texture prompts:

1. Generate or import an isolated effect image with padding.
2. Animate it with a locked camera using a short image-to-video job; retain the
   original clip and model/job provenance. [Wan image-to-video](https://fal.ai/models/fal-ai/wan/v2.2-5b/image-to-video/api)
   is an available candidate, not a selected or quality-tested winner.
3. Produce transparency when required. [Bria video background removal v3](https://fal.ai/models/bria/video/background-removal/v3/api)
   supports transparent output, but requires explicitly selecting Transparent
   rather than its black background default and an alpha-capable codec such as
   WebM VP9. Check returned warnings and decoded alpha; a file extension is not proof.
4. Extract/resample frames locally into a bounded RGBA sprite atlas with frame
   metadata. ffmpeg/ffprobe are installed locally; integration remains untested.
   Keep a consistent canvas/crop across frames, padded atlas cells and an explicit
   alpha convention to avoid filtering halos. Preview loop seams and transparency.
5. Select the resulting asset in an emitter. Particles share atlas textures;
   video is an authoring input rather than a decoder for each runtime particle.

Static sprites and imported atlases remain supported. Background matting may
erase smoke wisps or cause temporal flicker; validate dark and light smoke on a
checkerboard and against scene geometry. Additive fire can use a suitable black
background source, but that does not solve smoke alpha. No claim of volumetric
smoke or automatically seamless generated loops.

Each paid generation stage is user-triggered, shows its configured model and
available cost information, and does not automatically repeat uncertain paid
submissions. No paid fal calls have been made for this research.

## Delivery and validation

1. Prove static/animated smoke and fire assets, transparent billboard rendering
   and depth intersections before committing to a generation model.
2. Implement bounded simulation, effect asset loading and scene integration;
   test deterministic stepping, lifetime/emission boundaries, caps, local/world
   transforms, stale handles, deletion and asset round trips.
3. Build the generator and connected fal jobs; test failure/reload recovery,
   media limits, alpha checks and atlas metadata without paid calls in unit tests.
4. Measure optimized builds with 1,000 and 10,000 live particles, including a
   three-emitter fire/smoke/embers scene. Record hardware, resolution, CPU/GPU
   frame times, allocation behavior and transparent overdraw. These are workloads
   to measure, not promised performance targets.

Run formatting, relevant Rust tests and Clippy, IDE tests/build, and native visual
checks. Use port 3001 for any test service and stop it afterwards. Update public
API documentation and the currently planned effects.particles capability only
when the corresponding behavior is implemented and validated.

## Implemented boundaries

World owns its particle store and synchronizes validated ParticleEffect components
after fixed simulation. The renderer owns sprite resources and a reused instance
buffer. Alpha and additive particles share one depth-sorted premultiplied pass;
additive instances output zero alpha while preserving emissive RGB, so foreground
smoke can attenuate background fire without pipeline changes per particle.

The first editor uses linear birth/death transitions, fixed-step advance and a
browser preview plus native still capture. Scrubbing, GPU simulation, mesh
particles, trails and collision are deferred. Main-editor VFX are independently
sampled at two seconds and frozen; transform edits regenerate this authoring
snapshot without ticking gameplay or physics. Effects are visible during Run
through the engine's actual fixed simulation.

Assets are a versioned .particle.toml recipe plus a .entity.toml reference. A
scene/store reserves at most 100,000 particles; each effect has at most 16
emitters and 64 MiB of decoded unique sprites. Repeated sprite paths share image
storage. All source/media decoding and atlas baking stay outside frame-critical
code. Soft-alpha checks run on the source before transparent atlas padding.

Source video remains in the private IDE media journal; generated atlas assets
contain provenance and are sufficient for runtime playback. Jobs use exclusive
submission claims and never automatically repeat uncertain paid submissions.
Generated loop seams, temporal matting and real fal output quality are unverified.

## Particle Factory effect library

The factory header owns a horizontal swatch carousel over the project's existing
effect/entity assets. Saves and copies appear immediately; updates replace the
same swatch. Thumbnail requests render a small 192x128 native still, auto-framed
around the particles, and cache it in the private IDE data directory. Cache keys
include recipe/entity revisions and sprite bytes. Captures share a bounded serial
queue with manual native previews; no new runtime identity or hierarchy is added.
Short one-shot effects use an earlier sample with at least one simulation step.

Swatch selection uses a native dialog to confirm discarding a dirty recipe;
cancel preserves all edits. Adding a saved swatch to a scene validates and writes
only the scene reference, preserving the current draft and saved effect files.
The destination scene and recipe/sprite dependencies are revision-checked.

The preview/workshop column uses max-content grid rows and one vertical scroller,
so a long image/video history cannot shrink into a clipped grid row. Media frames
reserve their display space before loading. Mobile retains one combined content
scroller with the library fixed above it.


### Appearance controls and blend modes (29 October 2026)

Emitter colours use a colour swatch picker beside compact editable RGBA values
(RGB 0–255, alpha 0–1).
The sprite button opens a project-scoped thumbnail dialog; selecting a different
image resets the atlas layout, while reselecting the current image preserves it.
Queued image/video jobs display a spinner and refresh existing request status
every three seconds. Uncertain submissions require manual review and never resubmit.

Screen uses premultiplied source plus destination times one minus source colour;
Multiply uses source times destination plus destination times one minus source
alpha. Both retain soft intersections and zero-opacity neutrality. Renderer-owned
pipelines are created once; sorted draws split on sprite and blend mode. Canonical
SlotMap particle ownership, instance layout and simulation limits are unchanged.
The browser preview uses matching blend factors with its existing per-emitter
sorting limitation. Native GPU smoke covers ordering, opacity neutrality and
lightening/darkening with both new modes.


Completed workshop sprite candidates and videos expose Delete. Sprite deletion
removes only that candidate's private cached file; sibling results and saved
project images remain. Empty completed image jobs disappear from history while
their submission claim and journal remain, preventing accidental paid resubmission.
Videos use a persisted deleted flag and reject further refresh, baking or reuse of
the same generation ID. Unbaked temporary clips are removed; baked flipbooks retain
their project asset and source clip for provenance. Deletion shares each job's
existing mutation lock. Tests cover scoped deletion, sibling and saved-asset
preservation, reload persistence, API failures and repeat deletion.

### Emitter type library (29 October 2026)

The recipe's Emitter library opens 20 built-in starting points: Fire, Smoke,
Embers, Dust, Rain, Snow, Fog, Falling leaves, Sparks, Steam, Fountain, Water
spray, Bubbles, Fireflies, Pollen, Ash, Confetti, Explosion, Magic aura and Comet
trail. The former four whole-effect preset buttons have been replaced by this
emitter-level picker. The initial campfire recipe remains a layered effect.

Applying a type replaces only the selected emitter, including its sprite and
flipbook settings, marks the effect dirty and resumes the preview. The effect's
identity, name, saved asset and all sibling emitters remain intact. Presets are
cloned before use. They are authoring data using the existing emitter contract;
no runtime ownership, hierarchy or particle simulation changes are required.

These presets use the procedural soft circle. Assign a project sprite to obtain
leaf, snowflake, paper or other specific silhouettes. Falling presets use downward
acceleration from an elevated spawn volume; Confetti and Explosion are one-shot
bursts, and Comet trail leaves world-space particles behind a moving effect.
There is no new collision, flutter or turbulence simulation in these presets.

Validation: all 20 recipes round-trip through TOML, simulate deterministically for
600 steps within their pool limits and pass the native Rust particle loader. The
browser smoke in scripts/particle-emitter-library-smoke.ts applies and saves every
type to the middle layer of a three-emitter effect, checking sibling preservation,
effect identity, dirty state, sprite reset, Escape and mobile selection.


### Sprite-centred video generation (29 October 2026)

Video generation belongs to the clicked image, independent of emitter selection.
The former Animate selected sprite button and emitter seed/source dependencies
are removed from the workshop. Clicking or right-clicking a generated sprite opens
Sprite options, showing that image, an editable motion prompt, Generate video,
Use on selected emitter and Delete result. Choosing an action closes the native
dialog before any async work. A synchronous submission guard rejects double-clicks.

The same Sprite options dialog is available by right-clicking a saved image or
candidate in Image Generator. Generated videos are listed in its project Image
library, with queued/running status, polling, playback and deletion. The existing
project-scoped media journal remains the durable video record; deleting a result
uses the existing scoped deletion contract.

Unsaved generated images use a texture-job:<job UUID>:<result UUID> source handle.
The server resolves that exact candidate through TextureJobs against the current
project and private cache, and uploads those bytes. It does not silently promote
the image to project assets or change any emitter. Existing project PNG/JPEG paths
remain supported. Request IDs, single-submission provider transport and uncertain
submission handling retain their existing contracts.

Validation includes source-byte selection, project isolation, missing/deleted
candidates, and duplicate submissions. Browser smoke verifies right-click and
mobile access, dialog closure before submission, emitter independence, queued to
ready playback, and video visibility in Image library after reload. Provider calls
are mocked; no paid fal requests are needed by these checks.

Image Generator also exposes Delete beside each completed candidate and in its
Sprite options dialog. It uses the same scoped candidate-deletion endpoint as the
workshop. Deleting the last candidate removes its history entry and selects the
next request; saved project-library copies remain. The image-delete browser smoke
covers failures, sibling preservation, context actions and mobile layout.

### Unified Image, Sprite and Video library

Image Generator owns image generation, sprite labels, video generation/import,
background removal and local video-to-atlas baking. Sprite is metadata attached to
an existing image, not a separate generated asset or engine object. Project-relative
`<image>.library.json` sidecars store a versioned Image/Sprite label and columns,
rows, frame count and fps. They do not replace generation provenance. Existing
images default to Image. Videos retain their existing scoped generation journals.

Right-click opens image actions and sprite-sheet configuration with row-major
animated playback. Library configuration is validated and persisted on the server;
particle authoring copies those settings into the selected emitter on assignment.
Runtime particle identity, SlotMap ownership, rendering and file formats remain
unchanged. The particle editor owns only emitter controls, preview/capture and
library assignment; it links back to Image Generator for media authoring.

Validation gates: metadata bounds and path/symlink isolation, classification and
atlas settings surviving reload, correct preview frame cycling, exact emitter
assignment, video import/baking retained in Image Generator, and no workshop in
the particle editor. Existing deletion/reference protection must remain intact.

### Sprite animation families — 30 October 2026

Sprite animation uses the existing project-scoped media journal with an explicit
sprite output intent. A durable family UUID in image-library sidecars groups the
original image and generated atlas variants. No runtime identities change. Fal
produces an intermediate clip; server refresh prepares the existing 32-frame atlas
locally and publishes it as a Sprite variant. The original is never overwritten.
Sprite jobs stay out of Video UI. Repeat generations retain independent job/asset
UUIDs, so deletion affects one variant. Runtime references still prevent deletion;
generation-provenance links do not pin otherwise unused originals. Failures remain
visible and local atlas preparation can retry without another paid submission.

## fal model selection (2026-10-30)

Image Generator, sprite/video animation options and Texture Creator share a server-side fal catalogue reader. `GET https://api.fal.ai/v1/models` supplies active models by category, search and pagination; selecting a non-default model retrieves its OpenAPI schema. This authoring metadata has no runtime identity or SlotMap storage. The saved fal key stays on the server. Lists are cached for one minute and schemas for five minutes, each bounded to 100 entries; requests have time and response-size limits.

The editor supports prompt-to-image models returning `images[].url`, and image-to-video models accepting `image_url` or `start_image_url` and returning `video.url`. Unsupported output shapes, composed schemas or additional required inputs without defaults block submission with an explanation. Supported alternatives use model defaults, with PNG output and disabled audio where those fields are exposed. The existing defaults retain their established explicit settings. Native image/video validation and download limits still apply.

A selected endpoint and actual input settings are persisted in the existing job journal and provenance. Submit/status/result all use that endpoint after a restart; selection changes cannot alter an existing job. No new paid-job subsystem or frame-critical work is introduced. Playback and generation explanations are available through focusable `(i)` tooltips.

## Ping-pong sprite playback

Sprite sidecars and emitter recipes gain a backward-compatible `ping_pong` boolean, defaulting to false. It is authoring data; runtime emitters retain existing SlotMap ownership. No atlas pixels, extra textures, or fal jobs are created. Frame selection reflects a bounded integer phase over `2 * (frames - 1)`, with one-frame sheets remaining static and no duplicated endpoint holds. Image preview, browser particle preview and Rust billboard extraction use the same sequence. Random starting frame offsets retain their existing meaning. Validation covers forward/reverse/wrap sequences, one/two/partial sheets, persistence, assignment, old recipes and native capture. This adds constant-time arithmetic to existing frame extraction, with no new allocations; no frame-time improvement is claimed.
