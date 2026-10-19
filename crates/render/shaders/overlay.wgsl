// The debug overlay: triangles in target pixels (y down), font-atlas texture times
// vertex colour, alpha-blended over the target.

struct Target {
    size: vec2<f32>,
    pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> target_size: Target;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let clip = vec2(
        in.pos.x / target_size.size.x * 2.0 - 1.0,
        1.0 - in.pos.y / target_size.size.y * 2.0,
    );
    out.position = vec4(clip, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color * textureSample(atlas, atlas_sampler, in.uv);
}
