# ADR 0005: Optional graphics and simulation-to-render extraction

Status: accepted for the first visible 3D foundation.

## Decision

Emergent uses `wgpu` 30.0.1 for GPU access and stable `winit` 0.30.13 for native window events. Graphics remains behind the opt-in `graphics` Cargo feature, so the canonical `Engine`, SlotMap world, schedules, behaviours, and headless tests do not require a window or graphics device.

The platform loop owns the window and advances the existing engine through `Engine::start`, `Engine::advance_frame`, and `Engine::stop`. `advance_frame` accepts elapsed platform time and preserves the existing fixed timestep, maximum frame delta, and bounded catch-up policy. It reports whether catch-up was limited.

The renderer owns GPU resources and receives copied render values. The first example reads a canonical entity's propagated `GlobalTransform`, copies its matrix into one frame uniform, and draws a fixed indexed cube. The renderer never owns an entity, stores a Bevy entity, or keeps a borrow into `World`. SlotMap `EntityId` therefore remains the object's canonical runtime identity.

## Surface and lifecycle policy

- Create the window and renderer after the first `winit` `Resumed` event.
- Use a surface-compatible high-performance adapter with no optional device features.
- Use the adapter's default surface configuration and an explicit `Depth32Float` target.
- Ignore zero-size resize events; recreate the depth target and surface configuration for non-zero sizes.
- Skip timeouts and occlusion, reconfigure outdated or suboptimal surfaces, and recreate a lost surface from the retained window and instance.
- Stop game callbacks before leaving the event loop.

## Consequences

The cube proves a 3D pipeline, WebGPU zero-to-one depth projection, indexed geometry, depth testing, resize, fixed simulation integration, and Metal presentation. It is deliberately not a general mesh or material API. General render extraction should batch copied data and retain `EntityId` only for diagnostics or selection; GPU offsets remain transient renderer details.

Windows and Linux are supported by the selected upstream crates and enabled `wgpu` backends, but Emergent runtime support remains unverified until this smoke example is built and run on those operating systems.
