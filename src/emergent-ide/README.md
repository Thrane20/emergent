# Emergent IDE

## Editor navigation

The left rail is shared across all editor and settings screens. Its SVG icons
represent Scene, Source, Terrain, Object Creator, Object Builder, Materials,
Images, Textures, Particles, Tweens, Player settings, Settings, and Reload.
Hover for a label, or use keyboard focus; the current destination is highlighted.
The rail scrolls on shorter screens and carries the scene query between tools.

Main-editor navigation keeps its existing edit checks. Particle, material, object,
tween, and generated/edited terrain drafts also guard rail navigation so unsaved
work can be retained. `npx tsx scripts/editor-navigation-smoke.ts` checks all
destinations and draft guards against port 3002 without saving project files.

## UI Designer (UI-1)

Open **UI Designer** in the shared rail (`/ui`) to author game UI assets. The library
is scoped to the linked project. Create a blank UI or open the Options sample; add
panels, labels, buttons, toggles, sliders, steppers, text boxes and dividers. Select
widgets in the outline or preview, drag/resize them, or edit anchored pixel/percentage
rectangles and stacked layouts in the inspector. Give controls dotted action labels
and optional value names for future game bindings.

**Save UI** validates through the linked game's Rust host before writing
`assets/ui/<id>.ui.toml`. Revision checks preserve newer external edits. After saving,
the embedded chat can edit that UI using the engine's widget catalogue; generated UI
assets also pass native validation before publication.

Build with AI accepts multiline drafts even on a new or unsaved UI. Save the UI
before sending; the draft stays with that UI through the first save. The centre
workspace scrolls on shorter screens so the full chat stays reachable, and its
prompt box can be resized vertically for longer instructions.

Screen preview is SVG with shared Rust/TS layout fixtures and approximate monospace
glyphs. World preview is a perspective illustration. In-game rendering, placement,
pointer interaction and action dispatch remain UI-2/3; see the
[design](../../docs/design/ui-designer.md) and [evidence](../../docs/platform/ui-designer-macos.md).

Validation (from this directory):

```sh
pnpm test
EMERGENT_NEXT_DIST_DIR=.next-ui-build pnpm build
pnpm test:ui-designer
```

The designer smoke starts its own production server on port 3001, writes only to a
temporary copy of the standard template, mocks chat/provider traffic, and stops the
server and removes the temporary project when finished. It requires installed Chrome.

## Appearance

Open **Settings → Appearance** to switch between **Charcoal · soft depth** and
**Emergent green · original**. Charcoal is the default, with neutral graphite
surfaces, raised panels, inset fields, and visible keyboard focus. Image previews
and rendered content keep their own colours; status colours remain meaningful.
The selection is saved in this browser's local storage and restored before paint.

Shared palette and depth tokens live in `src/app/themes.css`. Existing green
colours remain the fallback values in each tool's stylesheet. New interface
styles should use the same tokens; content colours belong to their assets.
For a separate development instance, use
`EMERGENT_NEXT_DIST_DIR=.next-charcoal-dev npm run dev -- --port 3002`.

## Image Generator and library

Open **Image Generator** from the project rail, or visit `/images`. The library
shows existing PNG/JPEG/WebP assets across the linked project's asset roots as
named tiles. Click a tile for a large preview; Escape or Close dismisses it.
Use Refresh after adding files outside the IDE, or Import image for PNG/JPEG files.

Enter an image name and prompt in **Create with AI**. Generation uses the saved
fal key and the existing FLUX.1 Schnell pipeline with a plain text prompt and one
1024 × 1024 candidate. Completed results remain previews until you drag them into
the library or choose **Add to library**. Saved images live in project assets with
their generation provenance and can be referenced by other project tools. This
library belongs to the linked project; it is not a cross-project store.

`npm run test:images` exercises this workflow in a temporary project with mocked
fal submission and real image validation/saves. It uses port 3001 by default
(`EMERGENT_IMAGE_TEST_PORT` overrides it), and a production build in
`.next-images-build` (`EMERGENT_NEXT_DIST_DIR` overrides it). Build first with
`EMERGENT_NEXT_DIST_DIR=.next-images-build npm run build`. See
[the image library design](../../docs/design/image-generator-library.md).

Emergent IDE is a local, reference-oriented project-mode companion for an Emergent game project. It reads `emergent.toml` and the project’s `assets/` folder, then presents asset references and declared output targets without pretending to be a runtime editor or an asset importer.

## AI connections: MCP server and provider

The IDE exposes the linked project to AI coding tools through MCP, so a conversation can
refer to objects by name ("the pyramid"). The tool resolves the name to a durable ref
such as `instance:main/pyramid`, `asset:pyramid` or `scene:main`, then edits it.

- **Read:** `project_overview`, `find`, `list`, `describe`, `view` (a native tick-zero
  render) and `history`.
- **Edit:** `set_transform`, `rename`, `reparent`, `delete`, `place_asset`,
  `set_material`, `set_physics`, `set_camera`, `set_environment`, `assign_ids` and `undo`.
- **UI authoring:** `list_ui_widgets`, `create_ui`, `edit_ui`, `preview_ui`.
  UI assets are discoverable with `find` / `list` (`kind: "ui"`), and `describe`
  returns the node tree, action labels and revision. Edits add/update/remove/move/
  reorder widgets by node ID; only panels hold children and the root stays in place.
  Preview returns paged canvas rectangles in pixels after Rust validation, with
  `offset` and `limit` (up to 200). It does not render UI in game.

Edits need the `revision` from `describe`, run the same Rust validation as the
inspector, accept `dryRun`, and are journalled for `undo`. An open IDE page refreshes
by itself; transforms and reparenting appear in the live view without a restart.
See [the design](../../docs/design/project-mcp-server.md).

The IDE must be running (`pnpm dev`). Clients connect through a stdio bridge that reads
a per-install token from `.data/mcp-token`; the bridge creates the token on first use.

- **Claude Code:** the repository's `.mcp.json` registers `emergent`. Start `claude`
  from the repository root and approve the server when asked.
- **Codex:** run `codex mcp add emergent -- node <repo>/src/emergent-ide/scripts/mcp-stdio.mjs`,
  using the absolute repository path. Interactive Codex asks before write tools; for
  non-interactive `codex exec`, set `mcp_servers.emergent.default_tools_approval_mode = "approve"`.

Smoke tests: `pnpm test:mcp-edit` runs scene and behaviour edits against a copy at
`target/mcp-smoke/game`, which has its own Cargo target. `scripts/mcp-live-smoke.mts`
checks live IDE updates in Chrome against an isolated dev server; see its header.
`pnpm exec tsx --conditions=react-server --test tests/mcp-ui.test.ts` exercises UI
tools through the authenticated HTTP endpoint and embedded adapter using a temporary
standard-template project. `pnpm test:ui-designer` also checks external MCP edits
refresh the UI library/editor and preserve unsaved local drafts.

`EMERGENT_IDE_URL` overrides the default `http://localhost:3000`, for example
`http://[::1]:3001` for a test server. Avoid `127.0.0.1` when VS Code is forwarding
that port.

Settings → **AI provider** chooses whether the IDE's own chat, Statements and
Implement use **Claude** (Anthropic Messages API, default model `claude-opus-5-5`),
**Codex** (OpenAI Responses), **Vercel AI Gateway** (Responses-compatible API), or a
local **Ollama** (Responses API at `http://localhost:<port>/v1`, default port 11434).
Each provider keeps its own model, URL and key.

Ollama needs no key: choose a port, check the live status line (it lists the models
installed on that port), pick a model and click **Activate Ollama**; **Deactivate**
turns it off. The status check and all requests use `localhost`, so an SSH tunnel bound
to `::1` is what the IDE talks to.

For Vercel, select **Vercel AI Gateway**, paste your Gateway API key, choose a model,
and click **Save and use Vercel AI Gateway**. The default URL is
`https://ai-gateway.vercel.sh/v1`. The model catalogue loads current language models
advertising tool use and structured output from Vercel's public `/v1/models` endpoint;
**Refresh models** reloads it without sending your key. You can also enter a
`creator/model` ID, including a custom model, directly. The initial model is
`anthropic/claude-sonnet-4.6`. Save a different model to use it for subsequent AI
requests without restarting the IDE. In-flight tool conversations retain their
starting model. Account access, credits and individual model capabilities still
apply; catalogue inclusion does not prove that a paid request will succeed.
Keys use the existing server-only `.data/ai-provider.json` store (owner-only file
permissions; masked hints in the UI), outside game projects. This is local file
storage, not an encrypted credential vault. Existing Claude/Codex settings are kept.
See [research and validation](../../docs/platform/vercel-ai-gateway.md).

Configured AI providers appear in every shared top bar beside the fal balance.
The small **Refresh AI balances** icon is the only trigger for Gateway balance
requests: no AI balance polling occurs on load, navigation, focus or a timer.
Before refresh, Vercel displays a dash; afterwards it shows available USD credit
and the check time in its tooltip. OpenAI and Anthropic show **Billing ↗** because
their configured inference keys do not expose a supported remaining-credit API.
Those links open the provider billing page; spend/usage is not reported as credit.
Changing provider credentials clears the displayed snapshot without querying the
provider. The fal badge retains its existing independent refresh behavior.
See [AI balance evidence](../../docs/platform/ai-balances.md).

Embedded project chat uses these providers with a local MCP client. Tool definitions
and instructions come from the same server used by external clients; no public MCP
URL or additional key is needed. The assistant can inspect the project, edit scenes
and behaviours, read resources and view scene renders. Calls run sequentially, with
at most 24 model turns / 32 tool calls per request. Switching projects stops the
conversation. Successful edits include change IDs and remain reported if a later
step fails; each can be undone through MCP. Multi-step requests are not atomic.
This applies to project chat; scoped editor AI and Statement requests keep their
existing structured-output contracts.

## Model Parts: cut with a box

In **Model Parts** (`/models`), open a generated model and click **Cut with box**.
Drag a rectangle over the viewport. The box starts aligned to that view, through
the model's centre, with depth equal to the rectangle's width.

Drag any of its eight corner handles to resize it. Orbit around the model to adjust
depth from another view; the box keeps its original orientation. Arrow keys resize
a focused handle in the current view plane (Shift gives a smaller step). The local
centre and size fields provide exact adjustment. **Redraw box** starts another
rectangle; **Cancel box cut** or Escape discards the draft without changing geometry.

Click **Cut** to remove surfaces inside the box from every intersected part, then
**Save To Object Library** to keep the edit. The boundary clips triangles while retaining texture
coordinates and normals. Fully removed parts are dropped and their children reattach
to a surviving ancestor. Removing the entire model is refused. Cuts leave open edges;
they do not fill the hole with new faces. Existing part IDs, pivots and materials stay
intact. See [validation](../../docs/platform/model-box-cut.md).

**Save To Object Library** also publishes the complete part hierarchy as a reusable
prefab in the project's Object Library (`/object-builder`), alongside Object Builder
objects. Repeated saves update the same entry. Click its tile to reopen Model Parts;
right-click it and choose **Add to scene** to place the whole object, including child
parts and their pivots. Saving does not insert it into a scene automatically. Existing
part sets enter the library the next time you save them. See
[save/library validation](../../docs/platform/model-parts-library.md).

## Run

```sh
pnpm dev
```

With no saved selection, the IDE shows **Open or create a project**. New games are
created from `templates/standard`; the template is not opened for editing. The
selected project is remembered in the private IDE workspace. To pin a local project:

```sh
EMERGENT_PROJECT_PATH=/absolute/path/to/game pnpm dev
```

The selected path must contain `emergent.toml` and `assets/`.

## Persistent inspection

Open the **Scene** tab to inspect the selected scene automatically. This starts the linked project's
`cargo run --release -- --inspect-scene <path>` authoring worker, retaining the
world and native renderer with gameplay tick stopped at zero. Drag to orbit,
Shift-drag to pan, scroll to zoom, or Reset view. The Transform inspector's
edits save automatically and update the retained object without
reconstructing the scene. Normal game Run remains independent.

**Load** refreshes code/geometry/scene membership and external asset changes.
Leaving the Scene tab stops its owned inspection process; returning starts a fresh
session. Selecting another scene switches inspection automatically. A failed stream retains its last canvas image. Browser
disconnects expire after 30 seconds plus a lease-check interval, avoiding a React
Strict Mode cleanup race. One local inspection session is supported at a time;
stale session IDs and other project bindings are rejected.

This prototype delivers latest-frame 960×540 PNGs over bounded HTTP long polling,
targeting 30 Hz, with a separate acknowledged control endpoint. It keeps no frame
backlog and bounds diagnostics, command queues and readback/command deadlines.
It is not hardware-encoded video or a security sandbox. Measure the authoring
transport before increasing quality or rate; see the recorded macOS evidence.

`pnpm test:inspection` uses the running IDE, exercises real camera/property edits
and process lifetime, records a short reference-scene timing sample, and verifies
idle expiry. It cleans only its own temporary assets and makes no AI/provider call.

## Current boundary

The IDE discovers the manifest, authored assets, game-owned Rust, and generated Rust directly from the linked project. Selecting a file shows its text or supported model preview and incoming/outgoing connections. The asset creator writes new declarative assets, and Run starts the linked project's Cargo executable.

The full-width lower chat dock offers a Scope selector: Game uses the linked project, Scene uses the active scene, and Object uses the selected instance or object asset. Conversations and unsent drafts stay separate for each context. Context paths remain part of the request data without being shown as badges.

The `/settings` page configures Claude, Codex, Vercel AI Gateway and local Ollama, including an editable API base URL, model, and API key. Credentials are stored in the server-only, Git-ignored `.data/ai-provider.json` file with owner-only permissions and are never returned to the browser. Set `EMERGENT_IDE_DATA_DIR` to keep this state elsewhere.

Chat requests now call the configured provider through the server-side Responses API. Each request receives bounded context resolved from the live project graph for the selected project, file, or durable authored entity plus the recent scoped conversation. Edit requests now produce structured replacement files, inspect existing project/foundation code for reuse, validate the candidate with Cargo and native scene loading in a temporary copy, then publish guarded changes. Questions can return an answer with no file changes. Applied edits refresh the project graph and paused Scene inspection. Failures and concurrent edits are reported without publishing the candidate.

Object scope follows the current object selection using durable IDs and paths; it is unavailable when no object is selected. Explicit Game or Scene selections stay at that level as object selection changes. Instance context includes both its authored scene and referenced asset definition. The right inspector also exposes durable intent statements for that selection. Statements are stored in the linked project's `emergent.statements.json`, use stable subject references rather than runtime handles, and can be checked against the current project by the configured AI. AI checks record implementation status, confidence, and short evidence but do not edit the statement or game files.

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

Project-specific intentions can generate game-owned behaviour using engine
transforms and fixed timing. Shared components and tooling belong to the engine,
not a sample game. Inspection remains at tick zero; Run executes the game.

The IDE does not interpret authored scenes, package outputs, or retain runtime SlotMap handles. Runtime GLB importing, textures, and general model materials remain separate engine work.

## Rust scene preview

Select a scene in the top-bar **Scene viewer** dropdown and click **Load**. The IDE
runs the linked Cargo project with `--preview-scene <path> --preview-output <png>`.
The reference project constructs its registered Rust scene at tick zero, renders
it through Emergent's windowless wgpu backend, and returns a 1280×720 PNG to the
central canvas. Load again after changing Rust code to refresh it. Failed loads
retain the previous frame; selecting a dropdown option alone does not load it.

Every `.scene.toml` file is listed. The reference loads primitive/light/empty-node
instances through the engine's schema-1 loader. Scenes need no Rust builder; an
optional registered builder adds project behaviours. Add Asset can include a new
supported entity in the selected scene. Edit its asset values in your code editor
and Load again; native Run loads the manifest entry scene through the same path.
Unknown builders and invalid definitions fail explicitly. This version is a paused
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
   descriptor appear under the first declared asset root's `models/<unique-id>/` folder.
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

Saving never replaces an existing bundle. Display names may repeat; each generation
uses its own durable ID and bundle folder.
Publication uses a staging folder outside the asset tree and a short filesystem
lock. If the IDE crashes during saving, remove `assets/models/.object-save-lock`
(or its equivalent under your first asset root) only after confirming no save is
active, then retry the same candidate and name. Include `.emergent-object-staging/`
in an independent game's `.gitignore` to exclude abandoned staging files.

This preview has automated mocked-provider and browser validation. A real hosted
robot-arm GLB was saved and reopened through both model and descriptor selection
on 2026-10-24; see `docs/history/epic-log-2026-10.md` (OC-1).
Generated objects do not yet appear in the native game when Run is pressed.

## Verify

```sh
pnpm test
pnpm lint
pnpm exec tsc --noEmit
pnpm build
pnpm test:ui
pnpm test:authored
```

The UI smoke uses installed Chrome by default (`PLAYWRIGHT_CHANNEL` can override
the channel). It creates an isolated temporary game and IDE data directory, mocks
generation, tests real GLB preview and bundle saving, restarts the production
server, and reopens the model through the project tree. It never calls a paid API.

`test:authored` uses the running IDE (default localhost:3000), creates uniquely
named temporary scene/entity assets, proves that IDE inclusion and asset edits
change a real Rust-rendered frame, and checks invalid reload preservation. It
removes only its temporary assets afterward. Point `EMERGENT_PROJECT_PATH` at the
same project as the server when using a non-default project.

## Scene-instance transforms

Select an item in the primary **Scene graph** to open the right-side
Transform inspector. Use the header Local/Global switch to edit Position, Rotation (degrees) and Scale; Uniform
scale links all axes. Apply to scene saves an instance override in the scene and
refreshes the Rust frame. Reset to asset defaults removes the override. The
referenced entity asset and other instances remain unchanged.

The server checks scene/asset revisions, validates a temporary candidate through
the linked game's `--validate-scene <scene> --candidate <source>` CLI, then uses
guarded publication. Reload inspector rereads changed files and discards drafts.
The first editor supports table-form `[[entities]]` and `[entities.transform]`
syntax; unsupported editable forms fail without overwriting source.
`pnpm test:placement` exercises the real workflow against the running IDE using
smoke-owned temporary assets, including Rust rejection and external-edit conflicts.


## Scene graph and source editing

Drag an instance onto a parent, or onto **Scene root** to unparent it. The inspector
Parent selector provides the same operation by keyboard. Reparenting keeps local
TRS; the subtree inherits the new parent's translation, rotation and scale.
Global edits use Rust World math and persist parent-local values. If hierarchy
scale/rotation creates shear, Global exposes position only; use Local for the
remaining axes. Unsupported conversions fail without changing source.

Use the delete control on a scene graph row to remove that instance and its
descendants from the current scene; their entity asset files remain in the
project. In **Project files**, delete controls permanently remove unreferenced
files under declared asset roots. Load another scene before deleting the open
scene. Referenced assets and the entry scene are protected.

Project files remain available under the collapsed **Project files** section.
The **TOML** tab edits the selected scene or shared entity definition. Select
**Scene overrides** or **Entity asset** for a scene instance. Save validates a
staged project copy and every authored scene, detects external changes, then
publishes only the edited file. Shared assets affect every referencing instance.
Use Load after external whole-file TOML changes; saved numeric transforms
and parent edits update an active inspection world directly.

`pnpm test:graph` uses the running IDE and real native inspection worker, creates
only uniquely named temporary assets, and checks parenting, inherited/global
transforms and guarded TOML saves. It makes no AI/provider requests.

## Player settings

Open **Player settings** from the project-map rail to define display resolution,
target and minimum expected frame rates, and anti-aliasing preferences. These
project targets are saved in `player.settings.toml`; the current runtime does not
yet apply them to game output or rendering.


## Viewport picking and gizmos

Open the **Scene** tab, then click an object to select it. Picking tests
actual rendered triangles and selects the nearest visible surface; light and empty
nodes have origin markers. The tree and Transform inspector follow selection.
Choose **Translate**, **Rotate** or **Scale** above the viewport, then drag an XYZ
axis or rotation ring. The header **Local / Global** switch controls coordinates.
Local translation follows parent axes; local rotation/scale follow object axes.
Gizmo handles also accept arrow keys for small edits when keyboard-focused.

Movement previews in the retained world without saving files. Release to validate
and save once; Escape or pointer cancellation restores the initial transform.
External scene/asset edits reject the save and restore the preview. Near-parallel
axes are hidden; change the view if needed. Global rotation/scale that cannot be
represented without shear are rejected; use Local. Normal Run stays independent.

The viewport shows delivered fps against a 30 fps target. PNG transport is still
bounded to 960x540 with no frame backlog. This is a reference Mac implementation,
not a general frame-rate guarantee. `pnpm test:gizmos` covers real native picking,
all three drag tools, cancellation, stale gestures and source conflicts. Set
`EMERGENT_IDE_BASE_URL` if the running IDE uses another port.


Global Scale handles act along world axes about the selected object's pivot.
A rotated object may require shear for a nonuniform world-axis scale; that edit
is rejected and rolled back. Use Local for object-axis scaling. Keyboard handles
also support partially clipped rotation rings, and pointer cancellation follows
the same rollback path as Escape.


## Noise filter assets

Use **Add asset → Filters → Noise function**. Select the .noise.toml file to edit
Perlin/OpenSimplex/Worley settings in the right inspector. **Generate preview**
uses the draft inputs; **Save settings** separately validates/persists them.
Overview shows normalized grayscale (0 black, 1 white). A 3D request generates a
complete cube with selectable XY slices, not just a sampled plane.

Settings include seed, resolution/dimensions, frequency, octaves, lacunarity,
persistence, domain extent and origin offset. Rust owns the versioned sampling
contract, pinned to noise-rs 0.9.0. The headless noise_filter example is reusable by
linked projects. No noise code runs in the game's frame loop by default.

Maximums: 512² 2D, 128³ 3D, eight octaves and 2,097,152 octave evaluations. Generated
buffers are transient; the asset stores the function definition. Raw TOML edits
share Rust validation and source guards. Terrain/material consumers, filter
composition graphs and baked field exports are not implemented.

`pnpm test:noise` creates a uniquely named filter through the running IDE, calls
the real Rust generator and checks seeds, algorithms, grayscale pixels, 3D slices,
save/reload and invalid/stale edits. It removes its fixture afterward and makes
no AI/provider requests.


## Development watcher stability

Start from the repository root with `pnpm --filter emergent-ide dev` and open
http://localhost:3000. Tailwind's source base is explicitly the IDE `src/` tree;
its scanner does not need the package root, scripts, tests or generated output.
Keep that boundary when adding styles so generated files cannot feed a broad
source watcher.

`pnpm test:dev` runs against the existing dev server. It checks idle stability,
writes/removes a smoke-owned cache probe, then adds/removes a unique CSS custom
property and verifies a real hot update without a document reload or continuing
recompiles. It preserves concurrent CSS edits and does not alter game assets.
The compile/refresh-loop repair was checked with a fresh `.next/dev` cache, the
exact filtered start command, installed headless Chrome and a production build.


Warm-restart follow-up: clearing the development cache alone did not hold across
later launches. `next.config.ts` now disables only
`experimental.turbopackFileSystemCacheForDev`, so development starts do not restore
the problematic persisted compilation graph. Production filesystem caching is
unchanged. Re-enable the development cache only after repeat start/stop and CSS
hot-update checks pass with persistence enabled. Validation must include restarting
without deleting `.next/dev` between runs; a successful fresh-cache launch alone
is insufficient evidence for this failure.

The disabled-cache setting was checked with three consecutive launches using
`pnpm --filter emergent-ide dev`, without deleting the dev cache between starts,
including an interactive terminal launch. Each passed `test:dev`; TypeScript,
lint and the production build passed as well. This is a development-cache bypass,
not a repair to Turbopack's persistence implementation.

### Asset names and identity

Add asset assigns a new UUID to each asset and uses it in the filename and initial
scene-instance ID. `name` is the display label and can repeat, including within one
scene. Scene graph rows, inspector/parent labels, file lists and noise pickers show
the name; selection, parenting, references and statement scopes retain IDs/paths.
Existing files and IDs are retained; assets without names fall back to legacy labels.

### Actionable Build with AI

For example, choose Scene scope and ask “Add a new cube at the world centre”.
Chat reuses a suitable cube asset or creates one, adds a distinct UUID-backed root
instance at [0,0,0], validates and saves the change, and reloads paused inspection.
Existing instances stay intact. Requests carry the active scene even in Game scope.

For supported scene/behaviour operations, chat uses the MCP tools and their revision
checks, native validation and undo journal. For new code or asset types without an
MCP authoring tool, `implement_project_code` hands the original request to the checked
file workflow. This handoff can run only before any MCP edit in that request.

The file workflow shares the statement implementation's bounds: project Rust and asset TOML,
eight replacements / 256 KiB total, inspected-source conflict checks, journalled
publication and rollback. Engine source, dependency/manifests, file deletion and shell
commands are outside the file workflow. MCP does support scene-instance deletion.
At most 127 scenes are validated; at most
16 supplied new authored UUIDs per request. Only successful host publication is
reported as applied. Questions do not trigger file publication.

Reuse citations may reference inspected TOML assets by filename or source identifiers.
The AI receives exact citation examples. A citation-format rejection triggers one
automatic correction attempt before any writes; persistent mismatches name the
rejected path/symbol. Cargo/scene checks still gate publication.

### Rename objects and edit Source

Select a scene-graph object and press Enter to edit its local display name. Enter
or leaving the field saves; Escape cancels. Duplicate names are allowed. The
scene stores the override as `name` on that instance; IDs, parenting and asset
references are retained. Names contain 1–64 printable Unicode characters.

The Source tab for an entity asset now uses the guarded TOML editor. Change its
shared `name` and press Save TOML. Instances without a local name inherit that
change; locally named instances retain their overrides. Remove an instance's
`name` in scene TOML to restore inheritance. Returning to Scene constructs the
paused world using the saved names.

## Terrain Builder

Open `/terrain` or the △ Terrain Builder link in the project rail. Choose Hilly,
Mountainous, Rivers or Island, adjust seed, width, relief, frequency, roughness,
octaves and resolution, then Generate terrain. Drag to orbit, scroll to zoom,
and Reset view to return to the initial overview. A new seed explores another
landscape; the same preset and inputs reproduce the same result.

The native Rust terrain tool generates the height field; the browser shows
a smooth, untextured grayscale mesh. Resolutions are 32, 64, 128, 256, 512 and 1024 points per side. A 1024 grid has about 2.1 million triangles; choose it when the extra detail is useful. Three-way algorithm comparison remains capped at 256. Existing saved terrains retain their current resolution until explicitly regenerated and saved.
Generation is explicit and bounded, and failed requests keep the last good
preview. Navigation cancels the client request and releases old preview URLs.

Generation previews are ephemeral until you choose Save terrain to scene.
Rivers carve a winding valley without water simulation. Terrain chunks and runtime LOD remain future work. No AI/fal key or
paid generation is involved.

With the IDE running, `pnpm --filter emergent-ide test:terrain` checks native
generation, deterministic seeds, viewer interaction, failure recovery and a narrow
layout. It changes no project files. Set `EMERGENT_IDE_BASE_URL` for another port.

Terrain previews include an angled directional sun to reveal slopes. Mountainous
uses a Mountain count input (1–12, default 3) for spaced main peaks. Seed controls
their layout, while roughness/frequency/octaves add surface detail. Roughness is
a 0–1 slider with a numeric readout; press Generate terrain after changing inputs.

In Terrain Builder, the **Directional light** section controls Sun strength,
direction and elevation. Press Generate terrain to apply them; the preview
header shows its applied sun strength. Set strength to zero to compare lighting.
A dim neutral environment keeps shaded slopes readable without washing out relief.
These controls affect the preview and do not add a light to the game project.


### Terrain pipeline and comparison

Choose **Base generator** (Perlin noise or Diamond-square), adjust **Domain warp**,
**Ridge strength** (mountainous only) and **Thermal erosion passes** (0–64), then
Generate terrain. Frequency/octaves affect Perlin; roughness controls Perlin octave
amplitude or diamond-square displacement decay. Default enhancement is warp 0.25,
ridges 0.5, erosion 16 passes. Zero disables an enhancement.

**Compare algorithms** shows Original noise, Diamond-square and Enhanced pipeline
(the current settings) together. Seed, preset, width, relief, resolution, sunlight
and absolute initial camera match; cameras can orbit independently. At narrow
widths cards stack. If enhancements are off and Perlin is selected, the enhanced
card intentionally matches Original noise.

Each result shows Rust generation milliseconds, server GLB preparation milliseconds,
height-field KiB and GLB KiB. Timings exclude Cargo compilation/process launch,
HTTP transfer and rendering; sizes are outputs, not peak RAM/GPU use. GLBs embed
the versioned recipe in `asset.extras.terrainSettings`. Generation writes no user files; Save to scene is explicit.

The reusable engine contract is `TerrainSettings::generate()` → `TerrainField`,
with `values()`, `settings()` and `mesh(Color)` for game-owned integration. The IDE
now delegates all terrain shaping to Rust. Diamond-square uses a 2^k+1 intermediate
grid resampled to the selected output resolution. Independent patches do not yet
have seamless edges. Erosion is thermal slope relaxation, not hydraulic erosion.
Ray marching remains deferred. See `docs/design/terrain-generation-pipeline.md`
and `docs/benchmarks/terrain-comparison-2026-10-25/README.md` at the engine root.


Mountainous now defaults to algorithm version 2: each Mountain count unit is a
major group with an irregular crest, multiple uneven summits, saddles and broad
shoulders. Existing ridge links still connect those groups. Refresh the page and
Generate terrain to see it. Comparison retains the version-1 conical baselines
beside the enhanced version-2 terrain. Explicit version-1 recipes remain accepted
and reproduce their earlier shape; no new controls or saved project changes.


### Save terrain to the active scene

In Terrain Builder, generate first, then use **Save to scene**: choose a display
name, confirm **Destination scene**, and optionally check **Terrain collider**
(default on). The scene selected in the project IDE is carried into Terrain
Builder; opening `/terrain` directly defaults to the entry scene. Save uses the
last displayed generated result, including Enhanced pipeline in comparison mode;
editing inputs afterward does not alter what will be saved.

Save creates `<asset-root>/terrains/<uuid>.entity.toml` with kind `terrain` and a
versioned `[terrain]` recipe, then adds a distinct UUID-backed root instance to
the destination scene. Duplicate display names are allowed. Native staged checks
run before conflict-checked, journalled publication; stale scene saves are rejected
and a failure leaves no orphan asset. **View in scene** opens that scene with the
new instance selected. Inspection and Run regenerate the mesh from its recipe.
The Terrain Builder sunlight is preview lighting; game scene lights are retained.

The **Physics** inspector offers **Terrain — follows surface** for terrain assets.
It is a static Rapier heightfield matching the mesh grid and diagonal, including
positive scale and pose. Toggle Physics enabled to add/remove collision; friction
and bounce remain editable. Dynamic terrain is unsupported. Inspection remains
at tick zero; Run simulates contacts and gravity. Restart Run after authored edits.
Terrain collision has no separate box outline. Box colliders remain available.

Scene cameras now frame saved terrain with a wider bounded orbit and appropriate
far plane. This first slice does not add seamless chunks, LOD, editing height
samples, frozen heightfield export, or hydraulic erosion. Explicit old recipes
remain supported. `pnpm --filter emergent-ide test:terrain-save` uses its own
temporary game project and production server (port 3002 by default, override with
`EMERGENT_TERRAIN_TEST_PORT`); run the IDE build first. It leaves user assets intact.


### Diffuse images and terrain material layers

Select a renderable scene instance and open Material. Click **Diffuse image** to
choose a project PNG/JPEG or **Import PNG / JPEG…**. Imports create unique folders
under `<asset-root>/textures/`, preserving a readable filename without replacing
existing files. The image is checked natively before publication (16 MiB encoded,
4096 per axis, at most 4,194,304 pixels). Invalid files do not enter the project.

Changes save automatically and refresh native inspection; Run uses the saved
surface on its next start. **Repeat / local metre** controls image tiling. The
albedo colour tints the image, while existing noise/specular/emission/bump controls
remain independent. Triplanar projection wraps cubes, spheres and terrain without
UV authoring. Images are sRGB, mip-filtered and currently opaque; alpha cutouts
and transparent materials are not part of this slice.

Terrain instances also expose **Terrain materials**. Add and name up to eight
layers, assign their diffuse images, then choose **Surface layer** to see one
across the terrain. Layer IDs stay stable when names change. Removing the active
layer restores the base image/colour. This stores the palette for later height
and slope rules; brush painting, weight maps and automatic height blending are
not implemented yet. Material edits affect scene instances and can reset to asset
defaults. The Source/TOML editor can also define shared `[material]` defaults.

`pnpm --filter emergent-ide test:textures` runs against its own temporary project
and production server (port 3003, override `EMERGENT_TEXTURE_TEST_PORT`); build the
IDE first. Native coverage is also available via
`cargo run --release --features graphics --example diffuse_material_smoke`.


### Texture Creator — AI textures with fal

Open **Texture Creator** (▧) from the project rail, Object Creator or Terrain
Builder, or visit `/textures`. It uses the same fal key configured for object
and texture generation in Settings; `FAL_KEY` remains supported on the server.

Enter a texture name and prompt, choose 1–4 candidates and a seed, then Generate
textures. The first provider is `fal-ai/flux/schnell`: square 1024×1024 PNGs,
four inference steps. Optional tile-friendly prompting asks for a flat diffuse
surface without lighting/perspective; seamless edges are not guaranteed. Each
Generate click starts a paid provider request. Refreshing/checking an existing
request never submits it again, including after restart or an uncertain response.

Review candidates, tick the images to keep, optionally edit their project names,
and **Add selected to project**. Each saved image gets its own UUID folder under
the declared asset root's `textures/`, plus `texture.asset.json` provenance with
prompt, seed, endpoint/request ID and candidate index. Credentials are excluded.
Duplicate display names are allowed. Saves are idempotent; already saved images
are not duplicated on retry. Generation history and unsaved images stay in IDE
local data, with the latest 100 jobs shown. Results persist across page reloads.

Saved generated images appear automatically in **Diffuse image** choices on
objects and in each terrain material layer's image picker. No re-import step is
required. Existing terrain palettes can retain multiple named materials for later
painting; this editor does not add brush strokes or height/slope blending.

Queue/credential/credit failures are shown with the selected attempt. For an
uncertain submission, check fal's dashboard before explicitly starting a new paid
attempt. The IDE does not automatically resubmit it. If checking or native image
validation fails, **Check request** retries observation/validation of that same
request. Network downloads are restricted to fal media hosts and validated with
the existing native PNG/JPEG bounds before previews or project publication.

`pnpm --filter emergent-ide test:texture-creator` uses a temporary game project,
mocked fal responses and real native validation/project saves. It starts its own
production server on port 3004 (override `EMERGENT_TEXTURE_CREATOR_TEST_PORT`);
build the IDE first. It spends no fal credits and leaves user game assets intact.
Provider schema: https://fal.ai/models/fal-ai/flux/schnell/api.


### Audio

Open `/audio` or choose **Audio** in the project rail. The **Audio library** carousel shows
the project's `.audio.toml` clips (each tile draws the waveform the engine plays, coloured by
category and cached per clip version; a spark marks generated ones; **＋ New
clip** opens Generate). **Import file…** adds a WAV, MP3, OGG Vorbis or FLAC file (format
sniffed from its bytes, 50 MiB max) under `assets/audio/<id>/`. Select a clip to see its
waveform: drag the white handles to trim and the teal handles to set loop points (arrow
keys move a focused handle; Shift for bigger steps), set gain (**N** normalises the peak
to -1 dBFS), fades and category, and press **Play** to preview. **Save clip** validates
and decodes through the game binary (`--inspect-audio`) before writing; the audio file is
never rewritten. **Playback** chooses whether a clip streams from disk (Auto streams music)
or loads whole. **Add sound point** places a Point playing the clip in a scene.

**Generate** makes sound effects, music or speech with fal (default ElevenLabs models;
any compatible catalogue model can be chosen). Each candidate is one paid request; listen,
then **Keep** saves it as a clip with its prompt and model under `[generation]`. Voice
suggestions come from the chosen model (ElevenLabs presets, MiniMax voice IDs). The MCP
`generate_audio` tool does the same in one call; it requires `confirm_paid: true`.
`tsx scripts/audio-smoke.ts` drives the editor against a running IDE on port 3001 (no paid
calls).

### Scene map

Open `/map` or choose **Scene map** in the rail: the current scene from above, with object
outlines drawn by the engine (`--map-layout`). Drag empty space to pan and scroll to change
height. Pick a tool and click the map: **Waypoint**, **Waypoint path** (click points, Enter
to finish; they're linked and grouped), **Link** (click two waypoints), **Circle zone**
(drag the radius) and **Rectangle zone** (drag corners). **Select** moves items and Delete
removes them. The inspector edits names, zone shape, travel cost, blocked and layers,
waypoint links and metadata pairs. Edits go straight into the scene and **Undo** reverts
the last one. Games query the result with `World::zones_at`, `cost_at`, `segment_cost`,
`items_where`, `nearest_waypoint` and `waypoint_path`.
`scripts/map-smoke.ts` drives it against a scratch project (`MAP_SMOKE_PROJECT`).

### Tween Editor

Open `/tweens` or choose **Tween Editor** (∿) in the project rail. Select one of
10 easing variants, adjust duration/start/end/playback, and watch the cube move.
Pause, scrub or restart to inspect timing. **Save selected variant** creates a
uniquely identified `.tween.toml` asset under the linked project's first asset
root. Select a project variant to reopen it; saving again creates a new variant.
Names can repeat. Source/TOML edits are validated by the same Rust sampler.

These scalar recipes can be loaded with `TweenDefinition::load` and sampled from
ordinary game behaviours. Saving alone does not animate a scene instance.
`pnpm --filter emergent-ide test:tweens` exercises the UI against a temporary
project using a separate production server; run `build` first.


### Camera inspector and foundation search

**Add asset** now starts with a search field. Type to filter the foundation tree
and expand matching categories; clear to browse normally.

Select a camera in Scene graph for live projection, FOV (perspective) or view
height (orthographic), and near/far clipping controls. These are instance overrides;
**Use asset defaults** removes the override. Select the camera file under Project
files to edit its shared defaults instead. Invalid planes are retained as a draft
and do not alter the project. The selected camera's gold frustum follows its
transform and settings; orbit the scene view to see it from outside. The frustum
uses the current viewport aspect ratio and stays out of Run rendering.

`pnpm --filter emergent-ide test:cameras` runs the browser flow in a temporary
project and a separate production server (build first).


### Facing and viewport shortcuts

The Transform inspector shows **Look-at direction · world**, a normalized vector
computed from local -Z through the object's rotation and parent transforms.
Rotation remains the source of orientation.

- **F** frames the selected object's bounds with the editor camera.
- **Shift-F** moves the selected object to the editor camera's exact position and
  matches its facing. Scale is preserved and the scene override is saved.

These shortcuts work from the scene graph/viewport and ignore typing in inputs,
chat, dialogs and ongoing edits. Alignment rejects parent conversions that would
require shear. F changes only the editor view. Reload the scene once after an
engine update to refresh the native worker's shortcut commands.

### Terrain brush

After Generate terrain, paint the top-down heightmap to the left of the 3D preview.
Brush controls are stacked underneath the heightmap; the 3D viewer fills the
right-hand panel. Narrow screens stack the sculpting panel above the preview.
Choose Raise, Lower, Smooth or Set height / flatten. The circular grayscale mask
fades from white (opacity 1) at the centre to black (0) at its edge; strength
multiplies that opacity. Radius is in metres. Raise/lower shift world heights
while held; smooth blends a 3x3 neighbourhood; flatten blends toward the chosen
height in metres. Samples stay within zero and the generated relief limit.

Drag or hold to paint, or focus the heightmap and use arrows and Space. The circle
shows the brush footprint. The heightmap updates during a stroke and the 3D mesh
refreshes on release. Undo stroke restores the previous stroke; Generate replaces
all unsaved sculpting. Comparison mode stays read-only; generate one terrain to
sculpt. Save terrain to scene preserves edited samples in a `.heights.toml`
sidecar and uses them for both the mesh and static collider. Recipe-only assets
continue to load. Image brush import remains outside this editor slice.

`npm run test:terrain-save` uses its own temporary project and server on port 3001,
checks brush modes/undo/frozen saves and renders the saved terrain natively.


### Reopen and update terrain

Select a terrain instance in the main scene editor, then choose **Edit terrain**
at the top of its inspector. Terrain Builder loads the existing recipe and exact
saved height samples automatically. Recipe-only terrain is regenerated using its
saved algorithm version. Sculpt directly; Generate is only needed to replace the
surface using changed generator inputs.

**Save terrain changes** updates the original terrain asset, retaining asset and
instance IDs, scene placement, materials and physics settings. It does not append
a new instance. The asset can be shared: every instance referencing it receives
the new surface. Updates validate all project scenes natively before publication
and reject changed scene/asset/heightfield sources. Use **View in scene** to return
with the updated terrain selected and a refreshed native preview.

### Units and scale

One world unit is one metre. Position values and geometry lengths use metres;
transform scale is a dimensionless multiplier of the authored asset size.
A default 1 m cube at scale 0.5 is 50 cm per side. A terrain width of 1000 is 1 km
at unit scale. A plane authored with half-width 6 is 12 m wide at scale 1, and
21.6 m at scale 1.8. Parent scale also affects child size and local position.
The inspector labels Position (m) and Scale (×); geometry half-extents remain
half the full size. See `docs/design/world-units-and-scale-audit.md` at the engine
root for the audited paths and current main-scene dimensions.


New Plane assets use a built-in **1 × 1 m** surface, with no half-width/depth fields
in their definition. Set width and depth using Scale X/Z in the inspector. The
existing Ground asset has been converted while retaining its 12 × 12 m world size
and texture tiling; its X/Z scale is now 12. Older explicitly sized plane assets
remain supported. See `docs/design/unit-plane-defaults.md` at the engine root.

TOML revision checks stream large terrain/code dependencies separately from the
256 KiB editable-document limit, so a small entity file can open even when the
project contains larger heightfield sidecars.

### Material Generator and terrain painting

Open **Material Generator** from the project toolbar, terrain page or an object's Material section. All surface controls now live here: diffuse image/import/repeat and X/Y tiling, shading, shadow flags, albedo/specular/emission, shininess, bump, noise maps and map strength/resolution. Save a named material to the project library. Objects choose that material from their Material dropdown. Opening an existing inline material copies its settings into a new library entry; existing assets are not automatically migrated. Shared edits apply to every reference when inspection reloads.

In **Terrain Builder**, generate or open a terrain, then enable material painting. Add up to four library slots (River bed, Ground, Mountain, Summit). **Apply height painting** replaces coverage using ordered transitions and a blend width in metres above the terrain base. These are height bands; the lowest band does not detect river topology. Sculpting preserves coverage until the height process is reapplied.

Choose **Editing → Materials** beside the heightmap, select a slot, and click/drag in the **2D heightmap**. Radius, strength and soft circular opacity control blending; arrows and Space also paint. Release to update the 3D preview. Undo stroke restores the last brush stroke; Undo material process restores the last palette/rule process. Save to scene or Save terrain changes persists exact RGBA weights and palette references in a bounded `.paint.toml` sidecar. Clear terrain painting restores the terrain's previous base material.

The browser editing preview shows diffuse colours/images in a 512-pixel baked atlas. Native scene inspection renders all material channels and shadows. Native terrain layers use a shared diffuse texture array capped at 1024 pixels and noise volumes up to 64³ per slot; ordinary object materials still support 128³. Four slots, opaque surfaces, no slope rules, and no automatic height repaint are the current limits.

For a second local IDE instance, set `EMERGENT_NEXT_DIST_DIR` to a separate build directory and run on port 3001. Keep the user's port-3000 service running.

### Object Builder

Open **Object Builder** from the main toolbar. Add Cube, Plane, Pyramid, Cylinder, Cone or Sphere from the left palette. Primitives start at a 1 m size. The **Object graph** is local to this asset: drag a row above/below another to reorder, or onto its centre to parent. The Parent selector and Move up/down buttons provide keyboard alternatives. Reparenting keeps local transforms; deleting a part removes its descendants.

Choose **Object**, **Face** or **Point** above the right-hand viewer. Click a part/face/point and drag it directly, or use the XYZ handles. Object mode also has Rotate and Scale handles. Alt-drag or dragging empty space orbits; scrolling zooms; **F / Frame object** fits the model. Numeric position, rotation and scale controls are on the left; face and point selectors also allow choosing hidden components.

In Face mode, **Move face** stretches shared neighbouring geometry. **Extrude drag** duplicates the cap and creates connecting side faces once per gesture. Drag outward or inward; **Extrude face** accepts a signed depth in metres. **Inset face** moves an inner cap toward the face centroid by the selected percentage and adds a surrounding ring. This is polygon editing, without boolean subtraction, automatic welding or remeshing. Invalid collapsed/intersecting edits are rejected. Undo/Redo keep up to 20 snapshots, with one entry per drag; Ctrl/Cmd-Z and Shift-Ctrl/Cmd-Z work outside text inputs.

**Save object** retains the editable graph in a `.structure.toml` sidecar plus a normal entity asset. **Save and add to scene** also creates a scene instance. In the main editor select that instance and choose **Edit object** to reopen it. Updating an existing structure preserves scene placement and updates all shared instances. Choose a reusable material from the Material Generator library for the whole assembly. The Object Builder preview shows neutral geometry; native scene inspection renders the library surface.

Runtime preparation bakes the local graph into one Mesh on the existing canonical entity. Parts remain authoring data, rather than a second runtime identity system. Current limits are 128 parts, 4096 vertices per part, 65536 total vertices, 131072 triangles, at most 64 points per polygon and a 2 MiB structure source. Concave simple polygons are triangulated through their dominant projection; nonplanar polygons become triangles. Per-part runtime animation/behaviours are not part of this initial mesh authoring tool.

## Particle Generator

Open `/particles` (or the ✺ rail button). Start with Fire, Smoke, Embers or Dust,
edit emitter layers in metres/seconds, and use Save & place to add an effect to
the selected scene. Select that instance in the main hierarchy to reopen it.
Authoring freezes a separate effect preview at two seconds; Run animates it.
The generator's browser preview is approximate; native capture includes sorted
alpha/additive blending and soft intersections.

The workshop shares the image library and saved fal key. Paid sprite generation,
image-to-video and transparent matting are explicit separate actions. Importing
video and baking an atlas locally requires `ffmpeg` and `ffprobe` on PATH.
Baking emits 32 frames at 8 fps into a 2048×1024 PNG (256 px padded cells); short
clips hold their last frame. Use soft alpha for smoke and additive-only baking
for effects on black. The editor checks actual decoded alpha, not file extensions.
Media jobs survive reloads; Check image/video resumes observation without
repeating paid submissions. Retained source clips are in the IDE data journal.

Validation: `pnpm test`, `pnpm build`, and `pnpm test:particles` (uses an isolated
temporary project and port 3001; it needs the `.next-particle-build` production
build, created with `EMERGENT_NEXT_DIST_DIR=.next-particle-build pnpm build`).
The browser smoke does not make paid fal calls.

### Particle Factory swatches

The pinned **＋ New system** tile at the left creates an unsaved blank effect with
one editable emitter and no emission. If the current recipe is dirty, choose
**Save & create**, **Discard & create**, or **Keep editing**. Failed saves retain
the current draft and keep the prompt open. Existing library effects remain intact.
Run `npx tsx scripts/particle-new-system-smoke.ts` against the port 3002 instance
to verify this flow with mocked saves and no project-file changes.

The `/particles` heading is now Particle Factory. Save an effect to add or update
its native still swatch in the horizontal effect library. Select a swatch to
load it; unsaved recipe changes require an explicit discard confirmation. Use
its Add to scene action and the library's scene selector to place the saved
recipe while retaining any current draft. Thumbnail images are derived caches
in `.data/particle-thumbnails`, regenerated when recipes or sprites change.
The Sprite & Video Workshop scrolls with the right-hand column, including long
provider histories and baked atlas controls.

### Scene lighting and skyboxes

Open **Lighting** from the shared navigation rail. Choose the scene, pick its global
ambient colour and intensity, and optionally select a Skybox swatch. Background
colour applies when no skybox is selected. Rotation and brightness affect the
panorama; **Save lighting** validates and persists the scene's `[environment]`
table, then reloads the native preview. Scene entity lights remain independent.
The same settings load in main inspection, scene captures and gameplay. Unsaved
changes are guarded when navigating away.

Create or import panoramas from **Image Generator → Skybox**. Skybox is a library
image type beside Image, Sprite and Video; right-click changes the type or deletes
a result. Skyboxes require a 2:1 PNG/JPEG panorama within the existing native image
budget (16 MiB, 4,194,304 pixels, maximum 4096 pixels per axis). FAL generation uses
a panoramic prompt and requests a supported 2:1 size before charging; the default
FLUX Schnell size is 1024×512. Other models must advertise a compatible size field.
Candidates still need **Add to library**. AI output can have a visible seam.

Skybox pixels are an LDR background, while global ambient and authored scene lights
illuminate surfaces. HDR environment lighting and reflection probes are not part of
this version. Run `pnpm exec tsx scripts/lighting-smoke.ts` against its temporary
project after building with `EMERGENT_NEXT_DIST_DIR=.next-lighting-build pnpm build`.
The smoke starts its own IPv6 port 3001 server and shuts it down; no paid calls.

## Terrain methods and biomes

Open Terrain Builder and use the top **Height Field / Ray March** switch. The **Biome**
button opens a modal for cave/cavern, canyon, underwater/seabed, field/plain, hilly,
mountainous, island and rivers. Cave/cavern requires Ray March. Underwater and river
biomes generate terrain; water rendering/simulation comes separately.

Set seed/width and choose **Generate terrain**. Ray March stays live: drag to orbit,
Shift-drag to pan and scroll to zoom, using one retained native worker. Camera
projection and Reset view controls match the scene viewer. Save to the project
terrain library and use **Add to scene**. Select the instance for transforms or
**Edit terrain** to reopen its shared recipe. Scene inspection and Run render the
same implicit field. New ray-marched terrain defaults to 1024m
width, adjustable up to 4096m, world-space biome scale, relief, overhang strength
and cave radius. Fields transition into rocky mountains with procedural slope/
height grass, rock and snow colouring. Larger extents expose more terrain without
enlarging its features. Older assets retain their test shapes or mixed-biome recipe
until explicitly changed. Generate applies biome/parameter changes before Save.
Heightmap sculpting/material painting remain in the Height Field option.
Ray-marched assets currently have no physics collision.

The Ray March viewport has **Preview quality**: Auto uses half resolution while
camera commands arrive and returns to full after 250ms; Full stays at 960×540;
Half stays at 480×270. The displayed PNG is always 960×540. These controls affect
only preview rendering, not authored terrain detail or scene Run resolution.
**Ray-step heatmap** shows blue for few steps, red for many and magenta for rays
that exhaust their iteration budget. Disable it to return to normal materials.
The caption's capture time includes GPU completion/readback; PNG time also includes
upscaling when needed. See [performance evidence](../../docs/benchmarks/terrain-performance/README.md).

### Prefabs

Drag a parent from the scene graph onto the **Prefabs** capture area. A prefab
is constructed immediately using the object's current name and complete subtree;
the original stays in the scene. The section lists names and part counts in a
scrollable table. Drag a row onto the scene root/empty graph space to instantiate,
or onto an existing object to parent the new instance beneath it. Use the scene
row's existing rename action to rename an instance. There is no save button or
prefab dropdown.

Keyboard: select a scene object, focus the capture area and press Enter. Enter on
a prefab row instantiates it at the scene root. The viewport placement picker
also accepts prefabs. Copies get fresh IDs and internal references. Definitions
are snapshots; defaults/media remain shared. Rust can resolve a unique exact
label with `PrefabDefinition::load_named(&project, "Drone")` and spawn repeatedly
from the result. Duplicate/missing labels are errors; explicit path loading is
still supported. See [the engine contract](../../docs/design/prefabs.md).

Browser regression: `EMERGENT_NEXT_DIST_DIR=.next-prefab-build npm run build`, then
`npm run test:prefabs`. It uses a disposable standard-template project on port
3001 and closes its server and native inspection afterwards.

### Nested UI layouts

The UI Designer offers Horizontal layout, Vertical layout and Grid layout in the
Layout palette. Select a layout before adding children (including other layouts),
then use Width/Height, Padding, Gap, Main alignment and Cross alignment in the
inspector. Grid columns controls equal-sized cells; Column span / Row span reserve multiple cells, and Width/Height accept auto for content sizing. Move into and outline arrows
support reparenting/reordering; flow layouts own child positions, while resizing
still edits width/height. Legacy stacked panels are unchanged. Saved UIs can be added to the current scene with Add screen overlay or Add world panel. Both render in the shared native host; pointer interaction and value/action binding remain planned. Browser regression: build with
`EMERGENT_NEXT_DIST_DIR=.next-ui-layout-build npm run build`, then run
`npx tsx scripts/ui-layout-smoke.ts` (disposable project, port 3001).

UI surface settings live in the scene's `emergent.ui` component. Screen dimensions
are pixels (zero fills the viewport); world dimensions are metres and follow the
entity transform. The prepared surface includes authored widget state and a
built-in uppercase bitmap font. Reload the scene after asset edits. See
[the native UI contract](../../docs/platform/ui-runtime-macos.md).
