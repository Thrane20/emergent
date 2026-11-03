# Project MCP server (MCP-1) and AI provider selection — macOS, 2026-10-30

Apple Silicon, macOS, Node 22.22.0, Next 16.4.2, `@modelcontextprotocol/sdk` 1.32.2,
`@anthropic-ai/sdk` 1.3.0, `zod` 4.6.7 (all pinned). Design: [project MCP server](../design/project-mcp-server.md).

## Delivered

- `/api/mcp` stateless Streamable HTTP endpoint inside the IDE server, with JSON
  responses and one server per request. Requests must use a loopback host and a
  same-origin `Origin` (or none), and carry a bearer token. The token is created once
  in the IDE data directory with owner-only mode (0600) and is gitignored.
- `scripts/mcp-stdio.mjs` bridge for stdio clients. It needs no build step and writes
  only protocol messages to stdout; with the IDE down, requests fail with an
  actionable JSON-RPC error.
- `lib/project-index.ts` durable identity index. Refs are `asset:<id>`, `scene:<id>`
  and `instance:<scene id>/<instance id>`, built from authored IDs and never from paths
  or runtime keys. Naming follows the IDE (instance name, then asset name, then ID).
  Hierarchy paths are built from authored data. Duplicate IDs make the ref unusable
  and are reported, and stale refs fail instead of resolving to another object.
  Descriptor bundles own the media in their directory. The index is cached by file
  path, size and mtime.
- Tools: `project_overview`, `find`, `list`, `describe` (with optional Rust world
  transforms) and `view` (native tick-zero render). All are annotated read-only.
  Resources: `emergent://project`, `guide`, `capabilities` and `scene/{id}`.
- `.mcp.json` registers the bridge for Claude Code.
- IDE Settings choose **Claude** (Anthropic Messages API through the official SDK,
  with JSON-schema `output_config` and server-side fallback `default`) or **Codex**
  (OpenAI Responses). Each provider keeps its own model, URL and key. Legacy
  single-provider settings migrate to Codex without rewriting the file. Fresh installs
  default to Claude Opus 5.5.
- Fixed a pre-existing preview bug found by `view`: `--preview-scene` synced the active
  camera before `engine.start` propagated transforms, so previews rendered from inside
  geometry. A regression test fails without the fix (2 distinct colours) and passes with it.

## Measurements (dev server, first_project: 7 scenes, 54 assets, 34 instances)

| Operation | Time |
| --- | --- |
| Index build, cold (after project snapshot ~97 ms) | 5 ms; it was 6.9 s when it used full `scenePlacement`, which hashes textures and terrain data |
| `find` through the bridge | ~0.1 s |
| `describe` with `world: true` (spawns the Rust scene tool) | ~9.5 s |
| `view` of `scene:main` | ~1.5 s warm |

These are single dev-server samples, not latency budgets.

## Validation

- `pnpm test`: 139 pass. New tests cover:
  - index refs, naming, hierarchy, ranking, duplicates, stale and malformed refs,
    descriptor ownership and diagnostics
  - endpoint auth (no token, wrong token, foreign host, cross-origin) and token file mode
  - MCP initialize, tool list, find, describe, stale-ref errors and scene resources
  - provider migration and switching with per-provider keys
  - Claude request shape and refusal/max-token handling against a local mock (no paid call)
- `tsc --noEmit`, ESLint on changed files, `cargo fmt --check` and
  `cargo clippy -p emergent-first-project --all-features --all-targets -D warnings`
  pass. First-project tests: 16 pass, plus the known bounce-height failure.
- Isolated IDE on `[::1]:3001` (separate dist dir): all pages render. A stdio-bridge
  session covered initialize, tools, overview, find, describe, stale ref, resources and
  view. The test server was stopped and its `tsconfig.json` additions were removed.
- **Live Claude Code** (`claude -p`, strict MCP config, MCP tools only): found "the
  pyramid" as `instance:main/pyramid` in `scene:main`, with the correct local position
  and `asset:pyramid`. It also reported the identity diagnostic and the unused
  `asset:myfirstpyramid`.
- **Codex not verified:** `codex exec` with the same server config stopped at the
  account usage limit before any tool call.

## Notes and limits

- VS Code port-forwards IPv4 `127.0.0.1:3000/3001` on this machine, and those
  connections hang. The bridge therefore defaults to `http://localhost:3000`; set
  `EMERGENT_IDE_URL` to override it.
- Installing dependencies under the already-running IDE on port 3000 left its pages
  returning 500 while API routes worked. A fresh server renders every page, so restart
  the IDE after pulling these dependency changes.
- One legacy file (`explosion_atlas.png`) has no durable ID and is reported until
  `assign_ids` (MCP-2) exists. MCP-1 makes no writes.

# MCP-2 — guarded edits, undo and live IDE updates (2026-10-30)

## Delivered

- `lib/project-ops.ts` edit layer. Tools:
  - instance edits: `set_transform` (partial; local or global through Rust world math), `rename`, `reparent`, `delete` (whole subtree)
  - `place_asset` (new UUID instance with optional parent, name and transform)
  - `set_material`, `set_physics` and `set_camera` (field patches merged over current settings; `null` resets to the asset defaults)
  - `set_environment`
  - `assign_ids`
  - `undo` and `history`
- **Guarded writes.** Every edit requires the scene `revision` from `describe` and runs
  through `commitSceneEdit`, which is refactored out of the IDE's own guarded save: a
  revision check, a text-preserving edit, native `--validate-scene`, a revision
  recheck, then atomic publication. IDE behaviour is unchanged. `dryRun` validates
  and returns a diff without writing.
- **Undo journal.** Each applied edit is journalled under the IDE data directory
  (`mcp-changes/<project>/`, owner-only, last 200 entries). `undo` re-applies the
  pre-image only if every file still matches the post-image, so newer work is never
  overwritten. Undoing an undo is refused.
- **Live push.** Transform and reparent edits go straight into the running inspection
  world (`InspectionSessions.activeFor`). Other edits make the open page restart
  inspection.
- **IDE refresh.** `/api/project-events` (server-sent events, bounded subscribers,
  heartbeat) feeds the Project Studio. It refreshes the graph, inspector and TOML views
  when that's safe. With unsaved drafts or an active gesture it shows a notice instead.
- **`assign_ids`.** Adds a UUID `id` to TOML definitions without one, and creates a
  `media.asset.json` descriptor for loose media through a create-only, contained path.
  The shared publisher's Rust/TOML-only rule for AI writes is unchanged.

## Bugs found and fixed

- **Deleting the last instance before a trailing table.** `replaceSceneSubtree` treated
  a trailing `[environment]` table as part of the last instance, so the IDE refused to
  delete that instance. Instance blocks now end at the next table that isn't their own.
  A regression test covers it.
- **Inspection camera.** The inspection worker synced the active camera before
  `engine.start`, so the first live view rendered from inside geometry until you
  orbited. This is the same cause as the earlier preview fix and is fixed the same way.
- **Smoke-test build caches.** Smoke copies shared the repo's Cargo target directory.
  The game binary embeds `CARGO_MANIFEST_DIR`, so Cargo reused binaries pointing at
  other or deleted copies, and once overwrote `target/release/emergent-first-project`
  with a temporary copy. That binary was rebuilt for the real project and checked
  (`strings`). Smokes now use a fixed copy at `target/mcp-smoke/game` with its own
  `target/mcp-smoke/target`.

## Measurements (dev server; Rust validation warm)

| Operation | Time |
| --- | --- |
| Any edit (validation + publish + new revision) | ~0.9–1.0 s |
| First edit on a fresh copy (builds the game crate) | 7–16 s |
| `undo` | ~0.93 s |
| `describe`, `find`, `history` | ~0.09 s |
| `view` | ~1.5 s |

These are single dev-server samples, not budgets.

## Validation

- `pnpm test`: 140 pass. Type-check and ESLint on changed files are clean; `cargo fmt`
  and `cargo clippy -D warnings` (first project) are clean. First-project tests: 16
  pass, plus the known bounce-height failure.
- `pnpm test:mcp-edit` passes against a real copy of `first_project` with native validation. It covers:
  - stale-revision refusal, and a dry run that writes nothing
  - transform (omitted parts kept), material, rename (ref kept), place with parent,
    name and transform, reparent, physics, environment and subtree delete
  - invalid settings rejected, `view` after edits, history order and one event per edit
  - eight undos restoring **byte-identical** scene text, "nothing to undo", and undo
    refused after a hand edit
  - `assign_ids` dry-run, apply and undo
- `scripts/mcp-live-smoke.mts` passes in headless Chrome against an isolated IDE on
  `localhost:3001` (Next dev blocks its client runtime for the `[::1]` origin):
  - the inspection shows real scene content (more than 64 distinct colours)
  - an AI transform pushes live without a session restart and shows the notice
  - an AI material change restarts inspection, and the pyramid renders red
  - an AI rename appears in the scene graph
  Screenshots were inspected.
- **Live Claude Code** (strict MCP config, tools only): "make the pyramid green, raise
  it 1 m, and put a copy of the cube 2 m above it, parented to it, named Hat" produced
  four journalled, validated edits, confirmed in the scene file. Claude reported which
  cube asset it chose and used world space for "above" a flipped, stretched parent.
- **Live Codex** (`codex exec`): find and describe worked. The first rename was refused
  by Codex's non-interactive approval policy. With
  `mcp_servers.emergent.default_tools_approval_mode="approve"` it renamed the instance
  (change `074530e1…`), confirmed in the file. Interactive Codex asks before write tools.

# Behaviour authoring over MCP (2026-11-01)

## Delivered

- **Index**: `*.brain.toml` and `*.locomotion.toml` are assets (`asset:<id>`, kinds `brain`
  and `locomotion`), so `find`, `list` and `describe` cover them.
- **describe**: instances list their components (brain/profile paths shown as refs);
  brains return their node tree, profiles their type and fields, both with a revision and
  a `usedBy` list across scenes.
- **Read tools**: `list_brain_nodes`, `list_locomotion_types` (also the
  `emergent://behaviour` resource), `preview_brain` (runs the engine; returns centroid,
  speed, spread, distance to targets and closest pair at five samples) and
  `preview_locomotion` (distance, top speed, height range, steer/bank/pitch extremes,
  braking, final state).
- **Write tools**: `create_brain` (nodes parent-first), `edit_brain` (add, update,
  remove, move, reorder by node id; revision-guarded) and `write_locomotion` (create, or
  change name, type, fields). Each is validated by the engine (`--validate-brain` /
  `--validate-locomotion`) before writing, supports `dryRun`, is journalled for `history`
  and `undo` (undoing a create removes the file), and refreshes the open IDE.
- **set_component** accepts asset refs for brains, stacks and profiles and instance refs
  for targets; scene files keep paths and instance IDs.
- **Instructions** describe components, brains (channels, per-channel Priority, stacks,
  signals), locomotion and the one-mover / one-rotator rules.

## Validation

- `pnpm test`: 155 pass (tool list and read/write annotations updated).
- `pnpm test:mcp-edit` (real Rust validation on a copy of first_project): catalogues;
  find and describe a brain; dry-run, invalid (engine-rejected) and real `create_brain`;
  stale-revision refusal; `edit_brain` add/update/remove; `preview_brain`;
  `write_locomotion` create, planned-type refusal, update with a field reset;
  `preview_locomotion` (horizontal 6 m/s cap, √(6²+4²) overall with climb; reset tilt back
  to 30°); `set_component` by refs (brain + stack, locomotion) stored as paths and shown as
  refs; `usedBy`; history order; five undos restore the brain and scene; a sixth removes
  the created brain.
- **Live Claude Code** (`claude -p`, strict MCP config, Emergent tools only), asked in
  plain English for a patrol drone on the pyramid at 6 m that wanders within 10 m and faces
  the key light, previewing first: it created a drone profile and a brain (Priority over
  Altitude → Blend(Wander, Stay within) and Look at), previewed both (held 6 m, 3.7–4.8 m/s,
  7.3–8.7 m from start), attached Locomotion and Brain by ref with face-movement off, and
  noted the duplicate "Key light" name in another scene. The scene validated.
