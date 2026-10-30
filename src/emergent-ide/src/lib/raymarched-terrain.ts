import {isTerrainBiome,type TerrainBiome} from "./terrain-biomes";
export type RaymarchedSettings={version:number;seed:number;size:number;biome_scale:number;relief:number;overhang:number;cave_radius:number;biome:TerrainBiome|"blend"};
export const DEFAULT_RAYMARCHED:RaymarchedSettings={version:2,seed:42,size:1024,biome_scale:180,relief:120,overhang:6,cave_radius:8,biome:"hilly"};
export function raymarchedBiome(settings:RaymarchedSettings,biome:TerrainBiome):RaymarchedSettings {
 const recipe={...settings,version:2,size:Math.max(64,settings.size),biome};
 switch(biome){case "plain":return {...recipe,relief:24,overhang:0,cave_radius:0};case "hilly":return {...recipe,relief:80,overhang:2,cave_radius:0};case "mountainous":return {...recipe,relief:140,overhang:8,cave_radius:6};case "cave-cavern":return {...recipe,relief:120,cave_radius:16,overhang:10};case "canyon":return {...recipe,relief:120,overhang:4,cave_radius:4};case "underwater":return {...recipe,relief:80,overhang:4,cave_radius:4};case "island":return {...recipe,relief:100,overhang:3,cave_radius:0};case "rivers":return {...recipe,relief:80,overhang:2,cave_radius:0};}
}
export function raymarchedSettings(value:unknown):RaymarchedSettings {
 if(!value||typeof value!=="object"||Array.isArray(value))throw new Error("Choose ray-marched terrain settings.");
 const v=value as Record<string,unknown>,version=v.version??1;
 if(Object.keys(v).some(key=>!Object.keys(DEFAULT_RAYMARCHED).includes(key))||![1,2].includes(version as number)||typeof v.seed!=="number"||!Number.isInteger(v.seed)||v.seed<0||v.seed>4294967295)throw new Error("Choose a valid terrain version and integer seed 0–4294967295.");
 const settings={...DEFAULT_RAYMARCHED,...v,version,biome:v.biome??"blend"} as RaymarchedSettings;
 if(settings.biome!=="blend"&&!isTerrainBiome(settings.biome))throw new Error("Choose a supported terrain biome.");
 for(const [key,min,max] of [["size",version===1?8:64,version===1?64:4096],["biome_scale",80,512],["relief",8,160],["overhang",0,24],["cave_radius",0,16]] as const){const n=settings[key];if(typeof n!=="number"||!Number.isFinite(n)||n<min||n>max)throw new Error(`${key.replaceAll("_"," ")} must be ${min}–${max} m.`);}
 return settings;
}
