import {objectProject} from "@/lib/object-http";
import {terrainThumbnail} from "@/lib/terrain-library-preview";
export async function GET(request:Request){try{const query=new URL(request.url).searchParams,project=await objectProject(query.get("projectId"));return new Response(await terrainThumbnail(project,query.get("asset")),{headers:{"Content-Type":"image/svg+xml","Cache-Control":"private, no-store","X-Content-Type-Options":"nosniff"}});}catch{return new Response(null,{status:400});}}
