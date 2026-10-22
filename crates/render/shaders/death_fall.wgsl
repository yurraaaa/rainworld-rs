// `Futile/DeathFall` (`DeathFallGraphic`'s sprite in `Bloom`; not `strong`, which is the
// Remix `DeathFallHeavy`). Appended to sprite.wgsl after full_screen.wgsl (its grab
// and level helpers). It has a `GrabPass {}`; `_MainTex` is not read.

// `get_depth01`: `fmod(red - 1, 30) / 30` (HLSL `fmod`: -1/30 for red 0), 1 where red
// is 255 whatever the other channels.
fn depth01(red: i32) -> f32 {
    if red >= 255 {
        return 1.0;
    }
    return fmod_trunc(f32(red - 1), 30.0) / 30.0;
}

// Nothing over a creature drawn behind a level pixel deeper than 6/30. Else the grab
// `c` with lightness `l = max(r, g, b)` becomes `lerp(c * l * 0.2, (1 - c) * 0.1 * a,
// saturate(iLerp(0.2 + 0.1 sin(10 _RAIN), 0, l)))`, at alpha `amount / (1 - 0.1 a) * a`,
// where `amount = saturate(iLerp(0.1, 0.3, depth))^0.3 * smoothstep(1, -0.5, uv.y)`.
@fragment
fn fs_death_fall(in: VertexOut) -> @location(0) vec4<f32> {
    let s = in.scr;
    var dist = depth01(byte(level_at(effect_level_coord(s)).x));
    if dist > 6.0 / 30.0 {
        let g = textureLoad(behind_tex, frame_px(s), 0);
        if g.x > 1.0 / 255.0 || g.y != 0.0 || g.z != 0.0 {
            return vec4(0.0);
        }
    }
    let grab = grab_at(s);
    dist = clamp((dist - 0.1) / 0.2, 0.0, 1.0);
    // `smoothstep(1, -0.5, y)`, its edges reversed: HLSL's formula.
    let t = clamp((in.tex.y - 1.0) / -1.5, 0.0, 1.0);
    let amount = select(pow(dist, 0.3), 0.0, dist <= 0.0) * t * t * (3.0 - 2.0 * t);
    let a = in.color.a;
    let l = max(max(grab.x, grab.y), grab.z);
    let edge = 0.2 + sin(room.rain * 10.0) * 0.1;
    let k = clamp((l - edge) / -edge, 0.0, 1.0);
    let col = mix(grab.xyz * l * 0.2, (1.0 - grab.xyz) * 0.1 * a, k);
    return vec4(col, amount / (1.0 - a * 0.1) * a);
}
