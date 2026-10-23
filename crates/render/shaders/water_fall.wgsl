// `Futile/WaterFall` (a room `WaterFall`'s quad; no `RIPPLE`, a Watcher keyword). Appended
// to sprite.wgsl after placed_visuals.wgsl (its level, palette and noise helpers). The
// vertex colour is (density, 1 / the fade at the top, 1 / the fade at the bottom); `in.tex`
// is the shader's `i.uv` over the whole quad.
//
// `sincol` is a sine of the `_NoiseTex` noise scrolled with `_RAIN` at the pixel's level
// coordinate. The level is read shifted vertically by up to 0.013 of the image by it; the
// layer's depth `grad` (1 for sky) is mixed 20 % toward `sincol`, stepped in tenths and
// raised to 0.7. The pixel is cut away within the edge distance (10 times the distance
// to a side, or to the top and bottom scaled by the vertex colour) mixed halfway with
// `sincol` under 0.5, and where `sincol` is under `1 - density`. What is left is the
// palette's water colour (texels (5, 7) to (4, 7)) by `grad`.
@fragment
fn fs_water_fall(in: VertexOut) -> @location(0) vec4<f32> {
    let tc = level_coord(in.scr);
    let rain = room.rain;
    let sincol = sin((0.8 * rain + noise_at(vec2(tc.x * 15.0, 1.8 * rain + tc.y * 0.2)) * 3.0) * 3.14 * 2.0) * 0.5 + 0.5;
    let texcol = level_at(tc + vec2(0.0, mix(-0.013, 0.013, sincol)));
    var grad = fmod_trunc(round(texcol.x * 255.0) - 1.0, 30.0) / 30.0;
    if all(texcol.rgb == vec3(1.0)) {
        grad = 1.0;
    }
    grad = pow(floor(mix(grad, sincol, 0.2) * 10.0) / 10.0, 0.7);
    var edge = select((1.0 - in.tex.x) * 10.0, in.tex.x * 10.0, in.tex.x < 0.5);
    edge = min(edge, select((1.0 - in.tex.y) * (1.0 / in.color.z), in.tex.y * (1.0 / in.color.y), in.tex.y < 0.5));
    if mix(edge, sincol, 0.5) < 0.5 {
        return vec4(0.0);
    }
    if sincol < 1.0 - in.color.x {
        return vec4(0.0);
    }
    return mix(palette_at(vec2(5.5 / 32.0, 7.5 / 8.0)), palette_at(vec2(4.5 / 32.0, 7.5 / 8.0)), grad);
}
