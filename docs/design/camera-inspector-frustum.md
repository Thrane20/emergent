# Camera authoring and frustum inspection

Extend the existing Camera component and schema-1 scene assets; retain canonical
EntityId ownership. Camera instance settings override the asset as a complete
projection definition, just like material settings. The editor persists through
the existing revision-guarded native scene validator. Orthographic is the UI name
for the existing `isometric` projection, with `orthographic` accepted as a schema
alias. Orthographic orientation remains an ordinary transform, not a fixed angle.

The inspector edits FOV (perspective), view height (orthographic), near and far.
Successful saves update the retained inspection Camera component without changing
the editor view or advancing gameplay. Asset settings remain editable separately.

Frustum corners are bounded stack values derived from the same projection fields
as rendering. The reference inspection tool projects the selected camera's twelve
edges, clips segments against all six WebGPU clip planes, and sends at most twelve
screen-space segments through the existing acknowledged tool overlay protocol.
Camera scale does not stretch the viewing volume; use the renderer's orthonormal
view basis. This authoring overlay creates no geometry resources, new ownership
graph or normal-gameplay drawing work. Aspect ratio matches the current 960×540
viewport; target-specific aspect controls remain outside this slice.

Asset-modal search matches tokens against category/name/id/description and expands
all matching categories while typing. Clear restores the normal collapsible tree.
Verify filtered visibility, instance/asset persistence, projection validation,
clip-plane crossings, hierarchy transforms, live frustum updates and Run loading.
