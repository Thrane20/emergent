# Scene graph authoring — reference validation

Date: 2026-10-24. Apple M3 Pro, macOS/Metal, Rust/Cargo 1.99.1.

- Workspace debug and release/all-feature suites pass: 49 engine unit tests,
  nine authored-scene tests, two ECS tests, three project unit tests and one
  project-contract test. Added math tests cover rotated/scaled parents, global
  TRS conversion, stale handles, shear rejection, position-only edits, singular
  matrices and very small global scales (including small sheared matrices).
- Rust formatting, Clippy with warnings denied and rustdoc with warnings denied
  pass. No unsafe code or competing runtime identity store was introduced.
- IDE service tests (26), TypeScript, lint and production build pass. Tests cover
  derived hierarchy validation, surgical parent edits, comments/local TRS
  preservation, scoped TOML access, source bounds and external-reference conflicts.
- `pnpm test:graph` uses the running localhost:3000 IDE and actual native retained
  Rust renderer. It verifies drag-to-parent and root drops, keyboard parenting,
  inherited rotation/scale, global position/rotation/scale conversion and local
  persistence, rejected cycles, sheared global position edits and rejected lossy
  conversion. The inspection session UUID and worker PID survive parenting.
- The same smoke checks scene and shared-entity TOML saves through staged Rust
  validation, preservation of source on syntax/semantic failures and cancellation
  of navigation away from an unsaved draft. Temporary validation scenes never
  publish to the live project.
- Existing `test:placement`, `test:authored` and isolated production `test:ui`
  pass, including real Rust frame changes, stale placement rejection, asset
  creation/inclusion and browser model preview. Generation in test:ui is mocked;
  no paid provider or AI requests were made.
- Browser screenshots checked the unified graph, compact inspector, header mode
  switch and source editor, including a visible failed-validation message.

All integration fixtures use unique temporary asset names and clean up their own
files/workers. Existing authored game assets and the running IDE are retained.
Reparenting keeps local values rather than preserving world placement. A sheared
global transform has no exact rotation/scale TRS representation: position remains
editable; other axes use Local mode. Whole-file TOML changes require Load/Restart
inspection, while inspector and parent edits update the retained worker directly.

Windows/Linux runtime validation, viewport picking/gizmos, undo/redo and
preserve-world reparenting remain open. These checks demonstrate correctness on
the reference host; they do not establish new performance or platform claims.
