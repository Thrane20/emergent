# Add Asset prefab picker

Validated on macOS, 3 November 2026.

The main sidebar's **+ Add asset** modal includes an expanded **Prefabs** section
listing the linked project's current prefab assets. Search matches names and paths.
Selecting one shows its part count and **Add prefab to scene** places an independent
hierarchy at the active scene root using the same revision-guarded callback as drag
placement. Invalid definitions or unavailable scenes cannot be submitted; placement
errors remain visible in the modal. Existing asset creation keeps its own flow.

Validation:

- `pnpm exec tsc --noEmit --pretty false` passed.
- Targeted ESLint passed for the modal, studio and sidebar smoke.
- `pnpm exec tsx --conditions=react-server --test tests/prefabs.test.ts`: 7 passed.
- Extended `scripts/sidebar-cards-smoke.ts` passed against a disposable standard
  template project with editor fixtures: capture while collapsed, drag placement,
  reload/provenance icon persistence, Add Asset prefab search/selection/placement,
  two distinct scene instances and no browser page errors.

Browser validation used temporary port 3002 because another development task held
3001; the user's port 3000 was untouched. The native inspection session and test
service were stopped afterward. No production build or full engine suite rerun;
the change is confined to IDE presentation and the existing placement callback.
