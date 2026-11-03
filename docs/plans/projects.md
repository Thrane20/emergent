# Projects: new, open, save (PROJ-1)

## Goal

Work on more than one game project from the IDE without restarting it with a
different `EMERGENT_PROJECT_PATH`.

## Decisions

- A project is a folder: `emergent.toml` (durable `id`), a Cargo game crate and
  asset roots. The `id` is identity; the folder path is only its current location.
- One active project at a time.
- Projects may live anywhere. New projects default to `../emergent-projects/<slug>`
  beside this repository, so they are not nested inside the engine's git history.
- Save is a git commit scoped to the project folder.

## Ownership and data flow

- `src/lib/workspace.ts` owns `.data/workspace.json`:
  `{ active, recent: [{ id, name, root, openedAt }] }` (IDE-local, never in a project).
- `activeProjectRoot()` resolves `EMERGENT_PROJECT_PATH` (pins the IDE, disables
  switching), then `workspace.active`. With neither, the IDE shows New/Open;
  it does not select a sample game or the template implicitly.
  `readProjectSnapshot()` and `readProjectBinding()` read it on every request, so all
  pages, API routes and the MCP server follow the switch.
- Caches keyed by project root (component registry, brain catalogue, previews)
  stay valid across switches. IDE generation jobs are already scoped by project id.

## Actions (`/api/projects`)

- **Open** `{ action: "open", root }`: validates `emergent.toml` and `Cargo.toml`,
  stops the running game and the inspection session, sets the active project and
  updates the recent list.
- **New** `{ action: "new", name, parent? }`: copies `templates/standard`, assigns a
  new `id` and package name, rewrites Cargo path dependencies so the engine resolves
  from the new folder, initialises git with a first commit when the folder is not
  already in a repository, then opens it.
- **Duplicate** `{ action: "duplicate", name, parent? }`: copies the active project
  (without `target/`, `.git/`, staging), assigns a new `id`, then opens it.
- **Save** `{ action: "save", projectId, message? }`: `git add -A -- .` and
  `git commit -- .` inside the project folder, so only project files are committed
  even when the project sits inside another repository. Initialises git if needed.

Edits already write project files directly after validation; Save is a deliberate
snapshot, not a flush.

## Template and host (PROJ-01)

Three places, one rule each:

| Where | Holds | Reaches |
|---|---|---|
| `src/project_host/` (engine, `graphics`) | The IDE contract: every `--` command, the inspection protocol, previews, the native window. `ProjectGame` is the boundary: `load`, `loaded`, optional `components`. | Every project on its next build. |
| `templates/standard/` | Defaults for new projects: `main.rs` implementing `ProjectGame`, the starter scene, `AGENTS.md`, outputs. | Projects created after the change. |
| A project's `src/` | Its game only: behaviour, project component types, generated builders. | That project. |

Implement shared features in the engine host and new-project defaults in the
template, never in a game first. The template is a workspace member and the fixture
for the engine's preview test, so it always builds against the current engine. New
projects get `[workspace]` in `Cargo.toml` so they build anywhere, including inside
another Cargo workspace.

## Project isolation (PROJ-02)

The engine Cargo workspace consists of the engine and standard template fixture.
`projects/first_project` has its own workspace/lockfile and one dependency on the
public engine, like an external game. Its demo code, assets and test results are
not engine or IDE contracts. Build it explicitly through its manifest when working
on that game. Ordinary dependency recompilation against a changed engine is
expected; the engine build must not depend on the game's existence or contents.

With no selected/pinned project, project pages redirect to `/projects`, which
reuses the New/Open/recent-project switcher. Save and Duplicate are unavailable
until a project is open. Project APIs return a clear missing-project error.
Explicit stored selections remain valid, including an explicitly opened sample.
New projects still go beside the engine by default and use the current standard
template; the template itself is never the implicit editable project.

Validation data belongs to `tests/fixtures/`. Native IDE smoke projects are
assembled from `templates/standard` plus a small engine-owned asset overlay using
`scripts/project-fixture.ts`; they do not copy another game's code or live assets.
Smokes that connect to a running IDE require an explicit disposable
`EMERGENT_PROJECT_PATH`. Model import/split regressions share a frozen engine-owned
GLB. A regression guard scans engine/IDE production source and smoke scripts for
sample-game dependencies. Historical platform records remain historical evidence.

Acceptance: clean no-project browser flow; new project create/open/run/inspect and
shared CLI/MCP validation; current template and independent game compilation;
engine tests/build with only engine/template workspace members; no operational
first-project references in production code, tests' fixture paths or smokes.

## Validation

- Unit tests: create from template into a temporary folder, open it, check
  `readProjectSnapshot()` follows, Cargo path rewrite, duplicate id rewrite, save
  commits only project paths, pinned-environment refusal.
- Build check of a generated project with `--validate-scenes`.

## Known limits

- The IDE builds every project into the engine's `target/` (`--target-dir`), and new
  projects start with the engine's `Cargo.lock`, so a new project's first build compiles
  only its own crate (~10 s per profile). A project whose lockfile drifts from the
  engine's recompiles the drifted crates. Running `cargo` by hand inside a project uses
  that project's own `target/`.
- The host contract is not versioned: a project built against an older engine than
  the IDE expects fails at the first unknown command.
