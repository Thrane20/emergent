import {falModelId} from "./fal-model-types";
export const TEXTURE_ENDPOINT="fal-ai/flux/schnell";
export const TEXTURE_GENERATION_SETTINGS={image_size:"square_hd",num_inference_steps:4,output_format:"png",enable_safety_checker:true,sync_mode:false} as const;
export type TextureRequest={name:string;prompt:string;seed:number;count:number;tileFriendly:boolean;kind?:"image"|"sprite"|"skybox";model?:string};
export type TextureResult={id:string;index:number;file:string;saveName:string|null;savedPath:string|null};
export type TextureJob={schema_version:1;id:string;projectId:string;input:TextureRequest;submittedPrompt:string;endpoint:string;settings?:Record<string,unknown>;state:"submitting"|"uncertain"|"queued"|"generating"|"downloading"|"ready"|"failed";requestId:string|null;createdAt:string;updatedAt:string;error:string|null;outputs:TextureResult[]};
export function textureRequest(value:unknown):TextureRequest{
 if(!value||typeof value!=="object"||Array.isArray(value))throw new Error("Texture generation settings are required.");
 const v=value as Record<string,unknown>;
 if(typeof v.name!=="string"||!v.name.trim()||v.name.trim().length>64||/[\p{Cc}\p{Cf}]/u.test(v.name))throw new Error("Use a texture name of 1–64 printable characters.");
 if(typeof v.prompt!=="string"||v.prompt.trim().length<3||v.prompt.length>2000||/\u0000/.test(v.prompt))throw new Error("Describe the texture in 3–2,000 characters.");
 if(typeof v.seed!=="number"||!Number.isInteger(v.seed)||v.seed<0||v.seed>2147483647)throw new Error("Seed must be an integer in 0..2147483647.");
 if(typeof v.count!=="number"||!Number.isInteger(v.count)||v.count<1||v.count>4||typeof v.tileFriendly!=="boolean")throw new Error("Choose 1–4 texture candidates and a tile-friendly preference.");
 if(v.kind!==undefined&&v.kind!=="image"&&v.kind!=="sprite"&&v.kind!=="skybox")throw new Error("Choose Image, Sprite or Skybox.");
 return {name:v.name.trim(),prompt:v.prompt.trim(),seed:v.seed,count:v.count,tileFriendly:v.tileFriendly,...(v.model===undefined?{}:{model:falModelId(v.model)}),...(v.kind===undefined?{}:{kind:v.kind as "image"|"sprite"|"skybox"})};
}
export function texturePrompt(input:TextureRequest){return input.prompt+(input.kind==="skybox"?". Full 360-degree equirectangular environment panorama, 2:1 aspect ratio, level horizon at the vertical center, sky above and ground below, continuous left and right edges, no text, no borders, no panorama frame.":input.tileFriendly?". Seamless tileable game diffuse surface texture, flat orthographic top-down view, even neutral lighting, no perspective, no objects, no text or borders.":"");}
