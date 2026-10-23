# Iteration 10 — Light-source entity foundation

Status: complete.

## Outcome

Add a validated, headless `LightSource` component for canonical Emergent entities and prove its use from the independent first project. The component models ambient, directional, point, and spot lights with colour, intensity, bounded influence where applicable, spot cone angles, and an explicit shadow-casting request.

This iteration establishes scene and project vocabulary. It does not claim that the current unlit colour renderer evaluates lights or renders shadow maps.

## Ownership and SlotMap integration

- `World` retains canonical entity identity and hierarchy in its SlotMap.
- `LightSource` is dense component data stored on the entity's mapped `bevy_ecs` component entity. It does not introduce a light identity or ownership graph.
- Position comes from the entity's propagated transform. Directional and spot lights use the entity's local negative Z axis after global transform propagation. Ambient lights ignore transform; point lights ignore rotation.
- Runtime `EntityId` values are never persisted. Authored projects identify a light with a durable scene/entity ID and resolve it to a fresh entity during construction.

## Data and validation

- Colour uses the existing validated `Color` value.
- Intensity is a finite, non-negative engine scalar. Physical photometric units are deferred until material and exposure contracts exist.
- Point and spot ranges are finite and positive.
- Spot inner and outer half-angles are finite, non-negative, ordered, and no wider than 90 degrees.
- Shadow casting is an explicit disabled/enabled request. Ambient lights cannot request shadows. Renderer support and quality settings remain deferred.
- Private fields and named constructors prevent unchecked combinations.

## Data flow and hot paths

Projects construct a canonical entity, set its transform, and insert a `LightSource` through `World::insert_component`. Construction and validation occur outside the fixed-update hot path. Future render extraction will query light components and copy bounded frame data without changing canonical identity.

This iteration adds no per-frame light traversal, GPU buffer, shader branch, or shadow work, so it makes no rendering-performance claim.

## Expected scale

The component representation is suitable for ordinary ECS storage. A future renderer must define visible-light limits, selection/culling, clustered or tiled evaluation, and shadow budgets from measured workloads before large light counts are claimed.

## Validation

- Accept each supported light kind and expose its validated properties.
- Reject non-finite/negative intensity, invalid range, invalid spot cones, and ambient shadow requests.
- Verify a light component follows canonical entity lifecycle and becomes unavailable with its entity.
- Add a point-light entity to `projects/first_project` using only public Emergent APIs.
- Keep default builds headless and run formatting, workspace tests, optimized all-feature tests, Clippy, and rustdoc.

## Acceptance gate

Iteration 10 is complete when the public headless API can represent validated light-source entities, the independent project constructs and verifies one, documentation distinguishes the available component contract from planned illumination and shadows, and the full repository validation suite passes.
