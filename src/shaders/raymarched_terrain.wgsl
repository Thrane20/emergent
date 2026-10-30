struct Field {
 inverse_vp: mat4x4<f32>, vp: mat4x4<f32>, normal_model: mat4x4<f32>, params: vec4<f32>, detail: vec4<f32>, recipe: vec4<u32>
};
@group(0) @binding(0) var<uniform> field: Field;
fn random_cell(c:vec3<i32>, seed:u32)->f32 {
 var h=u32(c.x)*374761393u ^ u32(c.y)*668265263u ^ u32(c.z)*2147483647u ^ seed*1274126177u;
 h=(h^(h>>13u))*1274126177u;return f32((h^(h>>16u))&65535u)/65535.;
}
fn noise(p:vec3<f32>,salt:u32)->f32 {
 let c=vec3<i32>(floor(p));let v=fract(p);let f=v*v*(3.-2.*v);let seed=field.recipe.y+salt;
 let a=mix(mix(random_cell(c,seed),random_cell(c+vec3(1,0,0),seed),f.x),mix(random_cell(c+vec3(0,0,1),seed),random_cell(c+vec3(1,0,1),seed),f.x),f.z);
 let b=mix(mix(random_cell(c+vec3(0,1,0),seed),random_cell(c+vec3(1,1,0),seed),f.x),mix(random_cell(c+vec3(0,1,1),seed),random_cell(c+vec3(1,1,1),seed),f.x),f.z);
 return mix(a,b,f.y);
}
fn noise2(p:vec2<f32>,salt:u32)->f32 {return noise(vec3(p.x,0.,p.y),salt);}
fn biome(p:vec3<f32>)->f32 {
 let s=field.params.w;
 var mask=1.;if(field.recipe.z==0u || field.recipe.z==4u || field.recipe.z==5u){mask=smoothstep(0.35,0.65,noise2(p.xz/(s*2.),11u));}
 let rolling=4.*noise2(p.xz/(s*0.2),53u);
 var height=8.+rolling;
 // Plain, hills, seabed and canyon replace mountain height entirely.
 if(field.recipe.z!=1u && field.recipe.z!=2u && field.recipe.z!=7u && field.recipe.z!=6u) {
  let ridge=1.-abs(2.*noise2(p.xz/s,29u)-1.);
  height+=field.params.z*mask*(0.15+0.7*ridge+0.15*noise2(p.xz/(s*0.25),71u));
 }
 switch field.recipe.z {
 case 8u: {height=max(height,field.params.z*0.85+rolling);}
 case 1u: {height=8.+rolling;}
 case 2u: {height=8.+rolling+field.params.z*0.25*noise2(p.xz/(s*0.5),31u);}
 case 7u: {height=-field.params.z*0.25+rolling+field.params.z*0.18*noise2(p.xz/s,31u);}
 case 4u: {let coast=1.-smoothstep(0.15,0.48,length(p.xz)/field.params.x);height=(height+12.)*coast-12.;}
 case 5u: {let channel=p.x-s*0.3*sin(p.z/(s*0.8)+field.params.y*6.2831855);height-=field.params.z*0.5*exp(-pow(channel/(s*0.12),2.));}
 case 6u: {let channel=p.x-s*0.3*sin(p.z/(s*0.8)+field.params.y*6.2831855);height=8.+rolling+field.params.z*(0.6+0.15*noise2(p.xz/s,31u))-field.params.z*0.8*(1.-smoothstep(0.06,0.25,abs(channel)/s));}
 default: {}
 }
 var rock=0.;if(field.detail.x>0.){rock=(noise(p/(s*0.08),97u)*2.-1.)*field.detail.x*mask;}
 var d=(p.y-height-rock)/field.detail.z;
 if(field.recipe.z==8u){d=max(d,-(length(p-vec3(0.,field.params.z*0.25,0.))-max(field.detail.y*3.,24.)));}
 if(field.detail.y>0.) {
  let phase=field.params.y*6.2831855;
  let a=length(vec2(p.y-(field.params.z*0.28+8.*sin(p.x/50.+phase)),p.z-s*0.3*sin(p.x/(s*0.5)+phase)))-field.detail.y;
  let b=length(vec2(p.y-(field.params.z*0.22+6.*cos(p.z/(s*0.3)+phase)),p.x-(-s*0.2+s*0.15*sin(p.z/(s*0.4)+phase))))-field.detail.y*0.8;
  d=max(d,-min(a/1.18,b/1.24));
 }
 let lower=vec3(-field.params.x*0.5,-max(field.params.z,64.)*0.5,-field.params.x*0.5);
 let upper=vec3(field.params.x*0.5,12.+field.params.z+field.detail.x,field.params.x*0.5);
 let q=abs(p-(lower+upper)*0.5)-(upper-lower)*0.5;
 return max(d,length(max(q,vec3(0.)))+min(max(q.x,max(q.y,q.z)),0.));
}
fn distance(p_world: vec3<f32>) -> f32 {
 if(field.recipe.x==2u) {return biome(p_world);}
 let scale = field.params.x / 32.; let p = p_world / scale;
 let q = abs(p - vec3(0.,-1.,0.)) - vec3(16.,1.,16.);
 let ground = length(max(q,vec3(0.))) + min(max(q.x,max(q.y,q.z)),0.);
 let m = p - vec3(-5.,0.,field.params.y*2.-1.);
 let mountain = length(m)-6.;
 let tunnel = length(vec2(m.x,m.y-1.))-2.;
 let carved = max(min(ground,mountain),-tunnel);
 let a = p-vec3(7.,0.,0.);
 let arch = length(vec2(length(a.xy)-4.,a.z))-1.;
 return min(carved,arch)*scale;
}
struct Vertex { @builtin(position) position: vec4<f32>, @location(0) clip: vec2<f32> };
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> Vertex {
 let p = array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.))[index];
 return Vertex(vec4(p,0.,1.),p);
}
struct Hit { @location(0) color: vec4<f32>, @builtin(frag_depth) depth: f32 };
@fragment fn fs_main(v: Vertex) -> Hit {
 let near = field.inverse_vp * vec4(v.clip,0.,1.);
 let far = field.inverse_vp * vec4(v.clip,1.,1.);
 let origin = near.xyz/near.w;
 let delta = far.xyz/far.w-origin;
 let direction = normalize(delta);
 let scale = field.params.x/32.;
 let lower = select(vec3(-16.,-2.,-16.)*scale,vec3(-field.params.x*0.5,-max(field.params.z,64.)*0.5,-field.params.x*0.5),field.recipe.x==2u);
 let upper = select(vec3(16.,6.,16.)*scale,vec3(field.params.x*0.5,12.+field.params.z+field.detail.x,field.params.x*0.5),field.recipe.x==2u);
 var entry=0.; var limit=length(delta);
 for(var axis=0u;axis<3u;axis++) {
  if(abs(direction[axis])<0.0000001) {
   if(origin[axis]<lower[axis] || origin[axis]>upper[axis]) { discard; }
  } else {
   let a=(lower[axis]-origin[axis])/direction[axis];
   let b=(upper[axis]-origin[axis])/direction[axis];
   entry=max(entry,min(a,b)); limit=min(limit,max(a,b));
  }
 }
 if(entry>limit) { discard; }
 var t = entry; var found = false; var steps=0u;
 let budget=select(256u,1024u,field.recipe.x==2u);
 for(var i=0u;i<budget;i++) {
  steps=i+1u;
  let d = abs(distance(origin+direction*t));
  if(d<select(0.002,max(0.003,t*0.00003),field.recipe.x==2u)) { found=true; break; }
  t += d*0.9;
  if(t>limit) { break; }
 }
 if(!found || t>limit) {
  if(field.recipe.w==1u && !found && t<=limit && steps==budget) {return Hit(vec4(1.,0.,1.,1.),0.);}
  discard;
 }
 if(field.recipe.w==1u) {
  let heat=clamp(f32(steps)/f32(budget),0.,1.);
  let colour=mix(vec3(0.02,0.15,0.8),vec3(1.,0.05,0.),heat);
  let clip=field.vp*vec4(origin+direction*t,1.);
  return Hit(vec4(colour,1.),clip.z/clip.w);
 }
 let p = origin+direction*t; let e=select(0.002,0.05,field.recipe.x==2u);
 let normal = normalize(vec3(distance(p+vec3(e,0.,0.))-distance(p-vec3(e,0.,0.)),distance(p+vec3(0.,e,0.))-distance(p-vec3(0.,e,0.)),distance(p+vec3(0.,0.,e))-distance(p-vec3(0.,0.,e))));
 let world_normal=normalize((field.normal_model*vec4(normal,0.)).xyz);
 let shade = 0.2+0.8*max(dot(world_normal,normalize(vec3(-0.4,0.8,0.3))),0.);
 var colour = mix(vec3(0.18,0.28,0.10),vec3(0.5,0.4,0.27),smoothstep(0.,4.,p.y));
 if(field.recipe.x==2u) {
  let grass=mix(vec3(0.12,0.22,0.045),vec3(0.3,0.39,0.10),noise2(p.xz/24.,101u));
  let exposed=clamp(smoothstep(0.15,0.65,1.-normal.y)+smoothstep(field.params.z*0.35,field.params.z*0.85,p.y)*0.65,0.,1.);
  colour=mix(grass,vec3(0.28,0.265,0.24),exposed);
  let snow=smoothstep(field.params.z*0.85,field.params.z*1.05,p.y)*smoothstep(0.65,0.9,normal.y);
  colour=mix(colour,vec3(0.82,0.86,0.86),snow);
  if(field.recipe.z==6u){colour=mix(vec3(0.36,0.19,0.09),vec3(0.58,0.37,0.17),noise2(p.xz/32.,101u));}
  if(field.recipe.z==7u){colour=mix(vec3(0.15,0.25,0.22),vec3(0.3,0.36,0.25),clamp(normal.y,0.,1.));}
  if(field.recipe.z==8u){colour=mix(vec3(0.18,0.15,0.12),vec3(0.32,0.29,0.23),noise2(p.xz/20.,101u));}
 }
 let clip = field.vp*vec4(p,1.);
 return Hit(vec4(colour*shade,1.),clip.z/clip.w);
}
