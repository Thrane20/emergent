import type {SceneInstance} from "./scene-placement";
import type {ToolResult} from "./inspection-protocol";
export type OrbitView={yaw:number;pitch:number;distance:number;target:number[];projection:"perspective"|"isometric";fieldOfView:number;orthographicSize:number};

/** Transform local -Z through the same scale and XYZ rotation hierarchy as Rust. */
export function lookDirection(instance:SceneInstance,instances:SceneInstance[]):number[]|null {
 const nodes=new Map(instances.map(node=>[node.id,node])),seen=new Set<string>();let current:SceneInstance|undefined=instance,vector=[0,0,-1];
 while(current){
  if(seen.has(current.id))return null;seen.add(current.id);
  let [x,y,z]=vector.map((value,i)=>value*current!.transform.scale[i]);
  const [rx,ry,rz]=current.transform.rotation_degrees.map(value=>value*Math.PI/180);
  [x,y]=[x*Math.cos(rz)-y*Math.sin(rz),x*Math.sin(rz)+y*Math.cos(rz)];
  [x,z]=[x*Math.cos(ry)+z*Math.sin(ry),-x*Math.sin(ry)+z*Math.cos(ry)];
  [y,z]=[y*Math.cos(rx)-z*Math.sin(rx),y*Math.sin(rx)+z*Math.cos(rx)];
  const length=Math.hypot(x,y,z);if(!Number.isFinite(length)||length===0)return null;
  vector=[x/length,y/length,z/length];current=current.parent?nodes.get(current.parent):undefined;
 }
 return vector;
}

export function focusView(current:OrbitView,result:Pick<ToolResult,"bounds"|"view">):OrbitView {
 const {bounds,view}=result;if(!bounds||!view)throw new Error("Reload the scene to enable focus navigation.");
 if(bounds.center.some(n=>Math.abs(n)>10000))throw new Error("Selected object is outside the editor camera's ±10000 world bounds.");
 const radius=Math.max(0.1,bounds.radius),distance=Math.max(0.25,view.projection==="perspective"?radius/Math.sin(view.field_of_view_degrees*Math.PI/360)*1.2:radius*2.4);
 if(distance>30000)throw new Error("Selected object is too large for the current editor camera bounds.");
 return {...current,yaw:Math.atan2(-view.forward[0],-view.forward[2]),pitch:Math.max(-1.45,Math.min(1.45,Math.asin(Math.max(-1,Math.min(1,-view.forward[1]))))),target:[...bounds.center],distance,projection:view.projection,fieldOfView:Math.max(1,Math.min(178.999,view.field_of_view_degrees)),orthographicSize:Math.min(1000000,radius*2.4)};
}
