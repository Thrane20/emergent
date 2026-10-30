# 30 Hz inspection, picking and gizmos — macOS validation

Date: 2026-10-24. Apple M3 Pro, macOS/Metal, Rust/Cargo 1.99.1.
Native worker: Cargo release profile, fixed 960×540 RGBA capture, fastest PNG
encoding, latest-only HTTP frame delivery. Gameplay remains at tick zero.

## Reference measurements

Same checked-in main scene on this host. Initial baseline: 40 delivered frames,
first ten discarded. After: 25 frames, first five discarded. Short local samples,
not a general frame-time or large-scene acceptance benchmark.

| Measure | Before | After |
| --- | --- | --- |
| Target cadence | 10 Hz | 30 Hz |
| Delivered frame API rate | 9.60 fps | 30.06 fps |
| Render/readback | 1.50 ms mean | 1.71 ms median |
| PNG encode | 1.91 ms mean | 1.74 ms median |
| Median PNG payload | Not recorded in this baseline | 144,061 bytes |
| Camera acknowledgement | Not recorded in this baseline | 29 ms |

A first per-frame sleep implementation delivered about 25 fps because wakeup
delays accumulated. Deadline pacing corrects that drift without catching up
missed frames. Browser fixtures observed 30.0–30.2 delivered fps after removing
asset rescans from session traffic. The fixed scene is still a small workload;
PNG bandwidth/readback costs remain and heavier scenes can miss the target.

## Correctness and integration

- Rust workspace debug and release/all-feature suites pass: 53 engine unit tests,
  nine authored-scene tests, two ECS tests, three project unit tests and one
  project-contract test. New tests cover projection/unprojection, clip checks,
  triangle misses and distance ordering under nonuniform models, absolute drag
  snapshots, parented Local/Global translation/scale, rotation reconstruction, stale handles and shear rejection.
- Rust formatting, Clippy with warnings denied, rustdoc with warnings denied and
  a headless/no-default-features check pass.
- IDE service tests (29), TypeScript, lint and production build pass. Protocol
  checks cover bounded projected results, malformed controls, coordinate mapping,
  letterbox rejection and clamped pointer capture. Binding checks reject a changed
  project identity without depending on files being authored.
- `test:gizmos` uses headless installed Chrome and the actual retained native Rust
  renderer. A near object occludes a far instance. Real mouse drags translate X,
  rotate Z and scale X; moves leave files untouched and release publishes only
  the intended scene instance. Light-origin marker selection updates the tree.
  The shared asset and other instance retain their values.
- Escape restores the starting retained transform without saving. A scene edit
  during a drag rejects publication and preserves the external change; the native
  transform restores to the saved placement. A wrong gesture token cannot cancel
  the current drag. Session UUID, worker PID and tick-zero identity are retained.
- Existing `test:graph`, `test:placement` and `test:inspection` pass, covering
  hierarchy/global edits, guarded TOML authoring, real frame changes, orbit/pan/zoom,
  restart/stale rejection, stop and inactivity expiry. Production `test:ui` checks
  the surrounding IDE workflow with generation mocked; no paid AI/provider calls.
- Screenshots verified the toolbar, coloured projected handles, selected tree row,
  light marker and matching inspector, plus visible conflict feedback.

An integration failure exposed frame requests rescanning the project while a save
renamed a temporary file. Session routes now validate only the current manifest
ID/root; starting a session and publishing authored changes retain their complete
project validation. This removes that save/poll race and per-frame asset reads.

Tests use uniquely named temporary assets and clean up their own worker/files.
The user's port-3000 server stopped responding during testing; subsequent checks
used a separate dev server on localhost:3001. No unrelated listener was terminated.

Limits: one authored selection, XYZ axis/ring handles and fixed viewport size.
Near-parallel axes are hidden. Sheared Global rotation/scale cannot be represented
as local TRS and are rejected; use Local. Snapping, multi-select, plane/free
handles, undo/redo, accelerated mesh BVHs and hardware video remain open.
Windows/Linux runtime validation is pending. Runtime SlotMap keys are never sent
or saved; only authored scene IDs and transient gesture tokens cross the channel.


## Follow-up interaction validation

Global Scale now scales the world linear transform along the selected world axis
about the same pivot, then converts back to local TRS. A Z90 object maps global X
to local Y; a Z45 object rejects an anisotropic global X change that requires
shear. Returning to the initial pointer position returns the exact original
transform, avoiding decomposition drift on a no-op drag.

Expanded browser coverage includes a synthetic four-point clipped rotation-ring
projection (with native Rust edit/persistence), arrow-key editing, pointer-cancel
rollback, a Z90/scaled parent with Global translation, accepted global Z scaling
and rejected world-axis scaling that requires shear. Cancel/finish responses are
checked against saved source to verify native state restoration. Malformed tool
results with fewer than two projected points are rejected before reaching the UI.

A rapid sequence of Global-mode drags exposed a stale-revision window while the
Global inspector report refreshed. Gizmo publication now immediately installs its
returned placement snapshot. Global handles stay disabled until matching Global
metadata arrives; the next drag cannot reuse the pre-save revision.
