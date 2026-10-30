# Diffuse image materials and terrain layer palette

First slice: PNG/JPEG diffuse images for ordinary renderable instances, with
object-space triplanar projection and a repeat scale (tiles per local metre).
Albedo/geometry colour still tint the image; existing noise and bump maps remain
independent. Use sRGB sampling for colour images, mipmaps/repeat/linear filtering,
and opaque rendering. No new UV or object identity system.

Authored MaterialDefinition gains diffuse_map {asset,scale}, named terrain_layers
(up to eight {id,name,diffuse_map}), and active_layer (durable layer ID). A selected
layer supplies the terrain's diffuse map; inactive layers persist for later blend
rules and do not allocate GPU resources. Missing/unknown IDs and invalid assets
are errors. Brush painting, splat weights and height/slope blending are deferred.
This palette is reusable material data, but the first layer editor appears on
terrain instances. Geometry/collider data remain unchanged by surface edits.

Material resolution reads/decompresses assets on load, bounded to 16 MiB encoded,
4096 per axis and 4,194,304 pixels. Renderer-owned resources remain inside existing
GeometryId SlotMap entries; upload/mipmap preparation is outside steady-state
frame traversal. Cloneable decoded image data may share an immutable Arc buffer
at the asset/material boundary; this does not replace World ownership. Only the
active diffuse image is decoded/uploaded; every palette reference is validated.

IDE: compact image swatch/picker from project images, optional local PNG/JPEG
import, repeat scale, clear. Uploads use UUID filenames beneath declared asset
roots, bounded same-origin requests and native image validation before exclusive
publication. Material/terrain-layer edits use the existing live inspector and
revision/native validation path. Include image references in revision tracking
and the project graph. Preserve arrays of material layers on repeated edits.

Validate corrupt/oversized/path-escaping images, constant-material behaviour,
sRGB pixels/mips, deterministic projection, layer IDs/selection, repeated TOML
edits, upload persistence, actual native object/terrain texture renders and live
layer switches. Run Rust/IDE checks and a focused native/browser smoke. No frame
rate or Windows/Linux runtime claim without native evidence.

Delivered validation: [native macOS evidence](../platform/diffuse-images-macos.md).
The current reference inspector edits material palettes on scene instances;
shared defaults can be authored in an entity asset's [material] table. Image
bytes participate in revision checks and newly assigned references are stamped
across candidate validation. Inactive palette entries receive path/header/bounds
validation; full decode occurs when activated (and always at import).
