# World units and scaling audit

Date: 2026-10-28. Scope: authored geometry, scene-instance overrides, parent
transforms, native rendering, terrain preparation, collision and editor labels.
Existing assets and scene sizes are preserved.

## Contract

**One world length unit is one metre.** This specifies the convention for the
existing maths; there is no centimetre/metre conversion stage to switch on.

| Quantity | Unit and meaning |
| --- | --- |
| Geometry positions and dimensions | Metres in the object's local coordinate frame |
| Position / translation | Metres in the selected coordinate frame; parent scale affects local offsets |
| Terrain `size` | Full width and depth in metres before transforms |
| Terrain samples / relief | Heights in metres before transforms |
| `half_extent`, `half_width`, `half_depth`, `half_height` | Half the corresponding full dimension, measured in metres |
| Radius | Metres; diameter is twice the radius |
| Transform `scale` | Dimensionless multiplier; 1 preserves authored dimensions |
| Scene rotations | Degrees in the editor/TOML; Rust uses quaternions/radians internally |
| Camera near/far planes and orthographic view height | Metres; perspective FOV is angular, not a length |
| Terrain brush radius / flatten target | Metres; brush strength remains a unitless 0–1 weight |
| Collider half-extents and offsets | Metres before world scale is applied |
| Gravity | Metres per second squared; current physics default Y is -9.81 |

Thus a terrain with `size = 1000`, unit scale and an unscaled parent is 1 km
wide. The default cube is 1 m per edge, so uniform scale 0.5 produces a 50 cm
cube. `half_extent = 0.5` instead describes a 1 m cube before scaling.

World vertices follow the existing hierarchy:

`world position = parent world matrix × translation(m) × rotation × scale(×) × local geometry(m)`

A scene override replaces the asset's corresponding transform vector; asset
scale and instance scale are not two independently multiplied layers. Parent
transforms do compose. A parent scaled by 2 makes its child's lengths and local
position offsets twice as large. Rotation can change a world-axis bounding box
without changing the object's own edge lengths. Nonuniform scaled/rotated
hierarchies can introduce shear; existing global editing guards remain required.

## Saved main scene at the initial audit

Dimensions below use X × Y × Z. Scaled sizes are object-axis lengths before
rotation, not perspective pixel measurements or rotated world-axis bounds.

| Instance | Authored size (m) | Saved local scale (×) | Scaled size (m) |
| --- | --- | --- | --- |
| `ground` | 12 × 0 × 12 | 1.5913223 × 1 × 1.6593269 | 19.0958676 × 0 × 19.9119228 |
| `cube` / Cube3 | 1 × 1 × 1 | 1 × 1 × 1 | 1 × 1 × 1 |
| `pyramid` | 1.6 × 1.8 × 1.6 | 1.0532434 × 1.8545134 × 1 | 1.68518944 × 3.33812412 × 1.6 |

The ground's [asset](../../projects/first_project/assets/entities/ground.entity.toml)
uses `half_width = half_depth = 6`. That is a **12 m** plane at scale 1. At scale
1.8 in X/Z it would be **21.6 × 21.6 m**, not 1.8 m. Its current saved values
come from [main.scene.toml](../../projects/first_project/assets/scenes/main.scene.toml).
The cube's [asset](../../projects/first_project/assets/entities/cube.entity.toml)
uses `half_extent = 0.5`, so its scale-1 edge is 1 m.
The legacy [pyramid](../../projects/first_project/assets/entities/pyramid.entity.toml)
has a 1.6 m base and 1.8 m height, and the scene additionally rotates it around X.

This explains the large visual difference. There is no expectation that two
different assets at scale 1 have equal size. Perspective and camera distance also
affect how many pixels a metre occupies.

## Findings

1. **The main confusion is scale versus authored dimensions.** The inspector
   previously displayed only “Scale”, without explaining that it multiplies
   the base asset. It now labels Position (m), Scale (×), and explains scale 1.
2. **Half-size authoring parameters invite factor-of-two mistakes.** The cube
   and ground constructors deliberately accept half-extents. These are correct
   geometry APIs, but are not user-facing full-size fields. Asset creation
   descriptions now state the metre and half-extent convention.
3. **Defaults are not a uniform one-metre envelope.** New cube/pyramid templates
   are 1 m across their bounding extents; new planes are 2 × 2 m. Older ground
   and pyramid assets are larger again. Changing a template would not resize
   existing assets. No defaults or existing authored sizes were changed.
4. **Hierarchy affects physical size.** Local scale is insufficient to infer
   world size when parents are scaled. Global scale is not a world-axis size
   measurement either, particularly under rotation/shear.
5. **Rendering and collision use the same length/scale convention.** No hidden
   object normalization was found along the audited native authored-scene path.
6. **Other controls called scale are not object dimensions.** Texture repeat
   affects texture coordinates, not geometry size; gravity scale is a multiplier
   of acceleration. Resolution is a sample count, and brush strength is a weight.
7. **There is still no editable full Size (m) control or world-dimensions readout.**
   Those would be separate editor features, backed by actual mesh bounds and the
   full native world matrix. Imported/generated models need their own source-unit
   calibration; this audit does not claim live imported-model size compatibility.

## Source trace

- [Primitive constructors and tessellation](../../src/primitives.rs): plane
  coordinates span `[-half_width,+half_width]`; cube coordinates span ±half-extent;
  pyramid uses a full `height` split about its origin.
- [Scene loader](../../src/scene.rs): authored dimensions pass directly into
  primitives; instance transform vectors replace asset defaults.
- [World transforms](../../src/world.rs): TRS and propagated parent matrices,
  with no per-kind unit conversion and no alternate identity graph.
- [Renderer](../../src/graphics.rs) and [vertex shader](../../src/shaders/geometry.wgsl):
  model and model-view-projection matrices transform mesh positions directly.
- [Terrain](../../src/terrain.rs): sample spacing is `size / (resolution-1)`,
  spanning ±size/2. The IDE GLB exporter uses the same grid and metre samples.
- [Physics](../../src/physics.rs): box half-extents multiply world scale; terrain
  width/depth are `settings.size × scale.x/z`, with sample heights scaled on Y.
- [Inspector](../../src/emergent-ide/src/components/transform-inspector.tsx):
  metre positions, degree rotations and dimensionless scale; saves retain the
  existing scene/global-to-local conversion path.

## Validation

[Metre-contract regressions](../../tests/units.rs) measure actual native mesh
vertices after the engine's transforms: a 50 cm cube, a 12 m plane at scale 1,
a 21.6 m plane at 1.8×, a default 1 m pyramid, parent scale inheritance, and a
1000 m terrain patch. These checks are independent of camera framing. Both debug and optimized checks pass. All 74 IDE tests, TypeScript, focused
ESLint and the production build pass. Clippy with warnings denied and rustdoc pass.
The labels and unit explanation were verified in a separate port-3001 browser
session against native inspection; no transform fields were edited.

Full workspace debug and all-feature release runs pass the engine suites but fail
`game::tests::authored_offender_bounces_about_four_metres_above_ground` at
`projects/first_project/src/game.rs:307`, with maximum world Y 3.1047554. That is
a separate game-behaviour/clearance issue, not evidence of a metre conversion.
No performance or cross-platform runtime claim is made.


## Follow-up: unit-plane authoring

The Plane creation template now uses an implicit 1 × 1 m mesh. Ground was later
normalized while preserving the 12 × 12 m world size it had at migration time:
its half_width/half_depth fields were removed and its X/Z transform scale became
12. Texture repeat was compensated as well. The table above is the earlier audit
snapshot, not a live report of values edited afterward. See
[unit-plane defaults](unit-plane-defaults.md) for migration and validation details.
