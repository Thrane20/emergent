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
