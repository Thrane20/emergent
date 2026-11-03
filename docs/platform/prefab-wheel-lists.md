# Prefab capture: optional wheel references

2026-11-03. The reported error originated in generic entity-list validation. Locomotion declares steering_wheels and rolling_wheels with empty-array defaults; prefab remapping writes those defaults explicitly. The native validator previously rejected every explicit empty Entities value, so a valid object could fail capture.

Validation now permits an empty list only when that field declares an empty-array default. The 64-reference limit, valid ID checks and nonempty required-list contract remain intact. No user assets are rewritten or wheel references dropped.

- Native component tests: pass, covering optional empty defaults, malformed list values, oversized lists and required nonempty lists.
- Prefab unit tests: 7 pass, including capture materializing both empty wheel lists without changing the locomotion profile.
- Actual affected object in the linked project: captured and successfully validated with the same staged native candidate workflow used by the save route. Staging was cleaned up and no authored asset or scene files were written.
- Focused test lint and changed-file whitespace checks pass.

The developer can retry dragging the object; existing saved data needs no migration. This fix does not relax external-reference checks for nonempty wheel lists.

Final checks: all three focused component tests pass in debug and optimized builds; Clippy with warnings denied passes.
