# Statement-driven project implementation

Saving an intention immediately checks existing project evidence. It does not
assume missing implementation: an existing behaviour may already satisfy it.
The statement remains saved if checking is unavailable. Implement is an explicit
authoring action for an unmet intention, followed by another independent check.

The IDE searches the current project's source/assets and reads the linked
Emergent crate's public exports, relevant implementation source, capability
catalogue, and contributor guidance. The model must cite inspected reuse points
and reuse existing behaviours before introducing a small project-owned module.
Runtime SlotMap identity and ownership remain unchanged. This is source generation,
not executable statements or new frame-loop infrastructure.

The configured Responses connection returns bounded structured file replacements
and reuse evidence. Allowed writes are project Rust source/tests and TOML assets;
engine source, dependencies, credentials, manifests, deletion, and arbitrary
commands are outside this action. Read/resolve actual paths and reject symlinks
that escape the linked project. Source hashes prevent overwriting concurrent edits.

Before applying, copy the project into an isolated temporary directory, rewrite
only its existing relative Cargo dependency paths to their original absolute
locations, and run cargo check there. This validates compilation without running
the generated game/tests or overwriting live files. The current independent
project-manifest contract is supported; workspace-inherited manifests fail with
an actionable diagnostic. Existing dependencies and caches are reused.

Keep original-file backups and a change journal in the private IDE data directory.
Only publish after compilation and unchanged-source checks. Roll back failed
publication only where the file still matches our own write, preserving concurrent
user edits. A project-wide implementation lock prevents overlapping AI changes.
After publication, reload the project and independently evaluate the statement.
Compilation and AI evidence are reported separately from runtime verification.

Tests cover reuse-first context, response/path validation, immediate checking,
failure persistence, conflicts and rollback, staged compile failure, and UI actions.
The existing slow-Z-spin statement is the reference integration scenario. The
paused tick-zero scene capture cannot demonstrate rotation; native Run can.

## Reference evidence

The existing cube statement was sent through the live configured AI connection.
It generated `src/spin.rs` and wired the authored cube through the generated scene,
fixed update, native renderer and offscreen preview. Its proposal cited the
existing fixed timestep, canonical transform APIs, typed entity handles, primitive
generator and render extraction. The staged Cargo check passed, and a separate AI
evaluation found the statement implemented. Manual review removed an unused
generated constant and applied rustfmt; the result is not claimed as unattended.

Focused Rust tests verify 15° of rotation after 60 fixed ticks and preservation of
translation/scale. Debug/release tests, workspace Clippy and the 120-tick native
Metal smoke pass. The IDE tests verify discovery of an existing spin helper, bad
reuse citations, invalid paths/symlinks, compile failure, publication conflicts and
rollback. A production browser smoke verifies immediate checks, preservation of
new statements when checking fails, and the explicit Implement action.
