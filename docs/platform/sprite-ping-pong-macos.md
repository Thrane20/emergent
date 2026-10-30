# Sprite ping-pong playback — macOS, 2026-10-30

Sprite options now offers Playback → Forward loop / Ping-pong. Save image settings persists `ping_pong: true` in the existing image sidecar. Forward playback is the default for old files. The preview reflects frame indices at the ends without repeating endpoints: a four-frame animation plays `0,1,2,3,2,1,0…`. Static and two-frame sheets are handled explicitly, and frame count can be smaller than atlas capacity.

Particle Factory copies the setting when assigning a library sprite, and exposes a Ping-pong animation checkbox for existing emitters. The browser preview and Rust billboard pass implement the same bounded frame mapping. The saved emitter field defaults to false in TypeScript and Serde. This adds no textures, pixel rewriting, fal submissions or runtime object identity changes. Frame selection performs constant-time arithmetic without new allocation; no before/after performance gain is claimed.

## Validation

- `pnpm test`: 111 tests passed, including forward/reflected/wrap sequences, one/two/partial frame counts, invalid metadata and emitter recipe compatibility.
- TypeScript, focused ESLint and Next production build passed.
- Production Chromium media smoke passed on desktop/mobile: reverse frame sequence observed, sidecar value saved/reloaded, ping-pong copied to the emitter, modal layout/tooltips and existing media operations retained. Provider calls mocked; local file/ffmpeg/native validation real. Test service on IPv6 port3001 stopped; user port3000 untouched.
- `cargo test -p emergent` and `cargo test -p emergent --all-features --release` passed, including the new frame/legacy TOML regression. Optimized all-feature run: 89 engine unit tests and 38 integration tests passed.
- `cargo run --release --features graphics --example particle_render_smoke` passed on Apple M3 Pro / Metal. A new four-colour atlas check verifies forward frames and the descending blue/green/red sequence in actual native captures. Existing alpha, depth, blend and resize checks also passed.
- `cargo clippy --all-features --all-targets -- -D warnings`, `cargo doc --no-deps --all-features`, and rustfmt checks for touched Rust files passed.
- Full optimized workspace test attempted: engine checks passed; first_project's existing `game::tests::authored_offender_bounces_about_four_metres_above_ground` failed (reported height 3.340048). That unrelated project behavior was left unchanged.
