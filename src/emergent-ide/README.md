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

The first slice is read-only. It provides manifest-derived project discovery, asset-reference filtering, output visibility, and copyable project-relative paths. It does not import media, mutate scenes, execute generated scene builders, run games, package outputs, or retain runtime SlotMap handles.
