# Terrain Performance v1

Scope agreed 2026-10-30: measure the existing bounded ray marcher, reduce invisible
and preview pixel work, and expose expensive rays before introducing terrain LOD.
This is not completion of the continuous-world performance gates.

The renderer retains its typed, owner-scoped SlotMap terrain handles. Each pass
owns its GPU pipeline/uniform and cached projected bounds; no authored recipe,
entity identity, density function or collision contract changes. Runtime debug
options are not saved into terrain assets.

First capture warmed release baselines for fixed overview, grazing, cavern,
small projected region and offscreen views. Record raw capture wall times and
p50/p95/p99, adapter and viewport. Capture includes GPU completion and readback;
it must not be labelled isolated GPU timing or native Run frame time.

Then conservatively reject bounds outside homogeneous clip planes and restrict
remaining draws to projected pixel rectangles. Near-plane intersections use the
full viewport; camera-inside, orthographic and transformed bounds must remain
visible. Restore scissor before ordinary geometry. Work is bounded to eight
corners per region, without hot-path allocations. Compare exact captures against
the uncropped renderer and retain a runtime diagnostic bypass for measurement.

Add an optional ray-step heatmap: cool to hot for increasing iterations, magenta
for iteration exhaustion, with unchanged production iteration limits. Preview
quality is separate from authored resolution: a lower internal capture resolution
may be upscaled to the existing 960×540 inspection envelope. Preserve protocol
validation, session cleanup, camera commands and all other inspection consumers.

Acceptance: focused clip-bound tests plus native colour/depth/inside/transform
smokes; equivalent before/after warmed captures; browser quality/debug controls
and session lifecycle checks; Rust formatting/tests/Clippy and affected IDE checks.
Only measured changes are reported as improvements. GPU timestamp profiling,
depth-aware tracing, mesh LOD, density extraction and streaming remain subsequent
slices unless explicitly completed and evidenced.

## GPU profiling increment

Request the optional TIMESTAMP_QUERY device feature only when the adapter supports
it. A renderer-owned, lazily created profiler retains one query set, resolve/map
buffers and an offscreen colour target. Runtime SlotMap stores are unchanged.
Normal presentation and capture do not issue queries or map profiling buffers.
An explicit blocking diagnostic call profiles the existing shadow and main
(sky/ray/mesh) passes without copying colour pixels or encoding PNG. Particles
remain outside these pass timings. Unsupported devices return no timing sample.

Resolve only written query pairs, at 256-byte aligned buffer offsets. Reuse buffers
after unmapping; poll with the existing bounded timeout and preserve cleanup on
errors. Resize the diagnostic target when viewport dimensions change. Return CPU
preparation/encoding/completion durations separately from GPU pass durations.
This serial diagnostic workload is not the normal asynchronous presentation loop.

The M3 Pro produced reversed counters when resolve was encoded with the sampling
passes. [wgpu issue 9414](https://github.com/gfx-rs/wgpu/issues/9414) describes
incomplete Metal results when resolve is committed before host-observed sampling
completion. The diagnostic call therefore submits and waits for sampling first,
then encodes/submits resolve and maps the result. Both waits use a 10s timeout.
Completion CPU time includes both waits and resolve scheduling; it overlaps GPU
execution. Reject zero/unordered counters and invalid periods, without filtering
them into plausible measurements. No backend fork, unsafe Metal access or new
dependency is introduced. This workaround is validated on this machine only.

Validate supported/unsupported fallback, timing conversion and reversed counters,
main/shadow queries, query reuse, resize, camera traversal and unchanged capture
output. Record warmed release route distributions and explicit resource counts;
leave native presentation, simulation, allocator/driver memory and delivery budgets
open rather than presenting a blocking benchmark as gameplay acceptance.
