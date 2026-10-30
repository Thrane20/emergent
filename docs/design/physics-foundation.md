# Physics foundation

Status: preview implemented, 2026-10-24.

Use pinned Rapier 3D behind the engine's optional physics feature. World owns the
backend state; ECS PhysicsBody components reference canonical world-scoped SlotMap
entities through EntityIdentity. Backend handles stay private. Reused dense inputs
and a retained map reconcile component additions/removals at fixed-tick boundaries;
removed entities cannot map to replacements. No file I/O occurs in physics stepping.

Data flow: asset defaults -> complete instance override -> validated PhysicsBody ->
fixed gameplay/behaviours -> lifecycle commit -> initial global propagation ->
physics validate/reconcile/step -> dynamic pose writeback -> final propagation ->
post-fixed observers/render extraction. Invalid inputs fail before stepping.

For this slice dynamic bodies must be roots or children of a stationary identity
scene container with no physics component. Static bodies can use representable
positive-scale global TRS. Collider offsets and half-extents scale with geometry.
Changing body settings/scale rebuilds that body and resets velocity; a dynamic pose
write during play is rejected rather than silently overwritten. Static edits sync
before stepping. Engine rendering and inspection do not step physics.

Game code can call `World::add_force(entity, force)` before a fixed physics step.
Calls for the same body during a tick are summed; forces are cleared between steps,
so submit a force again on each tick it should remain active. Use force for sustained
acceleration and add an impulse API separately if one-shot velocity changes are needed.

Authored settings: enabled, body (static/dynamic), gravity_scale, mass, friction,
restitution and collider (shape=box, center, half_extents). Missing bounds are fitted
once from geometry; zero-thickness planes get a minimum collision thickness.
Explicit dimensions must be positive/finite. Disabled overrides remove defaults.

Expected first workload: tens to hundreds of boxes. No performance target claimed.
Validate gravity, stable ground contact, deletion/reuse, edits, hierarchy/scale
rejection, feature-off diagnostics and paused inspection. Run optimized workspace
checks and a native reference smoke. Defer joints, triggers and detailed mesh shapes.

Sources checked: https://rapier.rs/docs/user_guides/rust/rigid_bodies/ and
https://rapier.rs/docs/user_guides/rust/colliders/; alternatives:
https://github.com/avianphysics/avian and https://github.com/jrouwe/JoltPhysics.


## Use

The reference project already enables physics. Load `assets/scenes/physics.scene.toml`
in the IDE and Inspect to see a paused cube and collider outlines. Select an
instance to edit Physics settings beneath Transform. Apply physics writes a complete
instance override and reconstructs inspection. Asset defaults can be edited in TOML.

Run the independent demo from the repository root:

```sh
cargo run --release -p emergent-first-project -- --run-scene assets/scenes/physics.scene.toml
```

The IDE Run button launches the scene selected in the scene dropdown, using
`--run-scene`. The project entry scene remains the default for a plain Cargo launch.
Restart Run to reset from authored values.
To enable dynamic physics on a nested main-scene object, first move it to Scene root.

Example asset defaults (missing collider bounds are fitted):

```toml
[physics]
body = "dynamic"
gravity_scale = 1.0
mass = 1.0
friction = 0.5
restitution = 0.1

[physics.collider]
shape = "box"
center = [0.0, 0.0, 0.0]
half_extents = [0.5, 0.5, 0.5]
```

Use `[entities.physics]` / `[entities.physics.collider]` for complete scene overrides.
Use `[entities.physics] enabled = false` (on separate TOML lines) to disable inherited
physics. Static ground also needs a collider; dynamic objects collide with the box,
not with the detailed render mesh. Auto-fit planes receive finite collision thickness.

Observed validation: [PH-1 macOS evidence](../platform/physics-foundation-macos.md).
