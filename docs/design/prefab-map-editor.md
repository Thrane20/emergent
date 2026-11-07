# Prefab maps

The map editor can open either a scene or a single-root prefab. A prefab's root is the
local origin for authoring; new markers belong to that root by default. The editor
draws the prefab in its own XZ coordinates. When the game instantiates the prefab,
the existing `World` hierarchy resolves each marker's local transform to a global
transform. Waypoint links use prefab-local durable IDs in the asset and fresh
`EntityId` handles in each runtime copy.

Ownership stays with `World` and its typed handles. The prefab owns only authored
entity entries. The runtime `SceneMap` continues to read global transforms from
the world; it has no prefab-specific storage or frame work. Loading and outline
generation remain editor/setup work. One prefab can contain at most 1,024 members
and 256 KiB of TOML, matching the current scene/prefab contract.

The map page selects an authored graph, uses the same gesture and inspector tools,
and writes the selected file through a revision guard, native candidate validation,
journal and undo. Prefab mode cannot remove its root or edit scene wind. Placement,
links, zones and metadata remain ordinary components, including airport tags and
game-specific names such as truck drop-off. Scene placement is currently a snapshot:
edits to a prefab after a copy was placed do not update that already-expanded copy.

Validation: author a prefab path and zone, reload its top-down layout, instantiate
two copies at different transforms, and verify world-space positions and links stay
independent. Cover stale revisions, root deletion, undo and scene-mode regression.
