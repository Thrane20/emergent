# Iteration 6 macOS graphics smoke result

Recorded: 2026-10-22

## Environment

- Emergent 0.6.0 release profile
- Rust 1.99.1 (`aarch64-apple-darwin`, LLVM 23.1.1)
- Apple M3 Pro, 12 logical CPUs, 36 GB memory
- macOS 27.0.1 (26A434)
- `wgpu` 30.0.1 and `winit` 0.30.13

## Command

```sh
cargo run --release --features graphics --example first_window -- --smoke
```

## Observed result

The application selected the Apple M3 Pro adapter through the Metal backend with a `Bgra8UnormSrgb` surface. It created a native window, presented the depth-tested rotating cube, requested and observed a resize from 960x640 to 800x600, recreated surface-dependent resources, advanced 120 fixed simulation ticks, presented 239 frames, and stopped cleanly without a `wgpu` validation error.

This verifies the local macOS path only. Windows and Linux builds and runtime launch remain pending on suitable machines.
