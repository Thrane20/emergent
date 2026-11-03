# Game project — Agent Guidelines

This directory is a game project built on the public `emergent` crate.
Use engine public exports, capability documentation and this template as the
project contract. Other game projects are not sources of engine/IDE infrastructure
or new-project defaults; do not copy their code, assets or identifiers.

## Where code belongs

- Gameplay and project component types belong in `src/`. `src/main.rs` implements
  `emergent::project_host::ProjectGame` and hands control to `project_host::main`.
- The window, IDE inspection, previews, scene validation and every `--` tool command
  come from the engine's project host. Do not copy or reimplement them here; if the
  IDE needs a new command, it belongs in the engine so every project receives it.
- Generator-owned Rust belongs under `src/generated/`; `plugins.rs` there is the plugin
  list the IDE regenerates from `[[plugins]]` in `emergent.toml`. Do not edit it by hand.
- Reusable behaviour and assets meant for other projects belong in a plugin:
  `plugins/<publisher.name>/` with a `plugin.toml`, its own `assets/` and a crate whose
  `PLUGIN` implements `emergent::Plugin`. Create plugins from the IDE's Plugins page so
  the manifest, Cargo dependency and generated list stay consistent. Plugin IDs
  (components, brain nodes, locomotion types) start with the plugin id.
- Authored content belongs under `assets/`, referenced by project-relative paths.
  `emergent.toml` is the project contract; keep paths relative and the project `id` stable.

## Engine usage

- Use `EntityId` for live runtime references. Never persist runtime handles in assets;
  scenes use durable instance IDs.
- Attach behaviour with components (`[[entities.components]]`). Project types use the
  `project.<name>` prefix and are registered by overriding `ProjectGame::components`.
- Keep fixed-update work bounded and allocation-free in steady state where practical.
