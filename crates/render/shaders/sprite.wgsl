// Room sprites (atlas sprites and TriangleMeshes) in stage pixels (room minus camera),
// alpha blended (SrcAlpha, OneMinusSrcAlpha) by the pipeline, as every shader here.
// One fragment entry per game shader (StreamingAssets/shaders/<Name>.shader): fs_basic
// `Futile/Basic`, fs_flat_light `FlatLight`, fs_overseer_zip `OverseerZip`,
// fs_hologram `Hologram`.
//
// `uv` is where the port's atlas is sampled; `tex` is the shader's `i.uv` (they differ
// only for `Futile_White`, a texture of its own in the game).

struct Room {
    // xy: scale, zw: offset; clip = stage * scale + offset.
    transform: vec4<f32>,
    // `_spriteRect`: the level image in screen coordinates.
    sprite_rect: vec4<f32>,
    // `_screenSize`.
    screen: vec2<f32>,
    // `_RAIN`.
    rain: f32,
    // `_hologramThreshold`.
    hologram_threshold: f32,
};

@group(0) @binding(0) var<uniform> room: Room;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;
// `_NoiseTex2` (`Palettes/noise2`).
@group(1) @binding(0) var noise2: texture_2d<f32>;
@group(1) @binding(1) var noise2_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) tex: vec2<f32>,
    @location(2) color: vec4<f32>,
    // `ComputeScreenPos`: 0..1 over the game's screen, y up.
    @location(3) scr: vec2<f32>,
};

@vertex
fn vs_main(
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) tex: vec2<f32>,
    @location(3) color: vec4<f32>,
) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(pos * room.transform.xy + room.transform.zw, 0.0, 1.0);
    out.uv = uv;
    out.tex = tex;
    out.color = color;
    out.scr = pos / room.screen;
    return out;
}

// HLSL `fmod`: the remainder has the sign of x.
fn fmod_trunc(x: f32, y: f32) -> f32 {
    return x - y * trunc(x / y);
}

fn center_dist(uv: vec2<f32>) -> f32 {
    return clamp(distance(uv, vec2<f32>(0.5, 0.5)) * 2.0, 0.0, 1.0);
}

// The screen pixel's place in the level image: `floor(scrPos * _screenSize) /
// _screenSize`, minus `_spriteRect.xy`, over `_spriteRect.zw - _spriteRect.xy`.
fn level_coord(scr: vec2<f32>) -> vec2<f32> {
    let rect = room.sprite_rect;
    let tc = floor(scr * room.screen) / room.screen;
    return (tc - rect.xy) / (rect.zw - rect.xy);
}

fn noise2_at(uv: vec2<f32>) -> f32 {
    return textureSampleLevel(noise2, noise2_sampler, uv, 0.0).x;
}

// `Combine texture * primary`.
@fragment
fn fs_basic(in: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(atlas, atlas_sampler, in.uv) * in.color;
}

// The texture is not read: `c = pow(pow(1 - centerDist^2, 3.5), 1)`, output
// `(clr.rgb, c * clr.a)`.
@fragment
fn fs_flat_light(in: VertexOut) -> @location(0) vec4<f32> {
    let c = pow(pow(1.0 - pow(center_dist(in.tex), 2.0), 3.5), 1.0);
    return vec4<f32>(in.color.rgb, c * in.color.a);
}

// `dst = centerDist + 2 * noise2(tc.x * 4, tc.y * 8 + _RAIN * 100)`; nothing where
// `dst > lerp(0, 3, clr.a)`, else the opaque vertex colour.
@fragment
fn fs_overseer_zip(in: VertexOut) -> @location(0) vec4<f32> {
    let tc = level_coord(in.scr);
    var dst = center_dist(in.tex);
    dst += noise2_at(vec2<f32>(tc.x * 4.0, tc.y * 8.0 + room.rain * 100.0)) * 2.0;
    if (dst > mix(0.0, 3.0, in.color.a)) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(in.color.rgb, 1.0);
}

// Scan lines: `h = 2 noise2(tc.x * 4, tc.y * 8 - _RAIN * 10) - clr.a^2`, plus 0.25 on
// every third row of `round((tc.y - _RAIN * 0.15) * 400)`; nothing where
// `h > _hologramThreshold`. Else the vertex colour with the texture's alpha read
// `h2 * lerp(0.008 + 0.008 h, 0, clr.a^0.25)` to the side, `h2 = sin(16 noise2(tc.x *
// 0.5 + _RAIN * 0.002, tc.y * 0.4) + _RAIN * 145.14)` raised to `|h2|^lerp(0.5, 40,
// clr.a)`.
@fragment
fn fs_hologram(in: VertexOut) -> @location(0) vec4<f32> {
    let tc = level_coord(in.scr);
    let a = in.color.a;
    var h = noise2_at(vec2<f32>(tc.x * 4.0, tc.y * 8.0 - room.rain * 10.0)) * 2.0;
    h -= pow(a, 2.0);
    if (fmod_trunc(round((tc.y - room.rain * 0.15) * 400.0), 3.0) == 0.0) {
        h += 0.25;
    }
    if (h > room.hologram_threshold) {
        return vec4<f32>(0.0);
    }
    var h2 = sin(noise2_at(vec2<f32>(tc.x * 0.5 + room.rain * 0.002, tc.y * 0.4)) * 16.0
        + room.rain * 145.14);
    h2 *= pow(abs(h2), mix(0.5, 40.0, a));
    let shift = h2 * mix(0.008 + 0.008 * h, 0.0, pow(a, 0.25));
    let texel = textureSampleLevel(atlas, atlas_sampler, in.uv + vec2<f32>(shift, 0.0), 0.0);
    return vec4<f32>(in.color.rgb, texel.a);
}
