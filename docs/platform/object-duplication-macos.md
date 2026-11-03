# Scene object duplication — 2026-11-02

The scene object browser provides a Duplicate row button and D while the browser
has keyboard focus. The copy includes descendants, preserves the selected object's
parent and local transform/settings, receives fresh authored IDs and becomes the
selection. Internal component entity references and explicit brain bindings follow
the copied objects; external references retain their targets. Reusable assets are
shared and are not modified by duplication.

A trailing ` (N)` is treated as the copy suffix. New names use the highest existing
suffix plus one, including when duplicating a copy or when numbering has gaps.
Typing, modifier shortcuts, held-key repeats, renaming and pending edits do not
trigger D. The existing scene revision check, size/count bounds, native validation
and atomic save path protect publication. Runtime SlotMap identity is unchanged.

Validation on macOS arm64 / Chrome:

- 13 prefab/scene-placement regression tests pass, including descendant transforms,
  fresh IDs, internal/external links, numbered copies and collision rejection.
- Production Next build, TypeScript and focused ESLint pass.
- `EMERGENT_NEXT_DIST_DIR=.next-duplicate-build npm run test:duplicate` passes
  against a disposable standard-template project on port 3001: button, D, copy
  selection, copy-of-copy numbering, child copies, typing guard, native scene
  validation, rejection of stale revisions and unchanged entity assets.
- Browser and test server were stopped; disposable project/build output and generated
  type includes were removed. The user's project and port 3000 were untouched.

No Windows/Linux browser or native run was tested. This operation duplicates scene
instances; it does not create a separate reusable asset definition.
