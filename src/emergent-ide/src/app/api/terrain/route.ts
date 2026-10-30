import {readTerrainEdit} from "@/lib/terrain-assets";
import { objectProject, sameOrigin } from "@/lib/object-http";
import { boundedBody } from "@/lib/object-files";
import { runTerrainTool } from "@/lib/terrain-tool";
import { TERRAIN_COMPARE_MAX_RESOLUTION,TERRAIN_COMPARISON_LABELS, terrainGlb, terrainSettings } from "@/lib/terrain";

export async function POST(request:Request){
 try{
  sameOrigin(request);
  const body=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),16*1024)).toString("utf8"));
  if(body.compare!==undefined&&typeof body.compare!=="boolean")throw new Error("Compare must be a boolean.");
  const project=await objectProject(body.projectId),settings=terrainSettings(body.settings),compare=body.compare===true;
  if(compare&&settings.resolution>TERRAIN_COMPARE_MAX_RESOLUTION)throw new Error("Algorithm comparison supports resolutions up to 256. Use Generate terrain at 512 or 1024.");
  const fields=await runTerrainTool(project,settings,compare);
  const variants=fields.map((field,index)=>{
   const start=performance.now(),bytes=terrainGlb(field.settings,field.values),meshMs=performance.now()-start;
   return {label:TERRAIN_COMPARISON_LABELS[index],settings:field.settings,bytes,metrics:{generationMs:field.generationMs,meshMs,fieldBytes:field.values.byteLength,glbBytes:bytes.byteLength}};
  });
  if(compare)return Response.json({variants:variants.map(({bytes,...variant})=>({...variant,glb:Buffer.from(bytes).toString("base64")}))},{headers:{"Cache-Control":"no-store"}});
  return new Response(variants[0].bytes,{headers:{"Content-Type":"model/gltf-binary","Cache-Control":"no-store","X-Content-Type-Options":"nosniff","X-Terrain-Metrics":JSON.stringify(variants[0].metrics)}});
 }catch(error){return Response.json({error:error instanceof Error?error.message:"Terrain generation failed."},{status:400});}
}

export async function GET(request:Request){
 try{
  const query=new URL(request.url).searchParams,project=await objectProject(query.get("projectId"));
  const terrain=await readTerrainEdit(project,query.get("scene"),query.get("instance"));
  const field=terrain.heights?{values:terrain.heights,generationMs:0}:(await runTerrainTool(project,terrain.settings,false))[0];
  // Reject a mixed snapshot if source files changed during recipe generation.
  if((await readTerrainEdit(project,terrain.scenePath,terrain.instanceId)).revision!==terrain.revision)throw new Error("Terrain changed while loading. Reopen it.");
  return Response.json({paint:terrain.paint??null,settings:terrain.settings,heights:Array.from(field.values),generationMs:field.generationMs,name:terrain.name,assetPath:terrain.assetPath,editRevision:terrain.revision,assetRevision:terrain.assetRevision,assetName:terrain.asset.name,collider:(terrain.asset.physics as {enabled?:boolean}|undefined)?.enabled??false,sceneRevision:terrain.sceneRevision},{headers:{"Cache-Control":"no-store"}});
 }catch(error){return Response.json({error:error instanceof Error?error.message:"Terrain could not be loaded."},{status:400});}
}
