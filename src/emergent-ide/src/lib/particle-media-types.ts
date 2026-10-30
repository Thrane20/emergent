import {falModelId} from "./fal-model-types";
export const PARTICLE_VIDEO_ENDPOINT="fal-ai/wan/v2.2-5b/image-to-video";
export const PARTICLE_MATTE_ENDPOINT="bria/video/background-removal/v3";
export type ParticleMediaJob={schema_version:1;id:string;projectId:string;stage:"animate"|"matte"|"import";source:string;prompt:string;seed:number;endpoint:string;requestId:string|null;output?:"sprite";spriteFamily?:string;deleted?:boolean;state:"submitting"|"uncertain"|"queued"|"generating"|"baking"|"ready"|"failed";error:string|null;createdAt:string;settings?:Record<string,unknown>;file?:string;atlas?:{path:string;columns:number;rows:number;frames:number;fps:number;alpha:boolean}};
export function particleMediaRequest(value:unknown){
 if(!value||typeof value!=="object")throw new Error("Media request required.");const v=value as Record<string,unknown>;
 if(!["animate","matte"].includes(String(v.stage))||typeof v.source!=="string"||!v.source||v.source.length>512||typeof v.prompt!=="string"||v.prompt.length>2000||v.prompt.includes("\0")||typeof v.seed!=="number"||!Number.isInteger(v.seed)||v.seed<0||v.seed>2147483647)throw new Error("Choose an image or video job, prompt and integer seed.");
 if(v.stage==="animate"&&v.prompt.trim().length<3)throw new Error("Describe the particle motion.");
 if(v.output!==undefined&&(v.output!=="sprite"||v.stage!=="animate"))throw new Error("Choose a valid sprite animation output.");
 if(v.model!==undefined&&v.stage!=="animate")throw new Error("Model selection is only available for animation.");
 return {...(v.model===undefined?{}:{model:falModelId(v.model)}),...(v.output==="sprite"?{output:"sprite" as const}:{}),stage:v.stage as "animate"|"matte",source:v.source,prompt:v.prompt.trim(),seed:v.seed};
}
// Keep quality settings explicit so editor jobs do not silently use a lower-quality preset.
export const PARTICLE_VIDEO_SETTINGS={num_frames:81,frames_per_second:24,resolution:"720p",aspect_ratio:"auto",num_inference_steps:40,enable_safety_checker:true,enable_output_safety_checker:true,enable_prompt_expansion:false,guidance_scale:3.5,shift:5,interpolator_model:"film",num_interpolated_frames:0,adjust_fps_for_interpolation:true,video_quality:"high",video_write_mode:"balanced"} as const;
export function particleVideoInput(url:string,prompt:string,seed:number){return {...PARTICLE_VIDEO_SETTINGS,image_url:url,prompt,seed};}
export const particleMatteInput=(url:string)=>({video_url:url,background_color:"Transparent",output_container_and_codec:"webm_vp9",auto_zoom:false,preserve_audio:false});
