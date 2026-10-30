export const TERRAIN_BIOMES=[
 {id:"cave-cavern",name:"Cave / Cavern",symbol:"∩",description:"Enclosed rock chambers and connected tunnels. Requires Ray March.",volumetric:true},
 {id:"canyon",name:"Canyon",symbol:"⋁",description:"Steep-sided winding valleys cut through elevated plateaus."},
 {id:"underwater",name:"Underwater / Seabed",symbol:"≈",description:"Low rolling seabed and submerged rock. Terrain only; no water simulation."},
 {id:"plain",name:"Field / Plain",symbol:"▱",description:"Open, gently undulating ground with low relief."},
 {id:"hilly",name:"Hilly",symbol:"∿",description:"Rolling hills and shallow valleys."},
 {id:"mountainous",name:"Mountainous",symbol:"△",description:"Mountain ridges, exposed rock and high summits."},
 {id:"island",name:"Island",symbol:"◉",description:"Raised interior falling towards a low coastline."},
 {id:"rivers",name:"Rivers",symbol:"≋",description:"Winding carved river valleys. Water comes separately."},
] as const;
export type TerrainBiome=typeof TERRAIN_BIOMES[number]["id"];
export function isTerrainBiome(value:unknown):value is TerrainBiome{return TERRAIN_BIOMES.some(b=>b.id===value);}
