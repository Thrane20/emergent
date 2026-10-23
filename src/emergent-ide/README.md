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

It does not import media, mutate scenes, execute generated scene builders, run games, package outputs, or retain runtime SlotMap handles.
