# First-person walking — FPS-1

FPS-1 composes existing World-owned entities, CharacterController, dynamic box
physics and a child perspective camera. Camera-local MouseLook owns yaw/pitch;
the dynamic body remains upright and physics-owned. No second identity graph is
introduced. Authored camera/player IDs resolve to fresh typed World EntityIds.

`emergent.mouse_look` is authored on a camera, with degrees-per-device-pixel
sensitivity and a pitch limit. Only the ActiveCamera consumes mouse motion. The
native shared host captures the pointer on left click, releases it with Escape
or focus loss, and clears keys/motion on release. Keyboard movement for an FPS
scene is gated by capture; ordinary keyboard-controlled scenes are unaffected.
Raw motion accumulates until the next fixed tick, is consumed exactly once across
catch-up steps, and never receives delta-time scaling. Nonfinite/overflow input
is rejected. Camera transforms propagate before camera-relative walking; physics
keeps sole ownership of body poses. Existing yaw/pitch at loading is preserved.

Grounding uses the existing Rapier narrow phase: supporting non-sensor contact
normal with upward component >= 0.5 and contact distance <= 0.02 m. Jump also
requires non-rising vertical velocity. Walls, ceilings and an airborne apex do
not count as ground. This replaces the legacy near-zero-vertical-speed shortcut
for all dynamic character controllers. No capsule, automatic stairs, slope speed
policy, moving-platform attachment, crouch or ray-terrain collision is claimed.

The standard template and reference project get an explicitly selected FPS test
scene and reusable player prefab; their entry scenes stay as authored. The IDE
component catalogue exposes mouse-look settings. Native Run owns capture; browser
inspection retains its independent orbit controls and tick-zero behavior.

Frame work uses the existing dense, reused motion scratch lists and fixed-size
mouse accumulation; there is no loading I/O or per-tick hierarchy discovery.
Validation covers single consumption/catch-up, finite input, pitch limits, stale
camera handles, contact grounding/apex/wall rejection, collision against boxes and
heightfields, fixture loading, workspace/release checks and native host smoke.
Hardware sensitivity and movement feel require manual calibration; no production
frame budget is claimed.
