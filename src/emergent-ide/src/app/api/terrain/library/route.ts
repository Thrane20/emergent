import {TERRAIN_SAVE_MAX_BYTES} from "@/lib/terrain";
import {objectProject,sameOrigin} from "@/lib/object-http";
import {boundedBody} from "@/lib/object-files";
import {terrainLibrary,saveTerrainLibrary,placeTerrain} from "@/lib/terrain-library";
import {readTerrainAsset,terrainSceneRevision} from "@/lib/terrain-assets";
import {runTerrainTool} from "@/lib/terrain-tool";
import {projectScenes} from "@/lib/project-scenes";
export async function GET(request:Request){try{const query=new URL(request.url).searchParams,project=await objectProject(query.get("projectId")),path=query.get("asset");
 if(!path)return Response.json({items:terrainLibrary(project),scenes:projectScenes(project).map(scene=>({...scene,revision:terrainSceneRevision(project.files.find(file=>file.path===scene.path)?.content??"")}))},{headers:{"Cache-Control":"no-store"}});
 const asset=await readTerrainAsset(project,path),field=asset.heights?{values:asset.heights,generationMs:0}:(await runTerrainTool(project,asset.settings,false))[0];if((await readTerrainAsset(project,path)).revision!==asset.revision)throw new Error("Terrain changed while loading. Reopen it.");
 return Response.json({assetPath:asset.assetPath,revision:asset.revision,name:asset.name,settings:asset.settings,heights:Array.from(field.values),paint:asset.paint??null,collider:(asset.asset.physics as {enabled?:boolean}|undefined)?.enabled??false,generationMs:field.generationMs},{headers:{"Cache-Control":"no-store"}});
 }catch(error){return failure(error);}}
export async function POST(request:Request){try{sameOrigin(request);const body=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),TERRAIN_SAVE_MAX_BYTES)).toString()),project=await objectProject(body.projectId);if(body.action!==undefined&&body.action!=="save"&&body.action!=="place")throw new Error("Choose save or place.");return Response.json(body.action==="place"?await placeTerrain(project,body):await saveTerrainLibrary(project,body),{status:201});}catch(error){return failure(error);}}
function failure(error:unknown){return Response.json({error:error instanceof Error?error.message:"Terrain library request failed."},{status:400});}
