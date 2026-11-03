"use client";
import {AssetCarousel} from "@/components/asset-carousel";
import type {BrainSummary,NodeCatalogue} from "@/lib/brains";

const ROLE_COLOURS:Record<string,string>={composite:"#7aa7e0",decorator:"#e0c27a",steering:"#8fd4a3",reference:"#c79ae0"};

/** Miniature of a brain's node tree: leaves spread left to right, parents centred over their children. */
export function BrainThumbnail({tree,catalogue}:{tree:NonNullable<BrainSummary["tree"]>;catalogue:NodeCatalogue|null}){
 const children=new Map<string,string[]>();
 for(const node of tree)if(node.parent)children.set(node.parent,[...(children.get(node.parent)??[]),node.id]);
 const root=tree.find(node=>!node.parent),place=new Map<string,{x:number;depth:number}>();
 let leaves=0,depth=0;
 // Post-order with an explicit stack; brains are at most 32 levels deep.
 const stack:[string,number,boolean][]=root?[[root.id,0,false]]:[];
 while(stack.length){
  const [id,level,visited]=stack.pop()!,kids=children.get(id)??[];
  depth=Math.max(depth,level);
  if(!kids.length){place.set(id,{x:leaves++,depth:level});continue;}
  if(visited){const xs=kids.map(kid=>place.get(kid)?.x??0);place.set(id,{x:(Math.min(...xs)+Math.max(...xs))/2,depth:level});continue;}
  stack.push([id,level,true]);for(const kid of [...kids].reverse())if(level<32)stack.push([kid,level+1,false]);
 }
 const width=120,height=60,x=(n:number)=>leaves<=1?width/2:10+n*(width-20)/(leaves-1),y=(d:number)=>depth?8+d*(height-16)/depth:height/2;
 const role=(type:string)=>ROLE_COLOURS[catalogue?.nodes.find(node=>node.id===type)?.role??(type==="brain"?"reference":"")]??"#6f8076";
 return <svg className="brain-thumbnail" viewBox={`0 0 ${width} ${height}`} aria-hidden="true">
  {tree.map(node=>{const a=place.get(node.id),b=node.parent?place.get(node.parent):undefined;return a&&b?<line key={`l:${node.id}`} x1={x(b.x)} y1={y(b.depth)} x2={x(a.x)} y2={y(a.depth)} stroke="#44604f" strokeWidth="1"/>:null;})}
  {tree.map(node=>{const p=place.get(node.id);return p?<circle key={node.id} cx={x(p.x)} cy={y(p.depth)} r={node.parent?3.2:4.2} fill={role(node.type)}/>:null;})}
 </svg>;
}

/** Saved brains as a swatch carousel, the same strip the other editors use. */
export function BrainLibrary({brains,catalogue,selected,disabled,onSelect,onNew}:{brains:BrainSummary[];catalogue:NodeCatalogue|null;selected?:string;disabled:boolean;onSelect:(path:string)=>void;onNew:()=>void}){
 return <AssetCarousel label="Brain library" newLabel="brain" selected={selected} disabled={disabled} onSelect={onSelect} onNew={onNew}
  items={brains.map(brain=>({path:brain.path,name:brain.name,error:brain.error,title:`${brain.name}${brain.description?`\n${brain.description}`:""}\n${brain.path}`,
   thumbnail:brain.tree?.length?<BrainThumbnail tree={brain.tree} catalogue={catalogue}/>:<span className="particle-swatch-unavailable">{brain.error?"Invalid brain":"Empty"}</span>}))}/>;
}
