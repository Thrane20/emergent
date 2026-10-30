# Diffuse images and terrain palettes — native macOS evidence

Implemented and checked 25 October 2026 on Apple M3 Pro / macOS 27.0.1.

The engine resolves PNG/JPEG diffuse images from project assets with size and
allocation bounds. Uploaded images use sRGB sampling and linear-light CPU mip
preparation. Object-space triplanar projection needs no new mesh UV attributes.
The existing renderer GeometryId SlotMap owns the uploaded resources; inactive
terrain layers remain authored data rather than extra GPU textures.

Passed:

- Default workspace tests and release workspace/all-feature tests.
- Strict all-target/all-feature Clippy, rustdoc with warnings denied, formatting.
- 53 IDE tests, lint, type checking and production build.
- `cargo run --release --features graphics --example diffuse_material_smoke`:
  actual native offscreen cube/sphere texture output, 128-grey sRGB round trip,
  PNG/JPEG terrain layer switching and invalid image data rejection.
- `pnpm --filter emergent-ide test:textures`: temporary independent project,
  distinct Cargo package, owned production server (default port 3003). Browser
  imports PNG/JPEG, edits cube diffuse repeat, adds/renames two terrain layers,
  switches the active layer, verifies native rendered pixel changes and persisted
  TOML, and rejects a corrupt PNG without adding an asset folder.
- Repeated nested material-array edits preserve transforms, physics and other
  instances. Referenced image byte changes invalidate scene revisions; new image
  references are stamped before native validation and rechecked before writing.

The final browser screenshot was visually inspected: the cube displays the
checker texture and terrain displays the active green surface, with Grass and
Rock layers retained in the inspector. Temporary project/server/worker cleaned
up; no user game assets changed. One focused dependency was added:
`image = 0.25.10` with only PNG/JPEG codecs enabled.

A simultaneous camera change briefly left incomplete Rust fields and a TypeScript
assertion in the shared workspace. These settled; two no-op unit expressions and
one test type assertion were corrected without changing camera behaviour. All
checks above passed with the camera changes preserved.

Current limits: opaque colour textures, at most eight palette layers, one active
whole-surface texture, independent per-geometry GPU image uploads, no brush/splat
painting or height/slope blending yet. Image imports are at most 16 MiB encoded,
4096 per axis and 4,194,304 pixels. No production frame-time or Windows/Linux
native-runtime claim is made.
