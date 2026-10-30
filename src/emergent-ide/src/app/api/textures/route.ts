import {boundedBody} from "@/lib/object-files";
import {objectProject,sameOrigin} from "@/lib/object-http";
import {importTexture,validateTextureImage} from "@/lib/texture-assets";
import {configureLibraryImage,deleteLibraryImage} from "@/lib/image-library-assets";
import {AssetDeletionError} from "@/lib/project-assets";
import {particleMedia} from "@/lib/particle-media";
import {textureJobs} from "@/lib/texture-jobs";
import {DEFAULT_SPRITE_SHEET,projectImages} from "@/lib/image-library";
export async function GET(request:Request){
 try{const project=await objectProject(new URL(request.url).searchParams.get("projectId"));return Response.json(projectImages(project),{headers:{"Cache-Control":"no-store"}});}
 catch(error){return Response.json({error:error instanceof Error?error.message:"Image library could not be loaded."},{status:400});}
}
export async function POST(request:Request){
 try{sameOrigin(request);const params=new URL(request.url).searchParams,project=await objectProject(params.get("projectId"));const bytes=await boundedBody(new Response(request.body,{headers:request.headers}),16*1024*1024);const kind=params.get("kind");if(kind!==null&&kind!=="image"&&kind!=="sprite"&&kind!=="skybox")throw new Error("Choose Image, Sprite or Skybox.");if(kind==="skybox")await validateTextureImage(project,bytes,true);const saved=await importTexture(project,bytes,params.get("name")??"texture");if(kind&&kind!=="image")await configureLibraryImage(project,saved.path,{kind,sprite:DEFAULT_SPRITE_SHEET});return Response.json(saved,{status:201});}
 catch(error){return Response.json({error:error instanceof Error?error.message:"Texture could not be imported."},{status:400});}
}

export async function DELETE(request:Request){
 try{sameOrigin(request);const body=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),8192)).toString("utf8")),project=await objectProject(body.projectId);if(typeof body.path!=="string")throw new AssetDeletionError("Image path is required.");await deleteLibraryImage(project,body.path);await textureJobs.forgetSavedImage(project,body.path);await particleMedia.forgetAtlas(project,body.path);return Response.json({path:body.path});}
 catch(error){return Response.json({error:error instanceof Error?error.message:"Image could not be deleted."},{status:error instanceof AssetDeletionError?error.status:400});}
}

export async function PUT(request:Request){
 try{sameOrigin(request);const body=JSON.parse((await boundedBody(new Response(request.body,{headers:request.headers}),8192)).toString("utf8")),project=await objectProject(body.projectId);if(typeof body.path!=="string")throw new Error("Image path is required.");return Response.json(await configureLibraryImage(project,body.path,body.settings));}
 catch(error){return Response.json({error:error instanceof Error?error.message:"Image settings could not be saved."},{status:error instanceof AssetDeletionError?error.status:400});}
}
