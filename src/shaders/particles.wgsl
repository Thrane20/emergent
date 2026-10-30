struct Frame {
    vp: mat4x4<f32>,
    inverse: mat4x4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
}
@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var scene_depth: texture_depth_2d;
@group(1) @binding(0) var sprite: texture_2d<f32>;
@group(1) @binding(1) var sprite_sampler: sampler;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) world: vec3<f32>,
    @location(3) soft: f32,
}

@vertex fn vertex_main(
    @builtin(vertex_index) index: u32,
    @location(0) pos: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec4<f32>,
    @location(3) params: vec4<f32>,
) -> Out {
    let corners = array<vec2<f32>, 6>(
        vec2(-0.5, -0.5), vec2(0.5, -0.5), vec2(0.5, 0.5),
        vec2(-0.5, -0.5), vec2(0.5, 0.5), vec2(-0.5, 0.5),
    );
    let q = corners[index];
    let c = cos(params.x);
    let s = sin(params.x);
    let rotated = vec2(c*q.x - s*q.y, s*q.x + c*q.y) * pos.w;
    var out: Out;
    out.world = pos.xyz + frame.right.xyz*rotated.x + frame.up.xyz*rotated.y;
    out.clip = frame.vp * vec4(out.world, 1.);
    // Half-texel insets prevent filtering across neighboring atlas cells.
    out.uv = uv.xy + params.zw*0.5 + vec2(q.x+0.5, 0.5-q.y)*(uv.zw-params.zw);
    out.color = color;
    out.soft = params.y;
    return out;
}

@fragment fn fragment_main(input: Out) -> @location(0) vec4<f32> {
    let tex = textureSample(sprite, sprite_sampler, input.uv);
    var opacity = input.color.a;
    // Negative soft encodes additive mode; both modes share one sorted pass.
    let additive = input.soft < 0.;
    let soft = select(input.soft, -input.soft-1., additive);
    if soft > 0. {
        let size = vec2<f32>(textureDimensions(scene_depth));
        let depth = textureLoad(scene_depth, vec2<i32>(input.clip.xy), 0);
        let ndc = vec2(input.clip.x/size.x*2.-1., 1.-input.clip.y/size.y*2.);
        let world = frame.inverse * vec4(ndc, depth, 1.);
        let forward = -normalize(cross(frame.right.xyz, frame.up.xyz));
        let separation = dot(world.xyz/world.w - input.world, forward);
        opacity *= clamp(separation/soft, 0., 1.);
    }
    // Sprites are premultiplied in linear light before upload. Zero output alpha
    // gives additive emission while allowing nearer smoke to attenuate it.
    return vec4(tex.rgb*input.color.rgb*opacity, select(tex.a*opacity, 0., additive));
}
