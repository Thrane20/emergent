export type LightingSettings={ambient_color:number[];ambient_intensity:number;background_color:number[];skybox:string|null;skybox_rotation:number;skybox_intensity:number};
export const DEFAULT_LIGHTING:LightingSettings={ambient_color:[1,1,1],ambient_intensity:.18,background_color:[.025,.035,.06],skybox:null,skybox_rotation:0,skybox_intensity:1};
export function lightingSettings(value:unknown):LightingSettings{
 if(value===undefined)return {...DEFAULT_LIGHTING};
 if(!value||typeof value!=="object"||Array.isArray(value))throw new Error("Lighting settings must be a table.");
 const v={...DEFAULT_LIGHTING,...value} as LightingSettings;
 if(Object.keys(value).some(key=>!Object.hasOwn(DEFAULT_LIGHTING,key)))throw new Error("Unknown lighting setting.");
 for(const color of [v.ambient_color,v.background_color])if(!Array.isArray(color)||color.length!==3||!color.every(n=>typeof n==="number"&&Number.isFinite(n)&&n>=0&&n<=1))throw new Error("Lighting colours require three RGB values in 0..1.");
 for(const [key,max] of [["ambient_intensity",10],["skybox_intensity",10],["skybox_rotation",360]] as const)if(typeof v[key]!=="number"||!Number.isFinite(v[key])||v[key]<0||v[key]>max)throw new Error(`${key} must be in 0..${max}.`);
 if(v.skybox!==null&&(typeof v.skybox!=="string"||v.skybox.length>512||!/[.](png|jpe?g)$/i.test(v.skybox)||v.skybox.includes("\\")||v.skybox.includes(":")||v.skybox.split("/").some(p=>!p||p==="."||p===".."||p.startsWith("."))))throw new Error("Choose a project PNG/JPEG skybox.");
 return v;
}
