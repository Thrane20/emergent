struct MaterialUniform {
    albedo: vec4<f32>, specular: vec4<f32>, emission: vec4<f32>, params: vec4<f32>,
    map_strength: vec4<f32>, bounds_min: vec4<f32>, bounds_size: vec4<f32>, camera: vec4<f32>, diffuse:vec4<f32>,
    texture_tiling:vec4<f32>, shader_time:vec4<f32>, shader_parameters:array<vec4<f32>,4>,
};
struct ObjectUniform { model: mat4x4<f32>, model_view_projection: mat4x4<f32>, normal_matrix: mat4x4<f32>, material: MaterialUniform, };
struct FrameUniform { light_view_projection: mat4x4<f32>, shadow_enabled: u32, _padding: vec3<u32>, point_view_projections: array<mat4x4<f32>, 12>, };
struct GpuLight { position_kind: vec4<f32>, color_intensity: vec4<f32>, direction_range: vec4<f32>, spot_and_shadow: vec4<f32>, };
struct LightUniform { lights: array<GpuLight, 9>, count: u32, _padding: vec3<u32>, };

@group(0) @binding(0) var<uniform> object: ObjectUniform;
@group(1) @binding(0) var<uniform> frame: FrameUniform;
@group(1) @binding(1) var<uniform> light_buffer: LightUniform;
@group(1) @binding(2) var shadow_map: texture_depth_2d;
@group(1) @binding(3) var shadow_sampler: sampler_comparison;
@group(1) @binding(4) var point_shadow_map: texture_depth_2d_array;
@group(2) @binding(0) var material_noise: texture_3d<f32>;
@group(2) @binding(1) var material_sampler: sampler;
@group(2) @binding(2) var diffuse_texture:texture_2d_array<f32>;
@group(2) @binding(3) var diffuse_sampler:sampler;

@group(2) @binding(4) var terrain_weights:texture_2d<f32>;
struct TerrainLayers { materials:array<MaterialUniform,4>, };
@group(2) @binding(5) var<uniform> terrain_layers:TerrainLayers;
fn noise_sample(uv:vec3<f32>,layer:i32)->vec4<f32>{
    let side=object.material.diffuse.w;
    let p=(clamp(uv,vec3<f32>(0.0),vec3<f32>(1.0))*(side-1.0)+vec3<f32>(0.5))/side;
    return textureSampleLevel(material_noise,material_sampler,vec3<f32>(p.xy,(p.z+f32(layer))/object.material.diffuse.z),0.0);
}
struct VertexInput { @location(0) position: vec3<f32>, @location(1) color: vec4<f32>, @location(2) normal: vec3<f32>, @location(3) uv: vec2<f32>, };
struct VertexOutput { @builtin(position) clip_position: vec4<f32>, @location(0) color: vec4<f32>, @location(1) world_position: vec3<f32>, @location(2) normal: vec3<f32>, @location(3) shadow_position: vec4<f32>, @location(4) local_position: vec3<f32>, @location(5) local_normal:vec3<f32>, @location(6) uv: vec2<f32>, };

struct SurfaceGraph { color:vec3<f32>, emission:f32, opacity:f32, displacement:f32, };
fn surface_noise(x:f32)->f32 {
    let i=floor(x);let f=fract(x);let u=f*f*(3.0-2.0*f);
    return mix(fract(sin(i*127.1)*43758.5453),fract(sin((i+1.0)*127.1)*43758.5453),u);
}
fn surface_smooth(a:f32,b:f32,x:f32)->f32 { let t=clamp((x-a)/max(b-a,0.0001),0.0,1.0);return t*t*(3.0-2.0*t); }
// SURFACE_GRAPH_BEGIN
fn surface_graph(p:vec3<f32>,uv:vec2<f32>,facing:f32)->SurfaceGraph { return SurfaceGraph(vec3<f32>(1.0),0.0,1.0,0.0); }
// SURFACE_GRAPH_END
@vertex fn vertex_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let position = input.position; // SURFACE_VERTEX
    let world = object.model * vec4<f32>(position, 1.0);
    output.clip_position = object.model_view_projection * vec4<f32>(position, 1.0);
    output.color = input.color;
    output.local_position = input.position;
    output.local_normal = input.normal;
    output.uv = input.uv;
    output.world_position = world.xyz;
    output.normal = normalize((object.normal_matrix * vec4<f32>(input.normal, 0.0)).xyz);
    output.shadow_position = frame.light_view_projection * world;
    return output;
}

fn directional_shadow(input: VertexOutput) -> f32 {
    if object.material.params.w < 0.5 || frame.shadow_enabled == 0u || input.shadow_position.w <= 0.0 { return 1.0; }
    let projected = input.shadow_position.xyz / input.shadow_position.w;
    let uv = projected.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || projected.z <= 0.0 || projected.z >= 1.0 { return 1.0; }
    return textureSampleCompare(shadow_map, shadow_sampler, uv, projected.z - 0.0015);
}

fn point_shadow(input: VertexOutput, light: GpuLight) -> f32 {
    if object.material.params.w < 0.5 { return 1.0; }
    let ray = input.world_position - light.position_kind.xyz;
    let axis = abs(ray);
    if max(max(axis.x, axis.y), axis.z) < 0.000001 { return 1.0; }
    var face = select(5u, 4u, ray.z >= 0.0);
    if axis.x >= axis.y && axis.x >= axis.z { face = select(1u, 0u, ray.x >= 0.0); }
    else if axis.y >= axis.z { face = select(3u, 2u, ray.y >= 0.0); }
    let layer = u32(light.spot_and_shadow.z) * 6u + face;
    let bias = clamp(light.direction_range.w * 0.0001, 0.001, 0.02);
    let biased = input.world_position - ray / length(ray) * bias;
    let clip = frame.point_view_projections[layer] * vec4<f32>(biased, 1.0);
    if clip.w <= 0.0 { return 1.0; }
    let projected = clip.xyz / clip.w;
    let uv = projected.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || projected.z <= 0.0 || projected.z >= 1.0 { return 1.0; }
    return textureSampleCompareLevel(point_shadow_map, shadow_sampler, uv, i32(layer), projected.z);
}

fn highlight(normal: vec3<f32>, incoming: vec3<f32>, view: vec3<f32>, shininess: f32) -> f32 {
    let half_vector = incoming + view;
    if dot(normal, incoming) <= 0.0 || dot(half_vector, half_vector) < 0.000001 { return 0.0; }
    return pow(max(dot(normal, normalize(half_vector)), 0.0), shininess);
}

fn shade(input: VertexOutput, material:MaterialUniform, layer:i32) -> vec4<f32> {
    let uv = (input.local_position - material.bounds_min.xyz) / material.bounds_size.xyz;
    let noise = noise_sample(uv,layer);
    let masks = mix(vec4<f32>(1.0), noise, material.map_strength);
    var alpha = input.color.a;
    var albedo = select(input.color.rgb, material.albedo.rgb, material.albedo.w > 0.5) * masks.r;
    let p=input.local_position*material.diffuse.y;
    let dx=dpdx(p);let dy=dpdy(p);
    let tiled_uv=input.uv*material.texture_tiling.xy;
    let uv_dx=dpdx(tiled_uv);let uv_dy=dpdy(tiled_uv);
    if material.diffuse.x>1.5 {
        let image=textureSampleGrad(diffuse_texture,diffuse_sampler,tiled_uv,layer,uv_dx,uv_dy);
        albedo*=image.rgb;alpha*=image.a;
    } else if material.diffuse.x>0.5 {
        let axes=pow(abs(normalize(input.local_normal)),vec3<f32>(4.0));
        let weights=axes/max(axes.x+axes.y+axes.z,0.0001);
        let tiling=material.texture_tiling.xy;
        let image=textureSampleGrad(diffuse_texture,diffuse_sampler,p.yz*tiling,layer,dx.yz*tiling,dy.yz*tiling).rgb*weights.x
            +textureSampleGrad(diffuse_texture,diffuse_sampler,p.xz*tiling,layer,dx.xz*tiling,dy.xz*tiling).rgb*weights.y
            +textureSampleGrad(diffuse_texture,diffuse_sampler,p.xy*tiling,layer,dx.xy*tiling,dy.xy*tiling).rgb*weights.z;
        albedo*=image;
    }
    if alpha<0.001 {discard;}
    var emission = material.emission.rgb * material.emission.w * masks.b;
    // SURFACE_FRAGMENT
    let specular = material.specular.rgb * masks.g;
    if material.params.y > 0.5 || light_buffer.count == 0u { return vec4<f32>(albedo + emission, alpha); }
    var surface_normal = normalize(input.normal);
    if material.bounds_min.w > 0.5 { surface_normal = normalize(cross(dpdx(input.world_position),dpdy(input.world_position))); if dot(surface_normal,input.normal)<0.0 {surface_normal = -surface_normal;} }
    if material.map_strength.a > 0.0 && material.params.x > 0.0 {
        let step = vec3<f32>(1.0) / vec3<f32>(object.material.diffuse.w);
        let dx = vec3<f32>(step.x, 0.0, 0.0);
        let dy = vec3<f32>(0.0, step.y, 0.0);
        let dz = vec3<f32>(0.0, 0.0, step.z);
        let gx = noise_sample(uv + dx,layer).a - noise_sample(uv - dx,layer).a;
        let gy = noise_sample(uv + dy,layer).a - noise_sample(uv - dy,layer).a;
        let gz = noise_sample(uv + dz,layer).a - noise_sample(uv - dz,layer).a;
        let gradient_local = vec3<f32>(gx, gy, gz) / (2.0 * step * material.bounds_size.xyz);
        let gradient_world = (object.normal_matrix * vec4<f32>(gradient_local, 0.0)).xyz;
        let tangent_gradient = gradient_world - surface_normal * dot(surface_normal, gradient_world);
        surface_normal = normalize(surface_normal - material.params.x * material.map_strength.a * tangent_gradient);
    }
    let to_view = object.material.camera.xyz - input.world_position;
    let view = to_view / max(length(to_view), 0.0001);
    var illumination = vec3<f32>(0.0);
    var highlights = vec3<f32>(0.0);
    for (var index = 0u; index < light_buffer.count && index < 9u; index = index + 1u) {
        let light = light_buffer.lights[index];
        let kind = light.position_kind.w;
        let radiance = light.color_intensity.rgb * light.color_intensity.w;
        if kind < 0.5 { illumination += radiance; }
        else {
            var incoming = normalize(-light.direction_range.xyz);
            var attenuation = 1.0;
            if kind < 1.5 {
                attenuation = select(1.0, directional_shadow(input), light.spot_and_shadow.w > 0.5 && material.params.w > 0.5);
            } else {
                let to_light = light.position_kind.xyz - input.world_position;
                let distance_to_light = length(to_light);
                incoming = to_light / max(distance_to_light, 0.0001);
                attenuation = pow(max(1.0 - distance_to_light / light.direction_range.w, 0.0), 2.0);
                if kind < 2.5 && light.spot_and_shadow.w > 0.5 && material.params.w > 0.5 { attenuation *= point_shadow(input, light); }
                if kind > 2.5 {
                    attenuation *= smoothstep(light.spot_and_shadow.y, light.spot_and_shadow.x, dot(normalize(light.direction_range.xyz), -incoming));
                }
            }
            illumination += radiance * max(dot(surface_normal, incoming), 0.0) * attenuation;
            highlights += radiance * highlight(surface_normal, incoming, view, material.specular.w) * attenuation;
        }
    }
    return vec4<f32>(albedo * illumination + specular * highlights + emission, alpha);
}

@fragment fn fragment_main(input:VertexOutput)->@location(0) vec4<f32>{
    let count=i32(object.material.diffuse.z);
    if count<=1 { return shade(input,terrain_layers.materials[0],0); }
    let uv=(input.local_position.xz-object.material.bounds_min.xz)/object.material.bounds_size.xz;
    let size=vec2<f32>(textureDimensions(terrain_weights));
    let weights=textureSampleLevel(terrain_weights,material_sampler,(uv*(size-1.0)+0.5)/size,0.0);
    var result=vec4<f32>(0.0);
    for(var layer=0;layer<count && layer<4;layer+=1){result+=shade(input,terrain_layers.materials[layer],layer)*weights[layer];}
    return result;
}
