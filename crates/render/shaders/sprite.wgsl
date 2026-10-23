// Room sprites (atlas sprites and TriangleMeshes) in stage pixels (room minus camera),
// alpha blended (SrcAlpha, OneMinusSrcAlpha) by the pipeline, as every shader here.
// One fragment entry per game shader (StreamingAssets/shaders/<Name>.shader): fs_basic
// `Futile/Basic`, fs_flat_light `FlatLight`, fs_overseer_zip `OverseerZip`,
// fs_hologram `Hologram`, fs_light_source `LightSource`, fs_holo_grid `HoloGrid`,
// fs_hologram_image `HologramImage`, fs_shortcut `ShortCut0`, fs_colored_sprite3
// `ColoredSprite2` with `no_grab_tex` (the `ColoredSprite3` shader),
// fs_under_water_light `UnderWaterLight`, fs_specific_depth `SpecificDepth`,
// fs_water_splash `WaterSplash`, fs_water_light `WaterLight`, fs_light_beam `LightBeam`,
// fs_vector_circle `VectorCircle`, fs_custom_depth `CustomDepth`. The
// full-screen effects are in
// full_screen.wgsl, appended to this file, and the background scenes' in
// background_scene.wgsl.
//
// `uv` is where the port's atlas is sampled; `tex` is the shader's `i.uv` (they differ
// only for `Futile_White`, a texture of its own in the game).

struct Room {
    // xy: scale, zw: offset; clip = stage * scale + offset.
    transform: vec4<f32>,
    // `_spriteRect`: the level image in screen coordinates (`RoomGlobals::new`).
    sprite_rect: vec4<f32>,
    // `_screenSize`.
    screen: vec2<f32>,
    // `_RAIN`.
    rain: f32,
    // `_hologramThreshold`.
    hologram_threshold: f32,
    // The frame's size in pixels (the screen-sized textures of group 2).
    frame: vec2<f32>,
    // `_fogAmount`.
    fog_amount: f32,
    // `_waterLevel`: the water line's fraction of the screen from its top.
    water_level: f32,
    // `_camInRoomRect`: the game's window in room uv (position, size).
    cam_in_room_rect: vec4<f32>,
    // x: `_light`, y: `_cloudsSpeed`, z: `_WetTerrain`.
    light: vec4<f32>,
    // `_lightDirAndPixelSize`: xy the room's light angle, zw 1/1400 and 1/800.
    light_dir: vec4<f32>,
    // The background scenes' `_AboveCloudsAtmosphereColor`, `_MultiplyColor` and
    // `_WorldCamPos` (xy).
    scene_atmosphere: vec4<f32>,
    scene_multiply: vec4<f32>,
    scene_world_cam: vec4<f32>,
};

// Futile's `screenPixelOffset` (`Camera::SCREEN_PIXEL_OFFSET`): screen = stage + this.
const SCREEN_PIXEL_OFFSET: vec2<f32> = vec2(-0.5, 0.5);

@group(0) @binding(0) var<uniform> room: Room;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;
// `_NoiseTex2` (`Palettes/noise2`).
@group(1) @binding(0) var noise2: texture_2d<f32>;
@group(1) @binding(1) var noise2_sampler: sampler;
// `_LevelTex` (v = 0 at the bottom), the `_GrabTexture` of the last unnamed GrabPass
// before the level (what was drawn behind it), the frame as it was just before a
// `HologramImage` sprite (its own GrabPass) and `_PalTex` (32x8, point, clamp).
@group(2) @binding(0) var level_tex: texture_2d<f32>;
@group(2) @binding(1) var behind_tex: texture_2d<f32>;
@group(2) @binding(2) var grab_tex: texture_2d<f32>;
@group(2) @binding(3) var pal_tex: texture_2d<f32>;
// `_NoiseTex` (`Palettes/noise`, bilinear, repeat).
@group(2) @binding(4) var noise_tex: texture_2d<f32>;
@group(2) @binding(5) var noise_sampler: sampler;
// The room's `RainMask_<room>` (one texel per tile, v = 0 at the bottom), read with
// the grab sampler (bilinear, clamped: the generated texture's settings).
@group(2) @binding(7) var rain_mask: texture_2d<f32>;

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
    out.scr = (pos + SCREEN_PIXEL_OFFSET) / room.screen;
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
    let clip = (scr * room.screen - SCREEN_PIXEL_OFFSET) * room.transform.xy + room.transform.zw;
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

fn noise_at(uv: vec2<f32>) -> f32 {
    return textureSampleLevel(noise_tex, noise_sampler, uv, 0.0).x;
}

// `UnderWaterLight` (not `VOIDSEA`, not `strong`; no brain mold or ripple): the level
// read a little displaced by noise, near pixels lit most, 6/30 over a creature drawn
// behind the level (the last grab, as `LightSource`); a soft disc grown by depth,
// rippled by two noise sines, `(clr.rgb, dist * clr.a * lerp(0.75, 0.5, centerDist))`.
@fragment
fn fs_under_water_light(in: VertexOut) -> @location(0) vec4<f32> {
    var tc = level_coord(in.scr);
    let rbcol = sin((room.rain + noise_at(tc * 1.2) * 3.0) * 3.14 * 2.0) * 0.5 + 0.5;
    var distortion = vec2(
        mix(-0.002, 0.002, rbcol) * mix(1.0, 20.0, pow(in.tex.y, 200.0)),
        -0.02 * pow(in.tex.y, 8.0),
    );
    distortion = floor(distortion * room.screen) / room.screen;
    var dist = depth_of(level_at(tc + distortion));
    if dist > 6.0 / 30.0 && drawn(textureLoad(behind_tex, frame_px(in.scr), 0)) {
        dist = 6.0 / 30.0;
    }
    let dir = normalize(in.tex - vec2(0.5));
    let cd = clamp(distance(in.tex, vec2(0.5)) * 2.0, 0.0, 1.0);
    tc += cd * dir * 0.02;
    let d = dist;
    if dist < 0.2 {
        dist = pow(1.0 - dist * 5.0, 0.35);
    } else {
        dist = clamp((dist - 0.2) * 1.3, 0.0, 1.0);
    }
    dist = 1.0 - dist;
    dist *= pow(pow(1.0 - pow(min(cd + d * 0.5, 1.0), 2.0), 3.5), 1.0);
    var wave = room.rain * 6.0 + noise_at(vec2(
        d / 10.0 + mix(tc.x, 0.5, d / 3.0) * 2.1,
        room.rain * 0.1 + d / 5.0 + mix(tc.y, 0.5, d / 3.0) * 2.1,
    )) * 7.0;
    let col = sin(wave * 3.14 * 2.0) * 0.5 + 0.5;
    tc += dir * 0.15 * pow(1.0 - cd, 1.7);
    wave = room.rain * 2.7 + noise_at(vec2(
        d / 7.0 + mix(tc.x, 0.5, d / 5.0) * 1.3,
        room.rain * -0.21 + d / 8.0 + mix(tc.y, 0.5, d / 6.0) * 1.3,
    )) * 6.33;
    let col2 = sin(wave * 3.14 * 2.0) * 0.5 + 0.5;
    if pow(max(col, col2), 47.0) >= 0.8 {
        dist = pow(dist, 0.6 + 0.4 * cd);
    }
    dist *= mix(1.0, max(col, col2), pow(cd, 1.5));
    return vec4(in.color.rgb, dist * in.color.a * mix(0.75, 0.5, cd));
}

// `HoloGrid`'s `DepthAtTextCoord`: the level depth, 6/30 over a creature drawn behind
// the level (deeper than layer 6).
fn holo_grid_depth(tc: vec2<f32>, scr: vec2<f32>) -> f32 {
    let d = depth_of(level_at(tc));
    if d > 6.0 / 30.0 && drawn(textureLoad(behind_tex, frame_px(scr), 0)) {
        return 6.0 / 30.0;
    }
    return d;
}

// `HoloGrid` (the guide's light): nothing over sky; a grid of lines every 20 of 300
// across the sprite (pushed out by depth) on layers past 1; the sample pushed toward the
// eye (`clr.xy`, by `clr.z`); terrain edges of layers 3-7, 14-17 and 22-24 lit (1 on the
// near ones, else 0.35); cut by scan-line noise against `_hologramThreshold * clr.a`;
// (0.87, 0.72, 0.35) at `0.6 * light * (1 - centerDist)`.
@fragment
fn fs_holo_grid(in: VertexOut) -> @location(0) vec4<f32> {
    var tc = level_coord(in.scr);
    var scr = in.scr;
    var uv = in.tex;
    let a = in.color.a;
    var light = 0.0;
    var cd = clamp(distance(vec2(0.5), uv) * 2.0 + (1.0 - a), 0.0, 1.0);
    var dpth = holo_grid_depth(tc, scr);
    if dpth >= 0.999 {
        return vec4(0.0);
    }
    uv += ((tc - vec2(0.5, 0.66)) * 0.4 + (uv - vec2(0.5)) * 0.4) * dpth;
    if dpth > 0.03 && (fmod_trunc(floor(uv.x * 300.0), 20.0) == 10.0
        || fmod_trunc(floor(uv.y * 300.0), 20.0) == 10.0) {
        light = 0.35;
    }
    let push = normalize(uv - in.color.xy) * mix(-0.2, (0.2 - dpth) * cd / 40.0, in.color.z);
    scr -= push;
    tc -= push;
    dpth = holo_grid_depth(tc, scr);
    if (dpth > 3.0 / 30.0 && dpth < 7.0 / 30.0) || (dpth > 14.0 / 30.0 && dpth < 17.0 / 30.0)
        || (dpth > 22.0 / 30.0 && dpth < 24.0 / 30.0) {
        let px = vec2(1.0 / room.screen.x, 1.0 / room.screen.y);
        if holo_grid_depth(tc + vec2(-1.0 / 1400.0, 0.0), scr + vec2(-px.x, 0.0)) > dpth
            || holo_grid_depth(tc + vec2(1.0 / 1400.0, 0.0), scr + vec2(px.x, 0.0)) > dpth
            || holo_grid_depth(tc + vec2(0.0, 1.0 / 800.0), scr + vec2(0.0, px.y)) > dpth
            || holo_grid_depth(tc + vec2(0.0, -1.0 / 800.0), scr + vec2(0.0, -px.y)) > dpth {
            light = select(0.35, 1.0, dpth < 7.0 / 30.0);
        }
        cd = pow(cd, 2.0);
    }
    var h = noise2_at(vec2<f32>(tc.x * 4.0, tc.y * 8.0 - room.rain * 10.0)) * 2.0;
    h -= pow(a, 2.0) * (1.0 - pow(cd, 2.0));
    if fmod_trunc(round((tc.y - room.rain * 0.15) * 400.0), 3.0) == 0.0 {
        h += mix(0.6, 0.15, light);
    }
    if h > room.hologram_threshold * a {
        return vec4(0.0);
    }
    light *= 1.0 - cd;
    return vec4(0.87, 0.72, 0.35, light * 0.6);
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

// `ShortCut0`: the vertex colour where the level texel under this screen pixel is a
// shortcut pipe (`round(g * 255)` 8 to 10, blue 0), nothing elsewhere.
@fragment
fn fs_shortcut(in: VertexOut) -> @location(0) vec4<f32> {
    let texcol = level_at(level_coord(in.scr));
    let g = byte(texcol.g);
    if g > 7 && g < 11 && texcol.b == 0.0 {
        return in.color;
    }
    return vec4(0.0);
}

// `SpecificDepth` (the ant): the colour, with the texel's alpha, where the level layer
// under the pixel (`(round(r * 255) - 1) % 30`, sky 30, a deep layer over something
// drawn before the level 6: the level's grab) is the sprite's (`1 - alpha`, in 30ths).
@fragment
fn fs_specific_depth(in: VertexOut) -> @location(0) vec4<f32> {
    let texcol = level_at(level_coord(in.scr));
    let dpth = 1.0 - in.color.a;
    var terrain = f32((byte(texcol.x) - 1) % 30) / 30.0;
    if all(texcol.xyz == vec3(1.0)) {
        terrain = 1.0;
    }
    if terrain > 6.0 / 30.0 && drawn(textureLoad(behind_tex, frame_px(in.scr), 0)) {
        terrain = 6.0 / 30.0;
    }
    if floor(dpth * 30.0) == floor(terrain * 30.0) {
        return vec4(in.color.rgb, textureSample(atlas, atlas_sampler, in.uv).a);
    }
    return vec4(0.0);
}

// `CustomDepth` (the sky dandelions' and green sparks' depth-faded sprites): the colour,
// with the texel's alpha, where the sprite's depth (`1 - alpha`) is no deeper than the
// level layer under the pixel (`(round(r * 255) - 1) % 30`, sky 30; a deep layer over
// something drawn before the level counts as 6, the level's grab).
@fragment
fn fs_custom_depth(in: VertexOut) -> @location(0) vec4<f32> {
    let texcol = level_at(level_coord(in.scr));
    let dpth = 1.0 - in.color.a;
    var terrain = f32((byte(texcol.x) - 1) % 30) / 30.0;
    if all(texcol.xyz == vec3(1.0)) {
        terrain = 1.0;
    }
    if terrain > 6.0 / 30.0 && drawn(textureLoad(behind_tex, frame_px(in.scr), 0)) {
        terrain = 6.0 / 30.0;
    }
    if dpth > terrain {
        return vec4(0.0);
    }
    return vec4(in.color.rgb, textureSample(atlas, atlas_sampler, in.uv).a);
}

// `tex2D(_PalTex, uv)`: point filter, clamp.
fn palette_at(uv: vec2<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(pal_tex));
    let texel = clamp(vec2<i32>(floor(uv * vec2<f32>(size))), vec2(0), size - 1);
    return textureLoad(pal_tex, texel, 0);
}

// `TerrainAndLevelDepth / 30` with the base game's black sloped-terrain mask (depth 60,
// never the nearer): red over 90 less 90, `fmod(red - 1, 30)`, 30 for sky.
fn terrain_and_level_depth(c: vec4<f32>) -> f32 {
    if all(c.rgb == vec3(1.0)) {
        return 1.0;
    }
    var red = byte(c.r);
    if red > 90 {
        red -= 90;
    }
    return fmod_trunc(f32(red - 1), 30.0) / 30.0;
}

// `ColoredSprite3` (`Futile/ColoredSprite2`, `no_grab_tex`): depth `lerp(1 - clr.a, 1,
// tex.r)`; nothing behind the level's pixel, else the palette texel at column
// `lerp(0.5, 29.5, depth)`, row `lerp(0.5, 2.5, tex.b)`, a quarter toward a non-white
// vertex colour, fogged by `depth * _fogAmount`, with the texel's alpha.
@fragment
fn fs_colored_sprite3(in: VertexOut) -> @location(0) vec4<f32> {
    let texcol = textureSample(atlas, atlas_sampler, in.uv);
    let dpth = mix(1.0 - in.color.a, 1.0, texcol.x);
    let terrain = terrain_and_level_depth(level_at(level_coord(in.scr)));
    if dpth > terrain {
        return vec4(0.0);
    }
    var col = palette_at(vec2(mix(0.5, 29.5, dpth) / 32.0, mix(0.5, 2.5, texcol.z) / 8.0));
    if any(in.color.rgb < vec3(1.0)) {
        col = mix(col, in.color, 0.25);
    }
    col = mix(col, palette_at(vec2(1.5 / 32.0, 7.5 / 8.0)), dpth * room.fog_amount);
    return vec4(clamp(col.rgb, vec3(1.0 / 255.0), vec3(1.0)), texcol.w);
}

// `WaterSplash` (no `RIPPLE`, a Watcher keyword), a `Splash`'s quads. The falloff `dist`
// is radial, or with green sideways (`|uv.x - 0.5| * 2` to `0.7 - 0.4 clr.a`); the
// screen pixel's level coordinate is pushed by a noise wobble (in whole pixels, less
// toward the rim), and the level read twice that far. `h` is a noise pattern (red
// scrolls it), raised toward 1 by the alpha unless the sprite is foam (blue), less
// `dist`, then times the alpha (foam 0.05 less); under `Lerp(0.05, 0.01, alpha)` the
// pixel is left alone. The colour runs from the palette's `waterColor2` to
// `waterColor1` by the level depth `dp`, toward the fog colour with the alpha away from
// the water line (`_waterLevel`); foam is whitened (a slow sine glint, more on far
// layers). Deeper than layer 6, the pre-level grab (`_GrabTexture`: this shader has no
// GrabPass, the last one is the level's) decides: the background bubbles' `(0, 1/255,
// 0)` makes it lit water (`dp` 1, colour `waterColor1`), anything else drawn there
// makes `dp` 6/30 and the splash takes that colour (half of it near the water line).
@fragment
fn fs_water_splash(in: VertexOut) -> @location(0) vec4<f32> {
    let clr = in.color;
    let proximity = 1.0 - clamp((in.scr.y - ((1.0 - room.water_level) - 0.14)) * 5.0, 0.0, 1.0);
    var tc = level_coord(in.scr);
    let sideways = pow(abs(in.tex.x - 0.5) * 2.0, 0.7 - clr.a * 0.4);
    let dist = mix(center_dist(in.tex), sideways, clr.y);
    let wobble = sin((room.rain + noise_at(tc * 1.2) * 3.0) * 3.14 * 2.0) * 0.5 + 0.5;
    var distortion = vec2(mix(-0.002, 0.002, wobble) * (1.0 - dist));
    distortion = floor(distortion * room.screen) / room.screen;
    tc += distortion;
    let pattern = noise_at(vec2(tc.x * 18.2, room.rain * 2.4 + tc.y * 12.2));
    var h = 0.5 + sin((18.5 * room.rain + pattern * 3.0) * 3.14) * 0.5;
    h *= noise_at(in.tex - vec2(room.rain * 23.0 * clr.x));
    h = max(0.0, mix(h, 1.0, clr.a * (1.0 - clr.z)) - dist);
    let texcol = level_at(tc + distortion);
    var grab = textureLoad(behind_tex, frame_px(in.scr + distortion), 0);
    var dp = depth_of(texcol);
    if dp > 6.0 / 30.0 {
        if byte(grab.r) == 0 && byte(grab.g) == 1 && byte(grab.b) == 0 {
            dp = 1.0;
            grab = palette_at(vec2(4.5 / 32.0, 7.5 / 8.0));
        } else if drawn(grab) {
            dp = 6.0 / 30.0;
        }
    }
    h *= clr.a;
    if clr.z > 0.5 {
        h -= 0.05;
    }
    if h < mix(0.05, 0.01, clr.a) {
        return vec4(0.0);
    }
    var col = mix(
        palette_at(vec2(5.5 / 32.0, 7.5 / 8.0)),
        palette_at(vec2(4.5 / 32.0, 7.5 / 8.0)),
        dp,
    );
    col = mix(col, palette_at(vec2(0.5 / 32.0, 7.5 / 8.0)), (1.0 - clr.a) * 0.5 * dp * (1.0 - proximity));
    if clr.z > 0.5 {
        let glint = pow(0.5 + sin(room.rain * 25.0 + (tc.x - tc.y * 2.0) / 3.0) * 0.5, 7.0);
        col = mix(col, vec4(1.0), (0.3 + 0.2 * glint + 0.5 * dp) * (1.0 - proximity));
    }
    if dp >= 6.0 / 30.0 && drawn(grab) {
        col = mix(col, grab, mix(1.0, 0.5, proximity));
    }
    return col;
}

// `WaterLight` (the room camera's caustics over the water). Nothing on level layers
// nearer than 2 (`_waterPosition` is never set: 0), over anything drawn behind a level
// pixel deeper than 6/30 (the pre-level grab, any channel not 0) or on sky. Two
// noise-driven sines (the literal `3.14 * 2`) drift with `_RAIN`; their max to the
// 47th, times `1 - uv.y`, a squared noise read at the sprite's uv and the room mask's
// blue at this pixel's room position (`_camInRoomRect`), shows as white at
// `0.05 * alpha` where it reaches 0.1.
@fragment
fn fs_water_light(in: VertexOut) -> @location(0) vec4<f32> {
    let tc = level_coord(in.scr);
    let texcol = level_at(tc);
    let dist = fmod_trunc(f32(byte(texcol.r) - 1), 30.0) / 30.0;
    if dist < 2.0 / 30.0 {
        return vec4(0.0);
    }
    if dist > 6.0 / 30.0 {
        let grab = textureLoad(behind_tex, frame_px(in.scr), 0);
        if any(grab.rgb != vec3(0.0)) {
            return vec4(0.0);
        }
    }
    if all(texcol.rgb == vec3(1.0)) {
        return vec4(0.0);
    }
    let rain = room.rain;
    let n1 = noise_at(vec2(
        dist / 10.0 + mix(tc.x, 0.5, dist / 3.0) * 2.1,
        rain * 0.1 + dist / 5.0 + mix(tc.y, 0.5, dist / 3.0) * 2.1,
    ));
    let col1 = sin((rain * 6.0 + n1 * 7.0) * 3.14 * 2.0) * 0.5 + 0.5;
    let n2 = noise_at(vec2(
        dist / 7.0 + mix(tc.x, 0.5, dist / 5.0) * 1.3,
        rain * -0.21 + dist / 8.0 + mix(tc.y, 0.5, dist / 6.0) * 1.3,
    ));
    let col2 = sin((rain * 2.7 + n2 * 6.33) * 3.14 * 2.0) * 0.5 + 0.5;
    let n3 = noise_at(vec2(in.tex.x, in.tex.y + rain + dist * 0.1));
    var col = pow(max(col1, col2), 47.0) * (1.0 - in.tex.y) * n3 * n3;
    let cam = room.cam_in_room_rect;
    col *= textureSampleLevel(rain_mask, grab_sampler, cam.xy + cam.zw * in.scr, 0.0).z;
    if col < 0.1 {
        return vec4(0.0);
    }
    return vec4(1.0, 1.0, 1.0, 0.05 * in.color.a);
}

// `LightBeam` (no snow, ripple or terrain mask: the base game's sloped-terrain mask
// is black, so its depth 2 never wins): `uv.y * sin(uv.x * 3.14)` faded by the level
// depth under the pixel (`(int)(r * 254) % 30` in 30ths, sky 1, 6/30 over a creature
// drawn behind the level) in the third of the vertex alpha: third 1 by `(dp - 0.2) *
// 10`, third 2 by `(dp - 0.5) * 20`; times the alpha's place inside its third.
@fragment
fn fs_light_beam(in: VertexOut) -> @location(0) vec4<f32> {
    let texcol = level_at(level_coord(in.scr));
    var dp = f32(i32(texcol.r * 254.0) % 30) / 30.0;
    if all(texcol.rgb == vec3(1.0)) {
        dp = 1.0;
    }
    dp = min(dp, 2.0);
    if dp > 6.0 / 30.0 && drawn(textureLoad(behind_tex, frame_px(in.scr), 0)) {
        dp = 6.0 / 30.0;
    }
    let tp = i32(floor(in.color.w * 3.0));
    var alpha = in.tex.y * sin(in.tex.x * 3.14);
    if tp == 1 {
        alpha *= clamp((dp - 0.2) * 10.0, 0.0, 1.0);
    } else if tp == 2 {
        alpha *= clamp((dp - 0.5) * 20.0, 0.0, 1.0);
    }
    alpha *= in.color.w * 3.0 - f32(tp);
    return vec4(in.color.rgb, alpha);
}

// `VectorCircle` (a `SpotLight`): the vertex colour, opaque, inside the ellipse
// inscribed in the UV square; nothing where `centerDist < 1 - clr.w` or on the rim.
@fragment
fn fs_vector_circle(in: VertexOut) -> @location(0) vec4<f32> {
    let center_dist = clamp(distance(in.tex, vec2(0.5)) * 2.0, 0.0, 1.0);
    if center_dist < 1.0 - in.color.w || center_dist == 1.0 {
        return vec4(0.0);
    }
    return vec4(in.color.rgb, 1.0);
}
