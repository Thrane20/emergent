"use client";
import {useEffect,useRef} from "react";
import {spriteFrameAt,type SpriteSheetSettings} from "@/lib/image-library";
export function SpriteSheetPreview({src,settings}:{src:string;settings:SpriteSheetSettings}){
 const canvas=useRef<HTMLCanvasElement>(null),output=useRef<HTMLOutputElement>(null);
 useEffect(()=>{
  const image=new Image(),context=canvas.current?.getContext("2d");let timer:ReturnType<typeof setInterval>|undefined,alive=true,frame=0,step=0;
  function draw(){if(!alive||!context||!canvas.current)return;const width=image.naturalWidth/settings.columns,height=image.naturalHeight/settings.rows,scale=Math.min(192/width,192/height),x=(frame%settings.columns)*width,y=Math.floor(frame/settings.columns)*height;context.clearRect(0,0,192,192);context.drawImage(image,x,y,width,height,(192-width*scale)/2,(192-height*scale)/2,width*scale,height*scale);canvas.current.dataset.frame=String(frame);if(output.current)output.current.textContent=`Frame ${frame+1} / ${settings.frames} · ${settings.fps} fps`;}
  image.onload=()=>{draw();if(settings.frames>1)timer=setInterval(()=>{frame=spriteFrameAt(++step,settings.frames,settings.ping_pong);draw();},1000/settings.fps);};image.src=src;
  return()=>{alive=false;image.onload=null;clearInterval(timer);};
 },[src,settings.columns,settings.rows,settings.frames,settings.fps,settings.ping_pong]);
 return <div className="sprite-sheet-preview"><canvas ref={canvas} width={192} height={192} aria-label="Animated sprite sheet preview"/><div><output ref={output}/></div></div>;
}
