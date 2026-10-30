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
