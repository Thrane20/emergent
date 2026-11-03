# Scene sidebar cards

2026-11-03, macOS / Apple Silicon / Chrome, disposable standard-template project on port 3001.

Delivered independent Scene graph, Prefabs and Project files cards with the inspector's border/radius treatment. The graph has its own scroll area. Project files starts collapsed and its unsupported delete controls and callback path were removed; scene-instance deletion remains available. Prefab capture/drop/Enter target sits beside the header toggle and remains mounted when collapsed. Library assets and placed roots use a distinct primary-coloured package icon; ordinary objects use a cube icon.

Placement now writes optional `prefab_label` on the root. Native validation allows only nonempty printable labels up to 64 characters. It is authoring provenance, not a source dependency or live prefab link. Existing expanded objects lacking origin metadata keep their ordinary icon.

Validation:
- TypeScript and focused ESLint pass.
- Six prefab unit tests pass, including preserved origin metadata after independent placement.
- 31 native authored-scene tests pass, including a new provenance validation test without a live source prefab dependency.
- Chrome `sidebar-cards-smoke.ts` passes: ordered cards, initially collapsed project files with no delete controls, collapse-prefabs/header-drop visibility, actual capture by drag while collapsed, library icon, drag placement and root icon surviving page reload. Screenshot inspected.
- Production IDE build passes.
- Whole-workspace format check reports an unrelated formatting change in tests/plugins.rs; changed Rust files are rustfmt formatted. Initial compiler warnings in plugin.rs were unrelated concurrent work.

The test inspection worker/server were stopped and the temporary fixture removed. User port 3000 was left untouched. No frame-time or linked-prefab-update claim.

Final Clippy (`--workspace --all-features --all-targets -- -D warnings`) passes.
