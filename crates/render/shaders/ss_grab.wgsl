// The superstructure's screen-grab shaders: `Futile/DeepProcessing` (a `DeepProcessing`'s
// quad) and `Futile/GravityDisruptor` (a `GravityDisruptor`'s disc). Appended to sprite.wgsl
// after glyph_projection.wgsl (its bindings, vertex stage and helpers: `level_at`,
// `depth_of`, `level_coord`, `frame_px`, `drawn`, `noise_at`, `center_dist`).
//
// Both read `_GrabTexture`, the texture of the last unnamed `GrabPass {}`: the grab texture
// of `gpu.rs`, which holds the frame before the level until a grabbing draw replaces it.
// `GravityDisruptor` has a `GrabPass {}` of its own (the frame so far, drawn first);
// `DeepProcessing` has none and reads what the last grab left. D37.
//
// Unity picks the `_NoiseTex` mip from the quad's derivatives, which across the grid
// cells' edges of `DeepProcessing` are those of a jump: the taps here are all mip 0 (D37).

// `DeepProcessing`: a depth-windowed screen-space data rain. `textCoord` is the pixel's
// place in the level image; `dp` its layer depth (sky 1). Nothing outside the vertex
// colour's depth window `[clr.x, clr.y]`, nor (past layer 6) where the grab is not black
// (a creature drawn behind the level). The pixel is put on a grid of 10 to 6 px cells
// (finer on far layers) pushed by `dsplace`, a hash `rand` and two noise-driven sines
// `h2` and `h` decide which cells show (the vertex blue, `power`, the share), `h` is faded
// toward the cell's edges, and the result is nothing, dim blue `(0, 0, 0.5, 0.2)` or blue
// `(0, 0, 1, 0.4)`; the vertex alpha (the intensity) shifts the speed and the thresholds.
@fragment
fn fs_deep_processing(in: VertexOut) -> @location(0) vec4<f32> {
    let clr = in.color;
    let text_coord = level_coord(in.scr);
    var dist = 1.0 - center_dist(in.tex);
    let dp = depth_of(level_at(text_coord));
    if dp < clr.x || dp > clr.y {
        return vec4(0.0);
    }
    if dp > 6.0 / 30.0 && drawn(textureLoad(grab_tex, frame_px(in.scr), 0)) {
        return vec4(0.0);
    }
    let dsplace = (vec2(in.scr.x, 1.0 - in.scr.y) - vec2(0.5)) * 0.06 * dp;
    let grid_size = mix(10.0, 6.0, dp);
    var sample_coord = text_coord * room.screen / grid_size;
    sample_coord = floor(sample_coord) / room.screen;
    sample_coord *= grid_size;
    sample_coord += dsplace;
    let rain = room.rain;
    let rand = fract(
        sin(
            sample_coord.x * 12.98232 * sample_coord.y * 0.23532 + rain
                - noise_at(vec2(mix(sample_coord.y, mix(0.265, 0.9455, sample_coord.x), 0.5), rain))
        ) * 43758.5453
    );
    let speed = mix(0.5, 1.5, clr.w);
    let h2 = sin(
        (dp * 1.2 + 0.75 * rain * speed
            + noise_at(vec2(sample_coord.x * 1.2, sample_coord.y * 0.6)) * mix(1.0, 1.0, pow(dist, 0.5)))
            * 3.14 * 2.0
    ) * 0.5 + 0.5;
    var h = sin(
        (dp * mix(0.12, 2.2, h2) + 1.8 * rain * speed
            + noise_at(vec2(sample_coord.x * 8.2, sample_coord.y * 4.2)) * mix(1.0, 4.0, pow(h2, 3.0)))
            * 3.14 * 2.0
    ) * 0.5 + 0.5;
    if mix(h, 0.0, 1.0 - clr.z) < rand * (1.0 - clr.z) {
        return vec4(0.0);
    }
    h *= 1.0 - distance(
            text_coord + dsplace,
            sample_coord + vec2(grid_size * 0.5 / room.screen.x, grid_size * 0.5 / room.screen.y)
        ) * (room.screen.x + room.screen.y) / (grid_size * 2.0);
    dist = pow(dist, mix(1.2, mix(0.199, 0.001, clr.w), h2));
    h += mix(-1.0, 1.0, clr.w) * 0.2;
    if h * dist < 0.33 {
        return vec4(0.0);
    }
    if h * mix(dist, 1.0, 0.5) < 0.66 {
        return vec4(0.0, 0.0, 0.5, 0.2);
    }
    return vec4(0.0, 0.0, 1.0, 0.4);
}

// `GravityDisruptor`: the grabbed frame sampled at the pixel pushed round the disc's
// centre (along the tangent) by `0.05 * effect * sin(dist * 20 - _RAIN * 3.1)` of the
// screen, times the vertex alpha (the power); `effect` is a ring around `dist = 0.75`
// and a rising core inside it. The sample is the middle of a screen pixel. The result is
// the grab's own colour and alpha.
@fragment
fn fs_gravity_disruptor(in: VertexOut) -> @location(0) vec4<f32> {
    let dist = center_dist(in.tex);
    var effect = pow(pow(sin(clamp((dist - 0.5) * 2.0, 0.0, 1.0) * 3.14), 5.0), 0.65);
    if dist < 0.75 {
        effect = max(effect, pow(clamp((dist - 0.2) * 1.8, 0.0, 1.0), 2.0));
    }
    let from_centre = in.tex - vec2(0.5);
    var dir = vec2(0.0);
    if dot(from_centre, from_centre) > 0.0 {
        dir = normalize(from_centre);
    }
    dir = vec2(-dir.y, dir.x);
    var grab_pos = in.scr + dir * 0.05 * effect * sin(dist * 20.0 - room.rain * 3.1) * in.color.w;
    grab_pos = (floor(grab_pos * room.screen) + vec2(0.5)) / room.screen;
    return textureLoad(grab_tex, frame_px(grab_pos), 0);
}
