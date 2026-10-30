# Persistent inspection — macOS evidence

Date: 2026-10-24. Local Apple M3 Pro / arm64 / Metal. Inspection worker uses the
Cargo release profile, 960×540 RGBA capture, fastest PNG compression and a 10 Hz
maximum pacing target. Browser checks use headless Chrome and the user's existing
IDE at localhost:3000. This is an authoring prototype, not a video codec pipeline.

## Integration results

- A real persistent Rust worker constructs the world and uploads geometry once.
  Multiple received frames retain the same session UUID and worker PID, remain at
  tick zero, and render identical scene pixels until a camera/property edit.
- Pointer orbit, Shift-drag pan, wheel zoom and Reset view use acknowledged commands
  and the same renderer. The existing Transform inspector's Apply saves through
  guarded Rust validation, then updates the retained world without a worker restart.
  The shared asset remains unchanged. Geometry/source changes require Restart.
- Unknown instance controls are rejected. Restart creates a new session generation;
  old session commands cannot affect it. Explicit Stop exits the owned worker.
  With no browser/frame/control traffic, the inactivity lease stops the worker
  after 30 seconds plus the five-second lease-check interval.
- React development Strict Mode is exercised: effect cleanup aborts polling but
  does not terminate the session. This avoids the start/cleanup/start worker race.
- The browser retains its last canvas image after a frame/control failure. Load
  remains the independent snapshot fallback. Normal game Run uses a separate
  process manager and is not stopped by inspection controls.

`pnpm test:inspection` exercises the real browser/worker/inspector workflow with
smoke-owned temporary assets and a read-only reference-scene measurement. Protocol
tests cover fragmented packets, framing bounds, acknowledgements, tick-zero frame
validation, malformed view/transform commands and instance-ID validation. Rust
tests cover camera degeneracy and command/packet contracts in addition to the
existing scene/world/renderer suites.

## Small reference-scene timing sample

Two short runs, each with 25 frames received sequentially; discard the first five
and summarize the next 20. Workload: the reference main scene (seven uploaded renderables in this local
fixture). This was not an isolated benchmark; other local processes may run.

| Metric | Run 1 | Run 2 |
| --- | --- | --- |
| Delivered frames (10 Hz target) | 9.42 fps | 9.30 fps |
| Median render + blocking readback wall time | 1.530 ms | 1.514 ms |
| Median PNG encoding wall time | 1.877 ms | 2.164 ms |
| Median PNG payload | 144,061 bytes | 144,061 bytes |
| One camera command acknowledgement round trip | 106 ms | 101 ms |

The acknowledgement measurement includes local HTTP and command scheduling; it is
not pointer-to-photon latency. Render/readback is wall time, not an isolated GPU
timestamp measurement. The short sample does not establish tail latency, CPU/GPU
utilization, memory budgets or scalability. PNG full-frame delivery uses roughly
1.34–1.36 MB/s at this observed rate before HTTP overhead; no inter-frame compression
or hardware encoding is implemented.

## Checks

- `cargo test --workspace`
- `cargo test --workspace --release --all-features`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo doc --no-deps --all-features`
- IDE service tests, TypeScript, lint and production build
- A separate production Next server check verifies that start, frame, command and
  stop routes share the same retained Metal worker across route bundles.
- Real persistent inspection smoke, plus existing snapshot, authored placement
  and production UI regression checks
- A fault-path test starts an unresponsive owned worker and verifies forced process
  cleanup, rather than merely changing its reported state to stopped.
- Native reference-window smoke: seven geometry resources, 120 ticks, 239
  presented frames, resize observed and clean shutdown on Metal.

No model/AI provider call is needed for these checks. No Selkies packages, display
capture service or GPU container is involved. Windows/Linux runtime, Safari,
gamepad input, picking, gizmos, transient draft preview, general undo/redo and
hardware video encoding remain outside this validation.
