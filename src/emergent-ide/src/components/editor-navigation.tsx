"use client";

import Link from "next/link";
import {usePathname,useRouter,useSearchParams} from "next/navigation";
import {createContext,Suspense,useContext,useEffect,useRef,type ReactNode,type RefObject} from "react";

const NavigationGuard=createContext<RefObject<(()=>boolean)|null>|null>(null);

export function useEditorNavigationGuard(guard:()=>boolean){
 const currentRef=useContext(NavigationGuard);
 useEffect(()=>{if(!currentRef)return;currentRef.current=guard;return()=>{if(currentRef.current===guard)currentRef.current=null;};},[currentRef,guard]);
}

const editors=[
 {path:"/lighting",name:"Lighting",icon:"lighting"},
 {path:"/terrain",name:"Terrain Builder",icon:"terrain"},
 {path:"/objects",name:"Object Creator",icon:"create"},
 {path:"/object-builder",name:"Object Builder",icon:"build"},
 {path:"/materials",name:"Material Generator",icon:"material"},
 {path:"/images",name:"Image Generator",icon:"image"},
 {path:"/particles",name:"Particle Generator",icon:"particle"},
 {path:"/tweens",name:"Tween Editor",icon:"tween"},
 {path:"/brains",name:"Brain Editor",icon:"brain"},
] as const;

function EditorIcon({name}:{name:string}){
 const paths:Record<string,ReactNode>={
  scene:<><path d="M3 9h18v11H3zM3 9V4h18v5M7 4l3 5m4-5 3 5"/><path d="m10 12 5 3-5 3z"/></>,
  source:<><path d="m8 6-6 6 6 6m8-12 6 6-6 6m-3-15-2 18"/></>,
  lighting:<><circle cx="12" cy="12" r="4"/><path d="M12 1v3m0 16v3M1 12h3m16 0h3M4 4l2 2m12 12 2 2M4 20l2-2M18 6l2-2"/></>,
  terrain:<><path d="m2 20 7-15 5 10 3-6 5 11ZM6 12l3 2 3-2"/></>,
  create:<><path d="m3 8 7-4 7 4v9l-7 4-7-4Zm0 0 7 4 7-4m-7 4v9m9-19v6m-3-3h6"/></>,
  build:<><path d="M2 13h8v8H2zM14 13h8v8h-8zM8 1h8v8H8zM12 9v3m-6 1v-1h12v1"/></>,
  material:<><circle cx="12" cy="12" r="9"/><path d="M12 3c-5 4-5 14 0 18m0-18v18m0-11 6-6m-6 12 9-9m-8 14 8-8"/></>,
  image:<><rect x="2" y="3" width="20" height="18" rx="2"/><circle cx="8" cy="9" r="2"/><path d="m3 19 6-6 4 4 4-6 5 7"/></>,
  particle:<><path d="m12 4 1.7 5.3L19 11l-5.3 1.7L12 18l-1.7-5.3L5 11l5.3-1.7Z"/><circle cx="3" cy="3" r="1"/><circle cx="21" cy="5" r="1"/><circle cx="20" cy="20" r="1"/><path d="M3 18v4m-2-2h4"/></>,
  tween:<><path d="M3 20V4m0 16h18M4 18C14 18 9 5 20 5"/><circle cx="4" cy="18" r="2"/><circle cx="20" cy="5" r="2"/></>,
  brain:<><circle cx="12" cy="4" r="2"/><circle cx="5" cy="19" r="2"/><circle cx="12" cy="19" r="2"/><circle cx="19" cy="19" r="2"/><path d="M12 6v6M5 17v-2.5h14V17M12 12v5"/></>,
  player:<><path d="M7 7h10c4 0 6 12 3 13-2 1-4-4-5-4H9c-1 0-3 5-5 4C1 19 3 7 7 7ZM8 10v5m-2.5-2.5h5M15 11h.01M18 14h.01"/></>,
  settings:<><path d="m9 3 1-2h4l1 2 3 1 2 1 2 4-1 2v2l1 2-2 4-2 1-3 1-1 2h-4l-1-2-3-1-2-1-2-4 1-2v-2L2 9l2-4 2-1Z" transform="translate(1 1) scale(.9)"/><circle cx="12" cy="12" r="3"/></>,
  reload:<><path d="M20 7a9 9 0 1 0 1 8M20 2v6h-6"/></>,
 };
 return <svg viewBox="0 0 24 24" width="21" height="21" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">{paths[name]}</svg>;
}

export function EditorRail({scene,view,onView,beforeNavigate,onReload}:{scene?:string;view?:string;onView?:(view:"scene"|"source")=>void;beforeNavigate?:()=>boolean;onReload?:()=>void}){
 const pathname=usePathname(),router=useRouter(),guard=useContext(NavigationGuard);
 const canLeave=()=>beforeNavigate?beforeNavigate():guard?.current?.()??true;
 function href(path:string,source=false){const query=new URLSearchParams();if(scene)query.set("scene",scene);if(source)query.set("view","source");return `${path}${query.size?`?${query}`:""}`;}
 function destination(path:string,name:string,icon:string){const active=pathname===path;return <Link key={path} className={`rail-button${active?" active":""}`} href={href(path)} aria-label={name} title={name} aria-current={active?"page":undefined} onNavigate={event=>{if(!canLeave())event.preventDefault();}}><EditorIcon name={icon}/></Link>;}
 return <nav className="rail editor-rail" aria-label="IDE sections">
  {(["scene","source"] as const).map(mode=>onView?<button type="button" key={mode} className={`rail-button${view===mode?" active":""}`} title={mode==="scene"?"Scene editor":"Source editor"} aria-label={mode==="scene"?"Scene editor":"Source editor"} aria-pressed={view===mode} onClick={()=>{if(canLeave())onView(mode);}}><EditorIcon name={mode}/></button>:<Link key={mode} className="rail-button" href={href("/",mode==="source")} title={mode==="scene"?"Scene editor":"Source editor"} aria-label={mode==="scene"?"Scene editor":"Source editor"} onNavigate={event=>{if(!canLeave())event.preventDefault();}}><EditorIcon name={mode}/></Link>)}
  {editors.map(editor=>destination(editor.path,editor.name,editor.icon))}
  <span className="rail-spacer"/>
  {destination("/player","Player settings","player")}
  {destination("/settings","Settings","settings")}
  <button type="button" className="rail-button" title="Reload project from disk" aria-label="Reload project from disk" onClick={()=>{if(canLeave()){if(onReload)onReload();else router.refresh();}}}><EditorIcon name="reload"/></button>
 </nav>;
}

function RouteRail(){const params=useSearchParams();return <EditorRail scene={params.get("scene")??undefined}/>;}

export function EditorFrame({children}:{children:ReactNode}){
 const pathname=usePathname(),guard=useRef<(()=>boolean)|null>(null);
 return <NavigationGuard.Provider value={guard}><div className={pathname==="/"?"editor-frame editor-frame-home":"editor-frame"}>{pathname!=="/"&&<Suspense fallback={null}><RouteRail/></Suspense>}{children}</div></NavigationGuard.Provider>;
}
