import {objectProject} from "@/lib/object-http";
import {falModels} from "@/lib/fal-models";
import {DEFAULT_FAL_MODELS} from "@/lib/fal-model-types";
export async function GET(request:Request){try{const q=new URL(request.url).searchParams;await objectProject(q.get("projectId"));const kind=q.get("kind");if(kind!=="image"&&kind!=="animation")throw new Error("Choose image or animation models.");const id=q.get("model");if(id){if(id!==DEFAULT_FAL_MODELS[kind].id)await falModels.get(kind,id);return Response.json({supported:true});}return Response.json(await falModels.list(kind,q.get("q")??"",q.get("cursor")??""),{headers:{"Cache-Control":"no-store"}});}catch(error){return Response.json({error:error instanceof Error?error.message:"Unable to load fal models."},{status:400});}}
