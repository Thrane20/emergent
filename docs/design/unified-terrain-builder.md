# Unified Terrain Builder and live implicit preview

Height Field / Ray March is one renderer switch at the top of Terrain Builder.
Both use the same biome modal: cave/cavern, canyon, underwater/seabed, field/plain,
hilly, mountainous, island and rivers. Cave/cavern requires a volumetric field and
is explicitly unavailable for heightfields. Underwater means seabed generation;
this increment does not introduce a water simulation. Existing saved presets and
implicit recipes without a biome retain their prior output.

Keep backend-specific authoring components, save transactions and identity intact;
share the method/biome controls rather than merge unrelated height/paint and density
state. Switching methods preserves chosen compatible biome/scene and protects drafts.

Live Ray March preview reuses InspectionSessions process ownership, framed PNG
protocol, bounded command queues, lease/reaping and InspectionViewport orbit/pan/
zoom controls. Add an engine example worker with one persistent renderer; create
its field once and update camera uniforms as commands arrive. A private validated
terrain launch mode binds the worker to the current project without writing draft
assets or running gameplay. Replacing a draft stops/reaps its predecessor; unmount
stops the session and lease expiry reaps abandoned clients. No process per gesture.

World/renderer SlotMap ownership for saved scenes remains unchanged. Native field
and WGSL biome branches must agree, including conservative distance bounds; bump
no legacy output by default. Add bounded presets to heightfield generation, retain
existing four algorithms exactly, and test legacy fields, new biome distinction,
persisted choices, native depth/coverage and actual browser live orbit changes.
