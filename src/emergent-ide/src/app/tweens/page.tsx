import {TweenEditor} from "@/components/tween-editor";
import {readProjectForPage} from "@/lib/project-page";
import {tweenAsset,type TweenAsset} from "@/lib/tween";
export const instant=false;
export default async function TweensPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
  const project=await readProjectForPage(),params=await searchParams,assets:TweenAsset[]=[],warnings:string[]=[];
  for(const file of project.files.filter(file=>file.path.endsWith(".tween.toml")&&project.assetRoots.some(root=>file.path.startsWith(`${root}/`)))){
    try{assets.push(tweenAsset(file.path,file.content??""));}catch{warnings.push(file.displayName??file.name);}
  }
  return <TweenEditor projectId={project.id} projectName={project.name} assets={assets} warnings={warnings} initialAsset={typeof params.asset==="string"?params.asset:undefined} scenePath={typeof params.scene==="string"&&project.files.some(file=>file.path===params.scene&&file.path.endsWith(".scene.toml"))?params.scene:project.entryScene}/>;
}
