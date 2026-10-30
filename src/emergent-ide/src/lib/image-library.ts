import type {ProjectSnapshot} from "./project";
export type SpriteSheetSettings={columns:number;rows:number;frames:number;fps:number;ping_pong?:boolean};
export type ImageLibrarySettings={kind:"image"|"sprite"|"skybox";sprite:SpriteSheetSettings;family?:string};
export const DEFAULT_SPRITE_SHEET:SpriteSheetSettings={columns:1,rows:1,frames:1,fps:12};
export type LibraryImage={path:string;name:string;modifiedAt:string;kind?:"image"|"sprite"|"skybox";sprite?:SpriteSheetSettings;family?:string};
export function imageLibrarySettings(value:unknown):ImageLibrarySettings{
 if(!value||typeof value!=="object")throw new Error("Image settings are required.");const v=value as ImageLibrarySettings,s=v.sprite;
 const integer=(n:unknown,min:number,max:number)=>typeof n==="number"&&Number.isInteger(n)&&n>=min&&n<=max;
 if(!["image","sprite","skybox"].includes(v.kind)||!s||!integer(s.columns,1,16)||!integer(s.rows,1,16)||!integer(s.frames,1,s.columns*s.rows)||typeof s.fps!=="number"||!Number.isFinite(s.fps)||s.fps<1||s.fps>120)throw new Error("Choose Image, Sprite or Skybox, 1–16 rows/columns, a valid frame count and 1–120 fps.");
 if(s.ping_pong!==undefined&&typeof s.ping_pong!=="boolean")throw new Error("Choose a valid sprite playback mode.");
 if(v.family!==undefined&&!/^[a-f0-9-]{36}$/.test(v.family))throw new Error("Invalid sprite family.");
 return {...(v.family?{family:v.family}:{}),kind:v.kind,sprite:{columns:s.columns,rows:s.rows,frames:s.frames,fps:s.fps,...(s.ping_pong?{ping_pong:true}:{})}};
}
/** Images remain the same asset; sprite labels and playback settings live in sidecars. */
export function projectImages(project:ProjectSnapshot):LibraryImage[]{
 const files=new Map(project.files.map(file=>[file.path,file]));
 return project.files.filter(file=>/\.(png|jpe?g|webp)$/i.test(file.path)&&project.assetRoots.some(root=>file.path.startsWith(`${root.replace(/\/$/,"")}/`)))
  .map(file=>{const sidecar=files.get(`${file.path}.library.json`);let settings:ImageLibrarySettings={kind:"image",sprite:DEFAULT_SPRITE_SHEET};
   if(sidecar?.content)try{const data=JSON.parse(sidecar.content);if(data.schema_version===1)settings=imageLibrarySettings(data);}catch{/* Invalid metadata falls back to Image for repair. */}
   return {path:file.path,name:file.displayName??file.name.replace(/\.[^.]+$/,""),modifiedAt:sidecar&&sidecar.modifiedAt>file.modifiedAt?sidecar.modifiedAt:file.modifiedAt,kind:settings.kind,...(settings.family?{family:settings.family}:{}),...(settings.kind==="sprite"?{sprite:settings.sprite}:{})};
  }).sort((a,b)=>b.modifiedAt.localeCompare(a.modifiedAt)||a.path.localeCompare(b.path));
}

/** Reflect at endpoints without repeating them; frames may cover only part of the atlas. */
export function spriteFrameAt(step:number,frames:number,pingPong=false){
 if(frames<=1)return 0;
 const phase=Math.floor(step)%(pingPong?2*(frames-1):frames);
 return pingPong&&phase>=frames?2*(frames-1)-phase:phase;
}
