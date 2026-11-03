# Object Builder library, inspector and AI conversation

Validated on macOS, 3 November 2026, in a disposable standard-template project
with the engine-owned editor fixtures. The user app on port 3000 was untouched;
browser checks ran on port 3001. Test service stopped after validation.

Delivered:

- Shared top carousel with New object, saved geometry previews and selection.
  Swatches are static isometric SVG projections of composed polygon geometry,
  not material renders or separately animated WebGL scenes.
- Main viewport and primitive palette. The Make it with AI conversation now sits
  at the bottom of the right column, collapsed by default, freeing height for the viewport.
- Independent right-hand Object graph, Transform (position/rotation/scale plus
  face/point tools) and Material cards, preserving hierarchy and editing controls.
- Save updates the existing entity/sidecar through native candidate validation,
  without inserting an instance. Right-click or Shift+F10 on any saved tile opens
  a native Add to scene dialog with destination selection and Cancel. It places
  the saved asset through the shared revision-guarded API, retaining unrelated
  unsaved editor drafts.
- AI can answer questions or compose up to 128 primitive parts from a blank
  unsaved draft. Follow-ups receive the current draft and the last 12 messages.
  Existing edited meshes can be retained by durable ID. Proposals pass the
  structure validator and enter local undo/redo; only Save writes files.

Checks:

- TypeScript no-emit, targeted ESLint and the production Next.js build passed.
- `pnpm exec tsx --conditions=react-server --test tests/structure.test.ts tests/structure-ai.test.ts tests/structure-assets.test.ts`: **8 passed**.
- `scripts/object-builder-smoke.ts`: browser AI-from-empty and follow-up (simulated
  response), undo/redo, right-hand card order, native Save, unchanged scene after
  Save, carousel preview image, cancel/right-click/keyboard placement while a
  different unsaved draft stays intact, native scene insertion and save/reopen
  across reload. No browser page errors. Desktop and narrow screenshots reviewed.
- Server route test exercises the real provider adapter with a simulated Responses
  endpoint, question-only replies, invalid proposals, wrong-project/origin guards,
  and unchanged scene files.

Paid AI calls and the full engine suite were not rerun. Existing runtime object
ownership, mesh compilation and SlotMap identities are unchanged. AI material
editing, per-part materials and boolean mesh operations are outside this slice.

Layout follow-up: the AI panel is pinned below the scrollable attributes and starts
collapsed. Expanded chat reuses home screen message/composer styles, with a larger
scrolling transcript, settings link, pending response, Enter-to-send and Shift+Enter
newlines. Collapse keeps history and unsent text. TypeScript and targeted lint pass.
Browser smoke passed again for this follow-up, including collapsed initial state,
expand/collapse, retained unsent text and history, AI follow-up, Save and native
scene placement. Expanded desktop chat was visually reviewed.

Transform-space follow-up: Local/Global numeric controls and matching viewport
axis space were added. The AI proposal contract now treats primitive dimensions
as mesh geometry, so scaled parents no longer compress new descendants. An
undoable Bake part sizes action repairs authored drafts. Focused tests cover
nested scale, world/local conversion, shear rejection and baking. The existing
launcher draft was inspected read-only and was not saved or modified.

Line and group editing follow-up (2026-11-03): the viewport and inspector select polygon boundary lines; Shift-click selects multiple object parts; Backspace/Delete removes selected subtrees and D duplicates them. Right-click on a multi-selection opens transform, group-under-new-parent and delete actions. Group parents contain no mesh geometry. TypeScript compile and 10 focused structure tests passed, including line motion, hierarchy-safe duplication/deletion, shared-pivot rotation and unchanged world transforms after grouping. Native `empty_structure_parent_keeps_child_geometry` passed; Rust formatting was applied. Browser interaction was not exercised for this follow-up.

Multi-selection correction (2026-11-03): the viewport keeps all selected parts active while dragging one, attaches a common transform handle at the selected roots' midpoint, and opens the compact group dialog from a right-click on a selected viewport object. New group parents stay under the selected siblings' current parent; mixed-parent selections receive an explicit error. TypeScript, targeted lint, and the 10 focused structure tests passed. A disposable template project on port 3001 confirmed two selected cubes move together by the same X delta (0 to 0.66761 m and 1.25 to 1.91761 m), viewport right-click opens the group dialog, and the 278 px dialog was visually reviewed. The test service was stopped.
