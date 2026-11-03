# UI rendering, spans and auto sizing

2026-11-02, macOS / Apple Silicon. This is a rendering slice, not an interactive
widget runtime. The scene owns each UI entity through its existing World EntityId;
GPU textures/quad buffers use the renderer's existing generational GeometryId.

## Behavior

- `col_span` and `row_span` pack grid children in row-major first-fit order without
  overlap. Spans must be integer 1–64 and fit the parent's columns; resulting grids
  are bounded to 512 rows. Rust and browser implement the same packing rules.
- Width/height accept `auto`. Bottom-up intrinsic measurement precedes arrangement;
  text metrics, pixel dimensions, absolute pixel offsets, padding and gaps contribute.
  Percentages do not contribute intrinsic size, avoiding cyclic parent sizing.
- Prepared RGBA surfaces include panels, controls and built-in 5×7 uppercase ASCII
  glyphs (lowercase maps to uppercase; unsupported glyphs use '?'). Each surface is
  capped at 4,194,304 pixels and the scene at 64 MiB of UI RGBA pixels.
- `emergent.ui` specifies asset path, screen/world mode, width/height and screen x/y.
  Screen width/height zero fills the available viewport; the design canvas scales
  with it. Explicit screen dimensions/position use pixels. World dimensions use
  metres; width zero defaults to 2 m and height zero preserves canvas aspect.
- Screen quads composite after geometry/particles, with alpha and no depth. Their
  orthographic projection follows viewport size and ignores camera/entity transforms.
  World XY panels are unlit, centred on the entity, follow its world transform,
  obey scene depth and do not cast shadows. Transparent texels are discarded.
- Preparation/rasterization and GPU upload happen during scene construction. Frames
  reuse resources. Removal of a UI entity/component releases its geometry on extraction;
  stale geometry handles do not alias replacements. Asset/component edits require
  scene reconstruction; no per-frame rasterization or live state binding is claimed.
- Designer buttons place either mode via staged native validation, conflict checks,
  publication journal and project events. The shared project host handles inspection,
  native previews and Run for every game, including standard-template projects.

## Validation

- Rust layout suite: 9 passed, covering legacy stacks, nested layouts, spans,
  invalid spans/column counts, auto sizing and shared preview geometry.
- Runtime tests: 2 passed, including glyph/alpha pixels and native GPU readback.
  Camera movement leaves an overlay's pixels unchanged; fixed-size overlay bounds
  are correct; foreground 3D geometry occludes a world panel. Removal invalidates
  renderer handles and rendering a stale handle is rejected.
- Workspace all-feature tests passed after removing the two obsolete first-project
  authored spin/bounce tests requested by the user. Generic spin/bounce/physics
  tests remain. Optimized all-feature engine tests passed.
- Workspace/all-target/all-feature Clippy with warnings denied and rustdoc passed.
- Focused TypeScript layout tests (6) and ESLint passed. Production build and type
  checking initially encountered a concurrently edited audio test fixture type
  error; its owner corrected it, after which the build passed. No audio files were
  changed for UI work.
- Disposable standard-template browser smoke on port 3001 covers native catalogue,
  nested preview geometry, span/auto editing, native save/reopen, placement of screen
  and world entities and native inspection frames. The script closes its service
  and inspection worker and removes its temporary project.

Pointer/ray picking, UI actions, mutable widget state, focus, text entry, font
shaping and Unicode fonts remain open. No Windows/Linux native evidence or UI
frame/memory performance target is claimed. RGBA preparation is CPU work and may
be expensive for large surfaces; it is not performed on the frame path.

The complete IDE suite passed serially: **179 passed, zero failed**. The first
full run found one stale MCP test asserting eight catalogue entries; it now expects
eleven. TypeScript, focused ESLint and the production build passed on the current
workspace after the concurrent audio fixture correction. Repository-wide
`git diff --check` still reports the existing extra blank line at the end of
`docs/design/object-components.md`; that unrelated file was left unchanged.

Final browser smoke passed after the occupied port became available: grid spans
and auto dimensions edited in the inspector, saved/reopened through Rust,
screen/world instances placed, and live native scene frames received. Screenshots:
`/tmp/emergent-ui-layouts.png` and `/tmp/emergent-ui-runtime-scene.png` (ephemeral).
The GPU test additionally verified overlays composite over foreground geometry
and maintain fixed pixel placement after viewport resize. Final focused ESLint
and workspace Clippy passed. The isolated port-3001 server and inspection worker
were stopped; the port-3000 user service was left running.
