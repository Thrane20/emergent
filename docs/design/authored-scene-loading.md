# First authored-scene loader

The engine owns a small schema-1 scene loader. Scenes list explicit instances
(`id`, project-relative `asset`, optional `parent`); entity assets define a
transform and either a procedural primitive, an existing light, or an empty
hierarchy node. Scene-instance transform overrides now replace individual asset vectors; see the scene-transform-inspector design. This slice does not load nested compositions, arbitrary behaviour
ports, meshes, textures or GLB. Unsupported fields/types fail rather than being
silently ignored. Multiple instances may share one asset, but instance IDs are
unique within the scene.

Parse and validate all definitions, finite transforms, primitive/light parameters,
references and hierarchy cycles before touching a world. Bound files to 256 KiB,
scenes to 1,024 instances, and total primitive tessellation to a conservative
subdivision limit for this first loader. Primitive dimensions are capped at 1,000,000 to keep tessellation arithmetic finite. Project path checks apply to every asset.
Instantiation creates fresh canonical SlotMap entities under one root; on failure
remove the candidate subtree. A runtime lookup maps authored IDs to handles; it
does not replace World identity. Geometry is a copied CPU component, and rendering
owns its GPU handles. Loading and tessellation occur once during construction;
frame extraction reuses dense instance/light buffers.

The reference project retains its moving-chain behaviour builder as a project
extension. The builder only adds that chain; authored files own the ground,
pyramid, cube and lights. Both native Run and tick-zero preview use the same scene
construction and extraction. The cube's spin hook resolves its optional authored
instance ID; empty/custom scenes remain usable without the reference objects.

The IDE can include supported newly created entities in the selected scene.
Asset creation remains safe if scene inclusion fails: keep the new definition and
report the inclusion error explicitly. Existing source edits are never replaced
with generated scene defaults. Validate loader errors headlessly and verify both
native and offscreen paths, including changed asset values and failed reloads.
