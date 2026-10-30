export function imageRect(width:number,height:number){const scale=Math.min(width/960,height/540);return {left:(width-960*scale)/2,top:(height-540*scale)/2,width:960*scale,height:540*scale};}
export function viewportPoint(box:{left:number;top:number;width:number;height:number},x:number,y:number,clamp=false){
 const rect=imageRect(box.width,box.height);if(rect.width<=0||rect.height<=0)return null;
 const point={x:(x-box.left-rect.left)/rect.width,y:(y-box.top-rect.top)/rect.height};
 if(!clamp&&(point.x<0||point.x>1||point.y<0||point.y>1))return null;
 return {x:Math.max(0,Math.min(1,point.x)),y:Math.max(0,Math.min(1,point.y))};
}
