import assert from "node:assert/strict";
import test from "node:test";
import {TERRAIN_BIOMES,isTerrainBiome} from "../src/lib/terrain-biomes";
import {DEFAULT_RAYMARCHED,raymarchedBiome,raymarchedSettings} from "../src/lib/raymarched-terrain";
import {TERRAIN_PRESETS,terrainSettings,DEFAULT_TERRAIN} from "../src/lib/terrain";
test("all modal biomes map to validated fields and cave is explicitly volumetric",()=>{
 assert.equal(TERRAIN_BIOMES.length,8);assert.equal(new Set(TERRAIN_BIOMES.map(b=>b.id)).size,8);
 for(const biome of TERRAIN_BIOMES){assert.ok(isTerrainBiome(biome.id));const recipe=raymarchedBiome(DEFAULT_RAYMARCHED,biome.id);assert.equal(raymarchedSettings(recipe).biome,biome.id);if(biome.id!=="cave-cavern"){assert.ok(TERRAIN_PRESETS.some(p=>p.id===biome.id));assert.equal(terrainSettings({...DEFAULT_TERRAIN,preset:biome.id}).preset,biome.id);}}
 assert.equal(raymarchedSettings({version:2,seed:42,size:1024}).biome,"blend");assert.throws(()=>raymarchedSettings({...DEFAULT_RAYMARCHED,biome:"missing"}));assert.throws(()=>terrainSettings({...DEFAULT_TERRAIN,preset:"cave-cavern"}));
});
