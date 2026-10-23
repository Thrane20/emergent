# Rust-rendered IDE scene preview

The top selector lists authored scene files. Load launches the linked Cargo game
with --preview-scene <project-relative-path> --preview-output <PNG-path>. The game
validates the scene, resolves its registered Rust builder, constructs the normal
SlotMap world at tick zero, and extracts its real geometry/lights.

GeometryRenderer gains an offscreen target and bounded blocking RGBA readback.
Native presentation and offscreen captures share the exact shader pipelines,
resource ownership, light preparation, and render-pass encoder. GPU handles remain
renderer-scoped. Readback is an explicit authoring operation, never a normal
game-loop cost. PNG encoding belongs to the project host, not engine storage.

The IDE owns a temporary output directory and launches Cargo without a shell,
with bounded runtime and diagnostic output. A successful PNG replaces the previous
preview through an object URL; errors retain the last valid frame. Loading does
not change project assets or run gameplay ticks. The first bridge is one paused
frame per Load, not an interactive simulation stream.

The current reference project registers its existing generated main_scene builder.
Other scene files remain selectable but unsupported builders fail explicitly.
Arbitrary authored-world loading, live frame streaming, camera input transport,
and engine GLB support remain separate increments. Model Viewer remains appropriate
for standalone generated-asset inspection, not engine scene rendering.

Validate offscreen bounds and row alignment, keep existing renderer tests intact,
run Rust format/tests/release-all-features/Clippy/rustdoc, and compare native smoke
with a real offscreen PNG. Test the IDE's scene selection and failure handling,
then load the Rust frame in the running dev browser and production build.
