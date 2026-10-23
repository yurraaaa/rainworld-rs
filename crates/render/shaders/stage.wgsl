// Futile stage quads in screen pixels, alpha blended (SrcAlpha, OneMinusSrcAlpha) by
// the pipeline. One fragment entry per game shader (StreamingAssets/shaders/<Name>.shader):
// fs_basic `Futile/Basic`, fs_edge_fade `EdgeFade`, fs_flat_light `FlatLight`,
// fs_vector_circle `VectorCircle`, fs_vector_circle_fadable `VectorCircleFadable`,
// fs_hold_button_circle `HoldButtonCircle`, fs_menu_text `MenuText`,
// fs_flat_light_noisy `FlatLightNoisy`, fs_local_bloom `LocalBloom` (reads group 3),
// fs_scene_blur `SceneBlur`, fs_scene_blur_light_edges `SceneBlurLightEdges`,
// fs_scene_lighten `SceneLighten`, fs_scene_overlay `SceneOverlay`,
// fs_scene_soft_light `SceneSoftLight` (the last three read group 3).

struct Stage {
    // xy: scale, zw: offset; clip = pos * scale + offset.
    transform: vec4<f32>,
    // The stage size in pixels (Futile's screen).
    screen: vec2<f32>,
    // `_RAIN`: `ProcessManager.shadersTime / 5`.
    rain: f32,
    pad: f32,
    // `_spriteRect`: (0, 0, 0, 0) unless a room camera set it.
    sprite_rect: vec4<f32>,
    // `_BlurDepth`, `_BlurRange`, `_MenuCamPos` (set by `MenuScene.GrafUpdate`).
    menu_scene: vec4<f32>,
};

// Futile's `screenPixelOffset` (`Camera::SCREEN_PIXEL_OFFSET`): screen = stage + this.
const SCREEN_PIXEL_OFFSET: vec2<f32> = vec2(-0.5, 0.5);

@group(0) @binding(0) var<uniform> stage: Stage;
@group(1) @binding(0) var main_tex: texture_2d<f32>;
@group(1) @binding(1) var main_sampler: sampler;
// `_TextGradientTex` (`Palettes/textGradient`).
@group(2) @binding(0) var text_gradient: texture_2d<f32>;
@group(2) @binding(1) var text_gradient_sampler: sampler;
// `_NoiseTex2` (`Palettes/noise2`).
@group(2) @binding(2) var noise2: texture_2d<f32>;
@group(2) @binding(3) var noise2_sampler: sampler;

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
    out.scr = (pos + SCREEN_PIXEL_OFFSET) / stage.screen;
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

// `FlatLight`'s disc with its falloff exponent bent by the noise: `h` is four `_NoiseTex2`
// reads (mip level from the block's derivatives) at the pixel's place within `_spriteRect`, nudged towards the quad's centre;
// `d = 0.25 * sqrt(1 - a)`, exponent `1 - d + 2 d h`. With `_spriteRect` unset (menus)
// the place divides by zero, as in the game.
@fragment
fn fs_flat_light_noisy(in: VertexOut) -> @location(0) vec4<f32> {
    let rect = stage.sprite_rect;
    var tc = floor(in.scr * stage.screen) / stage.screen;
    tc = (tc - rect.xy) / (rect.zw - rect.xy);
    let towards = normalize(vec2<f32>(0.5, 0.5) - in.uv);
    var h = 0.0;
    for (var j = 0; j < 4; j++) {
        let at = vec2<f32>(tc.x * 6.0, tc.y * 3.0) + towards * 0.02 * f32(j);
        h += textureSample(noise2, noise2_sampler, at).x;
    }
    h /= 5.0;
    let d = 0.25 * pow(1.0 - in.color.a, 0.5);
    let c = pow(pow(1.0 - pow(center_dist(in.uv), 2.0), 3.5), 1.0 - d + 2.0 * d * h);
    return vec4<f32>(in.color.rgb, c * in.color.a);
}

// `LocalBloom`'s `_GrabTexture`: the frame drawn before the sprite (its GrabPass),
// sampled bilinearly, clamped.
@group(3) @binding(0) var grab_tex: texture_2d<f32>;
@group(3) @binding(1) var grab_sampler: sampler;

// The grab at a screen position (`ComputeScreenPos`: 0..1, y up).
fn grab_at(scr: vec2<f32>) -> vec4<f32> {
    let clip = (scr * stage.screen - SCREEN_PIXEL_OFFSET) * stage.transform.xy + stage.transform.zw;
    let uv = vec2<f32>((clip.x + 1.0) * 0.5, (1.0 - clip.y) * 0.5);
    return textureSampleLevel(grab_tex, grab_sampler, uv, 0.0);
}

// Inside the disc: three rings of eight grab reads (the diagonals at 2/3 weight), each
// ring 0.88 times the last, averaged and scaled by the vertex alpha and a linear falloff
// to the edge; the frame under it is raised to that blur and the blur added on top.
@fragment
fn fs_local_bloom(in: VertexOut) -> @location(0) vec4<f32> {
    let d = distance(in.uv, vec2<f32>(0.5, 0.5));
    if (d > 0.5) {
        return vec4<f32>(0.0);
    }
    let blur = 0.0012;
    let hor = stage.screen.y / stage.screen.x;
    var sum = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    var coef = 1.0;
    var div = 0.0;
    for (var j = 1; j <= 3; j++) {
        coef *= 0.88;
        let dx = f32(j) * blur * hor;
        let dy = f32(j) * blur;
        sum += grab_at(in.scr + vec2<f32>(0.0, dy)) * coef;
        sum += grab_at(in.scr + vec2<f32>(-dx, 0.0)) * coef;
        sum += grab_at(in.scr + vec2<f32>(dx, 0.0)) * coef;
        sum += grab_at(in.scr + vec2<f32>(0.0, -dy)) * coef;
        sum += grab_at(in.scr + vec2<f32>(-dx, dy)) * coef / 1.5;
        sum += grab_at(in.scr + vec2<f32>(-dx, -dy)) * coef / 1.5;
        sum += grab_at(in.scr + vec2<f32>(dx, dy)) * coef / 1.5;
        sum += grab_at(in.scr + vec2<f32>(dx, -dy)) * coef / 1.5;
        div += 6.0 * coef;
    }
    sum /= div;
    sum *= in.color.a * mix(1.0, 0.0, d * 2.0);
    let under = grab_at(in.scr);
    return vec4<f32>(max(under.rgb, sum.rgb), under.a) + sum;
}

// Menu depth illustrations. The texture is double height: colour in its upper half
// (v 0.5..1), a depth map (red: 1 = near) in its lower half; the quad shows it at half
// height, so `uv` spans the whole texture. The vertex colour carries the sprite's size
// (r: 1..1400, g: 1..800 px) and depth (b: 0..10); alpha is the sprite's alpha.
struct SceneTexel {
    // Where the colour is read: the colour half, shifted towards the camera by the depth map.
    pos: vec2<f32>,
    // `1 - depth map`: 0 nearest, 1 farthest within the layer.
    dp: f32,
    size: vec2<f32>,
    depth: f32,
};

fn scene_texel(in: VertexOut) -> SceneTexel {
    var t: SceneTexel;
    t.size = vec2<f32>(mix(1.0, 1400.0, in.color.r), mix(1.0, 800.0, in.color.g));
    t.depth = mix(0.0, 10.0, in.color.b);
    t.dp = 1.0 - textureSampleLevel(main_tex, main_sampler, vec2<f32>(in.uv.x, in.uv.y / 2.0), 0.0).x;
    var pos = vec2<f32>(in.uv.x, 0.5 + in.uv.y / 2.0);
    pos -= (stage.menu_scene.zw - in.uv) * t.dp * 0.025;
    pos.y = max(0.5 + 1.0 / t.size.y, pos.y);
    t.pos = pos;
    return t;
}

fn scene_tap(at: vec2<f32>, weighted: bool) -> vec4<f32> {
    let c = textureSampleLevel(main_tex, main_sampler, at, 0.0);
    if (weighted) {
        return c * c.a;
    }
    return c;
}

// The colour blurred by `|layer depth + 2 dp - _BlurDepth| * _BlurRange`: the texel
// (weight 1) and five rings of eight taps (diagonals at 0.72 of the distance, steps 2..6
// thousandths of the texture at full blur, x scaled to square pixels), each ring 0.92
// of the last; taps below the colour half are skipped. `weighted`: each tap's colour
// counts by its alpha (the divisor does not). Alpha times the vertex alpha.
fn scene_blur(in: VertexOut, weighted: bool) -> vec4<f32> {
    let t = scene_texel(in);
    let floor_y = 0.5 + 1.0 / t.size.y;
    let hor = t.size.y / t.size.x;
    let amount = clamp(abs(t.depth + t.dp * 2.0 - stage.menu_scene.x) * stage.menu_scene.y, 0.0, 1.0) * 0.001;
    var col = textureSampleLevel(main_tex, main_sampler, t.pos, 0.0);
    var div = 1.0;
    var coef = 1.0;
    var fi = 1.0;
    for (var j = 0; j < 5; j++) {
        fi += 1.0;
        coef *= 0.92;
        let s = fi * amount;
        let d = s * 0.72;
        if (t.pos.y - s > floor_y) {
            col += scene_tap(t.pos + vec2<f32>(0.0, -s), weighted) * coef;
            div += coef;
        }
        col += scene_tap(t.pos + vec2<f32>(-s * hor, 0.0), weighted) * coef;
        col += scene_tap(t.pos + vec2<f32>(s * hor, 0.0), weighted) * coef;
        col += scene_tap(t.pos + vec2<f32>(0.0, s), weighted) * coef;
        div += 3.0 * coef;
        if (t.pos.y - d > floor_y) {
            col += scene_tap(t.pos + vec2<f32>(-d * hor, -d), weighted) * coef;
            col += scene_tap(t.pos + vec2<f32>(d * hor, -d), weighted) * coef;
            div += 2.0 * coef;
        }
        col += scene_tap(t.pos + vec2<f32>(-d * hor, d), weighted) * coef;
        col += scene_tap(t.pos + vec2<f32>(d * hor, d), weighted) * coef;
        div += 2.0 * coef;
    }
    col /= div;
    col.a *= in.color.a;
    return col;
}

@fragment
fn fs_scene_blur(in: VertexOut) -> @location(0) vec4<f32> {
    return scene_blur(in, true);
}

@fragment
fn fs_scene_blur_light_edges(in: VertexOut) -> @location(0) vec4<f32> {
    return scene_blur(in, false);
}

// Screen blend of the unblurred colour (times its alpha) over the grab; the vertex
// alpha blends the result over the frame.
@fragment
fn fs_scene_lighten(in: VertexOut) -> @location(0) vec4<f32> {
    let t = scene_texel(in);
    let tex = textureSampleLevel(main_tex, main_sampler, t.pos, 0.0);
    let grab = grab_at(in.scr);
    return vec4<f32>(1.0 - (1.0 - grab.rgb) * (1.0 - tex.rgb * tex.a), in.color.a);
}

fn overlay_channel(grab: f32, col: f32) -> f32 {
    if (grab < 0.5) {
        return col * grab * 2.0;
    }
    return 1.0 - (1.0 - col) * 2.0 * (1.0 - grab);
}

fn soft_light_channel(grab: f32, col: f32) -> f32 {
    if (col < 0.5) {
        return 2.0 * grab * col + grab * grab * (1.0 - 2.0 * col);
    }
    return 2.0 * grab * (1.0 - col) + sqrt(grab) * (2.0 * col - 1.0);
}

// The unweighted blur overlaid on the grab; its alpha (already times the vertex alpha)
// is multiplied by the vertex alpha again, as in the game.
@fragment
fn fs_scene_overlay(in: VertexOut) -> @location(0) vec4<f32> {
    let col = scene_blur(in, false);
    let grab = grab_at(in.scr);
    return vec4<f32>(
        overlay_channel(grab.r, col.r),
        overlay_channel(grab.g, col.g),
        overlay_channel(grab.b, col.b),
        col.a * in.color.a,
    );
}

@fragment
fn fs_scene_soft_light(in: VertexOut) -> @location(0) vec4<f32> {
    let col = scene_blur(in, false);
    let grab = grab_at(in.scr);
    return vec4<f32>(
        soft_light_channel(grab.r, col.r),
        soft_light_channel(grab.g, col.g),
        soft_light_channel(grab.b, col.b),
        col.a * in.color.a,
    );
}
