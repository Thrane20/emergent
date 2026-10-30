import "server-only";
import {objectApiKey} from "./object-settings";
import {boundedBody} from "./object-files";
import {DEFAULT_FAL_MODELS,falModelId,type FalModelKind,type FalModelSummary} from "./fal-model-types";
// Only local schema references are resolved; catalogue data never chooses a network destination.
type Schema={ $ref?:string;type?:string;properties?:Record<string,Schema>;required?:string[];default?:unknown;minimum?:number;maximum?:number;enum?:unknown[];items?:Schema;anyOf?:Schema[];allOf?:Schema[] };
type OpenApi={components?:{schemas?:Record<string,Schema>};paths?:Record<string,{post?:{requestBody?:{content?:Record<string,{schema?:Schema}>}};get?:{responses?:Record<string,{content?:Record<string,{schema?:Schema}>}>}}>};
type CatalogModel={endpoint_id:string;metadata?:{display_name?:string;description?:string;category?:string;status?:string};openapi?:OpenApi};
const category={image:"text-to-image",animation:"image-to-video"};
function resolve(schema:Schema|undefined,api:OpenApi):Schema{for(let depth=0;schema?.$ref&&depth<12;depth++){if(!schema.$ref.startsWith("#/components/schemas/"))throw new Error("Unsupported model schema reference.");schema=api.components?.schemas?.[schema.$ref.slice(21)];}if(!schema||schema.$ref||schema.allOf)throw new Error("This model needs a dedicated input adapter.");return schema;}
export function modelAdapter(model:CatalogModel,kind:FalModelKind){
 if(model.metadata?.category!==category[kind]||model.metadata.status!=="active")throw new Error("This model is not an active generator for this media type.");
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
export class FalModels{
 private lists=new Map<string,{at:number;data:{models:FalModelSummary[];nextCursor:string|null}}>();
 private cache=new Map<string,{at:number;model:CatalogModel}>();
 constructor(private transport:typeof fetch=fetch,private credentials=objectApiKey){}
 private async request(params:URLSearchParams){const key=await this.credentials();const response=await this.transport(`https://api.fal.ai/v1/models?${params}`,{headers:key?{Authorization:`Key ${key}`}:{},redirect:"error",signal:AbortSignal.timeout(15000)});if(!response.ok)throw new Error(`fal model catalogue is unavailable (${response.status}). Try again or use the default model.`);const data=JSON.parse((await boundedBody(response,8*1024*1024)).toString("utf8"));if(!Array.isArray(data.models))throw new Error("fal returned an invalid model catalogue.");return data as {models:CatalogModel[];next_cursor?:string|null;has_more?:boolean};}
 async list(kind:FalModelKind,query="",cursor=""){
  if(query.length>100||cursor.length>512)throw new Error("Model search is too long.");
  const params=new URLSearchParams({category:category[kind],status:"active",limit:"50"});if(query)params.set("q",query);if(cursor)params.set("cursor",cursor);
  const key=params.toString(),cached=this.lists.get(key);if(cached&&Date.now()-cached.at<60000)return cached.data;
  const data=await this.request(params),result={models:data.models.filter(model=>model.metadata?.category===category[kind]&&model.metadata.status==="active").map((model):FalModelSummary=>({id:falModelId(model.endpoint_id),name:model.metadata?.display_name??model.endpoint_id,description:(model.metadata?.description??"").slice(0,1000)})),nextCursor:data.has_more?data.next_cursor??null:null};if(this.lists.size>=100)this.lists.delete(this.lists.keys().next().value!);this.lists.set(key,{at:Date.now(),data:result});return result;
 }
 async get(kind:FalModelKind,id:string){falModelId(id);const cached=this.cache.get(id);let model=cached&&Date.now()-cached.at<300000?cached.model:undefined;if(!model){const data=await this.request(new URLSearchParams({endpoint_id:id,expand:"openapi-3.0"}));model=data.models.find(model=>model.endpoint_id===id);if(!model)throw new Error("This model is no longer available in the fal catalogue.");if(this.cache.size>=100)this.cache.delete(this.cache.keys().next().value!);this.cache.set(id,{at:Date.now(),model});}return modelAdapter(model,kind);}
 async input(kind:FalModelKind,id:string,values:Parameters<typeof adaptedModelInput>[1]){if(id===DEFAULT_FAL_MODELS[kind].id)return undefined;return adaptedModelInput(await this.get(kind,id),values);}
}
export const falModels=new FalModels();
