# Persistent native inspection session

First slice: one local child process owned by the IDE, running the linked game's
`--inspect-scene <path>` contract. Construct the normal scene once, start the
engine once, retain its SlotMap world and renderer-owned GPU geometry, and never
advance gameplay ticks. A separate inspection loop handles view and transform
commands and bounded frame capture. This is authoring mode, not a security sandbox.

Use existing PNG support and native Metal/offscreen rendering rather than a new
codec dependency. The IDE starts a release-profile worker. Frames are 960×540,
paced to at most 10 Hz. This deliberately
uses the existing allocating/blocking readback path outside gameplay; it is a
prototype with measured render/readback and encoding costs, not a production video
pipeline or frame-time guarantee. Reuse extraction and encoding buffers where
practical. The IDE retains only the latest frame; browser delivery allows one
outstanding frame request, skipping stale frames rather than building a backlog.

Child stdout is a bounded binary protocol: `EMI1`, little-endian packet kind and
payload length. Frame payload: frame sequence, applied-command sequence, tick,
render/readback microseconds, PNG encoding microseconds, dimensions, worker PID,
PNG bytes. Frame delivery is a latest-frame HTTP long poll, not a video/WebSocket
codec protocol; command requests use a separate endpoint.
Acknowledgements contain command sequence, status and bounded error text. Stdin
is bounded TOML inline-table commands using existing serde/toml dependencies:
view, instance transform, and stop. Only authored instance IDs cross the boundary;
no SlotMap handles are serialized. Control responses acknowledge acceptance/errors.

The IDE owns process lifetime, session UUID/generation, project/scene binding,
bounded diagnostics, command deadlines, startup timeout and an inactivity lease.
Browser disconnection stops frame polling; an inactive session expires. Explicit
Stop and scene replacement terminate the owned process group, never the user's
normal Run process. Old session IDs cannot control a new session. Invalid commands
must leave the world/camera unchanged. Failures retain the browser's last frame and
offer the existing Load snapshot path as fallback.

Add Inspect/Stop alongside Load. Pointer drag orbits and Shift-drag pans. Wheel
movement dollies the perspective camera; for orthographic isometric projection it
changes view size because camera translation does not change orthographic framing. The existing instance list/Transform inspector stays
authoritative for selection. Apply validates/persists through the existing path,
then sends the resolved transform to the retained world without rebuilding it.
Reset behaves the same. A scene/code/geometry change needs a session restart in
this slice. Picking, gizmos, gamepad input, live drafts, general undo and hardware
video encoding are deferred.

Validate protocol framing/bounds, malformed controls, stale sessions, process
cleanup, tick-zero invariance, camera changes, shared world/resource lifetime,
and a real browser edit with unchanged process/session identity. Record hardware,
dimensions, pacing, measured timing and transport limitations before extending it.


Update VG-1: the current implementation targets 30 Hz with deadline pacing and
adds picking, axis gizmos and transient drag previews. See
[viewport tools](viewport-picking-gizmos.md) for the extended acknowledgement data,
gesture ownership and save/cancel flow. The original 10 Hz description above
records the initial proof; it is no longer the active cadence.
