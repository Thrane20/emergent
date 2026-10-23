# Iteration 9 macOS primitive smoke

Date: 2026-10-23

Command:

```sh
cargo run --release --features graphics --example primitive_gallery -- --smoke
```

Observed environment and result:

- Adapter: Apple M3 Pro
- Backend: Metal
- Surface format: `Bgra8UnormSrgb`
- Uploaded primitive resources: 8
- Fixed simulation ticks: 120
- Presented frames: 241
- Requested resize to 1000 by 700: observed
- Shutdown: clean

The resources were generated once from plane, cube, pyramid, UV-sphere, cylinder, cone, capsule, and torus parameters, then retained in renderer-owned SlotMap storage. This is native smoke evidence for resource creation, presentation, resize, and shutdown on this machine. It is not a frame-time or production-scale result. Windows and Linux runtime validation remain pending.
