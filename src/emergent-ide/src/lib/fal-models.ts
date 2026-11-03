import "server-only";
import {objectApiKey} from "./object-settings";
import {boundedBody} from "./object-files";
import {DEFAULT_FAL_MODELS,falModelId,type AudioModelKind,type FalModelKind,type FalModelSummary} from "./fal-model-types";
// Only local schema references are resolved; catalogue data never chooses a network destination.
type Schema={ $ref?:string;type?:string;properties?:Record<string,Schema>;required?:string[];default?:unknown;minimum?:number;maximum?:number;enum?:unknown[];examples?:unknown[];items?:Schema;anyOf?:Schema[];allOf?:Schema[] };
type OpenApi={components?:{schemas?:Record<string,Schema>};paths?:Record<string,{post?:{requestBody?:{content?:Record<string,{schema?:Schema}>}};get?:{responses?:Record<string,{content?:Record<string,{schema?:Schema}>}>}}>};
type CatalogModel={endpoint_id:string;metadata?:{display_name?:string;description?:string;category?:string;status?:string};openapi?:OpenApi};
// The first category is searched; models in any listed category are accepted (ElevenLabs TTS is filed under text-to-audio).
const categories:Record<FalModelKind,string[]>={image:["text-to-image"],animation:["image-to-video"],sound:["text-to-audio"],music:["text-to-audio"],speech:["text-to-speech","text-to-audio"]};
function resolve(schema:Schema|undefined,api:OpenApi):Schema{for(let depth=0;schema?.$ref&&depth<12;depth++){if(!schema.$ref.startsWith("#/components/schemas/"))throw new Error("Unsupported model schema reference.");schema=api.components?.schemas?.[schema.$ref.slice(21)];}if(!schema||schema.$ref||schema.allOf)throw new Error("This model needs a dedicated input adapter.");return schema;}
export function modelAdapter(model:CatalogModel,kind:"image"|"animation"){
 if(!categories[kind].includes(model.metadata?.category??"")||model.metadata?.status!=="active")throw new Error("This model is not an active generator for this media type.");
 const api=model.openapi;if(!api)throw new Error("fal did not provide the model schema.");
 const input=resolve(api.paths?.[`/${model.endpoint_id}`]?.post?.requestBody?.content?.["application/json"]?.schema,api),properties=input.properties??{};
 const output=resolve(api.paths?.[`/${model.endpoint_id}/requests/{request_id}`]?.get?.responses?.["200"]?.content?.["application/json"]?.schema,api);
 if(!properties.prompt)throw new Error("This model does not accept a text prompt.");
 const imageField=properties.image_url?"image_url":properties.start_image_url?"start_image_url":undefined;
 if(kind==="animation"&&!imageField)throw new Error("This animation model needs image inputs the editor does not support yet.");
 const media=resolve(output.properties?.[kind==="image"?"images":"video"],api);
 if(kind==="image"?(media.type!=="array"||!resolve(media.items,api).properties?.url):!media.properties?.url)throw new Error("This model returns a media format the library does not support yet.");
 const known=new Set(["prompt","seed","num_images",...(imageField?[imageField]:[])]);
 for(const field of input.required??[]){if(!known.has(field)&&resolve(properties[field],api).default===undefined)throw new Error(`This model requires an additional input: ${field}.`);}
 let panorama:Record<string,unknown>|undefined;
 try{
 const size=properties.image_size?resolve(properties.image_size,api):undefined;
 const sizeObject=size&&(size.anyOf??[size]).map(s=>resolve(s,api)).find(s=>s.properties?.width&&s.properties?.height);
 const dimension=(s:Schema|undefined,n:number)=>s&&["integer","number"].includes(s.type??"")&&n>=(s.minimum??0)&&n<=(s.maximum??Infinity);
 if(properties.aspect_ratio?.enum?.includes("2:1"))panorama={aspect_ratio:"2:1"};
 else if(sizeObject&&dimension(sizeObject.properties?.width,1024)&&dimension(sizeObject.properties?.height,512))panorama={image_size:{width:1024,height:512}};
 else if(dimension(properties.width,1024)&&dimension(properties.height,512))panorama={width:1024,height:512};
 }catch{/* Regular image generation can still use model defaults without panorama support. */}
 return {input,properties,imageField,panorama};
}
export function adaptedModelInput(adapter:ReturnType<typeof modelAdapter>,values:{prompt:string;seed:number;count?:number;imageUrl?:string;panorama?:boolean}){
 const input:Record<string,unknown>={prompt:values.prompt};
 for(const field of adapter.input.required??[])if(adapter.properties[field]?.default!==undefined)input[field]=adapter.properties[field].default;
 if(adapter.properties.seed)input.seed=values.seed;
 if(adapter.properties.num_images)input.num_images=values.count??1;else if((values.count??1)>1)throw new Error("This model generates one image per request. Choose one candidate.");
 if(adapter.imageField)input[adapter.imageField]=values.imageUrl;
 if(adapter.properties.output_format?.enum?.includes("png"))input.output_format="png";
 if(adapter.properties.generate_audio)input.generate_audio=false;
 for(const field of ["enable_safety_checker","enable_output_safety_checker"])if(adapter.properties[field])input[field]=true;
 if(values.panorama){if(!adapter.panorama)throw new Error("This model does not expose a supported 2:1 panorama size. Choose FLUX Schnell or another panorama-capable model.");Object.assign(input,adapter.panorama);}
 return input;
}
const DURATION_FIELDS=[["duration_seconds",1],["seconds_total",1],["duration",1],["music_length_ms",1000]] as const;
/** Maps the Audio editor's fields onto a catalogue model's inputs; refuses models it cannot drive. */
export function audioModelAdapter(model:CatalogModel,kind:AudioModelKind){
 if(!categories[kind].includes(model.metadata?.category??"")||model.metadata?.status!=="active")throw new Error("This model is not an active audio generator of this kind.");
 const api=model.openapi;if(!api)throw new Error("fal did not provide the model schema.");
 // Some catalogue IDs drop the owner prefix their schema paths use (elevenlabs/… under /fal-ai/elevenlabs/…).
 const paths=api.paths??{},path=(suffix:string)=>paths[`/${model.endpoint_id}${suffix}`]??Object.entries(paths).find(([key])=>key.endsWith(`/${model.endpoint_id}${suffix}`))?.[1];
 const input=resolve(path("")?.post?.requestBody?.content?.["application/json"]?.schema,api),properties=input.properties??{};
 const output=resolve(path("/requests/{request_id}")?.get?.responses?.["200"]?.content?.["application/json"]?.schema,api);
 const textField=properties.text?"text":properties.prompt?"prompt":undefined;
 if(!textField)throw new Error("This model does not take a text prompt.");
 const audioField=["audio","audio_file"].find(field=>output.properties?.[field]&&resolve(output.properties[field],api).properties?.url);
 if(!audioField)throw new Error("This model does not return an audio file the editor can save.");
 const duration=DURATION_FIELDS.find(([field])=>properties[field]),bounds=(field:string)=>{try{const schema=resolve(properties[field],api);const options=(schema.anyOf??[schema]).map(item=>resolve(item,api)).find(item=>["integer","number"].includes(item.type??""));return {min:options?.minimum,max:options?.maximum,integer:options?.type==="integer"};}catch{return {};}};
 // A voice is a top-level name, or a voice_id inside voice_setting (MiniMax); its enum or examples become suggestions.
 let voice:"voice"|"voice_setting"|undefined,voices:string[]=[];
 try{const setting=properties.voice_setting?resolve(properties.voice_setting,api):undefined;const schema=properties.voice?resolve(properties.voice,api):setting?.properties?.voice_id?resolve(setting.properties.voice_id,api):undefined;if(schema){voice=properties.voice?"voice":"voice_setting";voices=(schema.enum??schema.examples??[]).filter((item):item is string=>typeof item==="string").slice(0,100);}}catch{/* no voice control */}
 const known=new Set([textField,"seed","voice","voice_setting","loop","force_instrumental","output_format",...(duration?[duration[0]]:[])]);
 for(const field of input.required??[]){if(!known.has(field)&&resolve(properties[field],api).default===undefined)throw new Error(`This model requires an additional input: ${field}.`);}
 let outputFormat:string|undefined;try{const format=properties.output_format?resolve(properties.output_format,api):undefined;if(format?.enum?.includes("url"))outputFormat="url";}catch{/* keep the model default */}
 return {endpoint:model.endpoint_id,input,properties,textField,audioField,duration:duration&&{field:duration[0],scale:duration[1],...bounds(duration[0])},outputFormat,seed:Boolean(properties.seed),voice,voices,loop:Boolean(properties.loop),instrumental:Boolean(properties.force_instrumental)};
}
export function audioModelInput(adapter:ReturnType<typeof audioModelAdapter>,values:{text:string;seed:number;duration?:number;voice?:string;loop?:boolean;instrumental?:boolean}){
 const input:Record<string,unknown>={[adapter.textField]:values.text};
 for(const field of adapter.input.required??[])if(adapter.properties[field]?.default!==undefined&&input[field]===undefined)input[field]=adapter.properties[field].default;
 if(adapter.seed)input.seed=values.seed;
 if(adapter.duration&&values.duration!==undefined){const {field,scale,min,max,integer}=adapter.duration;let n=values.duration*scale;if(min!==undefined)n=Math.max(n,min);if(max!==undefined)n=Math.min(n,max);input[field]=integer||scale>1?Math.round(n):n;}
 if(adapter.voice&&values.voice)input[adapter.voice]=adapter.voice==="voice"?values.voice:{voice_id:values.voice};
 if(adapter.loop&&values.loop)input.loop=true;
 if(adapter.instrumental&&values.instrumental)input.force_instrumental=true;
 if(adapter.outputFormat)input.output_format=adapter.outputFormat;
 for(const field of ["enable_safety_checker","enable_output_safety_checker"])if(adapter.properties[field])input[field]=true;
 return input;
}
export class FalModels{
 private lists=new Map<string,{at:number;data:{models:FalModelSummary[];nextCursor:string|null}}>();
 private cache=new Map<string,{at:number;model:CatalogModel}>();
 constructor(private transport:typeof fetch=fetch,private credentials=objectApiKey){}
 private async request(params:URLSearchParams){const key=await this.credentials();const response=await this.transport(`https://api.fal.ai/v1/models?${params}`,{headers:key?{Authorization:`Key ${key}`}:{},redirect:"error",signal:AbortSignal.timeout(15000)});if(!response.ok)throw new Error(`fal model catalogue is unavailable (${response.status}). Try again or use the default model.`);const data=JSON.parse((await boundedBody(response,8*1024*1024)).toString("utf8"));if(!Array.isArray(data.models))throw new Error("fal returned an invalid model catalogue.");return data as {models:CatalogModel[];next_cursor?:string|null;has_more?:boolean};}
 async list(kind:FalModelKind,query="",cursor=""){
  if(query.length>100||cursor.length>512)throw new Error("Model search is too long.");
  const params=new URLSearchParams({category:categories[kind][0],status:"active",limit:"50"});if(query)params.set("q",query);if(cursor)params.set("cursor",cursor);
  const key=params.toString(),cached=this.lists.get(key);if(cached&&Date.now()-cached.at<60000)return cached.data;
  const data=await this.request(params),result={models:data.models.filter(model=>categories[kind].includes(model.metadata?.category??"")&&model.metadata?.status==="active").map((model):FalModelSummary=>({id:falModelId(model.endpoint_id),name:model.metadata?.display_name??model.endpoint_id,description:(model.metadata?.description??"").slice(0,1000)})),nextCursor:data.has_more?data.next_cursor??null:null};if(this.lists.size>=100)this.lists.delete(this.lists.keys().next().value!);this.lists.set(key,{at:Date.now(),data:result});return result;
 }
 private async catalog(id:string){falModelId(id);const cached=this.cache.get(id);let model=cached&&Date.now()-cached.at<300000?cached.model:undefined;if(!model){const data=await this.request(new URLSearchParams({endpoint_id:id,expand:"openapi-3.0"}));model=data.models.find(model=>model.endpoint_id===id);if(!model)throw new Error("This model is no longer available in the fal catalogue.");if(this.cache.size>=100)this.cache.delete(this.cache.keys().next().value!);this.cache.set(id,{at:Date.now(),model});}return model;}
 async get(kind:"image"|"animation",id:string){return modelAdapter(await this.catalog(id),kind);}
 async audio(kind:AudioModelKind,id:string){return audioModelAdapter(await this.catalog(id),kind);}
 async input(kind:"image"|"animation",id:string,values:Parameters<typeof adaptedModelInput>[1]){if(id===DEFAULT_FAL_MODELS[kind].id)return undefined;return adaptedModelInput(await this.get(kind,id),values);}
}
export const falModels=new FalModels();
