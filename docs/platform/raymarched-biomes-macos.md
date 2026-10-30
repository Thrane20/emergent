# Ray-marched biome validation — 2026-10-30

Apple M3 Pro, macOS, optimized Rust native wgpu/Metal. This is the first biome
generation increment, not production large-world performance acceptance.

## Implemented recipe

V2 defaults: seed 42, width 1024m, biome scale 180m, relief 120m, overhang 6m,
cave radius 8m. Width accepts 64–4096m. Features use local metre coordinates
independent of width. Procedural slope/elevation colouring blends grass, exposed
rock and snow. Assets without `version` retain the v1 sphere/tunnel/arch recipe.
Existing saved assets are not silently upgraded.

## Checks

- Rust headless tests pass for legacy decoding, v2 TOML roundtrip, size bounds,
  unchanged interior samples at different extents, sampled gradient bounds,
  cave subtraction through solid mountain and disabling caves.
- `cargo run --release --all-features --example raymarched_biome_smoke` passes:
  isolated overhang topology with cave carving off (24m displacement), 256 CPU/GPU
  top-down coverage queries across a kilometre region, and mesh occlusion matching
  three CPU reference surface queries. Native 4096m overview renders without a
  dense height/voxel allocation. This does not exhaust all parameter/camera cases.
- Initial 512-step tracing missed a sampled surface; rejected that budget. V2 now
  caps at 1024 steps and all fixture coverage queries pass. Grazing/extreme fields
  can still exhaust the budget; bounded misses are not a convergence guarantee.
- Legacy native depth/camera/transformed-resource smoke still passes.
- All 127 IDE tests pass. Production build, TypeScript, focused ESLint, Clippy,
  rustfmt and rustdoc pass. Engine all-feature release suite passes. Full workspace
  release still reaches the unrelated existing offender bounce failure (3.340048).
- Updated browser smoke passes default Biome landscape at 1024m, seed change,
  native preview, guarded save/load, placement, main native scene/transform save,
  reopening, mobile preview access and native Run. Disposable project, owned IPv6
  port 3001 service stopped; no user scene assets or paid calls.

## Visuals and timing limits

Reviewed `/tmp/emergent-biome-overview.png`, `emergent-biome-low.png`,
`emergent-biome-cave.png`, `emergent-biome-overhang.png`, `emergent-biome-4096.png`
and the Terrain Builder/browser screenshots. These establish regional variation
and 3D surfaces, not a finished art direction or biome ecosystem.

One native run measured individual 640x480 captures at approximately 24.5ms overview,
14.0ms low view, 16.4ms isolated overhang and 6.2ms cave view. These include GPU
completion/readback and possible first-use compilation, with other tools active.
No GPU timestamp, steady-state median, p95/p99 or gameplay frame budget is claimed.
This field is substantially more expensive than the analytical fixture; profile
grazing views, field complexity and multiple instances before expanding residency.

Collision, ecological simulation/vegetation, editable biome materials, scene-light/
shadow parity, streaming/LOD and Windows/Linux rendering remain open.
