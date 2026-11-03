# Model Parts box cut — validation

2026-11-03, macOS development workspace, Node 22.22.0, Chrome via Playwright.

The `/models` editor now has **Cut with box**. A screen rectangle defines an
oriented model-space box through the model centre, initially as deep as it is wide.
Eight projected corner handles resize it in the current camera plane while keeping
the opposite corner fixed. Orbiting changes the adjustment plane, not the cutter's
orientation. Handles support arrow keys and finer Shift steps; local centre/size
fields provide keyboard and exact adjustment. Redraw, Cancel and Escape keep the
draft separate from explicit Cut. Saving uses the existing part exporter and staged
native validation.

`model-box-cut.ts` subtracts surfaces from all intersected parts using six box planes.
It interpolates UVs/normals, preserves winding and emits only referenced vertices.
It prepares all geometry before replacing any parts. Surviving durable IDs, material
references, names and pivots stay unchanged; descendants of a fully removed part
reattach to their nearest surviving ancestor. Removing the whole model is refused.
Existing save guards still refuse deletion of a part referenced by a saved scene.

This is an IDE surface edit with open boundaries, not a capped solid boolean. A box
wholly inside a closed object, without intersecting its surface, removes nothing.
Plane cuts retain their existing whole-triangle behavior. Input/output are bounded
to the native per-part limits of 500k triangles and one million vertices; no
large-model latency or frame-time claim was measured. No engine storage, SlotMap
identity, runtime hot path, dependency or user game source changed.

## Checks

- `npx tsx --test tests/model-box-cut.test.ts tests/model-part-geometry.test.ts`:
  **10 passed**. New cases cut a hole through two large triangles and verify the exact
  removed area, interpolated UVs, normals, winding, compact output, rotated/translated
  frames, no-op/full removal, coherent parenting, invalid data/indices/limits, initial
  width-depth equality and opposite-corner invariance.
- `npx tsx --conditions=react-server --test tests/model-split.test.ts`:
  **2 passed**. Existing split lifecycle and part-list validation remain covered.
- Targeted ESLint on the geometry module, both editor components, geometry tests and
  `scripts/model-box-cut-smoke.ts`: passed.
- A clean TypeScript config extending the IDE config, including all source, tests
  and scripts but excluding generated build directories: passed.
- The direct production build compiled source, then failed type checking because
  existing `.next-charcoal-dev` validators referenced the removed `/textures` page.
  The same source was copied into a disposable `.box-cut-validation` directory,
  sharing the installed dependencies and using fresh generated types. Its production
  build **passed**, including TypeScript and route generation. The two existing
  `inspection-session.ts` filesystem-tracing warnings remained. The original dev
  output was preserved.
- Browser smoke against that production build on **port 3001** passed. It used a
  newly created independent project from `templates/standard` plus engine-owned
  fixtures and a synthetic 2m cube GLB; no sample-game assets or IDs were copied.
  Checks covered drawing, width-depth equality, eight corners, keyboard depth changes
  after orbiting, pointer resizing, Cancel and Escape without geometry edits,
  explicit Cut, unsaved state, native-validated Save parts and reload/reopen.
- The exported/reopened part retained its ID and `[0,0,0]` pivot. Reading its saved
  GLB independently confirmed UV counts and surface area **21.1583** from the original
  **24**, demonstrating persistent surface removal rather than a preview mask.
  Screenshots `/tmp/emergent-box-cut-handles.png` and
  `/tmp/emergent-box-cut-result.png` were visually inspected. No page errors or paid
  provider calls occurred.

Reproduce the fixture with `npx tsx scripts/model-box-cut-smoke.ts --prepare`, pin the
returned project using `EMERGENT_PROJECT_PATH` in an IDE on port 3001, then run
`EMERGENT_PROJECT_PATH=<returned-path> npx tsx scripts/model-box-cut-smoke.ts`.
The smoke refuses non-disposable project names because it saves the cut part set.

The test service, temporary clean build/source snapshot and generated type include
entries were cleaned up. The user's port 3000 service and game project were not used.
Unverified: complex textured production-model visual quality, maximum-size edit
latency, solid caps, and live paid part splitting.
