# Unified Terrain Builder acceptance — 2026-10-30

macOS, Apple M3 Pro/Metal, Chrome. This increment integrates authoring and live
preview; it does not establish a gameplay frame-time or streaming budget.

## Delivered

- One Terrain Builder route, shared Height Field / Ray March switch and biome modal.
- Eight biome choices: cave/cavern, canyon, underwater/seabed, field/plain, hilly,
  mountainous, island and rivers. Cave/cavern is visibly unavailable in Height Field.
- Plain/canyon/seabed extend heightfield generation without changing the prior four
  preset outputs. Explicit ray-march biome branches are mirrored in CPU and WGSL.
  Recipes without `biome` retain mixed v2 output; v1 fixtures remain compatible.
- Ray March uses the retained native `terrain_live_preview` worker and existing
  camera-only InspectionViewport. Drag, Shift-drag, wheel and projection controls
  update uniforms in the same process. No draft assets are published for preview.
- Session incarnation IDs prevent an old cleanup from stopping a new identical
  draft. Startup cancellation handles effect replay; stop/lease/reaping use the
  existing inspection manager. Command channels and framed PNG packets are bounded.
- Parameter changes require Generate before saving the displayed recipe. Existing
  project transactions, revisions, scene transforms and SlotMap identity stay intact.

## Validation

- 128 IDE tests passed. Focused inspection lifecycle, biome mapping, ray-march
  validation/persistence tests and targeted ESLint passed. Production build and
  TypeScript passed.
- Engine all-feature release tests passed, including distinct biome signatures,
  sampled distance-gradient bounds, legacy decoding and new bounded heightfields.
  Workspace release reaches the existing unrelated offender bounce failure (3.340048).
- Native biome example passed kilometre coverage/depth queries and 4km rendering.
  Captured all eight biome branches, including an inside-cavern view; visual gallery
  reviewed under `/tmp/terrain-biome-*.png`.
- Workspace Clippy with warnings denied, rustfmt and rustdoc passed.
- Browser smoke passed Height Field canyon generation, eight modal entries, cave
  incompatibility notice, method/biome transfer, live orbit/pan/zoom with stable
  worker PID, old-worker reaping after generation, cave interior preview, guarded
  save/load, placement, numeric scene transform, Edit terrain reopening, mobile
  access and native Run. Disposable project; no user asset writes or paid calls.
- Owned production service used IPv6 port 3001 and stopped with its workers. User
  port 3000 and the existing IPv4 helper were not changed.

Commands: `pnpm test`; `EMERGENT_NEXT_DIST_DIR=.next-unified-terrain-build pnpm build`;
`EMERGENT_NEXT_DIST_DIR=.next-unified-terrain-build pnpm exec tsx scripts/raymarched-terrain-smoke.ts`;
`cargo test -p emergent --all-features --release`; `cargo test --workspace --all-features --release`;
`cargo run --release --all-features --example raymarched_biome_smoke`;
workspace Clippy/rustdoc/rustfmt checks.

## Limits

Underwater is seabed geometry, and rivers are carved valleys: water rendering/
simulation is separate. Ray-marched collision, material-library painting and scene
lighting/shadow parity remain open. Rendering was not checked on Windows/Linux.
Framed preview capture is authoring infrastructure, not a game loop.
