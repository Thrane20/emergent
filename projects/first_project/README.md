# Emergent First Project

This is the first independent game project. It depends on the engine through its public Rust API, validates `emergent.toml` at startup, reads project-owned data from `assets`, and assembles its runtime scene from reviewable generated Rust. The scene includes linked motion behaviours and a canonical point-light entity carrying a validated `LightSource` component.

From the engine repository root:

```sh
cargo run -p emergent-first-project
```

The default project opens a native window and renders a flat-lit ground plane, moving leader/follower/tail cubes, and a pyramid using the canonical generated scene transforms and point-light component. Close the window to stop the run. The point light contributes illumination; its shadow-casting flag is retained as scene data, but point-light shadow maps are not rendered yet.

Use the bounded headless path for project validation:

```sh
cargo run -p emergent-first-project -- --headless
```

Use `-- --smoke` for a 120-tick native presentation, resize, and clean-shutdown check. The pyramid is currently constructed by generated Rust; generic scene/entity asset interpretation, media decoding, project-mode cameras, and target packaging remain later increments.

The IDE's Scene viewer → Load uses the same Rust renderer to capture the generated
scene at tick zero, with gameplay stopped and no native window:

```sh
cargo run -p emergent-first-project -- --preview-scene assets/scenes/main.scene.toml --preview-output /tmp/emergent-scene.png
```

The output file must not already exist. Only the registered
`emergent.first-project.generated.main_scene` builder is supported initially.
This is a fixed-camera PNG capture; live streaming and arbitrary authored scene
loading are later work.
