# Emergent First Project

This independent game depends on the public engine API, validates `emergent.toml`, and loads the entry scene's referenced entity assets. The ground, pyramid, cube and light come from TOML; project Rust adds the linked-motion chain and cube rotation.

From the engine repository root:

```sh
cargo run -p emergent-first-project
```

The default project opens a native window and renders a flat-lit ground plane, moving leader/follower/tail cubes, and a pyramid using the canonical generated scene transforms and point-light component. Close the window to stop the run. The point light contributes illumination; its shadow-casting flag is retained as scene data, but point-light shadow maps are not rendered yet.

Use the bounded headless path for project validation:

```sh
cargo run -p emergent-first-project -- --headless
```

Use `-- --smoke` for a 120-tick native presentation, resize, and clean-shutdown check. Media decoding, project-mode cameras, nested compositions and target packaging remain later increments.

The IDE's Scene viewer → Load uses the same Rust renderer to capture the generated
scene at tick zero, with gameplay stopped and no native window:

```sh
cargo run -p emergent-first-project -- --preview-scene assets/scenes/main.scene.toml --preview-output /tmp/emergent-scene.png
```

The output file must not already exist. A scene without `builder` loads its authored
instances alone. The optional registered
`emergent.first-project.generated.main_scene` builder adds the project motion chain;
unknown builder names fail explicitly. This is a fixed-camera PNG capture.

## Persistent inspection

In the IDE, select a scene and press **Inspect**. A native release-profile Rust
process constructs the scene once and streams bounded 960×540 PNG frames while
gameplay stays at tick zero. Drag to orbit, Shift-drag to pan, and scroll to zoom;
Reset view restores the initial camera. Select a scene instance and **Apply to
scene** to save and update its transform in the retained world. **Stop inspection**
ends that process independently of normal Run. Load remains the snapshot fallback.

Restart inspection after changing code, geometry, scene membership or external
assets. This first transport is paced to at most 10 Hz; it is not a production
video stream. Closing the view stops browser polling, and the server expires the
session after about 30–35 seconds without traffic. The `--inspect-scene <scene>`
CLI is the IDE's framed binary stdout/TOML-stdin contract, not a terminal viewer.
Project construction must reserve stdout for that protocol and use stderr for logs.

## Edit the authored scene

In the IDE, select a scene, Add asset → Cube (or another primitive/light), name it,
and leave **Include in selected scene** checked. Load refreshes the paused frame;
Plain Cargo launches the manifest's entry scene; IDE Run launches the selected scene. Existing assets can be included manually:

```toml
[[entities]]
id = "another-cube"
asset = "assets/entities/cube.entity.toml"
# parent = "ground"  # optional scene instance ID
```

Instance IDs are unique within the scene; multiple instances can share an asset.
Transform and geometry/light values live in the referenced asset, not generated
Rust. Edit those values in your code editor and Load again (or restart Run).
Rotation uses degrees with XYZ Euler composition. Parent transforms are inherited.
Optional scene-instance transform overrides are supported; nested asset composition remains deferred.

The loader validates the entire candidate before construction, rejects unknown
fields/types, unsafe paths, duplicate IDs, missing parents, cycles and invalid
values, and allocates fresh runtime handles. Limits: 256 KiB per file, 1,024
instances, eight lights, primitive dimensions up to 1,000,000, and 32 subdivisions per primitive axis. Failed IDE loads
keep the previous frame. If scene inclusion fails after creating an asset, the
asset stays saved and the IDE reports that it was not included.

## Place an instance in the IDE

Select a scene, then select an instance in the primary **Scene graph**.
The header Local/Global switch selects coordinates. The right Transform inspector edits Position, Rotation in degrees, and Scale.
Uniform scale links the three axes. **Apply & reload** validates and saves the
scene, then refreshes the Rust preview. **Reset to asset defaults** removes the
instance override and refreshes it. Asset geometry, colour and defaults stay in
the entity file; chat and Statements retain that asset-file scope.

```toml
[[entities]]
id = "sphere-near-base"
asset = "assets/entities/sphere.entity.toml"

[entities.transform]
translation = [2.0, 0.0, 0.0]
rotation_degrees = [0.0, 45.0, 0.0]
scale = [1.0, 1.0, 1.0]
```

Each supplied vector replaces the corresponding asset vector. Omitted vectors
inherit their asset defaults; transforms stay relative to the instance's parent.
**Reload inspector** discards the draft and rereads external edits. Stale scene or
asset revisions prevent saving until reloaded. The linked game exposes
`--validate-scene <path> [--candidate <source-file>]` for bounded, headless schema
and graph validation without changing authored files or running simulation.


Drag instances onto parents or Scene root; the Parent selector offers the same
operation by keyboard. Parenting keeps local TRS, so world placement can change.
Inherited translation, rotation and scale use the engine hierarchy. Global edits
persist equivalent local values; shear allows global position only and rejects
unsupported rotation/scale conversions. Active inspection applies saved transforms
and parent changes without recreating its world.

The TOML tab edits scene overrides or the shared entity asset. Save validates all
project scenes in an isolated copy and rejects invalid/stale source. Whole-file
source changes require Load or Restart inspection. The headless tool contract also
exposes `--validate-scenes <paths...>`, `--scene-transforms <scene>` and
`--convert-global <scene> <instance-id> <request.toml> [--position-only]`.
Transform tools print TOML reports and never advance gameplay.


Persistent inspection now targets 30 fps. Click an object or origin marker to
select it; Translate/Rotate/Scale gizmos support Local/Global axes. Pointer moves
preview in the native world; release saves once through revision checks and Rust
validation. Escape/pointer cancellation or a rejected save restores the start.
The optional inspection-only PickingMesh cache uses actual render tessellation;
normal native Run does not allocate that cache.


### First physics demo

```sh
cargo run --release -p emergent-first-project -- --run-scene assets/scenes/physics.scene.toml
```

A dynamic cube falls onto a static box ground. Restart to restore authored positions.
Select the Physics scene in the IDE and Inspect for paused collider outlines. Select
an instance and use Physics beneath Transform to edit its scene override. Inspection
does not simulate; IDE Run launches the scene selected in the dropdown. Dynamic bodies must
be at Scene root. See ../../docs/design/physics-foundation.md for contracts and limits.


### Materials demo

Select **Materials — noise and bump** in the IDE, click Inspect, then select
`material-sphere`. Scroll to Material to edit shading, albedo/specular/emission,
shininess and bump. Each channel can use an existing noise filter with a mixing
strength. Apply material saves an instance override and restarts paused inspection.
Use material defaults restores the shared entity defaults.

```sh
cargo run --release -p emergent-first-project -- --run-scene assets/scenes/materials.scene.toml
```

The public engine loader resolves material defaults and filters; project rendering
uploads them once through GeometryRenderer::insert_with_material. See
../../docs/design/material-editor.md for mapping, colour spaces and current limits.


### Object shadows

Select **Shadows — cast and receive** and Inspect. Select `caster` or `receiver`,
scroll to Material and toggle **Cast shadows** / **Receive shadows**, then Apply
material. The scene uses a tilted directional sun so changes are visible. Shadow-enabled
point lights also generate six-face shadow maps (up to two at once).
In other scenes use Add -> Light -> Directional, then rotate it toward the objects.
Both object options default on and can be overridden independently per instance.

```sh
cargo run --release -p emergent-first-project -- --run-scene assets/scenes/shadows.scene.toml
```


### Point-light shadows

The existing key-light already uses `casts_shadows = true`: reload the scene or
Restart inspection to use the new renderer. To opt in another point light, set
`casts_shadows = true` under its `[light]` TOML section; renderable materials still
need the appropriate Cast shadows / Receive shadows options.

Select **Point shadows — lamp and cube** to inspect the separate example. Moving
its lamp updates shadow position without needing to rotate the light.

```sh
cargo run --release -p emergent-first-project -- --run-scene assets/scenes/point-shadows.scene.toml
```

Two point lights can cast simultaneously; a third enabled request returns an
explicit error. The directional slot remains independent. See
../../docs/design/point-light-shadows.md for range, resolution and cost bounds.
