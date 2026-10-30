# Bounded ray-marched terrain prototype — 2026-10-30

Apple M3 Pro, macOS, optimized Rust build, native wgpu offscreen renderer.
One 32m analytical field, seed 42: bounded ground, spherical mountain with a
subtractive tunnel, torus arch. These are structural test shapes, not a finished
geological generator. No dense voxel allocation, streaming or physics collider.

## Correctness

`cargo run --release --example raymarched_terrain_smoke --all-features` passed:

- Mesh behind the field is occluded; foreground mesh remains visible.
- Removing the field restores the background.
- Perspective, orthographic and inside-solid exit rays render successfully.
- 256 overhead CPU reference ray queries agree with sampled GPU silhouettes,
  allowing three colour levels for framebuffer quantization.
- Native cave, arch and overview captures were visually reviewed under `/tmp/`:
  `emergent-raymarch-cave.png`, `emergent-raymarch-arch.png`,
  `emergent-raymarch-overview.png`.

Two headless regressions cover solid/empty points, cave/arch openings, seed
differences, ground hits, inside exits, invalid parameters and bounded misses.
These queries do not register collision with Rapier.

## Capture timings

One warm capture followed by five measured captures per camera/viewport. Medians
include CPU submission, GPU completion, allocation and RGBA readback; they are not
GPU timestamp durations or representative gameplay frame budgets. Other builds
were active; repeat under isolated conditions before a performance decision.
256-step cap, 0.002m hit tolerance, 200m trace limit. Fixed Lambert/ambient shader;
no scene shadows or material textures.

| Viewport | Overview median | Cave median | Arch median |
| --- | --- | --- | --- |
| 128 × 128 | 0.627 ms | 0.666 ms | 0.680 ms |
| 256 × 256 | 0.747 ms | 0.719 ms | 0.755 ms |
| 512 × 512 | 1.076 ms | 1.316 ms | 1.313 ms |
| 1280 × 720 | 2.248 ms | 2.710 ms | 3.277 ms |
| 1920 × 1080 | 3.090 ms | 3.045 ms | 2.772 ms |

1080p ranges: overview 2.941–4.843ms, cave 2.938–3.105ms, arch 2.755–3.085ms.
No p95/p99 or heightfield comparison is claimed from five samples. Resource bytes
and representative traversal remain unmeasured.

## Existing 1024 terrain regression

Production IDE build and `pnpm exec tsx scripts/terrain-scene-inspection-smoke.ts`
passed against owned IPv6 port 3001. Existing saved 1024 terrain loaded in the main
scene graph, selection enabled transform inputs, and native orbit changed pixels.
Project files were unchanged; owned service/inspection session stopped. User port
3000 and the existing IPv4 VS Code helper were untouched.

## Build and test checks

- `cargo test -p emergent --all-features --release`: passed (95 library tests plus
  authored-scene, ECS mapping, physics and units integration suites).
- `cargo test --workspace` and `cargo test --workspace --all-features --release`:
  engine tests pass; both reach the existing first-project
  `authored_offender_bounces_about_four_metres_above_ground` failure at 3.340048.
- Workspace all-target/all-feature Clippy with `-D warnings`, rustfmt check and
  workspace rustdoc: passed.
- `cargo check -p emergent --no-default-features`: passed with the existing
  unused `TerrainMaterial.layers` warning.
- IDE production build, native browser regression, capability TOML parsing and
  local plan-link validation: passed.

## Scene and Terrain Builder integration — 2026-10-30

Terrain Builder now has an additional Ray marching experimental option. Native
captures respond to seed/width and orbit/elevation inputs. A project-scoped carousel
supports save/load/copy and Add to scene. Shared updates retain asset identity and
transforms; staged native validation and before-source revisions guard publication.

`kind="raymarched-terrain"` with `[raymarched_terrain]` is an authored entity asset.
The loader creates ordinary World components and rejects physics/material/heightfield
attachments. Up to eight regions use renderer-scoped typed SlotMap handles. Local
ray-box clipping bounds work; local tracing/world-depth projection follows transforms,
with inverse-transpose normals. Renderer deletion rejects stale/foreign handles.

New verification:

- All **127 IDE tests** passed, including recipe bounds, independent save, stable
  identity/transforms/comments, exact recipe load, scene placement and stale conflicts.
- New authored-scene Rust test passed: multiple instances/parenting use distinct
  World identities, invalid sizes/mixed geometry/physics/materials and excess regions
  are rejected before construction. Engine all-feature release tests pass.
- Native resource smoke passed transformed regions, shared mesh depth, stale key
  retirement/reuse, foreign-renderer rejection and CPU/GPU silhouette agreement.
- `scripts/raymarched-terrain-smoke.ts` passed against an owned production service
  on IPv6 port 3001 with a disposable project. It exercises entry from Heightfield,
  real native preview, dirty Cancel/Discard, library save/load, scene placement,
  main native rendering/orbit, numeric transform save, Edit terrain reopening,
  explicit collision notice, stale API rejection, mobile layout and native Run smoke.
- Production build, TypeScript, targeted ESLint, workspace Clippy/rustfmt/rustdoc
  passed. Full release workspace still reaches the pre-existing bounce-test failure.
- Captures: `/tmp/emergent-raymarch-builder.png`, `/tmp/emergent-raymarch-scene.png`,
  `/tmp/emergent-raymarch-mobile.png`. Owned service/native sessions stop on cleanup.
  No user project assets or paid-provider requests were involved.

These checks establish scene/editor integration, not a streaming or gameplay
performance target. Earlier timing tables describe the initial prototype workload;
do not interpret them as measurements of eight transformed regions.

## Remaining acceptance

Collision extraction/agreement, scene lighting/shadows, material painting,
geological field generation, camera routes/step-exhaustion diagnostics and an
equivalent heightfield comparison remain open. Builder preview is a native capture
updated by controls; continuous free camera movement is available in scene inspection
and Run. Windows/Linux runtime rendering has not been tested.
