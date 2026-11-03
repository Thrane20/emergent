# Independent project validation (PROJ-02)

2026-11-02, macOS arm64, Apple M3 Pro, Rust 1.99.1, Node 22.22.0,
Next.js 16.4.2. [Ownership and acceptance contract](../plans/projects.md#project-isolation-proj-02).

## Audit and resulting ownership

The host implementation already lived in `src/project_host`, but the engine Cargo
workspace still included the sample game, the IDE defaulted to its folder, and
tests/smokes copied its source or read its live assets. Native window/log labels
also assumed a single sample game. These were remaining dependencies rather than
requirements of the public `ProjectGame` contract.

The root workspace now contains only the engine and standard template fixture.
The sample has its own Cargo workspace and lockfile, and only depends on the
public engine. Its gameplay source/assets are unchanged. Native labels now read
the loaded manifest name. Root/project/template AGENTS guidance names engine
source, public APIs and template defaults as authoritative; ordinary game files
cannot define shared contracts. A regression guard scans production source and
smokes for sample paths/identities and checks the workspace membership.

The IDE has no implicit sample or template fallback. Missing selection, or a
missing stored project, leads to `/projects`; New/Open reuses the existing project
switcher. Save/Duplicate require an active project. A saved explicit selection or
`EMERGENT_PROJECT_PATH` pin still works. Project selection APIs reject missing
projects clearly. New projects retain the current template source, independent
Cargo workspace, rewritten dependency and engine lockfile seed.

Engine-owned fixtures now supply model import/split data and small renderer/IDE
test scenes. Executable smokes assemble current template code plus a fixture
asset overlay. Smokes driven against a running IDE require an explicit disposable
project. No smoke copies game code or relies on game asset edits. Historical
platform records remain descriptions of their original runs.

## Template compatibility fix found by this audit

Newer template assets include valid single-quoted TOML IDs. The old index accepted
only double quotes, so `assign_ids` tried to insert duplicate IDs. Shared index
and project metadata reads now use the installed TOML parser; manifest creation,
opening and duplication support literal-quoted metadata. Regression coverage
protects existing IDs and roots rather than rewriting the template to hide the
parser mismatch. MCP native validation and undo pass against the template fixture.

## Verification

- `cargo test --workspace --all-features --release`: passes for the engine and
  standard template. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  and workspace formatting pass. `cargo doc --all-features --no-deps` completes
  with an existing public-to-private `map::refresh` documentation-link warning.
- Strong isolation check: copied `src`, engine `assets`, `tests`, `examples`,
  `templates` and root Cargo files into a disposable tree with **no game projects
  directory**. `cargo test --manifest-path <copy>/Cargo.toml --target-dir <shared-target>
  --workspace --release --all-features` passes in that tree. Tests, examples and
  template resolve their own data without a sample-game folder.
- `cargo check --manifest-path projects/first_project/Cargo.toml --target-dir target --all-targets`
  passes independently. Explicit game tests pass: 10 unit tests and one project
  contract test. These are separate from engine acceptance.
- Serial IDE suite: **185 tests pass**. Focused project/index tests and ESLint
  pass. Production Next build passes using a separate test output directory.
- `test:mcp-edit` passes against the standard-template/engine-fixture project:
  native validation, edits, discovery/view, revisions, undo, durable media IDs,
  brain creation/edit/preview, locomotion creation/preview and component assignment.
- Browser on a separate port 3001 IDE with empty private workspace: `/`, `/brains`,
  `/objects` and `/ui` reach the chooser; no implicit project is active. Save is
  absent and Duplicate disabled. Create “Isolation Fresh”, verify its source
  matches the template, receive live Rust inspection frames, duplicate it with a
  new project ID, then reopen the original. No page errors.
- The browser-created project runs `--headless` successfully, then native
  `--run-scene assets/scenes/main.scene.toml --smoke`: Apple M3 Pro / Metal,
  two render resources, stops after 120 ticks and 237 presented frames. Logs
  correctly say “Isolation Fresh graphics” and “Isolation Fresh stopped”.
- Material's existing bake test fixture was updated with `shader: None` to match
  the newly added material field; no shader runtime behaviour was changed here.

Validation used disposable projects/private IDE state. The browser, inspection
worker, native smoke and port 3001 server were stopped; test output directories
and generated type includes were removed. The user's port 3000 and existing
project selection/assets were not changed. No paid provider calls were made.
Windows/Linux execution and formal host protocol versioning remain unverified.
