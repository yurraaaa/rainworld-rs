// The region map's shaders (StreamingAssets/shaders/Map.shader, MapAerial.shader,
// MapShortcut.shader), appended to stage.wgsl. The map sprite covers the screen; its
// vertex colour carries the layer's out-of-focus amount (r), the depth zoom (g), the
// layer (b: 0, 0.5 or 1) and the alpha.

struct MapView {
    // `_mapSize`: the map texture's width and a third of its height.
    size: vec2<f32>,
    // `_mapPan`: the pan position as a fraction of `_mapSize`.
    pan: vec2<f32>,
    // `_MapCol`.
    color: vec4<f32>,
    // `_MapWaterCol`.
    water_color: vec4<f32>,
};

@group(0) @binding(1) var<uniform> map_view: MapView;
// `_mapFogTexture`: the reveal texture.
@group(2) @binding(4) var map_fog: texture_2d<f32>;
@group(2) @binding(5) var map_fog_sampler: sampler;

// The map texel and its place for this pixel; `aerial` picks MapAerial's lens.
fn map_grab_pos(in: VertexOut, aerial: bool) -> vec2<f32> {
    let dst = clamp(distance(in.scr, vec2<f32>(0.5, 0.5)), 0.0, 1.0);
    var displace: vec2<f32>;
    if (aerial) {
        displace = normalize(in.scr - vec2<f32>(0.5, 0.5)) * (1.0 - dst) * pow(dst, mix(4.0, 6.0, dst)) * 2.0;
    } else {
        displace = normalize(in.scr - vec2<f32>(0.5, 0.5)) * 0.275 * pow(dst, 3.14);
    }
    displace.y *= 0.5 / 3.0;
    displace.x /= map_view.size.x / 500.0;
    displace.y /= map_view.size.y / 500.0;
    var h = 0.5;
    if (in.color.z == 0.0) {
        h = 0.0;
    } else if (in.color.z == 1.0) {
        h = 1.0;
    }
    let zoom = mix(3.25, 4.75, in.color.y);
    var grab = vec2<f32>(
        map_view.pan.x + (in.uv.x - 0.5) * (stage.screen.x / map_view.size.x) / zoom,
        map_view.pan.y / 3.0 + (2.0 * h) / 3.0
            + ((in.uv.y - 0.5) * (stage.screen.y / map_view.size.y) / zoom) / 3.0,
    );
    grab -= displace;
    let low = mix(0.0, 2.0, h) / 3.0;
    let high = mix(1.0, 3.0, h) / 3.0;
    if (grab.y < low) {
        grab.y = low;
    } else if (grab.y > high) {
        grab.y = high;
    }
    return grab;
}

fn map_is_water(grab_col: vec4<f32>, grab: vec2<f32>) -> bool {
    let wave = sin((grab.x - (stage.rain * 75.0 / map_view.size.x)) * map_view.size.x * 0.25);
    return grab_col.z > grab_col.x * mix(0.5, 0.9, wave);
}

// `Futile/Map`: walls white, solid black, water in the water colour, open space in the
// map colour where revealed; unrevealed parts dark.
@fragment
fn fs_map(in: VertexOut) -> @location(0) vec4<f32> {
    let grab = map_grab_pos(in, false);
    let grab_col = textureSample(main_tex, main_sampler, grab);
    let fog = textureSample(map_fog, map_fog_sampler, grab).x;
    let dark = vec4<f32>(0.0, 0.0, 0.0, 0.7 * in.color.w * (1.0 - in.color.x));
    let grey = vec4<f32>(0.25, 0.25, 0.25, 0.4 * in.color.w * (1.0 - in.color.x));
    if (grab_col.y > 0.5 || fog < 0.5) {
        return dark;
    }
    let lght = grab_col.x;
    if (lght >= 0.3 && lght <= 0.7 && grab_col.y == 0.0) {
        return vec4<f32>(1.0, 1.0, 1.0, in.color.w);
    } else if (lght < 0.3) {
        return dark;
    } else if (map_is_water(grab_col, grab)) {
        return vec4<f32>(map_view.water_color.xyz, in.color.w * mix(0.5, 0.1, in.color.x));
    }
    if (fog < 0.54) {
        return vec4<f32>(map_view.color.xyz, in.color.w);
    }
    return grey;
}

// `Futile/MapAerial`: the sky layers, with the open air drawn in the map colour.
@fragment
fn fs_map_aerial(in: VertexOut) -> @location(0) vec4<f32> {
    let grab = map_grab_pos(in, true);
    let grab_col = textureSample(main_tex, main_sampler, grab);
    let fog = textureSample(map_fog, map_fog_sampler, grab).x;
    let dark = vec4<f32>(0.0, 0.0, 0.0, 0.7 * in.color.w * (1.0 - in.color.x));
    let grey = vec4<f32>(0.25, 0.25, 0.25, 0.4 * in.color.w * (1.0 - in.color.x));
    if (grab_col.y > 0.5 || fog < 0.5) {
        return grey;
    }
    let lght = grab_col.x;
    if (lght >= 0.3 && lght <= 0.7 && grab_col.y == 0.0) {
        return vec4<f32>(1.0, 1.0, 1.0, in.color.w);
    } else if (lght < 0.3) {
        if (fog < 0.54) {
            return vec4<f32>(map_view.color.xyz, in.color.w);
        }
        return dark;
    } else if (map_is_water(grab_col, grab)) {
        return vec4<f32>(map_view.water_color.xyz, in.color.w * mix(0.5, 0.1, in.color.x));
    }
    return grey;
}

// `Futile/MapShortcut`: a dashed line whose dashes run along `v` (length factor in g,
// direction in b), white towards the map colour by r.
@fragment
fn fs_map_shortcut(in: VertexOut) -> @location(0) vec4<f32> {
    let lnght = 1.0 / in.color.y;
    let cc = in.uv.y * lnght + stage.rain * 3.0 * (-1.0 + 2.0 * in.color.z);
    if (cc - floor(cc) < 0.25) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(mix(vec3<f32>(1.0, 1.0, 1.0), map_view.color.xyz, in.color.x), in.color.w);
}
