// `ElectricDeath` room effect's shaders: `Futile/ElectricDeath` (the web over the
// terrain) and `Futile/FlareBomb` (the flash of a `SparkFlash`; the game's
// `FlareBombBothSides` is the same shader with the Watcher's `ripple_both_sides` keyword,
// which does nothing without `RIPPLE`). Appended to sprite.wgsl after water_fall.wgsl
// (its level, noise and grab helpers). Neither has a GrabPass: `_GrabTexture` is what was
// drawn behind the level, as for `LightSource`. The base game's sloped-terrain mask is
// black, so `TerrainAtLevelPos(...).r` is `2 - 0 * 3 = 2`, which never wins a `min`. D33.

fn noise2_xy(uv: vec2<f32>) -> vec2<f32> {
    return textureSampleLevel(noise2, noise2_sampler, uv, 0.0).xy;
}

// Nothing over sky, depth under 1/30, or (past depth 5/30) a creature drawn behind the
// level (any non-black grab pixel). Else noise-driven sines `l`, `h2`, `h3`, `h4` (the
// sprite colour's blue and alpha as parameters) combine into `h`; above 0.5 the pixel is
// opaque green with alpha `(1 - depth) * 0.6 * clr.a`; elsewhere `(l, 0, 0, 0)`, which the
// blend leaves invisible.
@fragment
fn fs_electric_death(in: VertexOut) -> @location(0) vec4<f32> {
    let tc = level_coord(in.scr);
    let texcol = level_at(tc);
    let depth = fmod_trunc(f32(byte(texcol.r) - 1), 30.0) / 30.0;
    let rain = room.rain;
    let alpha = in.color.a;
    var gc = tc + (tc - vec2(0.5)) * 0.4 * depth;
    gc += (noise2_xy(vec2(gc.x * 0.5, gc.y * 0.25 + rain * 124.12)) - vec2(0.5)) * 0.01;
    // The five `_NoiseTex` taps come before any return: Unity picks the mip level from the
    // 2x2 block's derivatives, so every pixel of the block takes them.
    let n1 = noise_at(vec2(gc.x * 0.1 - rain * 0.012, gc.y * 0.15 + rain * 0.32));
    let n2 = noise_at(vec2(gc.x * 0.1 - rain * 0.012, gc.y * 0.15 + 0.5 + rain * 0.32));
    let n3 = noise_at(vec2(gc.x * 0.8 + rain * 0.0421, gc.y * 0.6 + rain * 0.091));
    let n4 = noise_at(vec2(gc.x * 1.2 + rain * 0.0621, gc.y * 0.8 + rain * 0.16));
    let n5 = noise_at(vec2(gc.x * 2.0 - rain * 0.0121, gc.y * 1.2 + rain * 0.182));
    if all(texcol.rgb == vec3(1.0)) {
        return vec4(0.0);
    }
    if depth < 1.0 / 30.0 {
        return vec4(0.0);
    }
    if depth > 5.0 / 30.0 {
        let grab = textureLoad(grab_tex, frame_px(in.scr), 0);
        if byte(grab.r) != 0 || byte(grab.g) != 0 || byte(grab.b) != 0 {
            return vec4(0.0);
        }
    }
    let h1 = noise2_at(vec2(depth * 0.055 + rain * 0.01, rain * 0.07));
    var l = 0.5 - 0.5 * sin(n1 * 3.14 * mix(4.0, 7.0, h1) - h1 * 2.0 + rain * 9.16);
    let l2 = 0.5 + 0.5 * sin(n2 * 3.14 * mix(7.0, 4.0, l) + h1 * 0.1 + rain * 9.16);
    l = l * min(l2 + 0.5, 1.0);
    var h2 = 0.5 - 0.5 * sin(n3 * 3.14 * mix(7.2, 5.4, h1) + l * mix(0.0, 4.0, h1) + rain * 14.6 + depth * 0.20);
    var h3 = 0.5 + 0.5 * sin(n4 * 3.14 * (5.0 + h2) - l * mix(3.0, 0.0, h1) + rain * 11.6 - depth);
    var h4 = 0.5 + 0.5 * sin(n5 * 3.14 * (6.0 - h3) + in.color.b * 2.21 + rain * 8.6 + depth * 0.95);
    h4 *= mix(0.9999, 1.0, l * alpha);
    h2 = max(h2, mix(l * l, l2 * l2, h3) * alpha);
    h3 *= mix(0.9999, 1.0, alpha);
    var h = 1.0 - (1.0 - h2) * (1.0 - h3) * (1.0 - h4);
    h = pow(h, mix(300000.0, 12000.0, l * alpha * alpha));
    h *= pow(l, mix(0.8, 0.2, alpha));
    if h > 0.5 {
        return vec4(0.0, 1.0, 0.0, (1.0 - depth) * 0.6 * alpha);
    }
    return vec4(l, 0.0, 0.0, 0.0);
}

// A bright flash over the level: a disc lit by depth (past depth 6/30 cut to 6/30 over a
// creature drawn behind the level), four shadow samples toward the centre, the alpha
// sharpened and pushed to white above 0.5.
@fragment
fn fs_flare_bomb(in: VertexOut) -> @location(0) vec4<f32> {
    let tc = level_coord(in.scr);
    var dist = depth_of(level_at(tc));
    dist = min(dist, 2.0 - 5.0 / 30.0);
    if dist > 6.0 / 30.0 && drawn(textureLoad(grab_tex, frame_px(in.scr), 0)) {
        dist = 6.0 / 30.0;
    }
    if dist == 1.0 {
        return vec4(0.0);
    }
    let dir = normalize(in.tex - vec2(0.5));
    let center_dist = clamp(distance(in.tex, vec2(0.5)), 0.0, 0.5);
    let shadow_pos = tc - dir * pow(center_dist, 1.25) * pow(dist, 2.0) * 0.3;
    let old_shadow_pos = in.scr - dir * pow(center_dist, 1.25) * pow(dist, 1.5) * 0.3;
    var shadow = 0.0;
    for (var j = 0; j < 4; j++) {
        let offset = dir * 0.02 * f32(j) * center_dist;
        var shadow_dist = depth_of(level_at(shadow_pos - offset));
        shadow_dist = min(shadow_dist, 2.0 - 5.0 / 30.0);
        if shadow_dist > 6.0 / 30.0 && drawn(textureLoad(grab_tex, frame_px(old_shadow_pos - offset), 0)) {
            shadow_dist = 6.0 / 30.0;
        }
        if dist > shadow_dist {
            shadow = clamp(shadow + 0.25, 0.0, 1.0);
        }
    }
    var a = dist;
    if a < 0.2 {
        a = 1.0 - a * 5.0;
    } else {
        a = clamp((a - 0.2) * 1.3, 0.0, 1.0);
    }
    a = mix(1.0 - a, 1.0, pow(dist, 0.5));
    a *= pow(pow(1.0 - pow(center_dist * 2.0, 2.0), 1.5), mix(0.5, 2.5, dist));
    a *= 1.0 - shadow;
    a *= in.color.a;
    a = pow(a, mix(1.0, 0.2, pow(a, 0.4)));
    a = mix(a, 1.0, pow(1.0 - center_dist * 2.0, 3.0) * in.color.a);
    // `if (alpha < 0 || 0 < alpha || alpha == 0)` is false only for NaN.
    if a != a {
        return vec4(in.color.rgb, 0.0);
    }
    a = clamp(a, 0.0, 1.0);
    if a < 0.5 {
        return vec4(in.color.rgb, a * 2.0);
    }
    return vec4(mix(in.color.rgb, vec3(1.0), (a - 0.5) * 2.0), 1.0);
}
