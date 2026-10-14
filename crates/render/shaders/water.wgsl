// The room's water as the game shades it: the surface band (`WaterSurface` shader)
// and the deep water body (`DeepWater` shader), base-game path (no ripple, gutter,
// brain mold or air pockets). Behaviour: rainworld-re/notes/water-rendering.md
// sections 4-6; the CPU reference in water_sprites/tests.rs mirrors this file.
//
// Vertices are in stage pixels (room pixels minus the camera position). The game's
// screen is `stage.zw` pixels; every screen-space quantity here is in that space,
// whatever size the frame is drawn at. Textures follow level.wgsl: v = 0 is the bottom.

struct Water {
    // xy: scale, zw: offset; clip = room * scale + offset.
    transform: vec4<f32>,
    // xy: camera position (stage origin) in room px, zw: the game's screen size.
    stage: vec4<f32>,
    // xy: room position of the level image's bottom-left, zw: its size.
    image: vec4<f32>,
    // xy: frame size in px, z: `_RAIN`, w: `_waterDepth`.
    frame: vec4<f32>,
};

@group(0) @binding(0) var<uniform> water: Water;
@group(0) @binding(1) var level_tex: texture_2d<f32>;
@group(0) @binding(2) var palette_tex: texture_2d<f32>;
@group(0) @binding(3) var noise_tex: texture_2d<f32>;
@group(0) @binding(4) var noise_sampler: sampler;
// The game's `_GrabTexture`: what was drawn before the level, frame-sized.
@group(0) @binding(5) var behind_tex: texture_2d<f32>;

const GAME_TAU: f32 = 6.28; // the shaders' 3.14 * 2

struct VertexIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) stage: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_main(v: VertexIn) -> VertexOut {
    let room = v.pos + water.stage.xy;
    var out: VertexOut;
    out.position = vec4(room * water.transform.xy + water.transform.zw, 0.0, 1.0);
    out.stage = v.pos;
    out.uv = v.uv;
    out.color = v.color;
    return out;
}

// Remainder with the sign of x (HLSL fmod).
fn fmod_trunc(x: f32, y: f32) -> f32 {
    return x - y * trunc(x / y);
}

fn palette(x: i32, row: i32) -> vec4<f32> {
    return textureLoad(palette_tex, vec2(x, row), 0);
}

fn noise(uv: vec2<f32>) -> f32 {
    return textureSampleLevel(noise_tex, noise_sampler, uv, 0.0).r;
}

// Level texture coordinate of the screen pixel at `stage` (`_spriteRect` mapping:
// the image sits at `image - camera`, shifted by (-0.5, +0.5) px).
fn level_coord(stage: vec2<f32>) -> vec2<f32> {
    let image_stage = water.image.xy - water.stage.xy + vec2(-0.5, 0.5);
    return (floor(stage) - image_stage) / water.image.zw;
}

// `tex2D(_LevelTex, uv)`: point filter, clamp.
fn level_at(uv: vec2<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(level_tex));
    let texel = clamp(vec2<i32>(floor(uv * vec2<f32>(size))), vec2(0), size - 1);
    return textureLoad(level_tex, texel, 0);
}

// `_GrabTexture` at screen uv `scr` (0..1, y up).
fn grab_at(scr: vec2<f32>) -> vec4<f32> {
    let room = scr * water.stage.zw + water.stage.xy;
    let clip = room * water.transform.xy + water.transform.zw;
    let px = vec2((clip.x + 1.0) * 0.5, (1.0 - clip.y) * 0.5) * water.frame.xy;
    let size = vec2<i32>(textureDimensions(behind_tex));
    return textureLoad(behind_tex, clamp(vec2<i32>(floor(px)), vec2(0), size - 1), 0);
}

fn is_sky(c: vec4<f32>) -> bool {
    return all(c.rgb == vec3(1.0));
}

fn byte(x: f32) -> i32 {
    return i32(round(x * 255.0));
}

// The grab is not black: `x > 1/255 || y != 0 || z != 0`.
fn drawn(c: vec4<f32>) -> bool {
    return byte(c.r) > 1 || byte(c.g) != 0 || byte(c.b) != 0;
}

// `TerrainAndLevelDepthUnclamped` with the base game's black sloped-terrain mask
// (terrain depth 60).
fn depth_unclamped(uv: vec2<f32>) -> f32 {
    let c = level_at(uv);
    if is_sky(c) {
        return 60.0;
    }
    return min(fmod_trunc(f32(byte(c.r) - 1), 30.0), 60.0);
}

// `TerrainAndLevelDepth`: the sun flag removed, sky 30.
fn depth_clamped(uv: vec2<f32>) -> f32 {
    let c = level_at(uv);
    var red = byte(c.r);
    if red > 90 {
        red -= 90;
    }
    var depth = fmod_trunc(f32(red - 1), 30.0);
    if is_sky(c) {
        depth = 30.0;
    }
    return min(depth, 60.0);
}

// `Futile/WaterSurface`: the band's vertex colour, hidden where terrain is nearer than
// the band's depth along its width, or where a sprite behind the level shows.
@fragment
fn fs_surface(in: VertexOut) -> @location(0) vec4<f32> {
    let offset = mix(0.02, -0.075, 1.0 - water.frame.w * 31.0);
    let uv = level_coord(in.stage);
    if depth_unclamped(uv) / 30.0 < in.uv.y + offset {
        discard;
    }
    if in.uv.y + offset > 6.0 / 30.0 && drawn(grab_at(in.stage / water.stage.zw)) {
        discard;
    }
    return vec4(in.color.rgb, 1.0);
}

// `Futile/DeepWater`: the level's depth seen through the water as a two-colour ramp,
// distorted, with caustics and the player's light; sprites behind the level as
// silhouettes, in their colours just under the surface.
@fragment
fn fs_deep(in: VertexOut) -> @location(0) vec4<f32> {
    let screen = water.stage.zw;
    let rain = water.frame.z;
    let scr = in.stage / screen;
    let t = level_coord(in.stage);
    let clr = in.color;

    let rb = sin((rain + noise(t * 1.2) * 3.0) * GAME_TAU) * 0.5 + 0.5;
    var dist = vec2(
        mix(-0.002, 0.002, rb) * mix(1.0, 20.0, pow(in.uv.x, 200.0)),
        -0.02 * pow(in.uv.x, 8.0),
    );
    dist = floor(dist * screen) / screen;

    let level = level_at(t + dist);
    var light = clamp(
        length(vec2((scr.x - clr.r) * (screen.x / screen.y), scr.y - clr.g)) * mix(21.0, 8.0, clr.b),
        0.0,
        1.0,
    );
    var grad = fmod_trunc(f32(byte(level.r) - 1), 30.0) / 30.0;
    var grab = grab_at(scr + dist);
    light = clamp(light + pow(1.0 - grad, 3.0), 0.0, 1.0);
    if is_sky(level) {
        grad = 1.0;
    }
    if byte(grab.r) == 0 && byte(grab.g) == 2 && byte(grab.b) == 0 {
        grad = 1.0;
    } else if grad > 6.0 / 30.0 {
        if drawn(grab) {
            if byte(grab.r) == 0 && byte(grab.g) == 1 && byte(grab.b) == 0 {
                grad = 1.0;
                grab = vec4(0.0);
            } else {
                grad = 6.0 / 30.0;
                grab *= mix(palette(5, 7), vec4(1.0), 0.75);
                light = 1.0;
            }
        }
    } else {
        grab = vec4(0.0, 0.0, 0.0, 1.0);
    }

    if depth_clamped(t) < water.frame.w * 31.0 {
        discard;
    }

    grad = pow(grad, clamp(1.0 - pow(in.uv.y, 10.0), 0.5, 1.0)) * in.uv.y;

    var phase = rain * 6.0 + noise(vec2(
        grad / 10.0 + mix(t.x, 0.5, grad / 3.0) * 2.1 + dist.x,
        rain * 0.1 + grad / 5.0 + mix(t.y, 0.5, grad / 3.0) * 2.1 + dist.y,
    )) * 7.0;
    var caustic = sin(phase * GAME_TAU) * 0.5 + 0.5;
    phase = rain * 2.7 + noise(vec2(
        grad / 7.0 + mix(t.x, 0.5, grad / 5.0) * 1.3 + dist.x,
        rain * -0.21 + grad / 8.0 + mix(t.y, 0.5, grad / 6.0) * 1.3 + dist.y,
    )) * 6.33;
    let caustic2 = sin(phase * GAME_TAU) * 0.5 + 0.5;
    caustic = pow(max(caustic, caustic2), 47.0);
    if caustic >= 0.8 {
        grad = min(grad + 0.1 * (1.0 - abs(0.5 - grad) * 2.0) * in.uv.y, 1.0);
    }

    light = pow(light, 3.0);
    grad = mix(grad, pow(grad, mix(0.2, 1.0, light)), clr.b);

    let colour = mix(palette(5, 7), palette(4, 7), grad);
    if drawn(grab) {
        return mix(colour, grab, pow(clamp((in.uv.y - 0.9) * 10.0, 0.0, 1.0), 3.0));
    }
    return colour;
}
