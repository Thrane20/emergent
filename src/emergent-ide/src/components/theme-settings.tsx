"use client";
import {useEffect,useRef} from "react";
import {DEFAULT_THEME,THEME_STORAGE_KEY,type Theme} from "@/lib/theme";

export function ThemeSettings(){
 const select=useRef<HTMLSelectElement>(null);
 useEffect(()=>{if(select.current)select.current.value=document.documentElement.dataset.theme??DEFAULT_THEME;},[]);
 function change(theme:Theme){
  document.documentElement.dataset.theme=theme;
  try{localStorage.setItem(THEME_STORAGE_KEY,theme);}catch{/* Keep the selection for this session if browser storage is unavailable. */}
 }
 return <section className="settings-panel" id="appearance">
  <div className="settings-panel-head"><div><span className="eyebrow">MAKE IT YOURS</span><h2>Appearance</h2></div><span className="local-only">THIS BROWSER</span></div>
  <div className="settings-form theme-settings"><label className="settings-field"><span>IDE theme</span><select ref={select} aria-label="IDE theme" defaultValue={DEFAULT_THEME} onChange={event=>change(event.target.value as Theme)}><option value="charcoal">Charcoal · soft depth</option><option value="green">Emergent green · original</option></select></label><p className="theme-description">Charcoal pairs neutral graphite surfaces with softly raised panels and inset controls. Your choice applies across the IDE and is remembered in this browser.</p></div>
 </section>;
}
