export type CapabilityStatus = "available" | "preview" | "planned";

export type AssetTemplate = {
  id: string;
  category: string;
  label: string;
  capability: string;
  status: CapabilityStatus;
  description: string;
  directory: string;
  extension: string;
  kind: string;
  definition: string;
};

const transform = `[transform]
translation = [0.0, 0.0, 0.0]
rotation_degrees = [0.0, 0.0, 0.0]
scale = [1.0, 1.0, 1.0]`;

const color = "color = [1.0, 1.0, 1.0, 1.0]";

export const assetTemplates: readonly AssetTemplate[] = [
  primitive("cube", "Cube", "half_extent = 0.5"),
  primitive("pyramid", "Pyramid", "half_extent = 0.5\nheight = 1.0"),
  primitive("torus", "Torus", "major_radius = 1.0\nminor_radius = 0.25\nmajor_segments = 32\nminor_segments = 16"),
  primitive("plane", "Plane", ""),
  primitive("sphere", "Sphere", "radius = 0.5\nsectors = 24\nstacks = 12"),
  primitive("cylinder", "Cylinder", "radius = 0.5\nhalf_height = 0.5\nsegments = 24"),
  primitive("cone", "Cone", "radius = 0.5\nheight = 1.0\nsegments = 24"),
  primitive("capsule", "Capsule", "radius = 0.5\nhalf_segment = 0.5\nsegments = 24\nhemisphere_rings = 8"),
  geometry("point", "Point", "position = [0.0, 0.0, 0.0]\nradius = 0.05"),
  geometry("line", "Line", "start = [-0.5, 0.0, 0.0]\nend = [0.5, 0.0, 0.0]\nradius = 0.025"),
  geometry("polygon", "Convex polygon", "vertices = [[-0.5, 0.0, 0.0], [0.5, 0.0, 0.0], [0.0, 0.75, 0.0]]"),
  geometry("mesh", "Indexed mesh", "vertices = []\nindices = []"),
  light("ambient", "Ambient light", "intensity = 0.2\ncolor = [1.0, 1.0, 1.0]"),
  light("directional", "Directional light", "intensity = 3.0\ncolor = [1.0, 1.0, 1.0]\ncasts_shadows = true"),
  light("point", "Point light", "intensity = 8.0\nrange = 24.0\ncolor = [1.0, 1.0, 1.0]\ncasts_shadows = false"),
  light("spot", "Spot light", "intensity = 8.0\nrange = 24.0\ninner_half_angle = 0.25\nouter_half_angle = 0.5\ncolor = [1.0, 1.0, 1.0]\ncasts_shadows = false"),
  { ...planned("scene.blank", "Scene", "Blank scene", "assets.authored-worlds", "assets/scenes", ".scene.toml", "scene", "entities = []"), status: "preview", description: "An empty loadable scene; include primitive, light or empty entity assets." },
  { ...planned("entity.blank", "Entity", "Blank entity", "assets.authored-worlds", "assets/entities", ".entity.toml", "entity", transform), status: "preview", description: "A loadable empty hierarchy node with an authored transform." },
  { id: "camera.perspective", category: "Cameras", label: "Perspective camera", capability: "render.cameras", status: "available", description: "A scene camera with perspective projection and an authored vertical field of view.", directory: "assets/entities", extension: ".entity.toml", kind: "camera", definition: `[transform]\ntranslation = [7.0, 5.0, 9.0]\nrotation_degrees = [-20.0, 38.0, 0.0]\nscale = [1.0, 1.0, 1.0]\n\n[camera]\nprojection = "perspective"\nfield_of_view_degrees = 60.0\nnear = 0.1\nfar = 1000.0` },
  { id: "camera.isometric", category: "Cameras", label: "Orthographic camera", capability: "render.cameras", status: "available", description: "An orthographic camera with an isometric starting angle and authored view size.", directory: "assets/entities", extension: ".entity.toml", kind: "camera", definition: `[transform]\ntranslation = [10.0, 10.0, 10.0]\nrotation_degrees = [-35.264, 45.0, 0.0]\nscale = [1.0, 1.0, 1.0]\n\n[camera]\nprojection = "isometric"\nview_size = 12.0\nnear = 0.1\nfar = 1000.0` },
  planned("terrain.heightmap", "Terrain", "Heightmap terrain", "terrain.heightmap", "assets/data/terrain", ".terrain.toml", "heightmap-terrain", "source = \"\"\nheight_scale = 32.0"),
  planned("terrain.polygonal", "Terrain", "Polygon terrain", "terrain.polygonal", "assets/data/terrain", ".terrain.toml", "polygon-terrain", "seed = 1\nchunk_size = 32"),
  {id:"procedural.noise",category:"Filters",label:"Noise function",capability:"procedural.noise",status:"preview",description:"Seeded Perlin, OpenSimplex or Worley fields with a grayscale 2D / 3D volume inspector.",directory:"assets/filters",extension:".noise.toml",kind:"noise-function",definition:'[noise]\nalgorithm_version = 1\nalgorithm = "perlin"\nseed = 1\ndimensions = 2\nsize = 128\nfrequency = 1.0\noctaves = 4\nlacunarity = 2.0\npersistence = 0.5\nextent = 8.0\noffset = [0.0, 0.0, 0.0]'},
  planned("material.basic", "Materials", "Material", "material.resources", "assets/data/materials", ".material.toml", "material", "base_color = [1.0, 1.0, 1.0, 1.0]\nroughness = 0.5\nmetallic = 0.0"),
  planned("texture.image", "Materials", "Texture reference", "texture.resources", "assets/data/materials", ".texture.toml", "texture", "source = \"\"\ncolor_space = \"srgb\""),
  {id:"effects.particle-emitter",category:"Effects",label:"Particle effect",capability:"effects.particles",status:"preview",description:"Layer smoke, fire, sparks and dust in the Particle Generator, then save and place a reusable effect.",directory:"assets/particles",extension:".particle.toml",kind:"particle-effect",definition:""},
  planned("audio.clip", "Audio", "Audio clip", "audio.clips-streams", "assets/audio", ".audio.toml", "audio-clip", "source = \"\"\nstreaming = false"),
  planned("audio.spatial-source", "Audio", "Spatial audio source", "audio.spatial", "assets/entities", ".entity.toml", "spatial-audio-source", `${transform}\n\n[audio]\nclip = \"\"\nvolume = 1.0\nlooping = false`),
  planned("input.action-map", "Input", "Input action map", "input.actions", "assets/data/input", ".input.toml", "input-action-map", "actions = []"),
];

export function assetTemplate(id: string): AssetTemplate | undefined {
  return assetTemplates.find((template) => template.id === id);
}

export function sceneCompatible(template: AssetTemplate): boolean {
  return template.kind === "primitive" || template.kind === "entity" || template.kind === "camera" || template.id.startsWith("light.");
}

export function assetSlug(name: string): string {
  return name
    .normalize("NFKD")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64);
}

export function assetPath(template: AssetTemplate, name: string): string {
  return `${template.directory}/${assetSlug(name)}${template.extension}`;
}

export function assetContents(template: AssetTemplate, name: string, id = assetSlug(name)): string {
  return `schema_version = 1
id = "${id}"
name = "${name}"
kind = "${template.kind}"
template = "${template.id}"
runtime_status = "${template.status}"

# Durable authored identity only. Runtime SlotMap handles are resolved later.
${template.definition}
`;
}

function primitive(id: string, label: string, fields: string): AssetTemplate {
  return {
    id: `object.${id}`,
    category: "Objects",
    label,
    capability: "geometry.primitives",
    status: "available",
    description: id==="plane"?"A 1 × 1 m plane. Set its width and depth with transform Scale X and Z.":`Authored ${label.toLowerCase()}. Dimensions are metres; half-extents are half the full size. Transform scale is a multiplier.`,
    directory: "assets/entities",
    extension: ".entity.toml",
    kind: "primitive",
    definition: `${transform}\n\n[geometry]\nprimitive = "${id}"\n${fields}\n${color}`,
  };
}

function geometry(id: string, label: string, fields: string): AssetTemplate {
  return {
    id: `geometry.${id}`,
    category: "Geometry",
    label,
    capability: id === "mesh" ? "geometry.meshes" : `geometry.${id}`,
    status: "available",
    description: `Authored ${label.toLowerCase()} backed by Emergent's validated geometry API.`,
    directory: "assets/entities",
    extension: ".entity.toml",
    kind: "geometry",
    definition: `${transform}\n\n[geometry]\ntype = "${id}"\n${fields}\n${color}`,
  };
}

function light(id: string, label: string, fields: string): AssetTemplate {
  return {
    id: `light.${id}`,
    category: "Lights",
    label,
    capability: "lighting.sources",
    status: "preview",
    description: `${label} using the preview bounded light-source contract.`,
    directory: "assets/entities",
    extension: ".entity.toml",
    kind: `${id}-light`,
    definition: `${transform}\n\n[light]\ntype = "${id}"\n${fields}`,
  };
}

function planned(
  id: string,
  category: string,
  label: string,
  capability: string,
  directory: string,
  extension: string,
  kind: string,
  definition: string,
): AssetTemplate {
  return {
    id,
    category,
    label,
    capability,
    status: "planned",
    description: `Creates a durable ${label.toLowerCase()} definition for the project graph; runtime loading is planned.`,
    directory,
    extension,
    kind,
    definition,
  };
}
