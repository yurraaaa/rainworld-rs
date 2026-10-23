// The level graphic: one camera image whose pixels are codes (depth, palette group,
// sunlight, floor-dark, decal, grime and effect-colour flags), coloured through the
// room palette. Behaviour follows the game's level colouring as described in
// rainworld-re/notes/room-rendering.md section 3; the CPU reference in level.rs
// mirrors this file.
//
// Texture rows are uploaded bottom row first, so every texture coordinate here uses
// the game's convention: v = 0 is the bottom of the image.

struct Level {
    // xy: scale, zw: offset; clip = room * scale + offset.
    transform: vec4<f32>,
    // xy: room position of the image's bottom-left, zw: image size in room pixels.
    image_rect: vec4<f32>,
    // x: rain time, y: light, z: grime, w: fog amount.
    shading: vec4<f32>,
    // x: darkness, y: contrast, z: saturation, w: hue in degrees.
    grade: vec4<f32>,
    // x: brightness, y: clouds speed, z: swarm room, w: rimFix.
    extra: vec4<f32>,
    // _lightDirAndPixelSize: xy room light angle, zw 1/1400 and 1/800.
    light_dir: vec4<f32>,
    // rgb: _AboveCloudsAtmosphereColor.
    atmosphere: vec4<f32>,
    // The game's window: xy origin in room pixels, zw size (sSize).
    view: vec4<f32>,
    // x: _WetTerrain, y: _waterLevel (from the screen's top).
    wet: vec4<f32>,
};

@group(0) @binding(0) var<uniform> level: Level;
@group(0) @binding(1) var level_tex: texture_2d<f32>;
@group(0) @binding(2) var palette_tex: texture_2d<f32>;
@group(0) @binding(3) var noise_tex: texture_2d<f32>;
@group(0) @binding(4) var noise_sampler: sampler;
// What was drawn before the level (the game's _GrabTexture), same size as the target.
@group(0) @binding(5) var behind_tex: texture_2d<f32>;
// Bilinear, clamped: how the object-shadow lookup reads the grab (INFERRED: Unity's
// render textures default to bilinear).
@group(0) @binding(6) var grab_sampler: sampler;

const TAU_APPROX: f32 = 6.28; // the game uses 3.14 * 2
// Futile's screenPixelOffset on the D3D build (Camera::SCREEN_PIXEL_OFFSET).
const SCREEN_PIXEL_OFFSET: vec2<f32> = vec2(-0.5, 0.5);

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    var corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
        vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0),
    );
    let corner = corners[index];
    let room = level.image_rect.xy + corner * level.image_rect.zw;
    var out: VertexOut;
    out.position = vec4(room * level.transform.xy + level.transform.zw, 0.0, 1.0);
    out.uv = corner;
    return out;
}

// Palette texel (x, row), clamped like the game's clamp-wrapped point sampling.
fn palette(x: i32, row: i32) -> vec4<f32> {
    return textureLoad(palette_tex, vec2(clamp(x, 0, 31), clamp(row, 0, 7)), 0);
}

fn noise(uv: vec2<f32>) -> f32 {
    return textureSample(noise_tex, noise_sampler, uv).r;
}

// Remainder with the sign of x (HLSL fmod).
fn fmod_trunc(x: f32, y: f32) -> f32 {
    return x - y * trunc(x / y);
}

fn rotate_hue(c: vec3<f32>, degrees: f32) -> vec3<f32> {
    let angle = radians(degrees);
    let k = vec3(0.57735);
    let cos_a = cos(angle);
    return c * cos_a + cross(k, c) * sin(angle) + k * dot(k, c) * (1.0 - cos_a);
}

// The game's screenPos at level uv `uv`: lerp over _spriteRect (the level image's rect
// on the screen, Futile's pixel offset included), then flipped as on D3D
// (UNITY_UV_STARTS_AT_TOP): x right, y down, 0..1 over the screen.
fn screen_pos(uv: vec2<f32>) -> vec2<f32> {
    let room = level.image_rect.xy + uv * level.image_rect.zw;
    let up = (room - level.view.xy + SCREEN_PIXEL_OFFSET) / level.view.zw;
    return vec2(up.x, 1.0 - up.y);
}

// The object-shadow lookup of a pixel at `depth` >= 5 (LevelColor: "casting shadows
// from objects", with FixEdgeShadowStretch): whether anything was drawn behind the
// level at the grab position toward the light. `here` is the fragment's frame pixel.
fn object_shadow(uv: vec2<f32>, here: vec2<f32>, depth: i32) -> bool {
    let sp = screen_pos(uv);
    let fix = vec4(saturate(sp / 0.3), saturate(sp / 0.1));
    let d = f32(depth - 5);
    let ld = level.light_dir;
    // Grab coordinates: x right, y up.
    var grab = vec2(sp.x - ld.x * ld.z * d * fix.x, 1.0 - sp.y + ld.y * ld.w * d * fix.y);
    grab = mix(grab, (grab - vec2(0.5, 0.3)) * (1.0 + d / 460.0) + vec2(0.5, 0.3), fix.zw);
    // As a frame-pixel offset from this fragment (frame y down, scaled like the window).
    let frame = vec2<f32>(textureDimensions(behind_tex));
    let scale = abs(level.transform.xy) * frame * 0.5;
    let offset = (grab - vec2(sp.x, 1.0 - sp.y)) * vec2(level.view.z, -level.view.w) * scale;
    let c = textureSampleLevel(behind_tex, grab_sampler, (here + offset) / frame, 0.0);
    return c.r != 0.0 || c.g != 0.0 || c.b != 0.0;
}

// `tex2D(levelTexture, uv)`: point filter, clamp.
fn level_code(uv: vec2<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(level_tex));
    return textureLoad(level_tex, clamp(vec2<i32>(floor(uv * vec2<f32>(size))), vec2(0), size - 1), 0);
}

// Wet terrain: how far (0..1, in steps of 0.001 of the image) the colour lookup moves
// up: thin bands where a noise sine peaks, offset per layer (`ugh`), only with
// `_WetTerrain` and above the water line (`screenPos.y > _waterLevel`, y down, gives 0).
fn wet_displace(uv: vec2<f32>, rain: f32) -> f32 {
    let red = round(level_code(uv).x * 255.0);
    let ugh = fmod_trunc(fmod_trunc(red, 90.0) - 1.0, 30.0) / 300.0;
    var displace = noise(vec2(uv.x * 1.5 - ugh + rain * 0.01, uv.y * 0.25 - ugh + rain * 0.05));
    displace = saturate((sin((displace + uv.x + uv.y + rain * 0.1) * 3.0 * 3.14) - 0.95) * 20.0);
    if level.wet.x < 0.5 || screen_pos(uv).y > level.wet.y {
        displace = 0.0;
    }
    return displace;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(level_tex));
    let rain = level.shading.x;
    // `uv2`: rimFix moves the colour lookup half a texel down-left; wet terrain moves
    // it up.
    let texel = vec2(1.0) / vec2<f32>(size);
    let uv2 = in.uv - texel * 0.5 * level.extra.w;
    let code = level_code(vec2(uv2.x, uv2.y + wet_displace(in.uv, rain) * 0.001));
    let uv = in.uv;
    let fog = palette(1, 7);
    // The noise taps come first, whatever the pixel is (the mip level is picked from the
    // 2x2 block's derivatives, so every pixel of the block takes them): the cloud shadow's
    // (offset by the layer, so a block over a layer's edge steps far in the texture) and
    // the effect colour's ripple.
    let raw_red = i32(round(code.r * 255.0));
    let drift = rain * 0.1 * level.extra.y;
    let layer = 0.003 * f32(raw_red % 30);
    let shadow_noise = noise(vec2(uv.x * 0.5 + drift - layer, 1.0 - uv.y * 0.5 + rain * 0.2 * level.extra.y - layer));
    let ripple_noise = noise(uv * 2.0);

    var colour: vec4<f32>;
    var shows_behind = false;
    if all(code.rgb == vec3(1.0)) {
        // Sky.
        colour = palette(0, 7);
        if level.extra.w > 0.5 {
            colour = vec4(level.atmosphere.rgb, 1.0);
        }
        shows_behind = true;
    } else {
        var green = i32(round(code.g * 255.0));
        var not_floor_dark = 1.0;
        if green >= 16 {
            not_floor_dark = 0.0;
            green -= 16;
        }
        var decal = false;
        if green >= 8 {
            decal = true;
            green -= 8;
        }

        // Moving cloud shadow from the noise texture: 0 = lit, 1 = shadowed.
        var shadow = shadow_noise;
        shadow = 0.5 + sin(fmod_trunc(shadow + drift - uv.y, 1.0) * TAU_APPROX) * 0.5;
        shadow = saturate((shadow - 0.5) * 6.0 + 0.5 - level.shading.y * 4.0);

        var red = raw_red;
        if red > 90 {
            red -= 90;
        } else {
            shadow = 1.0;
        }
        let group = i32(clamp(floor(f32(red - 1) / 30.0), 0.0, 2.0));
        let depth = i32(fmod_trunc(f32(red - 1), 30.0));
        let depth_f = f32(depth);
        let column = i32(f32(depth) * not_floor_dark);

        if shadow != 1.0 && depth >= 5 && object_shadow(uv, in.position.xy, depth) {
            shadow = 1.0;
        }

        colour = mix(palette(column, group + 3), palette(column, group), shadow);

        if green >= 4 {
            let rb = sin((rain + ripple_noise * 4.0 + depth_f / 12.0) * TAU_APPROX) * 0.5 + 0.5;
            colour = mix(colour, palette(i32(floor(5.5 + rb * 25.0)), 6), 0.2 * level.shading.z);
        }

        if decal {
            // Coloured prop: colour from the image's top row at x = 255 - B.
            let table_x = clamp(255 - i32(round(code.b * 255.0)), 0, size.x - 1);
            var decal_colour = textureLoad(level_tex, vec2(table_x, size.y - 1), 0);
            if group == 2 {
                decal_colour = mix(decal_colour, vec4(1.0), 0.2 - shadow * 0.1);
            }
            decal_colour = mix(decal_colour, fog, depth_f / 60.0);
            colour = mix(
                mix(colour, decal_colour, 0.7),
                colour * decal_colour * 1.5,
                mix(0.9, 0.3 + 0.4 * shadow, saturate((depth_f - 3.5) * 0.3)),
            );
        } else if green > 0 && green < 3 {
            // Effect colour A (1) or B (2), blended by B.
            let near_row = 5 - (green - 1) * 2;
            let near = mix(palette(30, near_row), palette(31, near_row), shadow);
            let far = mix(palette(30, near_row - 1), palette(31, near_row - 1), shadow);
            colour = mix(colour, mix(near, far, depth_f / 30.0), code.b);
        } else if green == 3 {
            colour = mix(colour, vec4(1.0), code.b * level.extra.z);
        }

        var fog_scale = 1.0;
        if depth < 10 {
            fog_scale = mix(not_floor_dark, 1.0, 0.5);
        }
        colour = mix(colour, fog, saturate(depth_f * fog_scale * level.shading.w / 30.0));
        shows_behind = depth >= 5;
    }

    if shows_behind {
        let behind = textureLoad(behind_tex, vec2<i32>(in.position.xy), 0);
        if behind.r > 1.0 / 255.0 || behind.g != 0.0 || behind.b != 0.0 {
            return vec4(behind.rgb, 1.0);
        }
    }

    var rgb = colour.rgb * level.grade.x;
    rgb = (rgb - 0.5) * level.grade.y + 0.5;
    let grey = dot(rgb, vec3(0.222, 0.707, 0.071));
    rgb = mix(vec3(grey), rgb, level.grade.z);
    rgb = rotate_hue(rgb, level.grade.w);
    rgb += level.extra.x;
    return vec4(rgb, 1.0);
}
