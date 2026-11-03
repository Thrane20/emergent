# Nested UI layout checks

2026-11-02, macOS / Apple Silicon. UI-1 authoring/layout only; this does not
claim in-game UI rendering, pointer input or a production frame budget.

- Rust `cargo test -p emergent --test ui`: 8 passed, including the legacy options
  fixture, legacy stack behavior, nested grid/row/column geometry, stretch/default
  dimensions, and integer-column rejection.
- Optimized all-feature UI suite: 8 passed.
- Browser layout unit suite: 5 passed. Rust and TypeScript consume the same nested
  asset and expected rectangle fixture, including percentage widths, asymmetric
  child sizes, gaps, padding, center/end alignment and mixed container nesting.
- Clippy (`-p emergent --all-features --all-targets -- -D warnings`), TypeScript,
  focused ESLint and isolated production build passed.
- Browser smoke script: `src/emergent-ide/scripts/ui-layout-smoke.ts` uses a
  disposable standard-template project on port 3001, compares nested container
  SVG rectangles to the shared fixture, edits padding and saves/reopens through
  native validation. It closes the service and removes the temporary project.

The first browser attempt timed out waiting for the native catalogue to compile;
the smoke now allows the same 120-second tool-build window as existing smokes.
Grid uses equal cells and a fixed column count; no spans, content auto-sizing or
flex growth. Width/height overflow is preserved, with no implicit shrink or clip.

Browser smoke passed on the rebuilt production app: all three palette entries,
nested container geometry, padding edit, native save and reopen, with no page
errors. Screenshot: `/tmp/emergent-ui-layouts.png` (ephemeral). Rustdoc passed.
Workspace/all-feature tests pass the engine suites but stop at the existing
first-project spin/missing-entity and bounce-height failures. The full IDE suite,
other browsers/platforms and layout performance benchmarks were not run for this
change. The user's port-3000 service was left running.
