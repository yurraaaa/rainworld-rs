// `Futile/Decal` (a `CustomDecal`'s grid mesh) and `Futile/Rainbow` (a `Rainbow`'s square).
// Appended to sprite.wgsl after death_fall.wgsl (its level, palette, noise, grab and
// frame helpers); the page bound as `atlas` is the decal image or the rainbow gradient,
// whole, so `in.uv` is the shader's `i.uv`.

fn screen_blend3(a: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
    return vec3(1.0) - (vec3(1.0) - a) * (vec3(1.0) - b);
}

// `Decal` (no `COMBINEDLEVEL`, `RIPPLE` or `URBANLIFE`, which are DLC keywords). The level
// pixel under the screen pixel (wet terrain moves the lookup up a little, by the decal's
// own red channel read at the level coordinate, as the shader does) gives the layer
// `red`; sky shows nothing. The cloud shadow `shadow` is the level shader's; layers
// before 90 are never in shadow. The decal shows only where `red / 30` lies between the
// vertex colour's red and green. A shadowed pixel from layer 5 on is lit again where
// something was drawn behind the level toward the light (`_lightDirAndPixelSize`, the
// edge fixes), and a pixel from layer 5 on hides behind anything drawn behind the
// level at itself. The terrain's palette colour (shadowed or lit, toward the fog colour
// by depth) is mixed with the decal texel, which is read about the centre scaled up to
// 1.5 times with depth and, with the vertex colour's blue, displaced upward by a noise
// ridge. The alpha is the texel's, quartered-ish in shadow, times the vertex colour's alpha.
@fragment
fn fs_decal(in: VertexOut) -> @location(0) vec4<f32> {
    let rain = room.rain;
    let light = room.light.x;
    let clouds_speed = room.light.y;
    let tc = level_coord(in.scr);

    let own_red = round(textureSampleLevel(atlas, atlas_sampler, tc, 0.0).x * 255.0);
    let ugh = fmod_trunc(fmod_trunc(own_red, 90.0) - 1.0, 30.0) / 300.0;
    var displace = noise_at(vec2(tc.x * 1.5 - ugh + rain * 0.01, tc.y * 0.25 - ugh + rain * 0.05));
    displace = clamp((sin((displace + tc.x + tc.y + rain * 0.1) * 3.0 * 3.14) - 0.95) * 20.0, 0.0, 1.0);
    if room.light.z < 0.5 || 1.0 - in.scr.y > room.water_level {
        displace = 0.0;
    }
    let texcol = level_at(vec2(tc.x, tc.y + displace * 0.001));
    if all(texcol.rgb == vec3(1.0)) {
        return vec4(0.0);
    }
    var red = byte(texcol.r);

    var shadow = noise_at(vec2(
        tc.x * 0.5 + rain * 0.1 * clouds_speed - 0.003 * fmod_trunc(f32(red), 30.0),
        1.0 - tc.y * 0.5 + rain * 0.2 * clouds_speed - 0.003 * fmod_trunc(f32(red), 30.0)));
    shadow = 0.5 + sin(fmod_trunc(shadow + rain * 0.1 * clouds_speed - tc.y, 1.0) * 3.14 * 2.0) * 0.5;
    shadow = clamp((shadow - 0.5) * 6.0 + 0.5 - light * 4.0, 0.0, 1.0);

    if red > 90 {
        red -= 90;
    } else {
        shadow = 1.0;
    }
    let palette_color = clamp(i32(floor(f32(red - 1) / 30.0)), 0, 2);
    red = i32(fmod_trunc(f32(red - 1), 30.0));
    let depth = f32(red) / 30.0;
    if depth < in.color.x || depth > in.color.y {
        return vec4(0.0);
    }

    if shadow != 1.0 && red >= 5 {
        // `FixEdgeShadowStretch(scrPos, true)`.
        let sp = vec2(in.scr.x, 1.0 - in.scr.y);
        let fix = vec4(clamp(sp / 0.3, vec2(0.0), vec2(1.0)), clamp(sp / 0.1, vec2(0.0), vec2(1.0)));
        let ld = room.light_dir;
        let d = f32(red - 5);
        var grab = vec2(in.scr.x - ld.x * ld.z * d * fix.x, in.scr.y + ld.y * ld.w * d * fix.y);
        grab = mix(grab, (grab - vec2(0.5, 0.3)) * (1.0 + d / 460.0) + vec2(0.5, 0.3), fix.zw);
        let behind = textureLoad(behind_tex, frame_px(grab), 0);
        if behind.r != 0.0 || behind.g != 0.0 || behind.b != 0.0 {
            shadow = 1.0;
        }
    }

    let column = f32(red) / 32.0;
    var terrain = mix(
        palette_at(vec2(column, (f32(palette_color) + 3.5) / 8.0)),
        palette_at(vec2(column, (f32(palette_color) + 0.5) / 8.0)),
        shadow);
    terrain = mix(
        terrain,
        palette_at(vec2(1.5 / 32.0, 7.5 / 8.0)),
        clamp(f32(red) * room.fog_amount / 30.0, 0.0, 1.0));

    if red >= 5 && drawn(textureLoad(behind_tex, frame_px(in.scr), 0)) {
        return vec4(0.0);
    }

    var uv = in.uv - vec2(0.5);
    uv *= mix(1.0, 1.5, f32(red) / 30.0);
    uv += vec2(0.5);
    var h = tc.x;
    h = mix(floor(h * 700.0) / 700.0, h, 0.5);
    let ridge = max(0.0, noise2_at(vec2(h * 4.0, tc.y * 0.01 + f32(red) / 100.0)) - 0.5) * 2.0;
    uv.y += pow(ridge, 3.0 - in.color.z) * 0.3 * in.color.z;

    let mine = textureSampleLevel(atlas, atlas_sampler, uv, 0.0);
    var lit = mix(terrain.rgb, mine.rgb, 0.5);
    lit = mix(
        lit,
        screen_blend3(terrain.rgb, mine.rgb),
        (1.0 - shadow) * select(0.25, 0.9, palette_color == 2));
    let rgb = mix(lit, terrain.rgb * mine.rgb, mix(0.2, 0.3, shadow));
    return vec4(rgb, mine.a * mix(1.0, 0.25 + 0.25 * in.color.a, shadow) * in.color.a);
}

// `tex2D(_MainTex, uv)` of the 64-wide rainbow gradient: bilinear, repeating (the
// texture's import settings).
fn rainbow_gradient(uv: vec2<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(atlas));
    let p = uv * vec2<f32>(size) - vec2(0.5);
    let f = fract(p);
    let i0 = vec2<i32>(floor(p));
    let a = textureLoad(atlas, ((i0 % size) + size) % size, 0);
    let b = textureLoad(atlas, (((i0 + vec2(1, 0)) % size) + size) % size, 0);
    let c = textureLoad(atlas, (((i0 + vec2(0, 1)) % size) + size) % size, 0);
    let d = textureLoad(atlas, (((i0 + vec2(1, 1)) % size) + size) % size, 0);
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

// `Rainbow` (it has a `GrabPass {}`): a ring between the vertex colour's red (the inner
// fraction) and the square's inscribed circle, its colour the gradient across the ring,
// its opacity a sparkle of two noise textures (rare glints brighten) under a sine
// envelope across the ring, scaled up with the level depth under it (none on the
// nearest layers, sky full), screened onto the frame so far, at the vertex colour's
// alpha.
@fragment
fn fs_rainbow(in: VertexOut) -> @location(0) vec4<f32> {
    let tc = level_coord(in.scr);
    let dst = distance(in.uv, vec2(0.5)) * 2.0;
    if dst > 1.0 || dst < in.color.x {
        return vec4(0.0);
    }
    let rain = room.rain;
    var n = 0.5 + 0.5 * sin((noise2_at(vec2(tc.x * 30.0, tc.y * 15.0 + rain * 0.12)) * 3.0
        + noise_at(vec2(tc.x * 4.0, tc.y * 2.0 + rain * 0.04))) * 6.28 * 2.5);
    n = n * 3.0 - floor(n * 3.0);
    n = 0.5 + 0.5 * n;
    let d = clamp((dst - in.color.x) / (1.0 - in.color.x), 0.0, 1.0);
    var col = rainbow_gradient(vec2(0.5, d));
    if n > 0.99995 {
        let add = textureSampleLevel(noise2, noise2_sampler, vec2(tc.x * 30.0, tc.y * 15.0 + rain * 0.2812), 0.0).y;
        col = vec4(clamp(col.rgb * (1.0 + 3.0 * add), vec3(0.0), vec3(1.0)), 1.0);
        n *= 1.0 + add;
    }
    n *= clamp(sin(d * 3.14) * 1.5, 0.0, 1.0);
    col.a *= n;
    let lev = level_at(tc);
    var dp = f32(i32(lev.x * 254.0) % 30) / 30.0;
    if all(lev.rgb == vec3(1.0)) {
        dp = 1.0;
    }
    col.a *= max(0.0, (dp - 0.2) * 1.5);
    let grab = textureLoad(grab_tex, frame_px(in.scr), 0);
    return vec4(vec3(1.0) - (vec3(1.0) - grab.rgb) * (vec3(1.0) - col.rgb * col.a), in.color.a);
}
