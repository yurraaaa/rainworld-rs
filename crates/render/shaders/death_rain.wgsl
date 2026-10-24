// `Futile/DeathRain` (a `RoomRain`'s sprite 0, in `GrabShaders`). Appended to sprite.wgsl
// after ss_grab.wgsl (its bindings, vertex stage and helpers: `noise_at`, `grab_at`). It has a
// `GrabPass {}`; `_MainTex` is the room's `RainMask_` (red where the sky is open above the
// tile), read through `_RainSpriteRect` (the camera's window in room uv). `tex` is the
// shader's `i.uv` over the screen-sized quad. The globals are `room.rain_a` (x
// `_rainDirection`, y `_rainIntensity`, z `_rainEverywhere`) and `room.rain_rect`
// (`_RainSpriteRect`); `smoothRippleClip` is the Watcher's ripple and does nothing in the
// base game (D46).
//
// A hash `rand` of the column (the tap's `_NoiseTex` value shifting the sine) averaged over
// the column and its two neighbours gives streaks; two noise sines `displace`/`displace2`
// move the grab. The mask `fac` is cut by `rand` against the intensity, raised in the open
// when `_rainEverywhere` is up, and faded out below the water line. The result is the grab
// sampled displaced along the streak, mixed toward dark grey when the rain is heavy, then
// slightly darkened or brightened by `lightness`.
@fragment
fn fs_death_rain(in: VertexOut) -> @location(0) vec4<f32> {
    let uv = in.tex;
    let dir = room.rain_a.x;
    let intensity = room.rain_a.y;
    let everywhere = room.rain_a.z;
    let rain = room.rain;
    let get_pos = vec2(uv.x * 2.0 + rain * 0.2, uv.y * 0.25 + rain * 3.0);
    let top_pos = floor((uv.x + dir * 0.05 * uv.y) * 1683.0) / 683.0;
    let step = 1.0 / 683.0;
    // The taps come first, in uniform control flow (the mip level follows the derivatives).
    let n_mid = noise_at(vec2(top_pos, rain));
    let n_up = noise_at(vec2(top_pos + step, rain));
    let n_down = noise_at(vec2(top_pos - step, rain));
    let n_two = noise_at(vec2(uv.x * 1.3, uv.y * 0.25 + rain * 2.0));
    let n_get = noise_at(get_pos);
    let mask = textureSampleLevel(
        rain_mask,
        grab_sampler,
        room.rain_rect.xy + uv * room.rain_rect.zw,
        0.0
    ).x;
    var rand = fract(sin(top_pos * 12.98232 + rain - n_mid) * 43758.5453)
        + fract(sin((top_pos + step) * 12.98232 + rain - n_up) * 43758.5453)
        + fract(sin((top_pos - step) * 12.98232 + rain - n_down) * 43758.5453);
    rand /= 3.0;
    let displace2 = 0.5 + 0.5 * sin((n_two + rain * 5.0) * 6.28);
    var displace = 0.5
        + 0.5 * sin(
            (n_get + rain * 3.0 + displace2) * 6.28 + uv.y * 7.0 + rain * 46.0
                + get_pos.x * 4.2 * dir
        );
    displace = mix(displace, displace2, 0.5);
    var fac = mask;
    var lightness = (-1.0 + mix(displace, rand, 0.5) * 2.0) * fac * pow(intensity, 0.25);
    if 1.0 - rand > intensity {
        fac = 0.0;
    } else {
        fac = max(0.0, fac - (1.0 - intensity));
    }
    if rand > 1.0 - everywhere * 1.5 {
        fac = mix(fac, 1.0, mix(displace / 5.0, 1.0, max(0.0, everywhere - 0.85) * 10.0));
        lightness = (-1.0 + mix(displace, rand, 0.5) * 2.0) * pow(intensity, 0.1);
    }
    let water_line = (1.0 - room.water_level) - 0.14;
    if in.scr.y < water_line {
        fac = mix(fac, 0.0, clamp((water_line - in.scr.y) * 5.0, 0.0, 1.0));
        lightness *= fac;
    }
    displace = mix(displace, rand, 0.8);
    fac *= intensity;
    var col = grab_at(vec2(
        in.scr.x - displace * dir * 0.05 * rand * displace2 * fac,
        in.scr.y + (-0.5 + displace) * 0.25 * fac
    ));
    col = mix(col, vec4(0.15 * rand, 0.15 * rand, 0.15 * rand, 1.0), max(0.0, everywhere - 0.8) * 3.0);
    if lightness < -0.3 {
        col = vec4(col.xyz * 0.9, col.w);
    } else if lightness > 0.5 {
        col = vec4(pow(col.xyz, vec3(0.8)), col.w);
    }
    return col;
}
