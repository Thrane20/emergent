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

`World::add_torque(entity, torque)` now supplies world-space N·m with the same
per-fixed-step accumulation/reset semantics. It changes angular acceleration rather
than prescribing a pose. `World::angular_velocity(entity)` reads committed
world-space radians/s, or `None` before the body's first physics step. Body/scale
reconstruction resets angular motion as well as linear motion; reads are backend
snapshots until the next fixed reconciliation. Stale/wrong-world handles are
rejected; removed-body commands cannot affect a replacement key. The flight pilot
must handle reconstruction explicitly. [Flight-control contract](flight-control.md),
[validation](../platform/flight-control-macos.md).

`World::physics_body_state(entity)` adds a reconstruction-aware observation of
committed pose, linear/angular velocity, actual mass and effective gravity. It is
`None` before initialization or when current settings/pose no longer match the
committed body. A successful reconstruction changes its world-local revision,
which is change bookkeeping for the same EntityId, never serialized identity.
The shared `PHYSICS_GRAVITY` constant supplies backend gravity; body observations
include the per-body gravity scale. `drone_physics` is an explicitly unpowered,
loadable locomotion boundary that requires this dynamic-body owner. It does not
execute flight goals or drive the transform. [FLIGHT-2 validation](../platform/physics-locomotion-boundary-macos.md).

Authored settings: enabled, body (static/dynamic), gravity_scale, mass, friction,
restitution and collider (shape=box, center, half_extents). Missing bounds are fitted
once from local geometry, or from descendant geometry when the parent has none; zero-thickness planes get a minimum collision thickness. Descendant bounds are transformed into the parent's local space during scene validation, before runtime construction. Truly empty groups still require explicit half_extents.
Explicit dimensions must be positive/finite. Disabled overrides remove defaults.

Expected first workload: tens to hundreds of boxes. No performance target claimed.
Validate gravity, stable ground contact, deletion/reuse, edits, hierarchy/scale
rejection, feature-off diagnostics and paused inspection. Run optimized workspace
checks and a native reference smoke. Defer joints, triggers and detailed mesh shapes.

Sources checked: https://rapier.rs/docs/user_guides/rust/rigid_bodies/ and
https://rapier.rs/docs/user_guides/rust/colliders/; alternatives:
https://github.com/avianphysics/avian and https://github.com/jrouwe/JoltPhysics.


## Use

The standard template enables physics through the engine host. Load `assets/scenes/first-person.scene.toml`
in the IDE and Inspect to see a paused cube and collider outlines. Select an
instance to edit Physics settings beneath Transform. Apply physics writes a complete
instance override and reconstructs inspection. Asset defaults can be edited in TOML.

Run the independent demo from the repository root:

```sh
cargo run --release -p emergent-standard-template -- --run-scene assets/scenes/first-person.scene.toml
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

### Live inspection attributes

Physics saves use the existing revision-guarded scene write, then send a `physics` command containing the authored instance ID to the retained worker. The worker validates the saved scene through the same project registry and resolves that ID to its existing SlotMap handle. Only that entity's PhysicsBody and retained collider outlines change. Removing an override restores the validated asset default; disabling physics removes the component and outline. Inspection stays at tick zero; orbit, selection, geometry and unrelated entities survive. Errors leave the saved scene available and report live application failure without an automatic worker restart. Validation currently parses the whole scene on edits (off the simulation hot path); narrow it if measured latency warrants it. This does not hot-edit a separate native Run process.

Model-parts prefabs use an empty root with renderable children. The Physics inspector defaults to a Static collider when an object has a kinematic locomotion profile; Static colliders follow authored movement. Dynamic bodies require a physical locomotion profile and remain incompatible with the kinematic drone profile.
