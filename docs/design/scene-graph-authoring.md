# Scene graph authoring, coordinate space and TOML editing

Merge scene instances and files into one left panel, with the selected scene's
hierarchy as the main workflow and project files under a collapsed section.
Dragging an instance onto another reparents it; dropping at Scene root clears its
parent. Provide a Parent selector for keyboard access. Reject stale drag data,
cycles, self-parenting and missing nodes before publication. Reparenting preserves
the local transform, matching World::reparent. Parent translation, rotation and
scale affect descendants through the existing SlotMap transform graph.

A header Local/Global switch controls inspector display/edit space. Persist only
parent-local TRS. Use Rust/glam math and the actual World graph for conversion;
do not create a second browser transform system. Global transforms that cannot
be decomposed without shear expose world position only; changing world rotation
or scale that would require local shear fails clearly rather than approximating.
All conversions and graph edits happen in authoring operations, outside ticks.

Persist through existing source revision checks, Rust candidate validation and
guarded publication. The persistent inspection worker receives an acknowledged
reparent command and propagates transforms at a command boundary, retaining its
world/geometry. Inspector Global Apply sends the resolved local transform.

Add a TOML tab beside Overview, Connections and Source. For a selected instance,
choose scene overrides or its shared entity asset. Edit ordinary whole-file TOML,
with clear scope, Save and Reload. Validate a staged project copy using the
existing candidate-check mechanism, including all authored scenes and a synthetic
single-instance scene for an unreferenced entity. Reject invalid schemas/graphs,
unsafe paths and concurrent changes. Source edits need Load/Restart inspection;
they do not silently alter a running world. Guard unsaved drafts during navigation.

Validation covers translated/rotated/scaled parents, Global conversion and shear,
hierarchy cycles/stale edits, independent instances, keyboard and drag parenting,
raw TOML save/rejection/conflicts, and live worker identity across reparenting.
No runtime SlotMap keys are persisted; authored IDs remain scene-local.
