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

The first slice is read-only. It discovers the manifest, authored assets, game-owned Rust, and generated Rust directly from the linked project. Selecting a file shows its metadata, text preview where available, and inferred incoming/outgoing file connections such as entry-scene declarations, asset reads, generated builders, and Rust module use.

The full-width lower chat dock keeps separate in-memory threads scoped to the whole project, the selected file, or a durable authored entity ID discovered from scene and entity definitions. It shows the definitions attached to the current scope.

The `/settings` page configures the initial OpenAI/Codex provider, including an editable API base URL, model, and API key. Credentials are stored in the server-only, Git-ignored `.data/ai-provider.json` file with owner-only permissions and are never returned to the browser. Set `EMERGENT_IDE_DATA_DIR` to keep this state elsewhere.

Chat requests now call the configured provider through the server-side Responses API. Each request receives bounded context resolved from the live project graph for the selected project, file, or durable authored entity plus the recent scoped conversation. This first connection is read-only: it returns guidance and proposed code but cannot yet apply changes to project files.

It does not import media, mutate scenes, execute generated scene builders, run games, package outputs, or retain runtime SlotMap handles.
