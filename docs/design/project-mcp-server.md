# Project MCP server — design brief

Status: **MCP-1 implemented** 2026-10-30 ([evidence](../platform/project-mcp-server-macos.md)); MCP-2 is next. Queue entry: [plan.md](../../plan.md).

## Goal

Let any MCP-capable AI client (Claude Code, Claude Desktop, the IDE's own chat) talk
about a linked Emergent project **by name**: "make the pyramid red", "put a point light
above the cube", "what's under Ground?". The client must also be able to make those
changes safely to authored assets and the live scene graph, and see the result.

Emergent is AI-first, so this server is a primary authoring interface, not an add-on.
It follows the GOALS.MD contract for AI generators. Callers use durable identifiers,
structured diagnostics and the same validation as human edits, and the result is
ordinary reviewable files.

## Where it runs

**An MCP endpoint inside the IDE server process** (`/api/mcp`, Streamable HTTP),
plus a thin stdio bridge (`pnpm mcp:stdio`) for clients that only launch processes.

Several things already live in that one Node process, held in `globalThis`:

- the persistent inspection worker (`InspectionSessions`)
- the run manager
- the serialized write queues
- the revision hashing
- the staged Rust validation (`--validate-scene(s)`)
- the journalled publication used by chat and Statements

A separate MCP process would duplicate this state and race it.

Alternatives rejected for v1:

- **A Rust MCP binary.** It would re-implement the TypeScript edit, revision and
  publication paths. Rust stays the validator and world authority, as today.
- **A standalone Node process importing `lib/`.** It would have separate globals,
  so it would race inspection and chat writes.

Consequence: the IDE must be running. The bridge connects to `http://localhost:3000/api/mcp`
(`EMERGENT_IDE_URL` overrides it); tests use port 3001. A headless mode (the same tool layer without the UI) is a later option.

## Naming and identity — the core of the design

The server **never exposes SlotMap or ECS keys**. Durable references are:

| Ref | Form | Source |
| --- | --- | --- |
| Scene | `scene:main` | scene `id` |
| Scene instance | `instance:main/cube` | scene `id` + instance `id` |
| Asset | `asset:cube`, `asset:8c37663b-…` | definition `id` (TOML) or descriptor `id` (`*.asset.json`) |

Refs contain **no file paths** (decision 4), so moving or renaming a file keeps its ref.
Sidecars (`.heights.toml`, `.paint.toml`, `.library.json`) belong to their owner, and a
descriptor owns the media in its directory. Files without a durable ID are reported
as `missing-id` and are not addressable until `assign_ids` (MCP-2) gives them one.

The display name of an instance is resolved in this order:

1. the instance `name` override
2. the asset `name`
3. the instance `id`

The hierarchy path (`Ground/Pyramid`) is shown alongside for context.

**Names are not unique** (IDE-NAME allows duplicates), so name lookup is a separate step:

1. `find("pyramid")` returns ranked candidates. Each carries its ref, display name,
   hierarchy path, kind, asset, scene and world position.
2. Mutating tools accept **refs only, never names**. If a name is ambiguous, the AI
   must choose or ask the user. Nothing is silently auto-picked.
3. Every read returns the scene `revision`, and every write requires `expectedRevision`.
   A stale or deleted ref is an explicit error and never resolves to a replacement
   object (AGENTS.md stale-handle rule).

Selection context comes later. The IDE pushes its current selection to the server so
that "this" or "the selected object" resolves to a ref. It is exposed as the
resource `emergent://selection`.

## Surface

### Resources (read-only, can be @-mentioned in Claude Code)

- `emergent://project`: manifest, asset roots, entry scene, outputs
- `emergent://scenes/{path}`: tree of instances with names, refs, assets, local/global
  transforms and component summary (geometry, light, material, physics, camera)
- `emergent://assets/{path}`: asset definition, referrers and dependencies
- `emergent://capabilities`: the capability catalogue, so the AI doesn't invent planned APIs
- `emergent://guide`: the project's AGENTS.md and authoring rules

### Tools — v1

| Tool | Purpose | Reuses |
| --- | --- | --- |
| `find` | Name or description → ranked candidate refs | project snapshot, scene placement |
| `describe` | Ref → full detail, including world transform and links | `scenePlacement`, `--scene-transforms` |
| `list` | Scenes or assets, optionally by kind | `readProjectSnapshot` |
| `view` | Render the scene from its authored camera → **image** | `--preview-scene` (an independent process, so it never moves the user's inspection camera; framing a ref is later) |
| `set_transform` | Partial local/global TRS | `saveScenePlacement` |
| `rename` / `reparent` / `delete` | Hierarchy edits with documented subtree semantics | `renameSceneInstance`, `reparentSceneInstance`, `deleteSceneInstance` |
| `place_asset` | Add an asset instance to a scene, with optional parent/transform | `prepareSceneAssetPlacement` |
| `set_material` / `set_physics` / `set_camera` / `set_environment` | Component edits | existing `saveScene*`, lighting settings |
| `create_asset` | New asset from a catalogue template | `asset-catalog` templates |
| `edit_toml` | Validated whole-file edit (escape hatch) | `saveTomlDocument` |
| `validate` / `run` / `run_status` | Check, launch or stop the game and read bounded logs | `--validate-scenes`, `projectRunManager` |
| `undo` | Revert the last MCP change if its revision still matches | new MCP change journal |

Every write supports `dryRun: true`, which returns the diff and validation result
without publishing. Every write result includes the new revision, a short text diff
and a change ID.

Rust behaviour code is **not** edited through MCP. Coding agents already edit files
well; they call `validate` and `run` to check their work. The IDE chat's existing
Implement path stays for clients without file access.

## Write pipeline

For every mutating tool:

`resolve ref → read revision → text-preserving TOML edit → staged native Rust validation
→ revision-checked atomic publish (shared write queue/lock with chat and Statements) →
change journal entry → push to the live inspection worker → notify open IDE pages`

- **The live worker** already accepts `transform` and `reparent` controls without a
  restart. Other edits restart the session, as the inspector does today.
- **IDE refresh** needs a new small server-sent-events channel (`project changed`, with
  revision), so the browser updates when an external AI makes an edit.
- **Undo** matters more here than in the IDE, because AI edits are frequent and
  sometimes wrong. The journal stores each pre-image with its revision. Undo refuses
  if the file has changed since; there is no blind overwrite.

## "Runtime scene graph" — three explicit targets

1. **Authored scene:** the source of truth. All persistent MCP edits land here.
2. **Live inspection world:** Rust world and renderer at tick zero. MCP edits are
   reflected here immediately, and `view` renders it. Camera and framing changes are
   transient.
3. **Play session:** a running game. **Out of scope for v1.** No control channel
   exists, and GOALS.MD says play mutations are transient and discarded on stop.
   Later, after engine pause/step lands: inspect, pause, step and transient tweaks,
   with no write-back to files.

## Safety and bounds

- **Network access:** bind to loopback only, and validate `Origin` against DNS
  rebinding (MCP spec). Require a per-install bearer token, stored in IDE server
  settings and never in the project.
- **Size bounds:** `find` returns at most 20 candidates. Scene resources are paged
  above 1,024 instances, the authored limit. `view` frames are capped at 1280×720.
  Logs are tail-bounded.
- **Paid fal generation** (textures, terrain, objects) is excluded from v1. When added,
  it needs an explicit `confirmCost` argument and the balance check.
- **Frame path:** all work runs in the IDE server, off the game's frame-critical path.
  Global transforms spawn the Rust tool, so cache them per revision. No latency
  target is claimed until it is measured.

## Phases

| Phase | Scope | Exit |
| --- | --- | --- |
| MCP-1 Read & see | Endpoint, auth, stdio bridge, resources, `find`/`describe`/`list`/`view`, `.mcp.json` for Claude Code | Claude Code finds "the pyramid" in `first_project`, describes it and shows a rendered frame |
| MCP-2 Edit | Transform, rename, reparent, delete, place, component edits, dry-run, journal/undo, live worker push, IDE SSE refresh | Conversational edits persist, appear live in an open IDE, and undo cleanly; conflicts are rejected |
| MCP-3 Build & run | `create_asset`, `edit_toml`, `validate`, `run`/`run_status`, IDE selection resource | "Add a red sphere above the selected cube and run it" works end to end |
| MCP-4 Generate & play | Cost-confirmed generators; play-session inspection after engine pause/step | Separate design once pause/step exists |

Later, the IDE chat moves onto the same tool layer, so humans' AI and external AIs
share one action surface.

## Implementation shape

- A `lib/project-ops.ts` layer that does name resolution, refs and a uniform
  result/diagnostic shape over the existing `lib/` functions. Both MCP and future
  chat use it. No logic is copied out of the existing write paths.
- `app/api/mcp/route.ts` hosts the official `@modelcontextprotocol/sdk` server
  (new, pinned dependency).
- A `scripts/mcp-stdio.ts` bridge.

## Validation

- **Unit:** resolver behaviour (duplicate names, renamed instance, deleted ref → stale
  error, hierarchy paths), ref parsing, and tool schema validation.
- **Integration (temporary project, port 3001):** each write tool's file result and
  Rust validation; a conflicting concurrent IDE edit; undo after an external change is
  refused; the live worker reflects edits; `view` returns a non-empty frame.
- **Client smoke:** MCP Inspector, then a real Claude Code session against a temporary
  project, recording any manual corrections (GOALS.MD AI-proof rule).
- **Measure:** `find`/`describe`/`view` latency on `first_project` and a 1,024-instance
  fixture.

## Decisions (2026-10-30)

1. v1 edits the authored scene and the live inspection world. A running game is not needed; generating as you go happens on authored data.
2. Claude Code and Codex are both MCP clients, through the same stdio bridge. The IDE's own AI can be set to Claude or Codex, each with a selectable model.
3. The IDE must be running; headless mode comes later.
4. Refs use durable IDs, never paths.

## Original open questions

1. Is changing a **running game** (play session) needed early, or is "authored +
   live inspection" enough for v1? (Recommended: the latter.)
2. Which clients come first: Claude Code only, or Claude Desktop too? (This affects
   bridge priority.)
3. Is requiring the IDE to be running acceptable, or is headless MCP needed early?
4. Should asset refs move from paths to durable IDs before MCP-2, so that moving an
   asset doesn't break saved conversations?
