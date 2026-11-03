# Plugins

Status: **PLUG-1 to PLUG-3 delivered** (2026-11-03); PLUG-4 (package/install) and PLUG-5 (library) open. Decided with the user: option A
(static crates) and v1 covers components, brain nodes and locomotion types.

## Goal

A **plugin** is a reusable bundle of behaviour and content that is neither engine core
nor one game. It can hold any number of assets (models, materials, brains, locomotion
profiles, audio, scenes/prefabs) and Rust code that registers component types and their
runtime with the engine. A project can author a plugin in place, package it into one
file with a manifest, and install a packaged plugin into another project. Later, a
library or marketplace publishes and subscribes to packages.

## Engine vs plugin vs project

| Layer | Holds | Identity prefix |
|---|---|---|
| Engine (`src/`) | Foundation: world, SlotMap hierarchies, rendering, physics, built-in components (`emergent.*`), the project host | `emergent.` |
| Plugin | Reusable components, behaviours and assets with a manifest and version | `<plugin id>.` e.g. `acme.jumping_bean` |
| Project (`projects/x/src`) | One game; may contain its own unpackaged code | `project.` |

The project's own `project.*` components stay as they are. A plugin is the same thing
with a name, a version and a boundary.

## What exists

- `ComponentRegistry::register` accepts only `project.*` IDs (`src/component.rs`).
- `ProjectGame::components()` returns one registry; the project's `attach` /
  `fixed_update` match on component IDs by hand (see the standard template comment).
- `emergent.toml` has `asset_roots` and `generated_code`; the IDE writes
  `src/generated/`.
- GOALS.MD §modules: "Begin with ordinary Rust modules and static registration …
  Runtime plugin loading and a general plugin ABI are deferred." Dynamic plugins are
  listed as deferred in the capability catalogue.

## Decision: how plugin code reaches the engine (A chosen 2026-11-02)

| | A. Static crate (recommended now) | B. Native dylib | C. WASM module |
|---|---|---|---|
| How | Plugin is a Cargo crate; installing it adds a path dependency and a generated registration line; the game rebuilds | `cdylib` loaded with `libloading` at startup, C ABI entry point | Sandboxed module run by an embedded runtime (wasmtime) via a host API |
| Registers at runtime | Yes, at startup through `Plugin::register`, but needs a rebuild to add/remove | Yes, no rebuild | Yes, no rebuild |
| Speed | Native, inlined, LTO | Native, but every call crosses `extern "C"` | Per-call host boundary; world access through copies |
| Safety | Safe Rust | `unsafe`; Rust has no stable ABI, so plugin must match the exact compiler and engine build | Sandboxed; best for untrusted marketplace code |
| Package contents | Source (portable, auditable) | One binary per OS/arch/engine version | One `.wasm` per engine API version |
| Fits GOALS.MD today | Yes | Needs an ADR superseding the deferral | Needs an ADR; adds a large dependency |

**Recommendation: A.** It satisfies "code registers itself with the engine at run
time" (registration happens at startup through a trait, not hand-written matches),
keeps hot paths static, needs no `unsafe`, and packages are source so they work on
every output target. The registration trait is the stable seam: B or C can later
provide the same trait from a loaded module without changing scenes, manifests or
packages' asset layout. A marketplace of untrusted code points to C, not B; decide
that when publish/subscribe starts.

## Model (option A)

### Plugin folder

Plugins live inside a project at `plugins/<id>/`:

```
plugins/acme.jumping_bean/
  plugin.toml          # manifest
  Cargo.toml           # crate `acme-jumping-bean`, depends on emergent
  src/lib.rs           # implements emergent::Plugin
  assets/              # any assets; added to the project's asset roots
```

### Manifest (`plugin.toml`)

```toml
schema_version = 1
id = "acme.jumping_bean"       # durable identity and component-ID prefix
name = "Jumping bean"
version = "0.1.0"              # semver
engine = ">=0.1, <0.2"         # engine API requirement, checked on install and load
description = "Pops dynamic bodies upward on a timer."
authors = ["…"]
license = "MIT"
crate = "acme-jumping-bean"    # empty for asset-only plugins
asset_roots = ["assets"]
dependencies = []              # other plugin ids + version requirements
```

The project lists installed plugins in `emergent.toml`:

```toml
[[plugins]]
id = "acme.jumping_bean"
version = "0.1.0"
path = "plugins/acme.jumping_bean"
```

### Code seam

```rust
pub trait Plugin {
    /// Adds this plugin's component types; IDs must start with `<plugin id>.`.
    fn register(&self, registry: &mut ComponentRegistry) -> Result<(), String>;
    /// Builds runtime state for this plugin's components in a loaded scene.
    fn attach(&self, loaded: &LoadedScene, world: &mut World) -> Result<Box<dyn PluginRuntime>, String>;
}
pub trait PluginRuntime {
    fn fixed_update(&mut self, context: &mut EngineContext<'_>);
}
```

`Plugin` also has default-empty hooks for brain nodes and locomotion types:

```rust
    fn brain_nodes(&self) -> Vec<(NodeSpec, &'static dyn BrainNode)> { Vec::new() }
    fn locomotion_types(&self) -> Vec<(LocomotionType, &'static dyn LocomotionModel)> { Vec::new() }
```

- Brain: built-in nodes stay in the closed `enum Op` (static dispatch). One new
  variant `Op::Plugin { node, params }` calls `BrainNode::evaluate(params, &Senses) -> Intent`
  (exact inputs follow the existing leaf signature). Node IDs are namespaced
  (`acme.circle_strafe`); the node catalogue export merges plugin specs so the
  brain editor, MCP `list_brain_nodes` and validation see them unchanged.
- Locomotion: same pattern, `Model::Plugin { model, params }` calling
  `LocomotionModel::step`. Catalogue export merges plugin types.
- The catalogues stop being free functions over a static list and are built once
  per project from built-ins + plugins (the same object `components()` returns
  today, grown into a `ProjectRegistry { components, brain_nodes, locomotion }`).
- Hot path: plugin nodes add one indirect call per plugin node per agent per tick.
  Measure against the equivalent built-in node before calling it acceptable.

- `ComponentRegistry::register` takes a namespace: `project` for project code, the
  plugin id for plugin code. Duplicate IDs across plugins are a load error.
- The IDE writes `src/generated/plugins.rs` (one `&acme_jumping_bean::PLUGIN` per
  installed plugin). The project host calls each plugin's `register`, then
  `attach` after scene load, then one `fixed_update` per plugin per tick. Dynamic
  dispatch is per plugin per tick, not per entity; each plugin keeps dense
  `Vec`s of its own runtime state, keyed by `EntityId`.
- Component runtime keeps engine SlotMap keys only at runtime. Scenes store the
  durable type ID and instance IDs, as today.
- A missing plugin for a component in a scene is a validation error naming the
  plugin id, not a silent skip.

### Assets

Plugin `asset_roots` join the project's roots under `plugins/<id>/`. Asset paths
in scenes stay project-relative (`plugins/acme.jumping_bean/assets/…`), so the
existing loaders, IDE asset browser and MCP refs need no new identity scheme.

### Package (`.emergent-plugin`)

A zip of the plugin folder with `plugin.toml` at the root, excluding `target/`.
Packaging validates the manifest, checks every component ID is in the plugin's
namespace, checks the crate builds against the current engine, and records a
content hash (SHA-256) in a `package.toml` sidecar inside the zip.

Install: verify hash and manifest, refuse id collisions and unsatisfied `engine` or
dependency requirements, unpack to `plugins/<id>/`, add the `[[plugins]]` entry and
the Cargo path dependency, regenerate `plugins.rs`, rebuild. Uninstall reverses this
and is refused while scenes still use its component types (listing them).

Zip extraction is a trust boundary: reject absolute paths, `..`, symlinks and
oversize entries.

### Publish / subscribe (later)

Out of scope for the first slices. The package format, `id` + `version` and the
content hash are the identity a registry needs. Subscribing to source packages
means building third-party Rust on the user's machine; that is the point at which
option C (sandboxed) or signing must be decided.

## Slices

1. **PLUG-1 Code seam (delivered 2026-11-03).** `emergent::plugin`: `Plugin` (`id`,
   `components`, `attach`) and `PluginRuntime::fixed_update -> Result`. Plugin ids are
   `publisher.name`; component IDs must be `<plugin id>.<name>`. `ProjectGame::plugins()`
   lists them; the default `components()` includes them; the host wraps every game in
   `Plugged<G>`, which attaches runtimes after `load` and runs them after the game's
   `fixed_update`. A runtime error stops the game. Fixture plugin lives in
   `tests/plugins.rs` (no separate crate needed until PLUG-3).
2. **PLUG-2 Brain and locomotion extension (delivered 2026-11-03).** `Plugin::brain_nodes`
   returns `&'static dyn BrainNode` (spec + `steer(params, BrainInput) -> Option<Vec3>`);
   `Plugin::locomotion_types` returns `&'static dyn LocomotionModel` (spec + kinematic
   `advance`). The project's `ComponentRegistry` carries them, so scene loading and the
   host's `--export-brain-nodes`, `--export-locomotion`, `--validate-*` and `--simulate-*`
   see them with no new plumbing. Runtime is one enum variant each (`Op::Plugin`,
   `Model::Plugin`) holding the hook and a packed `[f32; 16]` of number/bool fields, so
   ticks do no field lookups. v1 limits, checked at registration: brain nodes are
   Steering leaves (no composites/decorators, no per-agent node state); fields are
   numbers/bools plus `target`/`group`; locomotion is kinematic only; categories must be
   existing editor categories. Benchmark (release, Apple M3 Pro, brain preview, 7200
   ticks, 3 runs): plugin `chase` vs built-in `seek` — 64 agents 0.015 vs 0.014 ms/tick,
   256 agents 0.078–0.079 ms/tick for both; no measurable difference.
3. **PLUG-3 In-project plugins (delivered 2026-11-03).**
   - Engine: `emergent.toml` `[[plugins]] { id, path }`; `Project::load` reads each
     `plugin.toml` (`PluginManifest`: schema 1, id, name, semver version, description,
     optional `crate`, `asset_roots`), checks id match, duplicates and paths, and
     appends plugin asset roots to `Project::asset_roots()`. The `engine` requirement and
     plugin `dependencies` fields are deferred to PLUG-4, where install checks need them.
   - Host: at startup the compiled `ProjectGame::plugins()` must equal the listed plugins
     that declare a crate; a stale list names the plugin and says to regenerate and rebuild.
   - Templates: `templates/plugin/` (workspace member, example `drift` component) is the
     new-plugin default; the standard template returns `src/generated/plugins.rs`.
   - IDE: `/plugins` page and `/api/plugins` create a plugin (copy template, rewrite ids,
     add Cargo path dependency, `[[plugins]]` entry and generated list, rolled back as one
     unit on failure). Snapshots list plugin assets and plugin `src/`. Project duplicate
     and isolated AI checks keep in-project crate paths relative so copies use their own
     plugin code.
   - Not yet: removing a plugin, editing `plugin.toml` from the IDE, MCP tools for plugins.
4. **PLUG-4 Package and install.** `--package-plugin`, install/uninstall with
   validation, IDE actions.
5. **PLUG-5 Library.** Local library folder of packages shared across projects.
5. Later: remote publish/subscribe; optional dynamic loading by ADR.

## Validation

- Unit: namespace rules, duplicate IDs, manifest parsing, engine-version check,
  zip path traversal rejection, uninstall refusal while in use.
- Integration: template-derived disposable project installs the fixture plugin
  package, validates scenes, runs `--smoke`, uninstalls.
- Performance: brain/locomotion plugin node vs equivalent built-in at 1k and 10k
  agents; a scene with N plugin components vs the same behaviour as a
  `project.*` component; per-tick cost must match within noise (release build).

## Open questions

- Plugin-to-plugin code dependencies (one plugin's crate using another's types)?
  Cargo handles it; the manifest `dependencies` mirrors it.
