struct Sky { inverse_view_projection: mat4x4<f32>, params: vec4<f32> };
@group(0) @binding(0) var<uniform> sky: Sky;
@group(0) @binding(1) var panorama: texture_2d<f32>;
@group(0) @binding(2) var panorama_sampler: sampler;
struct Vertex { @builtin(position) position: vec4<f32>, @location(0) clip: vec2<f32> };
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> Vertex {
 let p = array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.))[index];
 return Vertex(vec4(p,1.,1.),p);
}
@fragment fn fs_main(v: Vertex) -> @location(0) vec4<f32> {
 let near = sky.inverse_view_projection * vec4(v.clip,0.,1.);
 let far = sky.inverse_view_projection * vec4(v.clip,0.99,1.);
 let ray = normalize(far.xyz/far.w - near.xyz/near.w);
 let c = cos(sky.params.x); let s = sin(sky.params.x);
 let direction = vec3(c*ray.x+s*ray.z,ray.y,-s*ray.x+c*ray.z);
 let uv = vec2(atan2(direction.z,direction.x)/6.28318530718+0.5,acos(clamp(direction.y,-1.,1.))/3.14159265359);
 return vec4(textureSample(panorama,panorama_sampler,uv).rgb*sky.params.y,1.);
}
