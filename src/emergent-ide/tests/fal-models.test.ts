import assert from "node:assert/strict";
import test from "node:test";
import {FalModels,adaptedModelInput,modelAdapter,falModels} from "../src/lib/fal-models";
import {falModelId} from "../src/lib/fal-model-types";
import {textureRequest} from "../src/lib/texture-job-types";
import {particleMediaRequest} from "../src/lib/particle-media-types";
import {createTextureProvider} from "../src/lib/texture-provider";
function catalog(kind:"image"|"animation",extra:Record<string,{type:string}>={}){
 const id=`fal-ai/test/${kind}`;
 const input:{type:string;properties:Record<string,{type:string;enum?:string[]}>;required:string[]}={type:"object",properties:{prompt:{type:"string"},seed:{type:"integer"},...(kind==="image"?{num_images:{type:"integer"},output_format:{type:"string",enum:["png","jpeg"]}}:{start_image_url:{type:"string"},generate_audio:{type:"boolean"}}),...extra},required:kind==="image"?["prompt"]:["prompt","start_image_url"]};
 const output:{type:string;properties:Record<string,{type?:string;items?:{$ref:string};$ref?:string}>}={type:"object",properties:kind==="image"?{images:{type:"array",items:{$ref:"#/components/schemas/File"}}}:{video:{$ref:"#/components/schemas/File"}}};
 return {
  endpoint_id:id,
  metadata:{category:kind==="image"?"text-to-image":"image-to-video",status:"active",display_name:"Test model"},
  openapi:{
   components:{schemas:{Input:input,Output:output,File:{type:"object",properties:{url:{type:"string"}}}}},
   paths:{
    [`/${id}`]:{post:{requestBody:{content:{"application/json":{schema:{$ref:"#/components/schemas/Input"}}}}}},
    [`/${id}/requests/{request_id}`]:{get:{responses:{"200":{content:{"application/json":{schema:{$ref:"#/components/schemas/Output"}}}}}}}
   }
  }
 };
}
test("fal catalogue uses live categories, pagination and cached model schemas on a fixed host",async()=>{
 const urls:URL[]=[];const models=new FalModels(async input=>{const url=new URL(String(input));urls.push(url);return Response.json({models:[catalog(url.searchParams.get("category")==="text-to-image"?"image":"animation")],has_more:true,next_cursor:"next-page"});},async()=>"");
 assert.equal((await models.list("image","fire","cursor")).nextCursor,"next-page");assert.equal(urls[0].origin,"https://api.fal.ai");assert.equal(urls[0].searchParams.get("category"),"text-to-image");assert.equal(urls[0].searchParams.get("cursor"),"cursor");assert.equal(urls[0].searchParams.get("q"),"fire");await models.get("animation","fal-ai/test/animation");await models.get("animation","fal-ai/test/animation");assert.equal(urls.length,2);assert.equal(urls[1].searchParams.get("expand"),"openapi-3.0");await assert.rejects(models.get("image","https://example.com"),/valid fal model/);
});
test("model adapters map start images, avoid image defaults on video, and reject unsupported required inputs",()=>{
 const image=adaptedModelInput(modelAdapter(catalog("image"),"image"),{prompt:"fire",seed:42,count:2});assert.deepEqual(image,{prompt:"fire",seed:42,num_images:2,output_format:"png"});
 const animation=adaptedModelInput(modelAdapter(catalog("animation"),"animation"),{prompt:"moving fire",seed:42,imageUrl:"https://fal.media/source.png"});assert.equal(animation.start_image_url,"https://fal.media/source.png");assert.equal(animation.generate_audio,false);assert.equal(animation.num_inference_steps,undefined);assert.equal(animation.num_frames,undefined);
 const unsupported=catalog("animation",{other_image:{type:"string"}});unsupported.openapi.components.schemas.Input.required.push("other_image");assert.throws(()=>modelAdapter(unsupported,"animation"),/additional input: other_image/);
 const badOutput=catalog("image");delete (badOutput.openapi.components.schemas.Output.properties as Record<string,unknown>).images;assert.throws(()=>modelAdapter(badOutput,"image"));assert.throws(()=>modelAdapter(catalog("image"),"animation"),/media type/);
});
test("model selection is validated and image submit, status and result retain the chosen endpoint",async t=>{
 const endpoint="fal-ai/test/image";assert.equal(textureRequest({name:"Fire",prompt:"Fire sprite",seed:42,count:1,tileFriendly:false,model:endpoint}).model,endpoint);assert.equal(particleMediaRequest({stage:"animate",source:"assets/fire.png",prompt:"Fire motion",seed:42,output:"sprite",model:"fal-ai/test/animation"}).model,"fal-ai/test/animation");for(const invalid of ["https://evil.test/model","fal-ai/../secret","bad?query=yes",""])assert.throws(()=>falModelId(invalid));assert.throws(()=>particleMediaRequest({stage:"matte",source:"video",prompt:"",seed:42,model:endpoint}));
 const seen:string[]=[];let paid=0;t.mock.method(falModels,"input",async()=>({prompt:"Fire sprite",seed:42,num_images:1}));
 const provider=createTextureProvider("mock",async(input,init)=>{const url=String(input);seen.push(url);if(url.startsWith("https://fal.media/"))return new Response(new Uint8Array([1]));if(init?.method==="POST"){paid++;assert.equal(JSON.parse(String(init.body)).num_inference_steps,undefined);return Response.json({request_id:"selected-model-job"});}if(url.includes("/status"))return Response.json({status:"COMPLETED"});return Response.json({images:[{url:"https://fal.media/result.png"}]});});
 await provider.submit("Fire sprite",42,1,endpoint);await provider.status("selected-model-job",endpoint);await provider.result("selected-model-job",endpoint);assert.equal(paid,1);assert.ok(seen[0].includes(endpoint));assert.ok(seen.filter(url=>url.includes("queue.fal.run")).every(url=>url.includes("/fal-ai/test")));
});

test("skybox model adaptation requires an advertised 2:1 size before submission",()=>{
 const values={prompt:"Panorama",seed:42,count:1,panorama:true};
 assert.throws(()=>adaptedModelInput(modelAdapter(catalog("image"),"image"),values),/2:1 panorama/);
 const ratio=catalog("image");ratio.openapi.components.schemas.Input.properties.aspect_ratio={type:"string",enum:["1:1","2:1"]};assert.equal(adaptedModelInput(modelAdapter(ratio,"image"),values).aspect_ratio,"2:1");
 const dimensions=catalog("image",{width:{type:"integer"},height:{type:"integer"}});assert.equal(adaptedModelInput(modelAdapter(dimensions,"image"),values).width,1024);assert.equal(adaptedModelInput(modelAdapter(dimensions,"image"),values).height,512);
});
