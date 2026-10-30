# First material editor

Status: preview implemented, 2026-10-24. Engine-owned material data and rendering, project-owned authored
values and noise references. EntityId remains canonical SlotMap identity. A Material
component owns resolved CPU settings; renderer-scoped GeometryId owns uploaded mesh,
material uniform values and bound noise texture. No public backend identity or
serialized runtime keys. Reinsert geometry to change material/shading.

Data flow: entity material defaults -> complete instance material override -> Rust
validation/resolution of contained .noise.toml references -> upload -> fixed-size
packed RGBA noise volume -> fragment diffuse/specular/emission/bump -> scene output.
IDE saves through the existing guarded candidate/scene revision path and reconstructs
inspection; native Run reads the same authored configuration. No noise generation,
file I/O or material allocation occurs during steady-state rendering.

Shading: flat/smooth/unlit; optional albedo overrides geometry colour; specular RGB
and shininess; emission RGB and intensity; bump strength. One noise asset and mixing
strength per albedo, specular, emission and bump channel. Missing colour maps are
white multipliers; no bump map means no normal perturbation. Local object bounds
normalize coordinates. 2D filters project onto XZ, 3D filters use XYZ; each filter's
extent, offset, frequency, octaves and seed determine its domain. Size is fixed at
32 cubed per material in this first slice (RGBA8, linear filtering, clamped edges),
independent of the filter inspector's preview resolution. Four channels with up to
8 octaves stay below the existing 2,097,152-evaluation budget. Bump uses screen-space
height derivatives and world-space normal perturbation, requiring no mesh UVs.

Colour values are linear RGB; UI colour pickers convert sRGB. Basic Blinn-Phong
specular uses the actual camera position; it is not a metallic/roughness PBR model.
Emission does not illuminate other objects and has no bloom. No image textures,
UV authoring, displacement/silhouette change, shared material asset registry or
shader graph in this slice. Initial workload is tens of authored objects; no speed
or frame budget claim. Validate shader compilation, visible channel changes and
seeded maps, safe instance edits/reference conflicts, existing geometry/shadow
behaviour and paused inspection with a native Metal capture and Rust/IDE checks.


## Usage

Select a scene instance with geometry, scroll to Material, adjust values and choose
noise assets from the project. Apply material saves a complete scene override and
reconstructs the paused inspection session (or reloads a snapshot). Use material
defaults removes the scene override. Shared entity material defaults remain editable
in its TOML. Editing a referenced filter takes effect when inspection/Run reloads.

```toml
[material]
shading = "smooth"
albedo = [0.2, 0.5, 0.8]
specular = [0.6, 0.6, 0.6]
shininess = 48.0
emission = [0.0, 0.1, 0.2]
emission_intensity = 0.5
bump_strength = 0.1
[material.bump_map]
asset = "assets/filters/material-grain.noise.toml"
strength = 1.0
```

Use `[entities.material]` and its nested map tables for instance overrides. Map
channels multiply their base colour (white is full base value); map strength mixes
between a constant factor of one and the noise sample. Bump strength and its map
strength multiply the sampled height. Missing albedo uses existing geometry colours.
Unlit surfaces use albedo and emission, with lighting/specular/bump inactive.

Try `assets/scenes/materials.scene.toml` in first_project, or run:

```sh
cargo run --release -p emergent-first-project -- --run-scene assets/scenes/materials.scene.toml
```

The public `MaterialDefinition::constant()` constructor is file-system independent;
`resolve(&Project)` validates/loads noise assets. Renderers upload resolved materials
with `insert_with_material`. Changing a Material component alone does not re-upload
an existing GeometryId: explicitly replace that renderer resource. Bound GPU objects
remain under renderer-scoped SlotMap ownership. No image texture import API is implied.

Validation: [M-1 macOS evidence](../platform/material-editor-macos.md).


## Object shadows (SH-1)

Materials also expose cast_shadows and receive_shadows (default true), available
as inspector checkboxes. These remain independent: a non-caster can receive and
a non-receiver can cast onto another object. A shadow-enabled directional light
is required by the first backend; unlit materials ignore received lighting but
can cast. See [shadow controls](object-shadows.md).
