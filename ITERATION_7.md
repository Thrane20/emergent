# Iteration 7 — First independent game project

Status: complete.

## Outcome

Establish the first honest boundary between the Emergent engine crate and a game project. The game is a separate Cargo package that depends only on Emergent's public API. It owns its manifest, authored and generated Rust, asset folders, output declarations, and runtime assembly.

This iteration validates and runs the project. It does not yet interpret scene or entity asset contents, decode media, package outputs, or provide a general type registry.

## Project layout

```text
projects/first_project/
  Cargo.toml
  emergent.toml
  AGENTS.md
  src/
    main.rs
    game.rs
    generated/
  assets/
    scenes/
    entities/
    data/
    textures/
    models/
    audio/
  tests/
```

`src/generated` is ordinary reviewable Rust and may be produced by an AI coding generator. `AGENTS.md` tells generators to use public Emergent handles and APIs and to avoid engine source edits. Authored asset paths are project-relative. Derived caches and build artifacts do not live under `assets`.

## Manifest contract

The root `emergent.toml` declares:

- schema version, stable project ID, and display name;
- an entry scene path and generated-code directory;
- one or more asset roots;
- named output nodes with target OS, architecture, Rust target triple, and development or release profile.

The engine loads and validates this startup contract. Paths must be relative, portable, free of `..`, and resolve within the project root. Declared roots, the entry scene, and generated-code directory must exist. Output IDs must be unique and at least one output must be enabled.

The manifest uses authored strings and paths rather than runtime SlotMap or ECS keys. Loading it creates no world entities and cannot partially activate a scene.

## Runtime boundary

- The project binary locates its root with `CARGO_MANIFEST_DIR` and calls `Project::load` before creating an engine.
- Project-owned code constructs its first runtime scene through public `World`, behaviour, component, and schedule APIs.
- Generated code supplies a scene builder in this proof. It returns canonical runtime handles to the project-owned game module.
- The project may inspect resolved asset paths, but media loading and authored scene expansion are deferred.

## Performance shape

Manifest parsing and filesystem validation occur once during startup and never enter fixed-tick work. Asset data is not scanned recursively. The runtime engine and SlotMap ownership model are unchanged.

## Validation

- Unit-test valid loading, unsupported schema, traversal/absolute paths, missing paths, duplicate outputs, and no enabled output.
- Build and test the independent project as a workspace member.
- Run the project headlessly and confirm its generated scene advances through public behaviour APIs.
- Keep engine formatting, tests, Clippy, and rustdoc passing.

## Deferred

- Scene/entity schema parsing, durable authored node IDs, composition expansion, registration, and graph export.
- Asset import, decoding, dependency discovery, hot reload, and derived caches.
- Output build planning, packaging, cross-compilation, and native verification.
- Project templates and creation commands beyond this checked-in reference project.

## Acceptance gate

Iteration 7 is complete when the separate project validates its manifest, uses generated and authored project files without engine source edits, builds against public Emergent APIs, runs a small headless scene, and fails malformed project contracts with explicit errors.
