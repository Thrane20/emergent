import {readFileSync} from "node:fs";
import assert from "node:assert/strict";
import {runInNewContext} from "node:vm";
import test from "node:test";
import {DEFAULT_THEME,THEME_STORAGE_KEY,themeScript} from "../src/lib/theme";

test("theme initializes before hydration, accepts only supported preferences, and tolerates denied storage",()=>{
 for(const saved of [null,"green","charcoal","invalid",'<script>alert(1)</script>']){
  const root={dataset:{theme:DEFAULT_THEME}};
  runInNewContext(themeScript,{document:{documentElement:root},localStorage:{getItem(key:string){assert.equal(key,THEME_STORAGE_KEY);return saved;}}});
  assert.equal(root.dataset.theme,saved==="green"?"green":"charcoal");
 }
 const root={dataset:{theme:DEFAULT_THEME}};
 assert.doesNotThrow(()=>runInNewContext(themeScript,{document:{documentElement:root},localStorage:{getItem(){throw new Error("Storage denied");}}}));
 assert.equal(root.dataset.theme,"charcoal");
});

test("charcoal semantic colour pairs have readable text contrast",()=>{
 const css=readFileSync("src/app/themes.css","utf8"),block=css.split(':root[data-theme="charcoal"]{')[1].split("}")[0];
 const colour=(name:string)=>{const hex=block.match(new RegExp(`--theme-${name}:#([0-9a-f]{6});`))![1];return [0,2,4].map(i=>{const v=parseInt(hex.slice(i,i+2),16)/255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4;}).reduce((sum,v,i)=>sum+v*[.2126,.7152,.0722][i],0);};
 for(const role of ["primary","secondary"]){const a=colour(role),b=colour(`on-${role}`);assert.ok((Math.max(a,b)+.05)/(Math.min(a,b)+.05)>=4.5);}
});
