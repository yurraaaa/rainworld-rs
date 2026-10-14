// Futile stage quads in screen pixels, alpha blended (SrcAlpha, OneMinusSrcAlpha) by
// the pipeline. One fragment entry per game shader (StreamingAssets/shaders/<Name>.shader):
// fs_basic `Futile/Basic`, fs_edge_fade `EdgeFade`, fs_flat_light `FlatLight`,
// fs_vector_circle `VectorCircle`, fs_vector_circle_fadable `VectorCircleFadable`,
// fs_hold_button_circle `HoldButtonCircle`, fs_menu_text `MenuText`.

struct Stage {
    // xy: scale, zw: offset; clip = pos * scale + offset.
    transform: vec4<f32>,
    // The stage size in pixels (Futile's screen).
    screen: vec2<f32>,
    // `_RAIN`: `ProcessManager.shadersTime / 5`.
    rain: f32,
    pad: f32,
};

@group(0) @binding(0) var<uniform> stage: Stage;
@group(1) @binding(0) var main_tex: texture_2d<f32>;
@group(1) @binding(1) var main_sampler: sampler;
// `_TextGradientTex` (`Palettes/textGradient`).
@group(2) @binding(0) var text_gradient: texture_2d<f32>;
@group(2) @binding(1) var text_gradient_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    // `ComputeScreenPos` with an orthographic camera: 0..1 over the screen, y up.
    @location(2) scr: vec2<f32>,
};

@vertex
fn vs_main(
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(pos * stage.transform.xy + stage.transform.zw, 0.0, 1.0);
    out.uv = uv;
    out.color = color;
    out.scr = pos / stage.screen;
    return out;
}

@fragment
fn fs_basic(in: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(main_tex, main_sampler, in.uv) * in.color;
}

fn center_dist(uv: vec2<f32>) -> f32 {
    return clamp(distance(uv, vec2<f32>(0.5, 0.5)) * 2.0, 0.0, 1.0);
}

// The texture is not read: the alpha is a function of the distance from the quad's
// centre in texture space and of the vertex alpha `a`.
@fragment
fn fs_edge_fade(in: VertexOut) -> @location(0) vec4<f32> {
    let a = in.color.a;
    let inner = 1.0 - pow(center_dist(in.uv), mix(5.0, 0.5, a));
    var c = 1.0 - pow(max(inner, 0.0), 2.0);
    c = mix(c, 1.0, max(0.0, pow(a, 2.0) - 0.5) * 2.0);
    return vec4<f32>(in.color.rgb, c * pow(a, 0.25));
}

// A soft disc: `(1 - d^2)^3.5` of the vertex alpha.
@fragment
fn fs_flat_light(in: VertexOut) -> @location(0) vec4<f32> {
    let d = center_dist(in.uv);
    let c = pow(1.0 - pow(d, 2.0), 3.5);
    return vec4<f32>(in.color.rgb, c * in.color.a);
}

// An opaque ring of the vertex colour, from radius `1 - a` to the edge.
@fragment
fn fs_vector_circle(in: VertexOut) -> @location(0) vec4<f32> {
    let d = center_dist(in.uv);
    if (d < 1.0 - in.color.a || d == 1.0) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(in.color.rgb, 1.0);
}

// The ring of `fs_vector_circle` in one of eight palette colours picked by
// `round(r * 255)`, with alpha from the vertex colour's blue.
@fragment
fn fs_vector_circle_fadable(in: VertexOut) -> @location(0) vec4<f32> {
    let d = center_dist(in.uv);
    if (d < 1.0 - in.color.a || d == 1.0) {
        return vec4<f32>(0.0);
    }
    let index = i32(round(in.color.r * 255.0));
    var col = vec3<f32>(1.0, 1.0, 1.0);
    if (index == 1) {
        col = vec3<f32>(1.0, 0.0, 0.0);
    } else if (index == 2) {
        col = vec3<f32>(0.0, 1.0, 0.0);
    } else if (index == 3) {
        col = vec3<f32>(0.0, 0.0, 1.0);
    } else if (index == 4) {
        col = vec3<f32>(0.529, 0.365, 0.184);
    } else if (index == 5) {
        col = vec3<f32>(0.0, 0.0, 0.0);
    } else if (index == 6) {
        col = vec3<f32>(1.0, 0.7, 0.35);
    } else if (index == 7) {
        col = vec3<f32>(0.373, 0.11, 0.831);
    }
    return vec4<f32>(col, in.color.b);
}

// The outer tenth of the disc, swept clockwise from the bottom up to the fraction `a`.
@fragment
fn fs_hold_button_circle(in: VertexOut) -> @location(0) vec4<f32> {
    let d = center_dist(in.uv);
    let dir = normalize(vec2<f32>(0.5, 0.5) - in.uv);
    let ang = 3.141592 + atan2(dir.x, dir.y);
    if (d < 0.9 || d == 1.0 || ang / (2.0 * 3.141592) > in.color.a) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(in.color.rgb, 1.0);
}

// Title lettering: colour from the text gradient, banded in 50 steps along a diagonal
// of the screen that moves with `_RAIN`; alpha from the texture.
@fragment
fn fs_menu_text(in: VertexOut) -> @location(0) vec4<f32> {
    let texel = textureSample(main_tex, main_sampler, in.uv);
    let h = (in.scr.x * 0.7 - in.scr.y * 0.19 + stage.rain * 0.7) * mix(0.75, 1.0, in.color.r)
        + pow(texel.r, 3.0) * -0.0125;
    let gradient = textureSample(text_gradient, text_gradient_sampler, vec2<f32>(floor(h * 50.0) / 50.0, 0.5));
    return vec4<f32>(gradient.rgb, texel.a);
}
