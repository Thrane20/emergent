# First project visual run

This describes one ordinary game demo. Engine/IDE contracts live in
`src/project_host` and new-project defaults in `templates/standard`; this game is
not an implementation reference or engine validation fixture.

## Scope

Make the default `emergent-first-project` executable open a native window and
render a small generated scene using Emergent's existing preview graphics API.
The scene contains a ground plane, moving leader/follower/tail cubes, a pyramid,
and the generated point light. A `--headless` mode preserves bounded fixed-step
validation, and `--smoke` provides a bounded native graphics check.

This slice does not introduce authored entity loading, camera entities, general
render extraction, materials, textures, editor project mode, or hot reload.
`assets/entities/myfirstpyramid.entity.toml` remains authored graph data rather
than executable input; generated Rust remains the runtime source of truth.

## Ownership and identity

- `FirstProjectGame` and its generated builder own the game-specific entity and
  behaviour composition through canonical engine `EntityId` values.
- `GeometryRenderer` owns uploaded GPU geometry through renderer-scoped
  generational `GeometryId` handles. Those handles remain inside the visual host
  and are never stored in the world or authored assets.
- The project visual host owns the `winit` window lifecycle and coordinates the
  engine clock with rendering. The engine's public graphics API remains
  unchanged.

## Data flow

1. Load and validate the project plus its speed parameter before opening a
   window.
2. Create the renderer and upload one ground plane, one shared cube, and one
   pyramid during window resume.
3. Start the existing fixed-step game and construct the generated scene.
4. For each redraw, advance fixed simulation time, read committed world
   transforms, and fill fixed-size arrays of geometry and light instances.
5. Render with the existing fixed camera and lit geometry pass.
6. Stop the engine coherently when the window closes or a smoke tick limit is
   reached. IDE Stop terminates its owned local process group; graceful runtime
   stop IPC remains a later control-plane improvement.

## Hot path and scale

The first scene renders five instances and two lights. Geometry is uploaded
once. Per-frame extraction uses stack arrays with no heap allocation and copies
only matrices plus light values. This is a correctness-oriented reference
workload, not a measured performance claim or general extraction architecture.

## Validation

- Keep generated scene and project-contract tests headless.
- Run the project with `--headless` and confirm its bounded tick output.
- Run formatting, workspace tests, all-feature Clippy, and rustdoc because the
  project now consumes the public graphics boundary.
- Run the default project with `--smoke` on macOS and require rendered frames,
  resize observation, and clean shutdown. This is macOS/Metal evidence only.
