// Room sprites (atlas sprites and TriangleMeshes) in stage pixels (room minus camera),
// alpha blended (SrcAlpha, OneMinusSrcAlpha) by the pipeline, as every shader here.
// One fragment entry per game shader (StreamingAssets/shaders/<Name>.shader): fs_basic
// `Futile/Basic`, fs_flat_light `FlatLight`, fs_overseer_zip `OverseerZip`,
// fs_hologram `Hologram`, fs_light_source `LightSource`, fs_hologram_image `HologramImage`.
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
    // The frame's size in pixels (the screen-sized textures of group 2).
    frame: vec2<f32>,
    pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> room: Room;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;
// `_NoiseTex2` (`Palettes/noise2`).
@group(1) @binding(0) var noise2: texture_2d<f32>;
@group(1) @binding(1) var noise2_sampler: sampler;
// `_LevelTex` (v = 0 at the bottom), the `_GrabTexture` of the last unnamed GrabPass
// before the level (what was drawn behind it), and the frame as it was just before a
// `HologramImage` sprite (its own GrabPass).
@group(2) @binding(0) var level_tex: texture_2d<f32>;
@group(2) @binding(1) var behind_tex: texture_2d<f32>;
@group(2) @binding(2) var grab_tex: texture_2d<f32>;

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

// `tex2D(_LevelTex, uv)`: point filter, clamp.
fn level_at(uv: vec2<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(level_tex));
    let texel = clamp(vec2<i32>(floor(uv * vec2<f32>(size))), vec2(0), size - 1);
    return textureLoad(level_tex, texel, 0);
}

// The frame pixel at screen uv `scr` (0..1, y up).
fn frame_px(scr: vec2<f32>) -> vec2<i32> {
    let clip = scr * room.screen * room.transform.xy + room.transform.zw;
    let px = vec2((clip.x + 1.0) * 0.5, (1.0 - clip.y) * 0.5) * room.frame;
    return clamp(vec2<i32>(floor(px)), vec2(0), vec2<i32>(room.frame) - 1);
}

fn byte(x: f32) -> i32 {
    return i32(round(x * 255.0));
}

// A level texel's depth in layers / 30: `fmod(round(r * 255) - 1, 30) / 30`, 1 for sky.
// (The base game's sloped-terrain mask is black, so its depth 2 never wins.)
fn depth_of(c: vec4<f32>) -> f32 {
    if all(c.rgb == vec3(1.0)) {
        return 1.0;
    }
    return fmod_trunc(f32(byte(c.r) - 1), 30.0) / 30.0;
}

// The grab is not black: `x > 1/255 || y != 0 || z != 0`.
fn drawn(c: vec4<f32>) -> bool {
    return byte(c.r) > 1 || byte(c.g) != 0 || byte(c.b) != 0;
}

// `LightSource` (no keywords: not bright, not deep; no snow, ripple or brain mold): a
// disc over the level, dimmer on deeper pixels, shadowed where a nearer pixel lies
// toward the centre, lit on edges, by palette colour; nothing over a creature drawn
// behind the level (deeper than layer 5), and shadows cut by them.
@fragment
fn fs_light_source(in: VertexOut) -> @location(0) vec4<f32> {
    let tc = level_coord(in.scr);
    let texcol = level_at(tc);
    var palette_color = i32(floor(f32(byte(texcol.r) % 90) / 30.0));
    if texcol.g >= 16.0 / 255.0 {
        palette_color = 3;
    }
    var dist = depth_of(texcol);
    let dir = normalize(in.tex - vec2(0.5));
    let center_dist = clamp(distance(in.tex, vec2(0.5)), 0.0, 0.5);
    let shadow_pos = tc - dir * pow(center_dist, 1.25) * pow(dist, 2.0) * 0.3;
    let highlight_pos = tc - dir * 0.003 * pow(center_dist * 2.0, 0.25);
    let old_shadow_pos = in.scr - dir * pow(center_dist, 1.25) * pow(dist, 1.5) * 0.3;
    var shadow_dist = depth_of(level_at(shadow_pos));
    let highlight_dist = depth_of(level_at(highlight_pos));
    if dist > 5.0 / 30.0 && drawn(textureLoad(behind_tex, frame_px(in.scr), 0)) {
        return vec4(0.0);
    }
    if shadow_dist > 5.0 / 30.0 && drawn(textureLoad(behind_tex, frame_px(old_shadow_pos), 0)) {
        shadow_dist = 6.0 / 30.0;
    }
    var shadow = dist - shadow_dist - select(2.0 / 30.0, 0.0, palette_color == 1);
    shadow = pow(clamp(shadow, 0.0, 1.0), mix(1.0 - dist, 0.5, 0.5));
    var highlight = 0.0;
    if highlight_dist > dist + 0.05 {
        highlight = sin(center_dist * 3.14 * 2.0);
    }
    if palette_color == 0 {
        let sd2_pos = tc - dir * 0.01 * center_dist;
        let sd2 = min(fmod_trunc(f32(byte(level_at(sd2_pos).r) - 1), 30.0) / 30.0, 2.0 + 1.0 / 30.0);
        if sd2 < dist && sd2 > dist - 0.1 {
            shadow = mix(shadow, 1.0, pow(center_dist * 2.0, 2.5 - 4.0 * center_dist));
        }
    }
    let d = dist;
    if dist < 0.2 {
        dist = pow(1.0 - dist * 5.0, 0.35);
    } else {
        dist = clamp((dist - 0.2) * 1.3, 0.0, 1.0);
    }
    dist = 1.0 - dist;
    dist *= pow(pow(1.0 - pow(center_dist * 2.0, 2.0), 3.5), mix(0.5, 3.5, d));
    dist = clamp(mix(dist, 0.0, shadow) - shadow * 0.3, 0.0, 1.0);
    if palette_color == 0 {
        dist *= 0.8;
    } else if palette_color == 2 {
        dist = pow(dist, 0.8);
    } else if palette_color == 3 {
        dist *= 0.2;
        highlight = 0.0;
    }
    dist *= textureSampleLevel(atlas, atlas_sampler, in.uv, 0.0).x;
    return vec4(in.color.rgb, dist * in.color.a * (0.65 + highlight * 0.35));
}

fn screen_blend(a: f32, b: f32) -> f32 {
    return 1.0 - (1.0 - a) * (1.0 - b);
}

fn overlay_blend(a: f32, b: f32) -> f32 {
    if a > 0.5 {
        return 1.0 - (1.0 - a) * (1.0 - b);
    }
    return a * b;
}

// `HologramImage`: cell `round(clr.b * 25)` of the 5x5 projection texture, panned by
// `clr.rg` and bulged toward the rim; overlaid with (1, 0.8, 0.3) more as `clr.a` falls,
// screened over the frame (its GrabPass), lerped to that colour by `(1 - clr.a)^1.5`;
// a disc, cut where scan-line noise passes `_hologramThreshold * clr.a`.
@fragment
fn fs_hologram_image(in: VertexOut) -> @location(0) vec4<f32> {
    let tc = level_coord(in.scr);
    let a = in.color.a;
    let dst = clamp(distance(in.tex, vec2(0.5)) * 2.0, 0.0, 1.0);
    let p = in.tex * 0.5 + in.color.xy * 0.5 + (vec2(0.5) - in.tex) * 0.2 * pow(dst, 4.0);
    let img = i32(round(in.color.z * 25.0));
    let cell = vec2(f32(img % 5), f32(img / 5)) / 5.0 + p / 5.0;
    var tex = textureSampleLevel(atlas, atlas_sampler, cell, 0.0);
    let grab = textureLoad(grab_tex, frame_px(in.scr), 0);
    let effect = vec4(1.0, 0.8, 0.3, 1.0);
    let overlaid = vec4(
        overlay_blend(tex.r, effect.r),
        overlay_blend(tex.g, effect.g),
        overlay_blend(tex.b, effect.b),
        tex.a,
    );
    tex = mix(tex, overlaid, mix(1.0, 0.56, a));
    tex = vec4(
        screen_blend(tex.r, grab.r),
        screen_blend(tex.g, grab.g),
        screen_blend(tex.b, grab.b),
        tex.a,
    );
    tex = mix(tex, effect, pow(1.0 - a, 1.5));
    var h = noise2_at(vec2<f32>(tc.x * 4.0, tc.y * 8.0 - room.rain * 10.0));
    if fmod_trunc(round((tc.y - room.rain * 0.15) * 400.0), 3.0) == 0.0 {
        h += mix(0.25, 0.05, a);
    }
    if dst >= 1.0 {
        return vec4(0.0);
    }
    h = pow(h, mix(mix(0.5, 10.0, a), 0.5, dst));
    if h > room.hologram_threshold * a {
        return vec4(0.0);
    }
    return tex;
}
