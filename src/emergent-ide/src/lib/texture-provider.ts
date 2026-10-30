import "server-only";
import {falModels} from "./fal-models";
import {createFalClient} from "@fal-ai/client";
import {objectApiKey} from "./object-settings";
import {boundedBody} from "./object-files";
import {ObjectCreatorError} from "./object-types";
import {ProviderFailure,singleSubmissionFetch} from "./object-provider";
import {TEXTURE_ENDPOINT,TEXTURE_GENERATION_SETTINGS} from "./texture-job-types";
export interface TextureProvider{
 submit(prompt:string,seed:number,count:number,endpoint?:string,settings?:Record<string,unknown>):Promise<string>;
 status(id:string,endpoint?:string):Promise<"queued"|"generating"|"completed">;
 result(id:string,endpoint?:string):Promise<Buffer[]>;
}
export async function falTextureProvider(){const key=await objectApiKey();if(!key)throw new ObjectCreatorError("Configure your fal key in Settings → Object & texture generation.",503);return createTextureProvider(key);}
function failure(error:unknown):ProviderFailure{
 if(error instanceof ProviderFailure)return error;
 const status=error&&typeof error==="object"&&"status"in error?Number(error.status):0;
 if([400,422].includes(status))return new ProviderFailure("fal rejected this texture request. Try a different prompt.",true);
 if([401,403].includes(status))return new ProviderFailure("fal rejected the credentials. Check the saved fal key and account access.",false);
 if(status===402)return new ProviderFailure("fal needs account credit to continue generation.",false);
 return new ProviderFailure("Unable to check this fal request. Check again to resume it.",false);
}
export function createTextureProvider(key:string,transport:typeof fetch=fetch):TextureProvider{
 const client=createFalClient({credentials:key,retry:{maxRetries:0},fetch:singleSubmissionFetch(transport)});
 return {
  async submit(prompt,seed,count,endpoint=TEXTURE_ENDPOINT,settings){try{const response=await client.queue.submit(endpoint,{input:settings??await falModels.input("image",endpoint,{prompt,seed,count})??{...TEXTURE_GENERATION_SETTINGS,prompt,seed,num_images:count},abortSignal:AbortSignal.timeout(30000)});if(typeof response.request_id!=="string"||!response.request_id)throw new Error("Missing request ID");return response.request_id;}catch(error){throw failure(error);}},
  async status(requestId,endpoint=TEXTURE_ENDPOINT){try{const response=await client.queue.status(endpoint,{requestId,logs:false,abortSignal:AbortSignal.timeout(15000)});return response.status==="COMPLETED"?"completed":response.status==="IN_QUEUE"?"queued":"generating";}catch(error){throw failure(error);}},
  async result(requestId,endpoint=TEXTURE_ENDPOINT){
   try{
    const response=await client.queue.result(endpoint,{requestId,abortSignal:AbortSignal.timeout(15000)}),data=response.data as {images?:{url:string}[];has_nsfw_concepts?:boolean[]};
    if(!Array.isArray(data.images)||data.images.length<1||data.images.length>4||data.has_nsfw_concepts?.some(Boolean))throw new ProviderFailure("fal returned no usable texture candidates.",true);
    const urls=data.images.map(image=>{let url:URL;try{url=new URL(image.url);}catch{throw new ProviderFailure("fal returned an invalid image URL.",true);}if(url.protocol!=="https:"||!(url.hostname==="fal.media"||url.hostname.endsWith(".fal.media"))||url.username||url.password||(url.port&&url.port!=="443"))throw new ProviderFailure("fal returned an unsupported texture download host.",true);return url;});
    const images:Buffer[]=[];
    for(const url of urls){const response=await transport(url,{redirect:"error",signal:AbortSignal.timeout(60000)});if(!response.ok)throw new ProviderFailure("Texture download failed. Check this request again.",false);images.push(await boundedBody(response,16*1024*1024));}return images;
   }catch(error){throw failure(error);}
  },
 };
}
