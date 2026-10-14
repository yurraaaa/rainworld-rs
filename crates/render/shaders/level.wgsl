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
    // x: brightness, y: clouds speed, z: swarm room.
    extra: vec4<f32>,
};

@group(0) @binding(0) var<uniform> level: Level;
@group(0) @binding(1) var level_tex: texture_2d<f32>;
@group(0) @binding(2) var palette_tex: texture_2d<f32>;
@group(0) @binding(3) var noise_tex: texture_2d<f32>;
@group(0) @binding(4) var noise_sampler: sampler;
// What was drawn before the level (sprites behind it), same size as the target.
@group(0) @binding(5) var behind_tex: texture_2d<f32>;

const TAU_APPROX: f32 = 6.28; // the game uses 3.14 * 2

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
    return textureSampleLevel(noise_tex, noise_sampler, uv, 0.0).r;
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

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(level_tex));
    let pixel = clamp(vec2<i32>(floor(in.uv * vec2<f32>(size))), vec2(0), size - 1);
    let code = textureLoad(level_tex, pixel, 0);
    let uv = in.uv;
    let rain = level.shading.x;
    let fog = palette(1, 7);

    var colour: vec4<f32>;
    var shows_behind = false;
    if all(code.rgb == vec3(1.0)) {
        // Sky.
        colour = palette(0, 7);
        shows_behind = true;
    } else {
        let raw_red = i32(round(code.r * 255.0));
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
        let drift = rain * 0.1 * level.extra.y;
        let layer = 0.003 * f32(raw_red % 30);
        var shadow = noise(vec2(uv.x * 0.5 + drift - layer, 1.0 - uv.y * 0.5 + rain * 0.2 * level.extra.y - layer));
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

        colour = mix(palette(column, group + 3), palette(column, group), shadow);

        if green >= 4 {
            let rb = sin((rain + noise(uv * 2.0) * 4.0 + depth_f / 12.0) * TAU_APPROX) * 0.5 + 0.5;
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
