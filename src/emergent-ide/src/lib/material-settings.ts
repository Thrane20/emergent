export type TextureMap={asset:string;scale:number};
export type MaterialLayer={id:string;name:string;diffuse_map?:TextureMap};
export type MaterialMap={asset:string;strength:number};
export type MaterialSettings={asset?:string;terrain_paint?:string;shading:"flat"|"smooth"|"unlit";cast_shadows:boolean;receive_shadows:boolean;albedo?:number[];specular:number[];shininess:number;emission:number[];emission_intensity:number;bump_strength:number;noise_resolution:number;diffuse_map?:TextureMap;terrain_layers:MaterialLayer[];active_layer?:string;albedo_map?:MaterialMap;specular_map?:MaterialMap;emission_map?:MaterialMap;bump_map?:MaterialMap};
export const materialChannels=["albedo","specular","emission","bump"] as const;
export function materialSettings(value:unknown):MaterialSettings {
  if(value!==undefined&&(!value||typeof value!=="object"||Array.isArray(value)))throw new Error("Expected material settings.");
  const input=(value??{}) as Record<string,unknown>;
  if(input.asset!==undefined){
    if(typeof input.asset!=="string"||!input.asset.endsWith(".material.toml")||input.asset.length>512||Object.keys(input).some(key=>key!=="asset"))throw new Error("Choose a library material without inline overrides.");
    return {asset:input.asset} as MaterialSettings;
  }
  if(input.terrain_paint!==undefined&&(typeof input.terrain_paint!=="string"||!input.terrain_paint.endsWith(".paint.toml")||input.terrain_paint.length>512))throw new Error("Invalid terrain paint asset.");
  const result={shading:"flat",cast_shadows:true,receive_shadows:true,specular:[0,0,0],shininess:32,emission:[0,0,0],emission_intensity:0,bump_strength:0.1,noise_resolution:32,terrain_layers:[],...input} as MaterialSettings;
  if(Object.keys(input).some(key=>!["asset","terrain_paint","shading","cast_shadows","receive_shadows","albedo","specular","shininess","emission","emission_intensity","bump_strength","noise_resolution","diffuse_map","terrain_layers","active_layer",...materialChannels.map(c=>`${c}_map`)].includes(key))||!["flat","smooth","unlit"].includes(result.shading))throw new Error("Invalid material field or shading mode.");
  if(typeof result.cast_shadows!=="boolean"||typeof result.receive_shadows!=="boolean")throw new Error("Shadow options must be booleans.");
  if(![32,64,128].includes(result.noise_resolution))throw new Error("Material noise resolution must be 32, 64 or 128.");
  for(const key of ["albedo","specular","emission"] as const){const rgb=result[key];if(key==="albedo"&&rgb===undefined)continue;if(!Array.isArray(rgb)||rgb.length!==3||!rgb.every(n=>typeof n==="number"&&Number.isFinite(n)&&n>=0&&n<=1))throw new Error(`${key} requires three linear colour values in 0..1.`);}
  for(const [key,min,max] of [["shininess",1,256],["emission_intensity",0,100],["bump_strength",0,2]] as const){const n=result[key];if(typeof n!=="number"||!Number.isFinite(n)||n<min||n>max)throw new Error(`Invalid material ${key}.`);}
  for(const channel of materialChannels){const map=result[`${channel}_map`];if(map!==undefined){if(!map||typeof map!=="object"||Array.isArray(map)||Object.keys(map).some(key=>!["asset","strength"].includes(key))||typeof map.asset!=="string"||!map.asset.endsWith(".noise.toml")||map.asset.length>512)throw new Error(`Choose a noise filter for ${channel}.`);const strength=map.strength??1;if(typeof strength!=="number"||!Number.isFinite(strength)||strength<0||strength>1)throw new Error("Map strength must be in 0..1.");result[`${channel}_map`]={asset:map.asset,strength};}}
  if(!Array.isArray(result.terrain_layers)||result.terrain_layers.length>8)throw new Error("Use at most eight terrain material layers.");
  const ids=new Set<string>();
  for(const layer of result.terrain_layers){
    if(!layer||typeof layer!=="object"||Array.isArray(layer)||Object.keys(layer).some(key=>!["id","name","diffuse_map"].includes(key))||typeof layer.id!=="string"||!/^[a-z0-9_-]{1,64}$/i.test(layer.id)||ids.has(layer.id)||typeof layer.name!=="string"||!layer.name.trim()||layer.name.length>64||/[\p{Cc}\p{Cf}]/u.test(layer.name))throw new Error("Material layers need unique IDs and names of 1–64 printable characters.");ids.add(layer.id);
  }
  if(result.active_layer!==undefined&&(typeof result.active_layer!=="string"||!ids.has(result.active_layer)))throw new Error("Choose an existing terrain material layer.");
  for(const map of materialTextures(result)){
    if(!map||typeof map!=="object"||Array.isArray(map)||Object.keys(map).some(key=>!["asset","scale"].includes(key))||typeof map.asset!=="string"||!isTexturePath(map.asset)||map.asset.length>512)throw new Error("Choose a PNG or JPEG project texture.");
    const scale=map.scale??1;if(typeof scale!=="number"||!Number.isFinite(scale)||scale<0.001||scale>100)throw new Error("Texture repeat must be in 0.001..100.");map.scale=scale;
  }
  return result;
}
export function linearToHex(rgb:number[]):string {return "#"+rgb.map(value=>{const n=Math.min(1,Math.max(0,value));return Math.round((n<=0.0031308?12.92*n:1.055*Math.pow(n,1/2.4)-0.055)*255).toString(16).padStart(2,"0");}).join("");}
export function hexToLinear(hex:string):number[]{return [1,3,5].map(i=>{const n=parseInt(hex.slice(i,i+2),16)/255;return n<=0.04045?n/12.92:Math.pow((n+0.055)/1.055,2.4);});}

export const isTexturePath=(path:string)=>/\.(png|jpe?g)$/i.test(path);
export function materialTextures(material:MaterialSettings):TextureMap[]{return [material.diffuse_map,...(material.terrain_layers??[]).map(layer=>layer.diffuse_map)].filter((map):map is TextureMap=>map!==undefined);}
