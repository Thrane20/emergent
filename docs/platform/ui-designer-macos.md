# UI Designer UI-1 validation

Date: 2026-11-01. Host: macOS 27.0.1, arm64. Chrome headless, Node 22.22.0,
Next.js 16.4.2; Rust debug tests and all-feature Clippy. No GPU/runtime UI rendering
or performance budget is claimed.

## Delivered boundary

[UI-1](../design/ui-designer.md#slices) is implemented: `*.ui.toml` assets, eight
widget schemas, Rust validation/layout and catalogue export, `/ui` authoring and
project-scoped library, SVG previews, native-validated saves, and AI asset edits.
The standard template and reference project include an Options sample.

`UiDefinition` is immutable compiled asset data using indices for layout; it is not
a mutable runtime object store. UI-2/3 must implement the planned typed SlotMap
instance/node ownership, screen/world rendering, pointer input and action dispatch.
`ui.assets` is preview; `ui.runtime`, `render.2d` and `render.text` remain planned.

## Checks

Commands run from the engine root unless noted:

| Check | Result |
| --- | --- |
| `cargo test --test ui` | 6 pass: shared layout fixture, anchors/stacks, validation failures, catalogue export, node/canvas limits and parent-before-child compilation |
| `cargo test -p emergent-standard-template` | Pass; template compiles (0 unit tests); its sample UI is exercised by the engine and IDE fixture tests |
| `rustfmt --edition 2021 --check src/ui.rs tests/ui.rs` | Pass |
| `cargo clippy --all-features --test ui -- -D warnings` | Pass |
| `cargo doc --no-deps --all-features` | Pass |
| IDE `pnpm test` | 160 pass; includes native manual-save/AI-publication regression tests, invalid-widget rejection, external revision conflicts and shared Rust/TS layout parity |
| IDE `pnpm exec tsc --noEmit --incremental false` | Pass |
| Targeted ESLint | UI designer, route/page, UI libraries, AI context/publication changes, tests and browser smoke pass |
| IDE `EMERGENT_NEXT_DIST_DIR=.next-ui-build pnpm build` | Pass; production `/ui` and `/api/ui` routes built |
| IDE `pnpm test:ui-designer` | Pass against the production build with installed Chrome; desktop 1600×1100 and mobile 390×844 |

The browser smoke starts its own server on port **3001**, pins it to a temporary
copy of `templates/standard`, and uses separate temporary IDE data. It checks the
native widget catalogue, rail entry, blank creation, inspector edits, drag back to
origin, action labels, screen/perspective preview, native save/reload, clearing an
optional action, chat-triggered reload, unsaved navigation protection and absence
of mobile horizontal overflow or browser page errors. The server and temporary
project are removed on completion. Port 3000 and the user's project assets are untouched.

Provider traffic is mocked: browser chat synthesizes a rename through the real
save API. Separately, `tests/ui-assets.test.ts` sends synthetic AI output through
the actual project-context, isolated Cargo/native validation and publication
pipeline, proving invalid UI edits do not overwrite saved assets. No paid AI call
or real-model output quality is claimed.

Removed obsolete `.next-charcoal-dev` type includes from `tsconfig.json`: that old
test cache still referenced the removed `/textures` page. The normal `.next` type
includes remain; the isolated build's temporary type includes were removed after
validation. The final normal-config TypeScript check passes.

Desktop and mobile screenshots were inspected at
`/tmp/emergent-ui-designer-desktop.png` and `/tmp/emergent-ui-designer-mobile.png`.

## Chat visibility and drafting fix — 2026-11-01

The centre workspace now scrolls when its canvas, toolbar and chat exceed the
available height. The chat composer accepts vertically resizable multiline drafts
while UI changes are unsaved. Sending stays blocked until native save succeeds;
the shared chat's other callers retain their previous disabled behaviour. A new
UI uses its eventual asset path as its chat scope so the first save preserves the
draft instead of switching from the scene's thread to a new asset thread.

The production Chrome smoke additionally covers 1366×768 and 1280×720: scroll to
the full chat, verify its header/composer/send control are in view, type multiline
detail into a new unsaved UI, and verify both button and Enter submission stay
blocked with zero chat requests. The draft survives the first native save and
sending becomes enabled. The test also types into a modified saved UI and on mobile.

Validation: production build, targeted ESLint, the five focused chat/UI tests and
the expanded `test:ui-designer` smoke pass. No paid provider traffic is used.
Short-screen screenshot: `/tmp/emergent-ui-designer-chat-720.png`.

## MCP UI authoring — 2026-11-01

The same server now exposes `list_ui_widgets`, `create_ui`, `edit_ui` and
`preview_ui` to embedded and HTTP/stdio clients. The index recognises `.ui.toml`,
so `find`, `list(kind="ui")` and structured `describe` return durable refs,
the complete node tree, action labels and a revision. New assets get server-issued
UUIDs. Edits address durable node IDs, enforce explicit panel parents, reject
cycles, missing nodes and root removal/movement, and validate the final candidate
with Rust before publication. `null` clears a field override. Batches publish only
the validated final tree; no partial node edits are written.

UI tools reuse the brain/locomotion asset publisher, shared authoring lock,
revision-checked atomic writes, history/undo journal and project-change events.
Manual UI saves also use that publication lock. Dry-run validates without creating
an asset or journal entry; unchanged edits do not create undo entries. External MCP
changes refresh a clean open UI and its library. Dirty local drafts stay intact
and show a reload/conflict notice instead of being overwritten.

`preview_ui` returns rectangles in canvas pixels from the existing Rust/TS
parity-tested designer layout after native validation, not a rendered image. It
supports `offset`/`limit` (maximum 200 nodes per page), reports the revision and all
action labels, and explicitly states that game rendering/dispatch are unavailable.
The embedded adapter directs UI edits to these tools rather than the file fallback.

Validation from `src/emergent-ide`:

- `pnpm test`: **170 passed**. `tests/mcp-ui.test.ts` calls the real authenticated
  HTTP endpoint with a temporary standard-template project and exercises embedded
  chat with synthetic tool choices. It covers schema discovery, UI indexing and
  descriptions, native validation, creation, field updates/resets, add/remove/move/
  reorder, rectangle inspection/pagination, dry-run, stale revisions, duplicate IDs,
  invalid fields/graphs/operations and node bounds, unchanged edits, change events,
  history, undo conflicts and stale refs after undoing creation.
- Production `.next-ui-build` build, targeted ESLint and TypeScript checks pass.
- `pnpm test:ui-designer`: Chrome desktop/laptop/mobile checks include real external
  HTTP MCP edits, automatic library/editor refresh, and preservation of dirty local
  drafts; the server runs on port 3001 against a disposable template project and is
  stopped afterward. Chat/provider traffic is mocked; no paid request is made.

No Rust runtime or engine ownership model changed. Existing native candidate
validation and shared fixtures are reused. UI-2/3 remain planned. No live Claude
or Codex decision quality is claimed by these scripted client checks.

## Remaining validation

No Windows/Linux or Safari/Firefox smoke, runtime UI benchmark, live provider call,
screen-space rendering, world-space picking or action/signal bridge was exercised.
Those runtime paths are not implemented in UI-1. The full Rust workspace suite was
not rerun for this authoring slice; the master plan retains its existing unrelated
`first_project` bounce-test failure. Default-feature UI tests emit the existing
unused `TerrainMaterial.layers` warning; all-feature Clippy passes.

## New UI generation correction — 2026-11-02

The starter panel marked a new UI dirty, so the chat allowed typing but disabled
Send until a placeholder was saved. New starter UIs now submit at project scope
with draft name, description and canvas context. Exactly one newly created valid
UI is loaded automatically. Unsaved widget edits remain guarded. Chat request
drafts use the UI identity so they survive the first save and its scope change.

Validation: eight UI unit tests and all 194 IDE tests pass; targeted ESLint and
the production Next build pass. Chrome on a private port 3001 server confirms starter generation without a
prior save, automatic library/editor selection, multiline request retention through
the first save, saved-UI AI refresh, external MCP refresh, dirty-draft protection,
and desktop/mobile layout. AI replies were mocked; generated assets and saves used
real native validation in a disposable template project. No paid provider call was
made. The test server and browser were stopped afterward.
