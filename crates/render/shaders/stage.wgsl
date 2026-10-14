// Futile stage quads in screen pixels, alpha blended (SrcAlpha, OneMinusSrcAlpha) by
// the pipeline. fs_basic is `Futile/Basic`; fs_edge_fade is `Futile/EdgeFade`
// (StreamingAssets/shaders/EdgeFade.shader).

struct Camera {
    // xy: scale, zw: offset; clip = pos * scale + offset.
    transform: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var main_tex: texture_2d<f32>;
@group(1) @binding(1) var main_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(pos * camera.transform.xy + camera.transform.zw, 0.0, 1.0);
    out.uv = uv;
    out.color = color;
    return out;
}

@fragment
fn fs_basic(in: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(main_tex, main_sampler, in.uv) * in.color;
}

// The texture is not read: the alpha is a function of the distance from the quad's
// centre in texture space and of the vertex alpha `a`.
@fragment
fn fs_edge_fade(in: VertexOut) -> @location(0) vec4<f32> {
    let a = in.color.a;
    let center_dist = clamp(distance(in.uv, vec2<f32>(0.5, 0.5)) * 2.0, 0.0, 1.0);
    let inner = 1.0 - pow(center_dist, mix(5.0, 0.5, a));
    var c = 1.0 - pow(max(inner, 0.0), 2.0);
    c = mix(c, 1.0, max(0.0, pow(a, 2.0) - 0.5) * 2.0);
    return vec4<f32>(in.color.rgb, c * pow(a, 0.25));
}
