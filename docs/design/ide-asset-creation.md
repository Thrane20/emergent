# IDE asset creation

## Scope

The first asset-creation slice lets a developer choose an Emergent foundation
from a categorized tree, name it, and create a durable TOML definition inside
the linked project's declared asset root. It does not compile generated Rust,
mutate a running world, or claim that every authored definition is loaded by
the current runtime.

## Ownership and identity

- The game project owns the created file and its durable, kebab-case `id`.
- The IDE owns the template catalogue and validates all creation requests on
  the server. A client cannot supply a destination path or arbitrary content.
- Runtime subsystems remain responsible for their SlotMap storage. Authored IDs
  are resolved to typed generational handles when a loader is implemented;
  SlotMap keys are never written to project assets.
- Files are created with exclusive-create semantics. An existing definition is
  never silently replaced.

## Data flow

1. The modal presents templates grouped by foundation category and shows the
   matching capability status: available, preview, or planned.
2. The developer chooses a template and enters a display name.
3. The browser sends only the linked project ID, template ID, and name to the
   asset route.
4. The route resolves the known template, derives a safe durable ID and path,
   verifies the path is inside a declared project asset root, and exclusively
   creates the TOML definition without replacing an existing file.
5. The IDE refreshes its project snapshot and selects the new file in the
   existing file and connection views.

## Performance and scale

This is editor-only work and never executes on a frame-critical thread. The
catalogue is a small static array and creation writes one bounded text file.
The existing project scan remains the current scaling limit; incremental file
watching is deliberately deferred until project sizes justify it.

## Capability honesty

`available` means the underlying engine API exists, not that the generic
authored-asset loader is complete. `preview` denotes an engine capability whose
contract can still change. `planned` definitions are useful design graph nodes
but require later engine and loader work before a game can execute them.

## Validation

- Reject blank or un-sluggable names, unknown project IDs, unknown templates,
  traversal attempts, and duplicate targets.
- Exercise creation against a disposable linked-project copy so repository
  assets are not polluted by validation.
- Run IDE lint and production build after the implementation.
