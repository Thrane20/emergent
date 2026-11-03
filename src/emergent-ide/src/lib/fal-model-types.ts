export type FalModelKind="image"|"animation"|AudioModelKind;
export type AudioModelKind="sound"|"music"|"speech";
export const AUDIO_MODEL_KINDS=["sound","music","speech"] as const;
export type FalModelSummary={id:string;name:string;description:string};
export const DEFAULT_FAL_MODELS={image:{id:"fal-ai/flux/schnell",name:"FLUX.1 Schnell",description:"Default image generator"},animation:{id:"fal-ai/wan/v2.2-5b/image-to-video",name:"Wan 2.2 5B",description:"Default sprite / video animation generator"},sound:{id:"fal-ai/elevenlabs/sound-effects/v2",name:"ElevenLabs Sound Effects v2",description:"Default sound effect generator; can make seamless loops"},music:{id:"fal-ai/elevenlabs/music",name:"ElevenLabs Music",description:"Default music generator"},speech:{id:"fal-ai/elevenlabs/tts/eleven-v3",name:"ElevenLabs TTS v3",description:"Default text-to-speech voice"}} as const;
export function falModelId(value:unknown){if(typeof value!=="string"||value.length>180||!/^\w[\w.-]*(?:\/[\w.-]+)+$/.test(value)||value.split("/").some(part=>part===".."||part==="."))throw new Error("Choose a valid fal model endpoint.");return value;}
