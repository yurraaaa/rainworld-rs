// The room camera's full-screen effects (`RoomCamera.fullScreenEffect`): a
// `Futile_White` quad over the screen, its alpha the effect's strength. Appended to
// sprite.wgsl (its bindings, vertex stage and helpers). One fragment entry per game
// shader: fs_bloom `Bloom`, fs_sky_bloom `SkyBloom`, fs_light_bloom `LightBloom`,
// fs_light_and_sky_bloom `LightAndSkyBloom`, fs_fog `Fog`. All but `Fog` grab the frame
// first (`GrabPass {}`, the grab texture); their output alpha is the grab's plus more,
// so with `SrcAlpha OneMinusSrcAlpha` they replace the pixel.
//
// Taps are in screen uv (`scrPos`, y up): steps of 0.0012 (0.0018) of the screen's
// height, times `_screenSize.y / _screenSize.x` sideways. The level is read at
// `textCoord`, `scrPos` mapped through `_spriteRect` (unfloored), offset by the same
// amounts.

// The frame grab, bilinear and clamped (INFERRED: Unity's grab texture defaults).
@group(2) @binding(6) var grab_sampler: sampler;

// The frame uv of screen uv `scr`: the inverse of the vertex stage's `scr`.
fn frame_uv(scr: vec2<f32>) -> vec2<f32> {
    let clip = (scr * room.screen - SCREEN_PIXEL_OFFSET) * room.transform.xy + room.transform.zw;
    return vec2((clip.x + 1.0) * 0.5, (1.0 - clip.y) * 0.5);
}

// `tex2D(_GrabTexture, scr)` of the effect's own grab.
fn grab_at(scr: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(grab_tex, grab_sampler, frame_uv(scr), 0.0);
}

// `textCoord`: the level image uv shown at screen uv `scr`.
fn effect_level_coord(scr: vec2<f32>) -> vec2<f32> {
    let rect = room.sprite_rect;
    return (scr - rect.xy) / (rect.zw - rect.xy);
}

fn hor_fac() -> f32 {
    return room.screen.y / room.screen.x;
}

// `CreatureMask`: false where the level pixel lies deeper than layer 5 and something
// was drawn before the level at this fragment (`_PreLevelColorGrab`, its red channel).
fn creature_mask(red: i32, scr: vec2<f32>) -> bool {
    let deep = ((red - 1) % 30) > 5;
    return !deep || textureLoad(behind_tex, frame_px(scr), 0).x == 0.0;
}

// `IsLevelLit`: a sunlit level pixel (red 91..254) not masked by a creature. The
// sloped-terrain mask is black in the base game (no `AddTerrain` change); snow and urban
// life are off.
fn is_level_lit(tc: vec2<f32>, scr: vec2<f32>) -> bool {
    let red = byte(level_at(tc).x);
    return red > 90 && red < 255 && creature_mask(red, scr);
}

// `tex2D(_PalTex, float2(0, 7))`: the sky colour (uv clamped to the top row).
fn sky_color() -> vec4<f32> {
    return palette_at(vec2(0.0, 7.0));
}

fn is_sky(c: vec4<f32>, sky: vec4<f32>) -> bool {
    return c.x == sky.x && c.y == sky.y && c.z == sky.z;
}

// `Bloom`: a plus-shaped blur (4 rings, 0.92^j), times alpha and a vignette over the
// quad, maxed into the grab, plus its own power 1.5.
@fragment
fn fs_bloom(in: VertexOut) -> @location(0) vec4<f32> {
    let s = in.scr;
    let hor = hor_fac();
    var sum = vec4(0.0, 0.0, 0.0, 1.0);
    var div = 0.0;
    var coef = 1.0;
    for (var j = 1; j <= 4; j++) {
        let d = f32(j) * 0.0012;
        coef *= 0.92;
        sum += grab_at(vec2(s.x, s.y + d)) * coef;
        sum += grab_at(vec2(s.x - d * hor, s.y)) * coef;
        sum += grab_at(vec2(s.x + d * hor, s.y)) * coef;
        sum += grab_at(vec2(s.x, s.y - d)) * coef;
        div += 4.0 * coef;
    }
    sum /= div;
    sum *= in.color.a * mix(1.0, 0.5, distance(in.tex, vec2(0.5)) * 2.0);
    var grab = grab_at(s);
    grab = vec4(max(grab.xyz, sum.xyz), grab.w);
    sum = vec4(pow(sum.xyz, vec3(1.5)), sum.w);
    return grab + sum;
}

// `SkyBloom`: sky-coloured pixels on the four diagonals (5 rings, 0.62^j, steps of
// `0.0018 * amount`) blurred in, raising channels toward it by `amount`, which is
// strongest at (0.2, 0.2) of the screen.
@fragment
fn fs_sky_bloom(in: VertexOut) -> @location(0) vec4<f32> {
    let s = in.scr;
    let amount = pow(clamp(1.0 - distance(vec2(0.2), s), 0.0, 1.0), 0.5) * in.color.a;
    let blur = 0.0018 * amount;
    let hor = hor_fac();
    let sky = sky_color();
    var texcol = vec4(0.0, 0.0, 0.0, 1.0);
    var div = 1.0;
    var coef = 1.0;
    for (var j = 1; j <= 5; j++) {
        let d = f32(j) * blur;
        coef *= 0.62;
        var taps = array(
            vec2(s.x - d * hor, s.y + d),
            vec2(s.x + d * hor, s.y + d),
            vec2(s.x + d * hor, s.y - d),
            vec2(s.x - d * hor, s.y - d),
        );
        for (var t = 0; t < 4; t++) {
            let g = grab_at(taps[t]);
            if is_sky(g, sky) {
                texcol += g * coef;
                div += coef;
            }
        }
    }
    var grab = grab_at(s);
    div *= 0.75;
    texcol = (grab + texcol) / div;
    for (var c = 0; c < 3; c++) {
        if texcol[c] > grab[c] {
            grab[c] = mix(grab[c], texcol[c], amount);
        }
    }
    return grab;
}

// `LightBloom`: the grab at sunlit level pixels on a plus (4 rings, 0.82^j) summed,
// over half the full weight, times alpha, added to the grab.
@fragment
fn fs_light_bloom(in: VertexOut) -> @location(0) vec4<f32> {
    let s = in.scr;
    let tc = effect_level_coord(s);
    let hor = hor_fac();
    var texcol = vec4(0.0, 0.0, 0.0, 1.0);
    var div = 0.0;
    var coef = 1.0;
    for (var j = 1; j <= 4; j++) {
        let d = f32(j) * 0.0012;
        coef *= 0.82;
        var offsets = array(vec2(0.0, d), vec2(-d * hor, 0.0), vec2(d * hor, 0.0), vec2(0.0, -d));
        for (var t = 0; t < 4; t++) {
            if is_level_lit(tc + offsets[t], s) {
                texcol += grab_at(s + offsets[t]) * coef;
            }
        }
        div += 4.0 * coef;
    }
    let grab = grab_at(s);
    div *= 0.5;
    return grab + (texcol / div) * in.color.a;
}

// `LightAndSkyBloom`: `SkyBloom`'s diagonal sky blur (6 rings, fixed 0.0018 steps, half
// of it added to the grab first) plus the brighter of the centre and each sky tap,
// weighted 0.9^(j-1), where the level is sunlit on the mirrored diagonal; that sum less
// `lerp(0.7, 0.35, amount)`, halved, raises channels by `amount`.
@fragment
fn fs_light_and_sky_bloom(in: VertexOut) -> @location(0) vec4<f32> {
    let s = in.scr;
    let tc = effect_level_coord(s);
    let amount = pow(clamp(1.0 - distance(vec2(0.2), s), 0.0, 1.0), 0.5) * in.color.a;
    let hor = hor_fac();
    let sky = sky_color();
    let mid = grab_at(s);
    var texcol = vec4(0.0, 0.0, 0.0, 1.0);
    var add = vec4(0.0);
    var div = 1.0;
    var coef = 1.0;
    var coef2 = 1.0;
    for (var j = 1; j <= 6; j++) {
        let d = f32(j) * 0.0018;
        coef *= 0.62;
        // Sky taps and the level taps checked after each: the level's diagonal is
        // the sky tap's mirrored vertically.
        var sky_taps = array(vec2(-d * hor, d), vec2(d * hor, d), vec2(d * hor, -d), vec2(-d * hor, -d));
        for (var t = 0; t < 4; t++) {
            let g = grab_at(s + sky_taps[t]);
            if is_sky(g, sky) {
                texcol += g * coef;
                div += coef;
            }
            let lit = vec2(sky_taps[t].x, -sky_taps[t].y);
            if is_level_lit(tc + lit, s) {
                add += vec4(max(mid.xyz, g.xyz), 1.0) * coef2;
            }
        }
        coef2 *= 0.9;
    }
    var grab = grab_at(s);
    grab += texcol * 0.5;
    div *= 0.75;
    texcol = (grab + texcol) / div;
    grab = vec4(max(grab.xyz, texcol.xyz), grab.w);
    let cut = mix(0.7, 0.35, amount);
    add = vec4(max(vec3(0.0), add.xyz - cut), add.w) * 0.5;
    for (var c = 0; c < 3; c++) {
        if add[c] > grab[c] {
            grab[c] = mix(grab[c], add[c], amount);
        }
    }
    return grab;
}

// `Fog`: two noise sines over the level (`fog1`) thinned on near layers by the depth,
// cut by the alpha and below the water line (`_waterLevel`), drawn as 0.2 or 0.6 of the
// palette's fog colours; over sky a third sine makes opaque light patches. Something
// drawn before the level at a deep pixel counts as layer 6 (the level's grab).
@fragment
fn fs_fog(in: VertexOut) -> @location(0) vec4<f32> {
    let s = in.scr;
    let tc = effect_level_coord(s);
    let rain = room.rain;
    let a = in.color.a;
    let amount = clamp((s.y - ((1.0 - room.water_level) - 0.11)) * 3.0, 0.0, 1.0);
    var fog2 = 0.5 + 0.5 * sin((noise_at(vec2(tc.x * 0.6 + rain * 0.08, tc.y * 1.0 - rain * 0.01)) + rain * 0.04 + in.tex.x) * 6.28);
    var fog1 = 0.5 + 0.5 * sin((noise_at(vec2(tc.x * 0.44 - rain * 0.113, tc.y * 1.2 - rain * 0.0032)) + rain * 0.05 + in.tex.y) * 6.28);
    fog1 = mix(fog1, fog2, 0.5);
    let texcol = level_at(tc);
    var dp = fmod_trunc(f32(byte(texcol.x) - 1), 30.0) / 30.0;
    if all(texcol.xyz == vec3(1.0)) {
        dp = 1.0;
    }
    if dp > 6.0 / 30.0 {
        let g = textureLoad(behind_tex, frame_px(s), 0);
        if g.x > 1.0 / 255.0 || g.y != 0.0 || g.z != 0.0 {
            dp = 6.0 / 30.0;
        }
    }
    if dp >= 0.99999997 {
        fog2 = 0.5 + 0.5 * sin((noise_at(vec2(in.tex.x * 1.7 + rain * 0.113, in.tex.y * 2.82)) + rain * 0.14 - in.tex.x) * 6.28);
        fog2 *= clamp(1.0 - distance(in.tex, vec2(0.0, 0.9)), 0.0, 1.0);
        fog2 = pow(fog2, 0.2);
        fog2 *= amount;
        fog2 *= 1.0 - pow(fog1, 1.5);
        fog2 *= a;
        if fog2 > 0.5 {
            return mix(palette_at(vec2(0.0, 7.0 / 8.0)), vec4(1.0), select(0.1, 0.25, fog2 > 0.6));
        }
    }
    fog1 = pow(fog1, 3.0);
    fog1 *= a;
    fog1 = pow(fog1, 1.0 + (1.0 - pow(dp, 0.1)) * 30.0);
    fog1 = max(0.0, fog1 - (1.0 - amount));
    fog1 = pow(fog1, 0.2);
    if fog1 > 0.1 {
        let col = mix(palette_at(vec2(0.0, 2.0 / 8.0)), palette_at(vec2(0.0, 7.0 / 8.0)), 0.5 + 0.5 * dp);
        return vec4(col.xyz, select(0.2, 0.6, fog1 > 0.5));
    }
    return vec4(0.0);
}
