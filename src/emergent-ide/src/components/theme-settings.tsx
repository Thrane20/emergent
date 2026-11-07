"use client";
import {useEffect,useRef} from "react";
import {DEFAULT_FONT,DEFAULT_THEME,FONT_STORAGE_KEY,THEME_STORAGE_KEY,type Font,type Theme} from "@/lib/theme";

export function ThemeSettings(){
 const select=useRef<HTMLSelectElement>(null);
 const fontSelect=useRef<HTMLSelectElement>(null);
 useEffect(()=>{
  if(select.current)select.current.value=document.documentElement.dataset.theme??DEFAULT_THEME;
  if(fontSelect.current)fontSelect.current.value=document.documentElement.dataset.font??DEFAULT_FONT;
 },[]);
 function change(theme:Theme){
  document.documentElement.dataset.theme=theme;
  try{localStorage.setItem(THEME_STORAGE_KEY,theme);}catch{/* Keep the selection for this session if browser storage is unavailable. */}
 }
 function changeFont(font:Font){
  document.documentElement.dataset.font=font;
  try{localStorage.setItem(FONT_STORAGE_KEY,font);}catch{/* Keep the selection for this session if browser storage is unavailable. */}
 }
 return <section className="settings-panel" id="appearance">
  <div className="settings-panel-head"><div><span className="eyebrow">MAKE IT YOURS</span><h2>Appearance</h2></div><span className="local-only">THIS BROWSER</span></div>
  <div className="settings-form theme-settings"><label className="settings-field"><span>IDE theme</span><select ref={select} aria-label="IDE theme" defaultValue={DEFAULT_THEME} onChange={event=>change(event.target.value as Theme)}><option value="charcoal">Charcoal · purple & cyan</option><option value="green">Emergent green · original</option></select></label><label className="settings-field"><span>IDE font</span><select ref={fontSelect} aria-label="IDE font" defaultValue={DEFAULT_FONT} onChange={event=>changeFont(event.target.value as Font)}><option value="krypton">Monaspace Krypton · default</option><option value="jetbrains">JetBrains Mono</option><option value="fira">Fira Code</option></select></label></div>
 </section>;
}
