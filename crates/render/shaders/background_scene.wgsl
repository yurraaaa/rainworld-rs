// A room's background scenes (`AboveCloudsView`, `RoofTopView`), appended to sprite.wgsl
// (it shares its bindings, `VertexOut` and helpers): fs_background `Background`,
// fs_cloud `Cloud`, fs_cloud_distant `CloudDistant`, fs_distant_bkg_object
// `DistantBkgObject`, fs_distant_bkg_object_repeat `DistantBkgObjectRepeatHorizontal`,
// fs_bkg_floor `BkgFloor`, fs_house `House`, fs_dust `Dust`, fs_spores `Spores`
// (StreamingAssets/shaders/<Name>.shader).
//
// The scene sprites sit in the `Water` container, drawn after the level image; they stay
// behind the level by reading it: nothing is drawn where the level image has no
// 255 channel (`BackgroundClipVanilla`), nor where something was drawn behind the level
// (`_GrabTexture`). The base game's sloped-terrain mask is black, so its last test never
// cuts. `RIPPLE` (`smoothRippleClip`) is Watcher's and left out.
//
// The sprite's atlas page is its whole image (a scene image is its own page), so the
// shaders sample it themselves with the image's settings: `Background` and
// `DistantBkgObject` through the page sampler (point, clamped), the others point or
// bilinear with repeat or clamp as `LoadGraphic` set them. A sprite's colour is its
// vertex colour, which these shaders use as parameters.
//
// Globals: `room.scene_atmosphere` `_AboveCloudsAtmosphereColor`, `room.scene_multiply`
// `_MultiplyColor`, `room.scene_world_cam.xy` `_WorldCamPos`; `_windDir` is 1.

@group(1) @binding(2) var clouds_tex: texture_2d<f32>;
@group(1) @binding(3) var clouds_sampler: sampler;
@group(1) @binding(4) var apartments_tex: texture_2d<f32>;
@group(1) @binding(5) var city_palette_tex: texture_2d<f32>;

const WIND_DIR: f32 = 1.0;

// `BackgroundClipVanilla` (and the `House`, `Dust` and `CloudDistant` copies of its first
// two tests): true where the fragment is cut.
fn background_clipped(scr: vec2<f32>) -> bool {
    let level = level_at(level_coord(scr));
    if all(level.rgb != vec3(1.0)) {
        return true;
    }
    return drawn(textureLoad(behind_tex, frame_px(scr), 0));
}

// The page's texel `p` with `repeat` or clamped addressing.
fn page_texel(p: vec2<i32>, repeat: bool) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(atlas));
    var q = clamp(p, vec2(0), size - 1);
    if repeat {
        q = ((p % size) + size) % size;
    }
    return textureLoad(atlas, q, 0);
}

// `tex2D` of a point-filtered, repeating image.
fn page_point_repeat(uv: vec2<f32>) -> vec4<f32> {
    let size = vec2<f32>(textureDimensions(atlas));
    return page_texel(vec2<i32>(floor(uv * size)), true);
}

// `tex2D` of a bilinear image (no mips), repeating or clamped.
fn page_bilinear(uv: vec2<f32>, repeat: bool) -> vec4<f32> {
    let size = vec2<f32>(textureDimensions(atlas));
    let p = uv * size - 0.5;
    let base = floor(p);
    let f = p - base;
    let i = vec2<i32>(base);
    let a = page_texel(i, repeat);
    let b = page_texel(i + vec2(1, 0), repeat);
    let c = page_texel(i + vec2(0, 1), repeat);
    let d = page_texel(i + vec2(1, 1), repeat);
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

// `tex2D(_NoiseTex, uv)` where the texture is magnified.
fn scene_noise(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(noise_tex, noise_sampler, uv, 0.0);
}

// The cloud shaders' `half4(0, 0, 0, 0)` where cut, which a SrcAlpha blend leaves
// untouched.
fn scene_cut(clipped: bool, colour: vec4<f32>) -> vec4<f32> {
    if clipped {
        return vec4(0.0);
    }
    return colour;
}

// `tex2D(_PalTex, (0.5 / 32, 7.5 / 8))`: the palette's sky texel.
fn sky_texel() -> vec4<f32> {
    return palette_at(vec2(0.5 / 32.0, 7.5 / 8.0));
}

// `Background`: the texel times the colour.
@fragment
fn fs_background(in: VertexOut) -> @location(0) vec4<f32> {
    let colour = textureSample(atlas, atlas_sampler, in.uv) * in.color;
    return scene_cut(background_clipped(in.scr), colour);
}

// `DistantBkgObject`: the texel, its colour toward the atmosphere by `clr.x`, times
// `_MultiplyColor`.
@fragment
fn fs_distant_bkg_object(in: VertexOut) -> @location(0) vec4<f32> {
    let c = textureSample(atlas, atlas_sampler, in.uv);
    let colour = vec4(mix(c.rgb, room.scene_atmosphere.rgb, in.color.x), c.a) * room.scene_multiply;
    return scene_cut(background_clipped(in.scr), colour);
}

// `DistantBkgObjectRepeatHorizontal`: the strip repeated sideways with the camera.
@fragment
fn fs_distant_bkg_object_repeat(in: VertexOut) -> @location(0) vec4<f32> {
    let clr = in.color;
    let u = (in.tex.x / clr.z) / clr.y + room.scene_world_cam.x / 1400.0 + clr.w;
    let c = page_point_repeat(vec2(u, in.tex.y));
    return scene_cut(background_clipped(in.scr), c * room.scene_multiply);
}

// `BkgFloor`: a perspective ground from the image, fogged toward the atmosphere by depth.
@fragment
fn fs_bkg_floor(in: VertexOut) -> @location(0) vec4<f32> {
    let clr = in.color;
    let use_y = in.tex.y * (1.0 - clr.y);
    let ground_dist = 1.0 / (1.0 - use_y) - 1.0;
    let coord = vec2(
        in.tex.x + (in.tex.x - 0.5) * ground_dist + room.scene_world_cam.x / 1400.0,
        in.tex.y,
    );
    let c = page_point_repeat(coord);
    let near = clamp((1.0 / clr.x) / 15.0, 0.0, 1.0) * 0.9;
    let far = clamp((1.0 / clr.y) / 15.0, 0.0, 1.0) * 0.9;
    let fog = mix(near, far, pow(in.tex.y, 12.0));
    let colour = vec4(mix(c.rgb, room.scene_atmosphere.rgb, fog), c.a) * room.scene_multiply;
    return scene_cut(background_clipped(in.scr), colour);
}

// `Cloud` (`CloseCloud`): layered density from the image and `_CloudsTex`, posterised
// from the palette's sky to the atmosphere colour.
@fragment
fn fs_cloud(in: VertexOut) -> @location(0) vec4<f32> {
    let clipped = background_clipped(in.scr);
    let clr = in.color;
    let rain = room.rain;
    var sample_coord = vec2(clr.y + in.tex.x - 0.025 * rain * WIND_DIR * (1.0 - clr.x), in.tex.y);
    sample_coord.y = min(1.0 - 0.01, sample_coord.y);
    // `half2(0.5, 2/3)`: the integer division 2/3 is 0.
    var dp_offst = vec2(0.5, 0.0) - in.scr;
    dp_offst.y = dp_offst.y / 2.0;

    let wobble = scene_noise(vec2(sample_coord.x * 1.5, sample_coord.y * 0.75 + clr.y)).x;
    let h2 = 0.5 - sin((WIND_DIR * 0.07 * rain + wobble * 2.0) * 3.14 * 2.0) * 0.5;
    var dp = page_bilinear(
        sample_coord + dp_offst * 0.05 * h2 + vec2(0.0, mix(-1.0, 1.0, h2) * 0.01),
        true,
    ).x;
    dp = dp - 0.1 * h2;
    dp = pow(max(dp - 0.15, 0.0), mix(0.2, 0.35, h2 * h2));
    dp = min(1.0, dp + max(0.0, ((1.0 - in.tex.y) - 0.9) * 15.0));

    var clds = textureSample(
        clouds_tex,
        clouds_sampler,
        vec2(sample_coord.x * 5.0 * (1.0 / clr.z), sample_coord.y * 1.5 + rain * WIND_DIR * 0.025)
            + dp_offst * 0.11 * dp,
    ).x;
    let clds2 = textureSample(
        clouds_tex,
        clouds_sampler,
        vec2(sample_coord.x * 8.0 * (1.0 / clr.z), sample_coord.y * 2.5 + rain * WIND_DIR * 0.018)
            * mix(2.0, 1.0, sin(clr.x * 3.14))
            + dp_offst * mix(0.1, 0.2, clds) * dp,
    ).x;
    if clds2 > 0.5 {
        clds = 2.0 * clds * clds2;
    } else {
        clds = 1.0 - 2.0 * (1.0 - clds) * (1.0 - clds2);
    }
    if clds > 0.5 {
        dp = 2.0 * dp * clds;
    } else {
        dp = 1.0 - 2.0 * (1.0 - dp) * (1.0 - clds);
    }

    let tex_col = page_bilinear(sample_coord + dp_offst * 0.15 * dp, true);
    var noise = scene_noise(vec2(
        (sample_coord.x + dp_offst.x * 0.5 * dp) * 3.0,
        (sample_coord.y + dp_offst.y * 0.5 * dp) * 1.5,
    )).x;
    noise = 0.5 + sin((WIND_DIR * 0.0025 * rain + noise * 2.0) * 3.14 * 2.0) * 0.5;

    var col = pow(tex_col.y, mix(1.4, 0.7, noise)) * clamp((dp - 0.3) * 6.0, 0.5, 1.0);
    if col > 0.5 {
        col = 2.0 * col * clds;
    } else {
        col = 1.0 - 2.0 * (1.0 - col) * (1.0 - clds);
    }
    col = clamp(col * 1.4, 0.0, 1.0);
    col = mix(col, 0.5, clamp(((1.0 - in.tex.y) - mix(0.8, 0.5, noise)) * mix(5.0, 2.0, noise), 0.0, 1.0));

    var out_col = mix(
        pow(sky_texel(), vec4(mix(1.6, 0.4, round(col * 4.0) / 4.0))),
        room.scene_atmosphere,
        clr.x,
    );
    clds = mix(clds, 1.0, col);
    out_col.a = round(
        (pow(clamp(dp, 0.0, 1.0), mix(1.2, 0.05, clds)) * 1.25 - (1.0 - clds) * 0.2 + noise * 0.05) * 3.0,
    ) / 3.0;
    return scene_cut(clipped, out_col);
}

// `CloudDistant` (`DistantCloud`, `FlyingCloud`, a roof's smoke): the image's red as
// density, shaded by the clouds texture, faded by the colour's alpha.
@fragment
fn fs_cloud_distant(in: VertexOut) -> @location(0) vec4<f32> {
    let clipped = background_clipped(in.scr);
    let clr = in.color;
    let rain = room.rain;
    var sample_coord = vec2(clr.y + in.tex.x - 0.025 * rain * WIND_DIR * (1.0 - clr.x), in.tex.y);
    sample_coord.y = min(1.0 - 0.01, sample_coord.y);
    let main_col = page_bilinear(sample_coord, true);
    var dp = main_col.x;
    let clds = textureSample(
        clouds_tex,
        clouds_sampler,
        vec2(sample_coord.x * 5.0 * (1.0 / clr.z), sample_coord.y * 1.5 + rain * WIND_DIR * 0.025),
    ).x;
    if clds > 0.5 {
        dp = 2.0 * dp * clds;
    } else {
        dp = 1.0 - 2.0 * (1.0 - dp) * (1.0 - clds);
    }
    let col = main_col.y * clamp((dp - 0.3) * 6.0, 0.5, 1.0);
    var out_col = mix(
        pow(sky_texel(), vec4(mix(1.6, 0.4, round(col * 4.0) / 4.0))),
        room.scene_atmosphere,
        clr.x,
    );
    out_col.a = (round((pow(clamp(dp, 0.0, 1.0), mix(1.2, 0.05, clds)) * 1.25 - (1.0 - clds) * 0.2) * 3.0) / 3.0) * clr.w;
    return scene_cut(clipped, out_col);
}

// `Dust` (a roof's low smoke): like `CloudDistant` with a drifting sine mask.
@fragment
fn fs_dust(in: VertexOut) -> @location(0) vec4<f32> {
    let clipped = background_clipped(in.scr);
    let clr = in.color;
    let rain = room.rain;
    var sample_coord = vec2(clr.y + in.tex.x - 0.035 * rain * WIND_DIR * (1.0 - clr.x), in.tex.y);
    sample_coord.y = min(1.0 - 0.01, sample_coord.y);
    let main_col = page_bilinear(sample_coord, true);
    var dp = main_col.x;
    let clds = textureSample(
        clouds_tex,
        clouds_sampler,
        vec2(sample_coord.x * 5.0 * (1.0 / clr.z), sample_coord.y * 1.5 + rain * WIND_DIR * 0.035),
    ).x;
    if clds > 0.5 {
        dp = 2.0 * dp * clds;
    } else {
        dp = 1.0 - 2.0 * (1.0 - dp) * (1.0 - clds);
    }
    let wobble = scene_noise(vec2(sample_coord.x, sample_coord.y * 0.25 + clr.y)).x;
    dp = dp * (0.5 - sin((WIND_DIR * 0.32 * rain + wobble * 2.0) * 3.14 * 3.0) * 0.5);
    let col = main_col.y * clamp((dp - 0.3) * 6.0, 0.5, 1.0);
    var out_col = mix(
        pow(sky_texel(), vec4(mix(1.6, 0.4, round(col * 4.0) / 4.0))),
        room.scene_atmosphere,
        clr.x,
    );
    out_col.a = (round((pow(max(dp, 0.0), mix(1.2, 0.05, clds)) * 1.25 - (1.0 - clds) * 0.2) * 3.0) / 3.0) * clr.w;
    return scene_cut(clipped, out_col);
}

// `random2f`: two reads of `_NoiseTex2` multiplied.
fn random2f(coord: vec2<f32>) -> vec3<f32> {
    let a = textureSampleLevel(noise2, noise2_sampler, coord / 256.0, 0.0).xyz;
    let b = textureSampleLevel(noise2, noise2_sampler, coord / 366.0, 0.0).xyz;
    return a * b;
}

// `House` (`RoofTopView.Building`): windows picked per cell of the city image, lit from
// the apartments texture and the city palette, fogged by depth.
@fragment
fn fs_house(in: VertexOut) -> @location(0) vec4<f32> {
    let clipped = background_clipped(in.scr);
    let clr = in.color;
    var div = vec2(0.01, 0.025) * (1.0 / clr.z) * vec2(clr.x * 4000.0, clr.y * 1500.0);
    var grab = in.tex;
    var cell = floor(grab * div) / div;
    grab.y += mix(-0.7, 0.7, random2f(vec2(cell.x * div.x, 0.5)).x) / div.x;
    grab.x += mix(-0.1, 0.1, random2f(cell * vec2(102.21, 91.3232)).x) / div.y;
    cell = floor(grab * div) / div;
    var tex_cell = page_bilinear(cell, false);
    let d = floor((1.0 - tex_cell.x) * 4.0) / 4.0;

    div = vec2(0.01, 0.025) * (1.0 / clr.z) * vec2(clr.x * 4000.0, clr.y * 1500.0) * mix(0.75, 3.0, d);
    grab = in.tex;
    cell = floor(grab * div) / div;
    grab.y += mix(-0.7, 0.7, random2f(vec2(cell.x * div.x, 0.5)).x) / div.x;
    grab.x += mix(-0.1, 0.1, random2f(cell * vec2(102.21, 91.3232)).x) / div.y;
    cell = floor(grab * div) / div;
    tex_cell = page_bilinear(cell, false);
    let dark = tex_cell.x < 0.1;

    let cell_colour = random2f(cell * vec2(754.21, 1.3232));
    grab = vec2(floor(cell_colour.x * 5.0) / 5.0, floor(cell_colour.y * 5.0) / 5.0) + (grab - cell) * div / 5.0;
    let palette_row = floor(random2f(cell * vec2(mix(44.0, 60.0, cell_colour.z), 0.1232)).z * 32.0) / 32.0;

    let apt = textureSampleLevel(apartments_tex, atlas_sampler, grab, 0.0).x;
    var tex_col = textureSampleLevel(city_palette_tex, atlas_sampler, vec2(1.0 - apt, palette_row), 0.0);
    let light = ((1.0 - tex_cell.z) * cell_colour.z * 2.0 + cell_colour.y) / 3.0;
    if light < 0.2 {
        tex_col = vec4(tex_col.xyz * 0.6, tex_col.w);
    }
    let fog = pow(clamp((1.0 / clr.z) / 15.0, 0.0, 1.0), 0.6) * 0.9;
    tex_col = mix(tex_col, room.scene_atmosphere, fog) * room.scene_multiply;
    return scene_cut(clipped || dark, tex_col);
}

// `Spores` (`DustPuff`): a puff of noisy bands in the colour, hatched by a hash.
@fragment
fn fs_spores(in: VertexOut) -> @location(0) vec4<f32> {
    let rain = room.rain;
    let rect = room.sprite_rect;
    let raw = vec2(
        floor(in.scr.x * room.screen.x) / room.screen.x,
        floor(in.scr.y * room.screen.y + rain * 153.2) / room.screen.y,
    );
    var tc = (raw - rect.xy) / (rect.zw - rect.xy);
    tc.y += 0.04;
    let dist = clamp(1.0 - distance(in.tex, vec2(0.5, 0.5)) * 2.0, 0.0, 1.0);
    var h = sin((1.77 * rain + scene_noise(vec2(tc.x * 5.2, rain * -0.1 + tc.y * 2.6)).x * 3.0) * 3.14 * 2.0) * 0.5 + 0.5;
    h = h * (sin((3.5 * rain + scene_noise(vec2(tc.x * 12.2, rain * -0.25 + tc.y * 6.6)).x * 3.0) * 3.14 * 2.0) * 0.5 + 0.5);
    h = h * (0.5 + 0.5 * sin((scene_noise(in.tex).x + rain) * 6.28 * 3.0));
    h = mix(h * dist, mix(h, 1.0, mix(0.3, 0.8, in.color.w)), dist);
    let rand = fract(sin(tc.x * 12.98232 + tc.y - scene_noise(tc).x) * 43758.5453);
    h = h - rand * mix(0.7, 0.3, in.color.w);
    if h * in.color.w < 0.35 {
        return vec4(0.0);
    }
    if h * in.color.w > 0.5 {
        return vec4(in.color.xyz * 0.7, 1.0);
    }
    return vec4(in.color.xyz, 1.0);
}
