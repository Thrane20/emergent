import {objectProject,sameOrigin} from "@/lib/object-http";
import {boundedBody} from "@/lib/object-files";
import {previewRaymarched,raymarchedLibrary,readRaymarchedAsset,saveRaymarched} from "@/lib/raymarched-assets";
import {raymarchedSettings} from "@/lib/raymarched-terrain";
import {projectScenes} from "@/lib/project-scenes";
import {terrainSceneRevision} from "@/lib/terrain-assets";
export async function GET(request:Request){try{const q=new URL(request.url).searchParams,project=await objectProject(q.get("projectId")),asset=q.get("asset");
 if(asset){const {source,asset:document,...saved}=await readRaymarchedAsset(project,asset);void source;void document;if(q.get("preview")==="1")return new Response(new Uint8Array(await previewRaymarched(project,saved.settings)),{headers:{"Content-Type":"image/png","Cache-Control":"no-store"}});return Response.json(saved,{headers:{"Cache-Control":"no-store"}});}
 return Response.json({items:raymarchedLibrary(project),scenes:projectScenes(project).map(s=>({...s,revision:terrainSceneRevision(project.files.find(f=>f.path===s.path)?.content??"")}))},{headers:{"Cache-Control":"no-store"}});
 }catch(error){return failure(error);}}
export async function POST(request:Request){try{sameOrigin(request);const body=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),16*1024)).toString()),project=await objectProject(body.projectId);
 if(body.action==="preview")return new Response(new Uint8Array(await previewRaymarched(project,raymarchedSettings(body.settings),body.yaw,body.pitch)),{headers:{"Content-Type":"image/png","Cache-Control":"no-store"}});
 return Response.json(await saveRaymarched(project,body),{status:201});
 }catch(error){return failure(error);}}
function failure(error:unknown){return Response.json({error:error instanceof Error?error.message:"Ray-marched terrain request failed."},{status:400});}
