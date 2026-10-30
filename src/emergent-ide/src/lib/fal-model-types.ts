export type FalModelKind="image"|"animation";
export type FalModelSummary={id:string;name:string;description:string};
export const DEFAULT_FAL_MODELS={image:{id:"fal-ai/flux/schnell",name:"FLUX.1 Schnell",description:"Default image generator"},animation:{id:"fal-ai/wan/v2.2-5b/image-to-video",name:"Wan 2.2 5B",description:"Default sprite / video animation generator"}} as const;
export function falModelId(value:unknown){if(typeof value!=="string"||value.length>180||!/^\w[\w.-]*(?:\/[\w.-]+)+$/.test(value)||value.split("/").some(part=>part===".."||part==="."))throw new Error("Choose a valid fal model endpoint.");return value;}
