// `Futile/BulletRain` (a `BulletDrip`'s streak, in `GrabShaders`). Appended to sprite.wgsl
// after death_rain.wgsl (its bindings, vertex stage and `grab_at`). It has a `GrabPass {}`;
// `tex` is the shader's `i.uv` over the streak (v 0 at its foot, the tip). The grab is read
// shifted by `(1 - uv.y) * 0.05` of the screen and mixed toward white by
// `pow(1 - uv.y, 10) * 0.5`, so the streak is a lens that bends the frame and glints at its
// foot. `rippleClip` is the Watcher's ripple and does nothing in the base game (D46, D50).
@fragment
fn fs_bullet_rain(in: VertexOut) -> @location(0) vec4<f32> {
    let col = grab_at(vec2(in.scr.x, in.scr.y + (1.0 - in.tex.y) * 0.05));
    return mix(col, vec4(1.0), pow(1.0 - in.tex.y, 10.0) * 0.5);
}
