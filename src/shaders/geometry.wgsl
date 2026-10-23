struct ObjectUniform { model: mat4x4<f32>, model_view_projection: mat4x4<f32>, normal_matrix: mat4x4<f32>, };
struct FrameUniform { light_view_projection: mat4x4<f32>, shadow_enabled: u32, _padding: vec3<u32>, };
struct GpuLight { position_kind: vec4<f32>, color_intensity: vec4<f32>, direction_range: vec4<f32>, spot_and_shadow: vec4<f32>, };
struct LightUniform { lights: array<GpuLight, 8>, count: u32, _padding: vec3<u32>, };

@group(0) @binding(0) var<uniform> object: ObjectUniform;
@group(1) @binding(0) var<uniform> frame: FrameUniform;
@group(1) @binding(1) var<uniform> light_buffer: LightUniform;
@group(1) @binding(2) var shadow_map: texture_depth_2d;
@group(1) @binding(3) var shadow_sampler: sampler_comparison;

struct VertexInput { @location(0) position: vec3<f32>, @location(1) color: vec4<f32>, @location(2) normal: vec3<f32>, };
struct VertexOutput { @builtin(position) clip_position: vec4<f32>, @location(0) color: vec4<f32>, @location(1) world_position: vec3<f32>, @location(2) normal: vec3<f32>, @location(3) shadow_position: vec4<f32>, };

@vertex fn vertex_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let world = object.model * vec4<f32>(input.position, 1.0);
    output.clip_position = object.model_view_projection * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    output.world_position = world.xyz;
    output.normal = normalize((object.normal_matrix * vec4<f32>(input.normal, 0.0)).xyz);
    output.shadow_position = frame.light_view_projection * world;
    return output;
}

fn directional_shadow(input: VertexOutput) -> f32 {
    if frame.shadow_enabled == 0u || input.shadow_position.w <= 0.0 { return 1.0; }
    let projected = input.shadow_position.xyz / input.shadow_position.w;
    let uv = projected.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || projected.z <= 0.0 || projected.z >= 1.0 { return 1.0; }
    return textureSampleCompare(shadow_map, shadow_sampler, uv, projected.z - 0.0015);
}

@fragment fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if light_buffer.count == 0u { return input.color; }
    let surface_normal = normalize(input.normal);
    var illumination = vec3<f32>(0.0);
    for (var index = 0u; index < light_buffer.count && index < 8u; index = index + 1u) {
        let light = light_buffer.lights[index];
        let kind = light.position_kind.w;
        let radiance = light.color_intensity.rgb * light.color_intensity.w;
        if kind < 0.5 { illumination += radiance; }
        else if kind < 1.5 {
            let incoming = normalize(-light.direction_range.xyz);
            let diffuse = max(dot(surface_normal, incoming), 0.0);
            let shadow = select(1.0, directional_shadow(input), light.spot_and_shadow.w > 0.5);
            illumination += radiance * diffuse * shadow;
        } else {
            let to_light = light.position_kind.xyz - input.world_position;
            let distance_to_light = length(to_light);
            if distance_to_light < light.direction_range.w {
                let incoming = to_light / max(distance_to_light, 0.0001);
                let attenuation = pow(max(1.0 - distance_to_light / light.direction_range.w, 0.0), 2.0);
                var cone = 1.0;
                if kind > 2.5 {
                    let from_light = normalize(input.world_position - light.position_kind.xyz);
                    cone = smoothstep(light.spot_and_shadow.y, light.spot_and_shadow.x, dot(normalize(light.direction_range.xyz), from_light));
                }
                illumination += radiance * max(dot(surface_normal, incoming), 0.0) * attenuation * cone;
            }
        }
    }
    return vec4<f32>(input.color.rgb * illumination, input.color.a);
}
