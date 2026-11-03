# UI assets, runtime and the UI Designer

Status: **UI-1 implemented** (2026-11-01), capability `ui.assets` is preview.
[Validation evidence](../platform/ui-designer-macos.md). UI-2/3 authored-state screen/world rendering is now preview; pointer input, actions,
mutable widget state, richer fonts and generic 2D rendering remain planned.
[Runtime evidence](../platform/ui-runtime-macos.md).

## Goal

Author game UIs in the IDE: panels, labels, buttons, toggles, sliders, steppers
(`◂ Ultra ▸`), text boxes, dividers and tab lists, built by hand or with AI chat. The target
look is Carrier Command 2: chunky pixel font, flat dark panels, inverted highlight
bars, and screens that sit *inside* the world on cockpit consoles.

Every UI is its own asset in the carousel and can be placed in a scene:

- **Screen space**: anchored to the screen in pixels or percentages.
- **World space**: a panel with width and height in metres, placed by an entity's
  transform (location and rotation). The scene camera renders it in perspective or
  orthographic projection like any other surface.

Widgets carry an **action label** (`graphics.apply`, `options.tab.audio`). The game binds
labels to functions, and brains can react to them as signals.

## Technology decision

| Option | Verdict |
|---|---|
| **bevy_lunex** | Rejected. It needs the full Bevy app, renderer, `bevy_text` and picking. Emergent uses only `bevy_ecs` with its own wgpu renderer, so Lunex would import another engine's architecture (against AGENTS.md). |
| bevy_ui / egui / iced | Rejected for game UI for the same reason, or because it's immediate mode with an editor look. egui stays fine for debug tools. |
| **Native retained UI (chosen)** | A SlotMap node tree, Lunex-style anchored rects for layout, one batched quad pipeline and a bitmap pixel-font atlas. No new dependency is needed for slice 1–2. |

Lunex's *layout model* is copied, not its code. Each node is a rect positioned relative to
its parent by anchor, offset and size, with each value in `px` or `%`. A `stack` helper
on a parent lays children out top to bottom with a gap (the options list in CC2). Full
flexbox is not needed for this style. If it is later, `taffy` is the focused crate to add.

Text starts as a **bitmap pixel font**: one PNG atlas and glyph table under
`assets/fonts/`. It matches the target look exactly and needs no shaping. TTF via
`fontdue` or `cosmic-text` is a later slice, behind the same glyph-quad interface.

## Asset model

`assets/ui/<id>.ui.toml`, a flat node list like brains (small diffs, AI-friendly).
This is the implemented UI-1 format (widget fields come from `widget_catalogue()`):

```toml
schema_version = 1
id = "options"
name = "Options"
kind = "ui"
canvas = [640, 480]          # design resolution in px; world panels render at this size

[[nodes]]
id = "root"
type = "panel"
padding = 8
stack = true
gap = 4

[[nodes]]
id = "tab_graphics"
type = "button"
parent = "root"
text = "GRAPHICS"
h = "28px"
action = "options.tab.graphics"
selected = true

[[nodes]]
id = "bloom"
type = "slider"
parent = "root"
text = "Bloom"
value = "bloom"
min = 0
max = 1
action = "graphics.bloom"
```

UI-1 supports panel, label, button, toggle, slider, stepper, textbox and divider.
Tab lists use buttons inside a stacked panel. Colours are linear RGB arrays; the
named theme/font/effect system is future runtime work. The complete options example
lives in `templates/standard/assets/ui/options.ui.toml` and the reference project.

- Node `id`s are durable within the asset. `value` names a bound value the game can read
  and write. `action` is a free-form dotted label.
- Validation (Rust, shared with the IDE through the project host): unique ids, one root,
  parent exists, no cycles, known types and fields, ≤ 512 nodes, depth ≤ 16.
- Planned UI-2/3 placement in a scene is a component, not a new hierarchy:
  `emergent.ui { ui = "options", space = "screen"|"world", anchor, size_px | size_m, interactive }`.
  In world space the entity transform gives location and rotation. In screen space the
  component's anchor and size are used.

## Runtime ownership (`src/ui.rs`)

The following runtime ownership and frame flow are the UI-2/3 design. UI-1 currently
provides `UiDefinition`, an immutable, validated asset with index-linked layout data;
it creates no live widget identity or runtime hierarchy. Typed SlotMap ownership
remains required for the mutable runtime described below.

- `UiDocument`: the compiled asset (index-linked node array, interned action labels),
  shared via `Arc` by every instance of that asset, like `CompiledBrain`.
- `UiSystem` owns `SlotMap<UiInstanceKey, UiInstance>`. Each instance owns
  `SlotMap<UiNodeKey, UiNode>` (runtime state: value, hover, pressed, focus, text) plus a
  cached layout array and a `dirty` flag. Scene entities refer to instances by key. A stale
  key returns `None`.
- No widget holds references. Parent and child links are keys, and the hierarchy rules
  match the scene graph (no cycles, coherent reparenting).

**Per frame**

1. Pointer input. In screen space, the cursor maps to canvas px. In world space, a camera
   ray is tested against the panel quad, and the hit UV maps to canvas px. Both use the same
   hit test, a reverse linear walk over the layout array.
2. Interaction updates node state and pushes `UiEvent { entity, action, value }` onto a
   reused per-tick `Vec`.
3. Layout runs only for dirty instances (linear, parents before children).
4. Draw. Each dirty instance re-batches its quads (rects and glyphs) into a reused vertex
   buffer and renders into its own offscreen texture at canvas resolution.
5. Composite. A screen-space instance is one textured quad after the scene pass. A
   world-space instance is one textured, emissive quad in the scene pass, so it gets depth,
   perspective or orthographic projection, and later CRT effects (scanlines, glow) as a
   material.

Rendering to a texture keeps screen and world space on one code path, and the steady-state
cost of an unchanged UI is a single quad.

**Actions**

- Game code: the project host exposes `ui.on("graphics.apply", |ctx, event| …)`,
  `ui.set(entity, "bloom", 0.4)` and `ui.get(...)`. Handlers run after the UI step, on the
  fixed tick, so replays stay deterministic.
- Brains: every UI action is also emitted as a brain `Signal` with the same name, so an
  `on_signal` node can react with no code.
- The IDE run panel logs actions, which helps when debugging bindings.

## IDE: UI Designer page (`/ui`)

- Left: asset carousel of `*.ui.toml` (reuses `asset-carousel`) and a widget palette.
- Centre: a canvas preview at design resolution with select, drag, resize, and a toggle
  that shows the panel on a sample world quad in perspective.
- Right: an inspector for rect, text, style, `action` and `value` (reuses `field-input`).
  It also lists every action label in the asset, so they are easy to wire.
- AI chat: the same pattern as the brain editor. It is given the widget catalogue
  (exported from Rust like `node_catalogue`) and the current TOML, and returns node edits.
  A new unsaved starter panel can generate a UI immediately using project scope and
  draft name/description/canvas context. Exactly one new valid asset is opened after
  generation. Unsaved widget edits still require saving before AI submission.
- Planned UI-2/3: "Place in scene" adds the `emergent.ui` component to a new or selected instance.

**UI-1 preview fidelity.** The browser uses SVG with the TS layout math and approximate
monospace glyphs. A shared fixture test (the same `.ui.toml` → the same rects in Rust
and TS) keeps layout in step. World preview applies CSS perspective to the same SVG;
it does not render an engine panel. Bitmap glyph fidelity and "Render in engine"
capture arrive with runtime rendering.

## Scale and budgets (to be measured, not assumed)

The target is tens of live instances with up to 512 nodes each, and per-frame cost close to
zero for unchanged UIs. A benchmark covers 100 instances × 200 nodes (full relayout and
redraw, and hit testing) in a release build before any budget is claimed.

## Validation

- Rust unit tests: validation errors, layout (anchors, %, stack), screen and world hit tests
  (ray → UV → node), stale instance and node keys, action dispatch order, signal bridge.
- TS/Rust layout parity fixture. The template gets one sample UI, so `cargo test` covers new
  projects.
- Benchmark as above. `cargo fmt`, `clippy` and the IDE tests.

## UI-1 MCP authoring boundary

Expose UI assets through the shared project index (`asset:<id>`), `describe` and
`list_ui_widgets`; add `create_ui`, revision-guarded `edit_ui` (add/update/remove/
move/reorder by node ID), and read-only `preview_ui` with paged canvas rectangles
and action labels. Reuse the designer's document/tree helpers, the native widget
catalogue and Rust candidate validation, and the existing MCP publication journal,
dry-run, undo and project-change events. Embedded and external MCP clients receive
the same tools. UI files stay project-owned; only durable asset/node IDs cross MCP.
This adds no runtime identity, graph or frame-path work. Preview uses validated
asset data and the existing Rust/TS parity-tested layout; runtime rendering stays
UI-2/3. Validate through the real MCP endpoint and embedded adapter against a
temporary template project, including invalid graphs, stale revisions and undo.

## Slices

1. **Asset + designer (UI-1, implemented).** `.ui.toml` format, Rust validation and layout, widget catalogue
   export, `/ui` page with preview, carousel and AI chat. Nothing rendered in game yet.
2. **Screen-space runtime (UI-2, rendering implemented; interaction planned).** Bitmap font, quad batch to texture, composite, mouse pointer
   input, actions to game handlers and signals, `emergent.ui` component.
3. **World-space panels (UI-3, rendering implemented; picking/effects planned).** Textured quad in the scene pass, ray picking, CRT theme effects.
4. **Later.** Keyboard and gamepad focus navigation, text-box editing and IME, TTF fonts,
   localisation, accessibility hooks.

## Decisions (2026-11-01)

- Build our own UI system; Bevy and Lunex are out.
- Bitmap pixel font first; TTF later.
- Browser preview with a Rust/TS layout parity test is the designer preview.
- World-space panels take mouse input in slice 3.

## Nested layout containers

Extend the immutable UI-1 graph with Horizontal, Vertical and Grid containers,
using the same durable node IDs, validated parent links and generated inspector.
No mutable runtime identity is introduced; the future runtime still owns SlotMap
nodes. Panel `stack=true` retains its existing behavior. A `layout` choice also
allows a panel to arrange children. Containers expose width/height lengths,
uniform padding, gap, main alignment and cross alignment; grid adds integer
columns (1–64). Grid cells share available width and height equally, with rows
computed from child count. Percent child sizes resolve against the content box
for rows/columns and against the cell for grids. Main alignment positions the
whole row/column, cross alignment positions/stretches individual children; grid
alignment positions children within cells. Oversized content overflows without
shrinking; negative remaining space aligns at the start. Flow containers own
positions (x/y/anchor are ignored); resize and tree reorder/reparent remain
available. Absolute and legacy stack editing remain compatible.

Layout uses bounded parent-before-child traversal, a sizing pass and placement
pass per container; setup/result buffers allocate as the existing API does. Both
Rust and browser implement the same rules with shared golden fixtures. No
performance target or in-game rendering/input capability is claimed. Scope
includes catalogue/validation, geometry, preview/gestures, AI instructions and
nesting tests; spans and content auto sizing are now implemented as described below; flex growth is deferred.

## Runtime rendering, spans and auto sizing

2026-11-02: both screen overlays and world panels consume the same immutable,
validated definition and prepared RGBA surface. A World-owned entity carries an
`emergent.ui` component (asset path, screen/world mode, dimensions and pixel
position). Runtime identity remains its typed EntityId; renderer resources use
existing renderer-owned generational GeometryId handles. There is no competing
widget ownership graph. Editable runtime nodes and input/action dispatch remain
separate work; this slice renders the authored state.

UI images rasterize only at load/preparation, with a built-in bitmap font and
bounded 4,194,304-pixel surface. World panels are unlit textured XY quads measured
in metres with normal scene depth. Screen instances are prepared quads composited
last with alpha blending, no depth and a pixel orthographic transform rebuilt on
viewport resize; they never receive camera perspective or cast shadows. GPU
buffers/textures are retained; per-frame extraction updates transforms/visibility
only. Scene/host authoring and Run share this integration, including teardown of
stale entity handles. No I/O or rasterization occurs on the frame path.

Grid `col_span`/`row_span` use ordered row-major first-fit occupancy, with column
bounds checked and at most 512 resulting rows (32,768 cells). `w`/`h="auto"`
measure content bottom-up, then arrange top-down. Percentages do not contribute to
intrinsic size (they resolve during arrangement); pixel offsets and text/widget
intrinsic metrics do. Root size follows the design canvas; responsive overlays
scale that canvas to the viewport. No font shaping, scrolling or interactive
controls are claimed. Validate parity, overflow/invalid spans, auto sizing and
pixel/readback evidence for camera-independent overlays and depth-tested world UI.
