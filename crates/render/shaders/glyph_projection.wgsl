// `Futile/GlyphProjection` (a `SuperStructureProjector`'s glyphs; no `rippleClip`, a Watcher
// call). Appended to sprite.wgsl (its `Room` globals, `noise_at`, atlas and sampler). The
// sprite's own shape only bounds the pixels drawn: the shader reads the screen position.
//
// The pixel's place on a 1400 x 800 grid (`floor(scrPos * texSize)`), moved by
// `_gridOffset / _screenSize` and put in the level image's frame (`_spriteRect`), picks a
// 15 px cell (`sampleCoord`, in units of 1 / texSize). The cell's hash `rand` (a sine of
// the cell and a `_NoiseTex` read, stretched by the vertex green) chooses one of the 50
// glyphs in the `glyphs` texture (750 x 15, one glyph a 15 px column); a second hash above
// the vertex alpha drops the cell. A black pixel where the glyph's texel is set (red under
// 0.5 is "unset"); inverted when the vertex red is 0, so a cursor (white) draws the
// glyphs themselves and a single glyph or matrix the field around them.
@fragment
fn fs_glyph_projection(in: VertexOut) -> @location(0) vec4<f32> {
    let tex_size = vec2<f32>(1400.0, 800.0);
    var tc = floor(in.scr * tex_size) / tex_size;
    tc -= room.grid_offset.xy / room.screen;
    let rect = room.sprite_rect;
    tc = (tc - rect.xy) / (rect.zw - rect.xy);
    var cell = tc * tex_size / 15.0;
    cell = floor(cell) / tex_size;
    let rand = fract(
        sin(cell.x * 12.98232 * cell.y * 0.23532 - noise_at(vec2(mix(0.265, 0.9455, cell.y), 0.5)))
            * mix(43758.5453, 23746.232, in.color.y)
    );
    let keep = fract(
        sin(cell.x * 7.62232 * cell.y * 0.19532 - noise_at(vec2(mix(0.235, 0.973, cell.y), 0.4)))
            * 102438.543
    );
    if keep > in.color.w {
        return vec4(0.0);
    }
    let strt = fract(tc * tex_size / 15.0);
    let glyph = floor(rand * 50.0) / 50.0;
    let texel = textureSampleLevel(atlas, atlas_sampler, vec2(glyph + strt.x / 50.0, strt.y), 0.0).x;
    if (texel < 0.5) == (in.color.x == 0.0) {
        return vec4(0.0, 0.0, 0.0, 1.0);
    }
    return vec4(0.0);
}
