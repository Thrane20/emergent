# Emergent First Project

This is the first independent game project. It depends on the engine through its public Rust API, validates `emergent.toml` at startup, reads project-owned data from `assets`, and assembles its runtime scene from reviewable generated Rust. The scene includes linked motion behaviours and a canonical point-light entity carrying a validated `LightSource` component.

From the engine repository root:

```sh
cargo run -p emergent-first-project
```

The project is headless and exits after 120 fixed ticks. The light is project-visible component data; illumination and shadow rendering, scene/entity asset interpretation, media decoding, and target packaging are later increments.
