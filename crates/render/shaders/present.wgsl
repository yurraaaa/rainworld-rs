// Shows the finished frame in the window: one triangle covering the viewport (set to
// the letterbox rectangle), sampling the frame texture.

@group(0) @binding(0) var frame_tex: texture_2d<f32>;
@group(0) @binding(1) var frame_sampler: sampler;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    let clip = vec2(f32(index & 1u) * 4.0 - 1.0, f32(index >> 1u) * 4.0 - 1.0);
    var out: VsOut;
    out.position = vec4(clip, 0.0, 1.0);
    // Texture rows run top to bottom, clip y bottom to top.
    out.uv = vec2(clip.x * 0.5 + 0.5, 0.5 - clip.y * 0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(frame_tex, frame_sampler, in.uv);
}
