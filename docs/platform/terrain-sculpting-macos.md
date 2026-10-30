# Terrain sculpting validation — macOS, 2026-10-28

Observed on the local macOS development host:

- IDE `npm test`: 72/72 tests pass. Targeted terrain/scene-placement rerun:
  14/14 pass. Tests cover opacity/strength, bounds, smoothing, flattening,
  GLB sample extraction, frozen saves and malformed fields.
- TypeScript `npx tsc --noEmit`, focused ESLint and `npm run build` pass.
- `npm run test:terrain-save`: Chrome browser smoke passes against a temporary
  project and its own port-3001 service. Exercises all brush modes, hold/drag,
  undo, narrow layout, exact frozen saved samples, native scene validation/render,
  collider inspection and disabling physics. Temporary files/service are cleaned.
  The inspector requires a longer wait for native recompilation on first load.
- Visually inspected the browser screenshot with sculpt controls, grayscale mask,
  footprint, top-down heightmap and updated 3D surface.
- `cargo test -p emergent --all-features` and the same with `--release` pass.
  New tests verify frozen mesh samples, invalid snapshot/path rejection, collider
  triangles matching the edited mesh, and a cube resting at 5.5m on a frozen 5m
  surface rather than the procedural recipe. After retaining the original
  recipe-only collider path, all 18 authored-scene tests pass again.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo doc --workspace --all-features --no-deps`, formatting and
  `cargo check -p emergent --no-default-features` pass.
- Full workspace debug and all-feature release runs reach the first project's
  `game::tests::authored_offender_bounces_about_four_metres_above_ground` failure
  at `projects/first_project/src/game.rs:307`, reporting 3.1047554. The engine
  suites pass; that game behavior was outside this change.

No performance target or Windows/Linux runtime validation is claimed. Brush
painting is top-down, with GLB refresh on stroke completion. Image-brush import and
direct 3D painting remain outside this slice. Reopening saved terrain is covered
below. Regeneration replaces unsaved edits; undo stores one stroke.


## Default brush interaction regression — 2026-10-28

A normal fast drag on the original Hilly 128x128 field at strength 0.5 changed
only 698 grayscale pixels, by at most 1/255. Pointer-down also prevented native
canvas focus, breaking the suggested click-then-Space workflow. The compact
window clipped the bottom of the heightmap.

With distance-based stroke deposition, initial click exposure and final-segment
flushing, the same browser drag at default settings changed 2580 pixels by up to
18/255. Canvas focus remains after clicking; Space changed 424 pixels and Undo
restored the previous image exactly. The full 200x200 painting square is visible
at 1920x900. Verified in an isolated browser tab/service on port 3001, without
changing the user's open terrain state or saving project assets.

All 10 terrain tests, TypeScript, focused ESLint and production build pass. The
new unit regression requires a fast default-strength stroke to change heights
by more than eight grayscale levels, including its final segment; split events
retain equivalent strength. The save smoke now also checks click focus, Space
and visible fast drags. The full native save smoke was not rerun for this IDE-only
interaction change.

## Reopen existing terrain — 2026-10-28

Observed in a temporary copy of the project served on port 3001:

- Selected a terrain in the main editor, opened Edit terrain, and loaded the
  exact saved 32x32 field without regeneration. The loaded name, extent and
  resolution matched the asset; Save terrain changes starts disabled.
- Painted and saved in place. Staged Cargo/native validation passed for all
  project scenes, including a second scene referencing the same terrain asset.
  The scene still had two original instances; terrain asset UUID/path, instance
  ID, translation [2,0,4], green material and friction 0.8 were preserved.
- View in scene returned with the same instance selected and the native preview
  showing the sculpted surface. Further edits and two successive saves succeeded
  with updated revisions.
- Each Edit terrain action now starts a fresh keyed editor load through client
  navigation. Reopening showed no old save status or undo history, and Save
  remained disabled until a new stroke. This fixes reuse of cached editor state.
- API checks confirmed exact sample roundtrip, rejection of stale revisions,
  non-terrain targets and wrong project IDs, and actual Rust regeneration when
  loading a recipe-only terrain.
- All 74 IDE tests passed, plus the targeted terrain-save test after adding asset
  and scene conflict cases. Tests prove unchanged material/physics/transform
  fields, asset IDs and scene text; conflict-checked publication does not overwrite
  heights if an asset changes after preparation. TypeScript, focused ESLint and
  production build pass.

No Rust implementation changed. The full Rust/platform suites were not rerun.
Updates intentionally apply to every instance sharing the edited terrain asset.
