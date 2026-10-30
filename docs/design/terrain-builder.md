# Terrain Builder preview

A project-bound /terrain page reuses Object Creator's split layout and ModelPreview
orbit controls. The server generates a bounded 2D field with the Rust terrain
tool, then builds a grayscale height-field mesh as an ephemeral GLB. The reusable
[pipeline contract](terrain-generation-pipeline.md) supersedes the initial
Rust-noise/TypeScript-shaping split and adds algorithm comparison.
Hilly, Mountainous, Rivers and Island are deterministic shaping presets; rivers
are carved valleys, not water or erosion simulation. No textures/material editor.

This first slice owns no runtime entities or durable IDs and changes no project
files. Browser blob URLs and viewer resources are released when replaced/unmounted.
SlotMap remains canonical when terrain is eventually instantiated in an engine
world. Saving and static terrain collision are now covered by
[saved-scene-terrain.md](saved-scene-terrain.md). Chunking and LOD remain future
work; do not mark the full runtime heightmap milestone complete.

Generation is on demand, outside the game loop. Bound resolution to 32/64/128/256
points per side (at most 65,536 vertices and 130,050 triangles), 1–8 noise octaves,
uint32 seed, finite width/height/frequency/roughness. Terrain settings add bounded warp/ridge inputs and at most 64 thermal erosion
passes. A terrain-tool lock serializes preview requests. Allocate one height array and one packed vertex/index buffer per
generation; do not resample noise per viewer frame. Requests are same-origin and
bound to the linked project. Invalid requests retain the last good preview.

Verify settings/bounds, deterministic presets, island edges, river carving,
positive-Y triangle winding, smooth unit normals, valid GLB and grayscale values.
Smoke the real Rust endpoint and browser orbit/zoom/reset, regeneration and layout.

## Relief lighting and mountain count

Terrain GLBs carry a fixed white directional sun using the standard
[KHR_lights_punctual extension](https://github.com/KhronosGroup/glTF/tree/main/extensions/2.0/Khronos/KHR_lights_punctual).
It points 35 degrees below the horizon with a 45-degree yaw. Terrain-only exposure
keeps the added direct light from washing out detail; object previews keep their
existing exposure. This adds relief lighting, not terrain self-shadow maps.

Mountainous now uses 1–12 seeded major mountain centres (default 3), separated
using shuffled/jittered cells. Rotated elliptical footprints and low-amplitude
noise shape the peaks above quiet lowlands. Count controls the major mountains;
noise frequency/octaves/roughness control surface detail, not mountain density.
Roughness uses a 0–1 slider with a live value. Generation stays explicit.

Validation checks disconnected main mountain regions at zero roughness for counts
1, 3, 8 and 12, seed repeatability, GLB light orientation and browser light presence.
The native browser smoke temporarily disables the loaded directional light and
compares the actual rendered output to verify that it contributes illumination.

## Visible preview-light controls

The bright default studio environment obscured the directional sun. Terrain now
uses a small generated uniform Radiance HDR environment (128x64, linear gray
approximately 0.12) for dim ambient fill, with terrain-only exposure 1. Object
Creator keeps its existing environment. The minimum environment size supports the
viewer's PMREM conversion; a tiny 4x2 image produced black fill and was rejected
by a rendered brightness regression.

The visible Directional light section exposes Sun strength (0–1, mapped to 0–5
light intensity), direction (0–360 degrees) and elevation (5–85 degrees). Defaults
are 0.65 / 315 / 35. Generate applies lighting together with terrain inputs. The
header shows the strength of the generated preview, so edited inputs are distinct
from the currently displayed result. The Generate action stays visible while the
desktop controls scroll. These are preview lights; no game-project light is added.


## Versioned Rust preparation and comparisons

`src/terrain.rs` owns base generation, preset shaping, ridge layout, coordinate
warping and bounded thermal erosion. The former TypeScript shaping implementation
is removed. `examples/terrain_builder.rs` exposes ETF1 dense float fields with Rust
generation timings. `terrain-tool.ts` passes a validated temporary TOML recipe,
then `/api/terrain` prepares GLBs using the existing smooth mesh exporter.

Comparison requests prepare three fields sequentially and return bounded GLBs
and metrics. The browser retains/revokes the three blob URLs together, preserves
the last successful result on failure and sets identical absolute camera orbit
and target values. Initial framing uses the shared width/relief, independent of
per-model bounds. Cameras orbit independently afterward. Timing excludes tool
startup/compilation, transport and GPU rendering. Output field/GLB byte sizes are
not peak memory metrics. No production performance target is claimed.
