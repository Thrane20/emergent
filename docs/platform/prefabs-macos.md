# Prefabs — macOS validation

Date: 2026-11-01. Host: macOS / arm64, Apple M3 Pro. Native browser smoke
used Chrome and the Rust renderer through the standard-template project host.
No performance target or Windows/Linux execution is claimed.

## Delivered

Named project-local `.prefab.toml` assets capture one parent and all descendants.
The IDE library and viewport placement path instantiate fresh-ID snapshots;
registered entity fields and shared-brain bindings resolve within each instance.
`PrefabDefinition` loads prepared construction data once, supports independently
named repeated spawning, and returns the real root plus member handles using
World's SlotMap identity. Media and entity defaults remain shared.

## Checks

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- `cargo doc -p emergent --no-deps --all-features`: passed.
- `cargo test -p emergent --release --all-features`: passed (190 tests, one ignored
  timing test). After adding the final named-instance and project-reference
  regression coverage, reran the affected optimized suites:
  `cargo test -p emergent --release --all-features --test authored_scene --test brains`:
  29 authored-scene tests and 9 brain tests passed; one timing test ignored.
- `cargo test -p emergent-standard-template --all-features`: passed; compiles the
  shared host fixture (no unit tests in the template).
- `cargo test --workspace --all-features`: engine tests passed; first-project demo
  tests failed: `authored_cube_spins_fifteen_degrees_in_one_second` cannot find its
  expected scene entity, and `authored_offender_bounces_about_four_metres_above_ground`
  measured 0.274 m against 4 +/- 0.2 m. Demo source/assets were not changed for
  prefabs. The template target was checked separately after Cargo stopped.
- `npm test` from `src/emergent-ide`: 164 passed, zero failed.
- `npx tsc --noEmit`: passed; production build also passed its TypeScript stage.
- ESLint on the changed IDE implementation, prefab tests and smoke script: passed.
- `EMERGENT_NEXT_DIST_DIR=.next-prefab-build npm run build`: passed.
- `npm run test:prefabs`: passed against that production build, using an isolated
  copy of `templates/standard` and isolated IDE state on **port 3001**.

The browser test creates a body plus four child rotors, a shared brain naming a
rotor and child Look at components naming the body. It saves the five-member
Drone prefab, places Drone A and Drone B through the actual editor controls,
checks distinct IDs and each instance's own references, waits for native frames,
and deletes Drone A through the tree while preserving Drone B and its children.
It also rejects stale revisions, invalid names and cross-origin saves. No browser
page errors were observed. Screenshot: `/tmp/emergent-prefabs.png` (ephemeral).
The script closes native inspection and the port-3001 service and removes its
project/state directory; the user's port-3000 service is left running.

Browser QA caught and corrected controls outside the internally scrollable graph
region. New prefab publication uses the existing staged Cargo/candidate checks
because native candidate-path validation requires an existing file. Rust tests
cover fresh/stale handles, independent deletion, repeated spawning after deleting
the loaded source file, root/child transforms, component links, invalid graphs,
project entity references, and shared brains with distinct bindings.

## Remaining boundaries

Copies are snapshots: no recursive prefab assets or linked update/override system.
External entity links are rejected; group names retain their existing world-wide
semantics. Audio remains a planned subsystem. Spawn allocations, large graph
memory and setup latency have not been benchmarked. Meshes/prepared entity data
are cloned on runtime instantiation; this is not allocation-free frame work.

Repository-wide `git diff --check` reports an existing extra blank line at the
end of `docs/design/object-components.md`; that unrelated file was left untouched.

## Drag library and name lookup update — 2026-11-01

The Prefabs section is now an immediate capture target and a scrollable name/part
count table. The save form and selector were removed. Drop a scene object anywhere
in this section to capture it under its current name, retaining the source. Drag
a prefab row onto the scene root/empty graph or onto an existing parent to
instantiate. Keyboard Enter supports capture of the selected subtree and root
placement of a table row. Scene-tree renaming remains the instance naming action.
Project, scene, revision and missing-object checks run at drag boundaries; native
candidate validation, journals and unsaved-work guards remain in use.

`PrefabDefinition::load_named` and `load_named_with` search the project's declared
asset roots for an exact unique label. Missing/ambiguous names are errors, and
explicit path loading remains supported. Discovery skips symlinks, including
cycles. The registry-aware method shares the existing scene validator. Discovery
is loading-time I/O, not a per-frame registry. No new performance claim is made.

Validation:

- Focused Rust prefab tests: 4 passed, including nested label discovery, exact
  case, missing/duplicate names, explicit-path fallback and symlink cycles/aliases.
- `cargo test -p emergent --release --all-features`: passed, including 30 authored
  scene tests and 9 brain tests; one timing test ignored.
- `cargo fmt --all -- --check`, workspace/all-target/all-feature Clippy with
  warnings denied, and all-feature rustdoc: passed.
- Full IDE suite run serially with
  `npx tsx --conditions=react-server --test --test-concurrency=1 tests/*.test.ts`:
  **170 passed, zero failed**. The initial parallel run passed 169/170; the UI
  chat/candidate fixture reported a missing staged `src/generated` directory.
  That fixture passed alone, then the entire suite passed serially. A shared
  candidate-binary/fixture race is suspected, not confirmed or fixed here.
- TypeScript, ESLint on the changed implementation/tests/smoke, and the isolated
  production build: passed.
- Disposable standard-template Chrome/native smoke on port 3001: checks real
  drag capture with automatic naming, table overflow scrolling with 12 library
  fixtures, removal of forms/selectors, root and parented prefab drops, existing
  instance renaming, isolated internal links, independent subtree deletion,
  keyboard capture/placement and rejection of stale drag/revision/name/origin.
  The smoke also asserts only one scene graph exists: browser QA caught a sibling
  React-key collision during this UI change, which was corrected. Successful IDE
  saves/placements now show their own result instead of a misleading AI-edit
  warning when the originating operation is still busy.
- Workspace tests again stop at the same two first-project demo-scene failures
  described above; the engine suites pass.

The browser smoke closes its native worker and port-3001 service and removes its
disposable project. The user's port-3000 service is not modified. Current screenshot
is `/tmp/emergent-prefabs.png`; no Windows/Linux run or spawn/discovery benchmark.
