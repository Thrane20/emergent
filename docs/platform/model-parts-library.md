# Model Parts → Object Library

2026-11-03, macOS, Node 22.22.0, Chrome/Playwright. IDE-only extension using the
existing prefab, library carousel, placement and staged native validation contracts.

**Save parts** is now **Save To Object Library**. It saves the current part set and
publishes `parts/model.prefab.toml` beside it, referencing the saved group asset and
part entities with their existing hierarchy and pivot-relative translations. The
prefab is in the same validated publication batch as the part manifests/GLBs; it
retains its durable ID on repeated saves. No global library database or mesh
conversion is introduced. Existing part sets become library entries on their next
explicit save.

Object Builder's **Object library** carousel lists these multipart prefab objects
beside structures. A model tile opens its original descriptor in Model Parts;
right-click/keyboard context action uses the shared **Add to scene** dialog and
prefab placement, preserving the whole hierarchy. Saving alone does not add scene
instances. Model Parts' **Open Object Library** link respects unsaved/busy guards.

Two owned manifests (`parts.toml` and `model.prefab.toml`) are rewritten together,
so their old references do not block removal of a part during that save. References
from scenes or other prefabs still block removal. The browser test also found and
fixed the existing plane-cut name suffix: `X+` was rejected by the saved-part name
validator; generated halves now use the valid `X high` suffix (likewise Y/Z).

## Validation

- `npx tsx --conditions=react-server --test tests/model-parts-library.test.ts tests/model-split.test.ts tests/prefabs.test.ts`:
  **11 passed**. Library discovery, one tile per saved set, descriptor routing,
  missing/invalid library entries, part validation and shared prefab remapping.
- Targeted ESLint on model saving, Object Builder page/component, Model Parts,
  tests and both affected browser smokes: passed.
- TypeScript covering IDE source, tests and scripts with existing generated output
  directories excluded: passed. The normal config still includes stale
  `.next-charcoal-dev` route validators from a removed `/textures` page.
- The final source passed a production build in a clean `.parts-library-validation`
  snapshot using the installed dependencies and fresh generated types. Two existing
  filesystem-tracing warnings in `inspection-session.ts` remained.
- `EMERGENT_PROJECT_PATH=<fresh-fixture> npx tsx scripts/model-parts-library-smoke.ts`
  against that production server on **3001**: passed. A fresh independent project
  from `templates/standard`, engine-owned fixtures and a synthetic cube was prepared
  using `npx tsx scripts/model-box-cut-smoke.ts --prepare`. Checks covered:
  - plane split into two parts, valid generated names, native-validated library save;
  - renamed part and repeated save retaining the prefab ID;
  - exactly one library tile, with click reopening the saved names in Model Parts;
  - part removal/resave despite references in the two owned manifests;
  - complete prefab placement from the Object Library, with fresh scene IDs and
    correct parent links;
  - refusal to delete a scene-referenced part, leaving the library prefab unchanged;
  - no page errors or paid provider calls.
- `/tmp/emergent-model-parts-library.png` was visually inspected.

The port 3001 test service and clean snapshot/build were removed. The user service
and real project assets were not used. No runtime engine changes or dependencies.

Existing limits remain: placed prefabs are expanded copies. Later hierarchy edits
do not automatically resynchronize those scene instances; referenced part deletion
is guarded. Model tiles use the shared prefab icon rather than a new thumbnail
rendering pipeline.
