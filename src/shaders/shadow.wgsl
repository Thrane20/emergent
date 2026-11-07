struct ObjectUniform {
    model: mat4x4<f32>,
    model_view_projection: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
};

struct FrameUniform {
    light_view_projection: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> object: ObjectUniform;
@group(1) @binding(0) var<uniform> frame: FrameUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) normal: vec3<f32>,
};

@vertex
fn shadow_vertex_main(input: VertexInput) -> @builtin(position) vec4<f32> {
    return frame.light_view_projection * object.model * vec4<f32>(input.position, 1.0);
}

@group(2) @binding(0) var<storage, read> joint_palette: array<mat4x4<f32>>;

struct SkinnedInput {
    @location(0) position: vec3<f32>,
    @location(4) joints: vec4<u32>,
    @location(5) weights: vec4<f32>,
};

@vertex
fn shadow_vertex_skinned(input: SkinnedInput) -> @builtin(position) vec4<f32> {
    var position = vec4<f32>(input.position, 1.0);
    if dot(input.weights, vec4<f32>(1.0)) >= 0.0001 {
        let j = input.joints;
        let w = input.weights;
        position = (joint_palette[j.x] * w.x + joint_palette[j.y] * w.y + joint_palette[j.z] * w.z + joint_palette[j.w] * w.w) * position;
    }
    return frame.light_view_projection * object.model * position;
}
