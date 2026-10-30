# Scene Transform inspector — validation

Date: 2026-10-24. Apple M3 Pro, Metal; Rust/Cargo 1.99.1.

- Rust workspace debug and release/all-feature suites pass, including nine
  authored-scene tests. New coverage verifies partial vector overrides, inherited
  asset scale, parent-local placement, independent shared-asset instances, XYZ
  degree rotation, candidate rejection without source mutation, and reset.
- Rust formatting, Clippy with warnings denied, and rustdoc pass.
- IDE service tests (21), TypeScript, lint and production build pass. Tests cover
  safe edits/reset of one instance, preservation of unrelated source/comments,
  malformed values, asset defaults and revision conflicts.
- `pnpm test:placement` passes against the user's running localhost:3000. A
  temporary scene uses two instances of one asset. Browser editing changes
  position, rotation and uniform scale, updates the real Rust frame, preserves
  the other instance and leaves the shared asset unchanged. Page reload restores
  saved values; reset removes the override. External source edits reject a stale
  save. A finite but overflowing scale is rejected by Rust before publication.
- Browser screenshot inspection verifies the compact instance list and inspector
  layout. Placement styles use a separate stylesheet to avoid the known stale
  Tailwind/Turbopack CSS cache path.
- Existing production `test:ui` and real-Rust `test:authored` smoke checks pass.
- Native `cargo run --release -p emergent-first-project -- --smoke` passes:
  seven geometry resources, 120 ticks, 240 presented frames, resize and clean
  shutdown. The user's already-running game process was left alone.

Smoke tests create and remove only uniquely named test assets. No AI/model-provider
calls are needed. Saved edits affect the paused preview and the next native Run;
they do not mutate a currently running game session. Windows/Linux runtime,
viewport picking, camera navigation, gizmos and undo/redo remain unverified or
unimplemented. This is correctness/integration evidence, not a performance claim.
