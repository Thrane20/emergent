# Facing, focus and alignment validation — 2026-10-25

Observed on macOS; no Windows/Linux native validation or frame-budget claim.

- New engine tests cover derived -Z facing under rotated/scaled parents and
  degenerate transforms. Native alignment tests cover parent conversion, world
  scale preservation, proposals leaving the world unchanged, and shear rejection.
- 68 IDE tests passed, including hierarchical facing and perspective/orthographic
  framing calculations. Types, lint, production build, Clippy, rustdoc and
  formatting passed.
- `pnpm --filter emergent-ide test:cameras` passed in a temporary game project on
  an owned production server. The browser exercised F in both projections,
  Shift-F on a rotated/scaled parented cube, read-only focus, persistence, retained
  session/tick zero and typing protection. The saved world position and facing
  were checked against a native Rust transform report and the actual editor view.
  The screenshot was visually reviewed. Temporary server/project were removed.
- Full workspace/default and release/all-feature runs passed engine tests and
  the new project tests, but the separate existing game test
  `authored_offender_bounces_about_four_metres_above_ground` failed, reporting
  a maximum world Y of 3.1047554 while checking a four-metre clearance target. This task does not modify
  that bounce behaviour or its test.

Look direction is derived, not separately persisted. Alignment preserves world
scale and rejects unrepresentable parent/shear combinations. Focus observes the
existing bounded editor world; dolly now reaches 0.25 world units for small objects.
