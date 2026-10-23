# Emergent IDE

Emergent IDE is a local, reference-oriented project-mode companion for an Emergent game project. It reads `emergent.toml` and the project’s `assets/` folder, then presents asset references and declared output targets without pretending to be a runtime editor or an asset importer.

## Run

```sh
pnpm dev
```

By default it opens this repository’s `projects/first_project`. To inspect another local project:

```sh
EMERGENT_PROJECT_PATH=/absolute/path/to/game pnpm dev
```

The selected path must contain `emergent.toml` and `assets/`.

## Current boundary

The IDE discovers the manifest, authored assets, game-owned Rust, and generated Rust directly from the linked project. Selecting a file shows its text or supported model preview and incoming/outgoing connections. The asset creator writes new declarative assets, and Run starts the linked project's Cargo executable.

The full-width lower chat dock keeps separate in-memory threads scoped to the whole project, the selected file, or a durable authored entity ID discovered from scene and entity definitions. It shows the definitions attached to the current scope.

The `/settings` page configures the initial OpenAI/Codex provider, including an editable API base URL, model, and API key. Credentials are stored in the server-only, Git-ignored `.data/ai-provider.json` file with owner-only permissions and are never returned to the browser. Set `EMERGENT_IDE_DATA_DIR` to keep this state elsewhere.

Chat requests now call the configured provider through the server-side Responses API. Each request receives bounded context resolved from the live project graph for the selected project, file, or durable authored entity plus the recent scoped conversation. This first connection is read-only: it returns guidance and proposed code but cannot yet apply changes to project files.

Chat scope follows the file selected in the project tree; there is no independent scope selector. The right inspector also exposes durable intent statements for that selection. Statements are stored in the linked project's `emergent.statements.json`, use stable subject references rather than runtime handles, and can be checked against the current project by the configured AI. AI checks record implementation status, confidence, and short evidence but do not edit the statement or game files.

New statements are checked immediately after saving when an AI key is configured.
If checking fails, the intention remains saved and unchecked, with an error shown.
An existing behaviour can already satisfy a new intention; saving never assumes
that its implementation must be missing.

For an unmet intention, **Implement** searches the current project and linked
Emergent foundation, asks the configured AI for reuse-first changes, and compiles
them in a temporary copy before applying them. Only project Rust source/tests and
TOML assets can change; manifests, dependencies, engine source, and credentials
cannot. Concurrent edits prevent application. Original-file backups are retained
under `.data/statement-implementations/` (or `EMERGENT_IDE_DATA_DIR`).

After applying, an independent Check evaluates the fresh code. **Reuse & changes**
shows reused APIs/helpers, changed files, and the compilation result. Cargo check
and AI evidence do not establish runtime behaviour; use tests and native Run for
that. The current isolated compiler supports independent Cargo manifests with a
local Emergent path dependency; workspace-inherited manifests are not yet supported.

The spinning-cube reference in `first_project` was generated through this action:
it reuses engine transforms and fixed timing with project-owned `src/spin.rs`,
rotates at 15°/second, and preserves translation and scale. Its native scene is
wired to render the cube. Load remains a tick-zero still; Run shows the motion.

The IDE does not interpret authored scenes, package outputs, or retain runtime SlotMap handles. Runtime GLB importing, textures, and general model materials remain separate engine work.

## Rust scene preview

Select a scene in the top-bar **Scene viewer** dropdown and click **Load**. The IDE
runs the linked Cargo project with `--preview-scene <path> --preview-output <png>`.
The reference project constructs its registered Rust scene at tick zero, renders
it through Emergent's windowless wgpu backend, and returns a 1280×720 PNG to the
central canvas. Load again after changing Rust code to refresh it. Failed loads
retain the previous frame; selecting a dropdown option alone does not load it.

Every `.scene.toml` file is listed. The initial reference supports only its
`emergent.first-project.generated.main_scene` builder; other builders need a
project implementation of this launch contract. This first version is a paused
fixed-camera capture, without gameplay, orbit controls, or streaming. Project
assets and runtime SlotMap handles are not serialized into the frame transport.
With the IDE running, `pnpm test:scene` verifies a real Rust capture in Chrome and
checks that a failed reload preserves the previous image. Set
`EMERGENT_IDE_BASE_URL` to use a different local IDE address.

## Object Creator (preview)

Open **Object Creator** (◇) in the navigation rail, or visit `/objects`.

1. In **Settings → Object generation**, enter your funded fal account's API key.
   Alternatively set `FAL_KEY` on the IDE server. The UI reports a configured key;
   the first successful generation establishes actual provider access.
2. Enter an object name and upload a PNG, JPEG, or WebP reference (up to 10 MiB).
3. Click **Generate object** to submit one paid `fal-ai/trellis` request. Inspect
   the resulting GLB using orbit, zoom, and Reset view.
4. Click **Save to project**. The complete model, original image, and provenance
   descriptor appear under the first declared asset root's `models/<name>/` folder.
5. Return to the Project Map. Selecting `model.glb` or `object.asset.json` opens
   the model preview; connections identify the model and its reference image.

fal credentials live in the Git-ignored `.data/object-provider.json` with owner-only
permissions. Jobs and candidates live under `.data/object-jobs/`, partitioned by
project identity and canonical path. `EMERGENT_IDE_DATA_DIR` relocates both. Keep
that directory stable across restarts to retain generation history. The latest
100 candidates are listed; saved bundles remain in the game project independently.

Progress polling resumes when Object Creator is open. A failed status/download
check retries the existing request; uncertain submission outcomes require checking
the fal dashboard before starting another attempt. Submissions are never
automatically repeated. Downloaded GLBs must be self-contained and at most 100 MiB.
Validation checks the GLB container/resource layout; browser display is an additional
compatibility check, not certification of mesh quality or runtime readiness.

Saving never replaces an existing bundle. Choose another name on collision.
Publication uses a staging folder outside the asset tree and a short filesystem
lock. If the IDE crashes during saving, remove `assets/models/.object-save-lock`
(or its equivalent under your first asset root) only after confirming no save is
active, then retry the same candidate and name. Include `.emergent-object-staging/`
in an independent game's `.gitignore` to exclude abandoned staging files.

This preview has automated mocked-provider and browser validation. Real hosted
generation/GLB compatibility remains pending a configured fal key; see `plan.md`.
Generated objects do not yet appear in the native game when Run is pressed.

## Verify

```sh
pnpm test
pnpm lint
pnpm exec tsc --noEmit
pnpm build
pnpm test:ui
```

The UI smoke uses installed Chrome by default (`PLAYWRIGHT_CHANNEL` can override
the channel). It creates an isolated temporary game and IDE data directory, mocks
generation, tests real GLB preview and bundle saving, restarts the production
server, and reopens the model through the project tree. It never calls a paid API.
