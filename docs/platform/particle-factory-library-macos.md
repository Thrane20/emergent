# Particle Factory library validation — 29 October 2026

The existing effect assets now appear as native still swatches in a horizontal
carousel beside the Particle Factory heading. Saving updates the same swatch;
Save a copy adds another asset. Selecting a swatch loads its saved recipe. A native
dialog asks before discarding a dirty draft, and cancel retains its values.

Each swatch can add its saved recipe to the selected scene without changing the
open draft or rewriting the effect asset. Placement validates the scene and
rechecks effect/sprite dependencies before publication. Existing revision guards,
project binding checks and durable authored identity remain in use.

Native thumbnails are 192×128, auto-framed around a seeded sample. Short one-shot
bursts are sampled earlier, with at least one simulation step. Captures share a
bounded serial queue with manual previews. Private IDE caches are keyed by recipe,
entity and sprite revisions; old cached revisions are retired. The runtime engine
and its SlotMap ownership model are unchanged.

The preview/workshop column uses max-content grid rows. Long media histories no
longer shrink into clipped rows, and videos/atlases reserve their display space
before loading. The header grid also bounds the carousel to the viewport width;
only the carousel scrolls horizontally. Mobile keeps the combined content scroll.

Verified on macOS / Apple M3 Pro:

- 94 IDE tests pass; focused tests cover thumbnail caching, duplicate-request
  coalescing, recipe invalidation, short bursts, invalid paths and scene placement.
- Production Next build, TypeScript and targeted ESLint pass.
- Engine release/all-feature tests, workspace all-target/all-feature Clippy with
  warnings denied, and formatting pass.
- Chrome smoke uses a temporary project and port 3001. It checks an overflowing
  library with 18 effects, visible scene/navigation controls, no horizontal page
  overflow, real native thumbnail dimensions, both discard-dialog choices, clean
  selection without a dialog, and saved-effect placement while retaining a dirty
  draft and byte-identical saved recipe.
- The same smoke creates a long workshop history with 20 image-result controls,
  an imported video and a baked atlas; the final candidate, flipbook action and
  footer are reachable. Mobile save controls are reachable too.
- Existing save/update/reopen, native captures, main-editor selection and a native
  120-tick Run with resize and shutdown pass. That smoke exposed a late redraw
  after shutdown; the native app now ignores redraws once it has stopped.

Evidence: [factory](../benchmarks/particle-factory-2026-10-29/factory.png),
[discard dialog](../benchmarks/particle-factory-2026-10-29/discard.png),
[workshop bottom controls](../benchmarks/particle-factory-2026-10-29/workshop-scroll.png).

No paid fal requests were made. First-use thumbnails require the existing native
renderer; cached stills are reused thereafter. Unsupported GPU environments show
a preview-unavailable tile while retaining edit/place actions. Thumbnails are
representative seeded stills rather than animated swatches. Windows/Linux were
not run. The previously documented full-workspace bounce-height failure remains
outside this change; the engine's own release tests pass.
