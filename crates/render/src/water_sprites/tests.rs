use super::*;

const EPS: f32 = 1e-4;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < EPS
}

fn close2(a: Vec2, b: [f32; 2]) -> bool {
    close(a.x, b[0]) && close(a.y, b[1])
}

fn close4(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| close(*x, y))
}

/// Distinct test colours: water colours 1/2 and surface colours 1/2, shine white.
fn palette() -> RoomPalette {
    RoomPalette {
        sky_color: [0.9, 0.9, 0.9, 1.0],
        fog_color: [0.5, 0.5, 0.5, 1.0],
        black_color: [0.0, 0.0, 0.0, 1.0],
        water_color1: [0.2, 0.4, 0.6, 1.0],
        water_color2: [0.0, 0.1, 0.2, 1.0],
        water_surface_color1: [0.3, 0.5, 0.7, 1.0],
        water_surface_color2: [0.1, 0.3, 0.5, 1.0],
        water_shine_color: [1.0, 1.0, 1.0, 1.0],
        fog_amount: 0.4,
        darkness: 0.0,
        shortcut_colors: [[0.0; 4]; 3],
        shortcut_symbol: [1.0; 4],
    }
}

/// Ten front points at x = 20i, back points at 20i + 10, all at y = 50.
fn flat_surface() -> WaterSurface {
    let front = (0..10).map(|i| [20.0 * i as f32, 50.0]).collect();
    let back = (0..10).map(|i| [20.0 * i as f32 + 10.0, 50.0]).collect();
    WaterSurface::from_rows(front, back, 50.0)
}

/// A 40x30 screen (depth point (20, 20), 7 points to render) at the room origin.
fn frame<'a>(surface: &'a WaterSurface, palette: &'a RoomPalette) -> WaterFrame<'a> {
    WaterFrame {
        surface,
        first_point: 0,
        cam_pos: [0.0, 0.0],
        screen_size: [40.0, 30.0],
        palette,
        // Shine 0 at x = y: 0.5 + 0.5 sin(-0.25 * 2pi) = 0.
        glitter: 0.25,
        light: None,
        time_stacker: 1.0,
    }
}

#[test]
fn apply_depth_on_vector() {
    let p = Vec2::new(683.0, 512.0);
    // d = -10: (v - p) / 0.975; d = 30: (v - p) / 1.075.
    let near = Vec2::new(683.0 + 97.5, 512.0 - 195.0).apply_depth(p, -10.0);
    assert!(close2(near, [783.0, 312.0]), "{near:?}");
    let far = Vec2::new(683.0 + 107.5, 512.0 - 215.0).apply_depth(p, 30.0);
    assert!(close2(far, [783.0, 312.0]), "{far:?}");
    assert!(close2(p.apply_depth(p, 30.0), [683.0, 512.0]));
}

#[test]
fn points_to_render_covers_the_screen() {
    assert_eq!(WaterSprites::points_to_render(200, 1366.0), 73);
    assert_eq!(WaterSprites::points_to_render(71, 1366.0), 71);
    assert_eq!(WaterSprites::points_to_render(200, 1024.0), 56);
}

#[test]
fn surface_band_front_and_back_rows() {
    let (surface, palette) = (flat_surface(), palette());
    let mut sprites = WaterSprites::new(surface.len(), 40.0);
    assert_eq!(sprites.surface.vertices().len(), 16, "2 * 7 + 2");
    sprites.draw(&frame(&surface, &palette));
    let v = sprites.surface.vertices();
    // Front point i: (20 + (20i - 20) / 0.975, 20 + 30 / 0.975).
    let front_y = 20.0 + 30.0 / 0.975;
    let back_y = 20.0 + 30.0 / 1.075;
    for k in 0..7 {
        let x = 20.0 + (20.0 * k as f32 - 20.0) / 0.975;
        assert!(close2(v[2 * k], [x, front_y]), "front {k}: {:?}", v[2 * k]);
        let mut bx = 20.0 + (20.0 * k as f32 + 10.0 - 20.0) / 1.075;
        if k == 0 {
            bx -= 100.0;
        } else if k == 6 {
            bx += 100.0;
        }
        assert!(
            close2(v[2 * k + 1], [bx, back_y]),
            "back {k}: {:?}",
            v[2 * k + 1]
        );
    }
    // The last pair: front point 7 twice (the base game's `v3`).
    let x7 = 20.0 + 120.0 / 0.975;
    assert!(close2(v[14], [x7, front_y]));
    assert!(close2(v[15], [x7, front_y]));
    // Above the vanishing point (y = 20) the far row sits below the near one.
    assert!(back_y < front_y);
}

#[test]
fn deep_body_skewed_bottom_and_fixed_ends() {
    let (surface, palette) = (flat_surface(), palette());
    let mut sprites = WaterSprites::new(surface.len(), 40.0);
    sprites.draw(&frame(&surface, &palette));
    let v = sprites.deep.vertices();
    let front_y = 20.0 + 30.0 / 0.975;
    for k in 1..7 {
        let x = 20.0 + (20.0 * k as f32 - 20.0) / 0.975;
        assert!(close2(v[2 * k], [x + front_y * 0.3, -10.0]), "bottom {k}");
        assert!(close2(v[2 * k + 1], [x, front_y]), "top {k}");
    }
    assert!(close2(v[0], [-10.0, -10.0]));
    assert!(close2(v[1], [-10.0, 50.0]), "still level - camPos.y");
    assert!(close2(v[14], [1400.0, 50.0]));
    assert!(close2(v[15], [1400.0, -10.0]));
}

#[test]
fn draw_follows_the_camera_and_first_point() {
    let (surface, palette) = (flat_surface(), palette());
    let mut sprites = WaterSprites::new(surface.len(), 40.0);
    let mut f = frame(&surface, &palette);
    f.cam_pos = [40.0, 10.0];
    f.first_point = 2;
    sprites.draw(&f);
    // Vertex 0 is front point 2 at stage (0, 40), projected.
    let v = sprites.surface.vertices()[0];
    assert!(
        close2(v, [20.0 - 20.0 / 0.975, 20.0 + 20.0 / 0.975]),
        "{v:?}"
    );
    // end = min(2 + 7, 9) = 9: only 7 segments still, the last pair is front point 9.
    let last = sprites.surface.vertices()[15];
    assert!(
        close2(last, [20.0 + 120.0 / 0.975, 20.0 + 20.0 / 0.975]),
        "{last:?}"
    );
}

#[test]
fn clamped_end_keeps_old_vertices() {
    // Five points, seven to render: the loop covers four segments; vertices past them
    // stay where they were (zero), as the base game never parks them.
    let front: Vec<[f32; 2]> = (0..5).map(|i| [20.0 * i as f32, 50.0]).collect();
    let back = front.iter().map(|p| [p[0] + 10.0, p[1]]).collect();
    let surface = WaterSurface::from_rows(front, back, 50.0);
    let palette = palette();
    let mut sprites = WaterSprites::new(surface.len(), 40.0);
    assert_eq!(
        sprites.surface.vertices().len(),
        12,
        "pointsToRender clamped to 5"
    );
    sprites.draw(&frame(&surface, &palette));
    let v = sprites.surface.vertices();
    assert!(close2(v[9], [20.0 + 60.0 / 0.975, 20.0 + 30.0 / 0.975]));
    assert_eq!(v[10], Vec2::ZERO);
    assert_eq!(v[11], Vec2::ZERO);
    // Surface colours: the second-to-last is waterSurfaceColor1, the last stays white.
    assert_eq!(
        sprites.surface.vertex_color(10),
        palette.water_surface_color1
    );
    assert_eq!(sprites.surface.vertex_color(11), [1.0; 4]);
}

#[test]
fn triangle_uvs_alternate_with_parity() {
    let (surface, palette) = (flat_surface(), palette());
    let mut sprites = WaterSprites::new(surface.len(), 40.0);
    sprites.draw(&frame(&surface, &palette));
    let mut out = Vec::new();
    let split = sprites.push_triangles(&mut out);
    assert_eq!(split, 14 * 3, "2 * 7 triangles of the band");
    assert_eq!(out.len(), 2 * split);
    let uvs: Vec<f32> = out[..6].iter().map(|v| v.uv[0]).collect();
    assert_eq!(uvs, [0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
    assert!(out.iter().all(|v| v.uv[0] == v.uv[1]));
    // Band: front (even) vertices carry uv 0, back (odd) ones uv 1, except the last
    // triangle's duplicated front point.
    let v = sprites.surface.vertices();
    for t in out[..split - 3].iter() {
        let i = v.iter().position(|p| p.to_array() == t.pos).unwrap();
        assert_eq!(t.uv[0], (i % 2) as f32, "vertex {i}");
    }
    // Body: bottom (even) uv 0, surface (odd) uv 1.
    let body = &out[split..];
    assert_eq!(body[0].pos, [-10.0, -10.0]);
    assert_eq!(body[0].uv[0], 0.0);
    assert_eq!(body[1].uv[0], 1.0);
}

#[test]
fn surface_colors_from_shine_and_slope() {
    let (surface, palette) = (flat_surface(), palette());
    let mut f = frame(&surface, &palette);
    let mut sprites = WaterSprites::new(surface.len(), 40.0);
    // No shine at x = y; flat tangent: front waterSurfaceColor1, back waterSurfaceColor2.
    f.glitter = 0.25 - 30.0 / 800.0;
    assert!(f.shinyness(Vec2::new(20.0, 50.0)) < 1e-12);
    sprites.draw(&f);
    assert!(close4(
        sprites.surface.vertex_color(2),
        palette.water_surface_color1
    ));
    assert!(close4(
        sprites.surface.vertex_color(3),
        palette.water_surface_color2
    ));
    // Full shine at point 1 (x - y = -30): 0.5 + 0.5 sin((-glitter - 30/800) 2pi) = 1.
    f.glitter = -0.25 - 30.0 / 800.0;
    assert!(close(f.shinyness(Vec2::new(20.0, 50.0)), 1.0));
    sprites.draw(&f);
    // s' = 1 * InverseLerp(0.8, 0.93, sin 60) = (0.8660254 - 0.8) / 0.13.
    let t = (0.866_025_4 - 0.8) / 0.13;
    let want = palette
        .water_surface_color1
        .lerp_color(palette.water_shine_color, t);
    assert!(close4(sprites.surface.vertex_color(2), want), "{want:?}");
    // Back: lerp(surface 2, shine, 1 * (1 - fogAmount 0.4)).
    let want = palette
        .water_surface_color2
        .lerp_color(palette.water_shine_color, 0.6);
    assert!(close4(sprites.surface.vertex_color(3), want));
    // A steep slope up to the right (tangent along 60 degrees) gets the full shine.
    let c = f.surface_color(Vec2::new(20.0, 50.0), Vec2::from_degrees(60.0));
    assert!(close4(c, palette.water_shine_color));
}

#[test]
fn body_color_is_the_player_light() {
    let (surface, palette) = (flat_surface(), palette());
    let mut sprites = WaterSprites::new(surface.len(), 40.0);
    let mut f = frame(&surface, &palette);
    sprites.draw(&f);
    assert_eq!(
        sprites.deep.color(),
        [0.0, 0.0, 0.0, 1.0],
        "no player: black"
    );
    f.light = Some(PlayerLight {
        pos: [10.0, 15.0],
        out_of_breath: [0.0; 4],
    });
    sprites.draw(&f);
    assert!(close4(sprites.deep.color(), [0.25, 0.5, 1.0, 1.0]));
    // Short of breath: b = 0.75 + 0.25 sin(2 * phase) at full shortness.
    f.light = Some(PlayerLight {
        pos: [100.0, -5.0],
        out_of_breath: [1.0, 1.0, 0.5, 0.5],
    });
    sprites.draw(&f);
    let b = 0.75 + 0.25 * 1.0_f32.sin();
    assert!(close4(sprites.deep.color(), [1.0, 0.0, b, 1.0]));
    assert!(
        (0..sprites.deep.vertices().len())
            .all(|i| sprites.deep.vertex_color(i) == sprites.deep.color())
    );
}

/// CPU version of `water.wgsl`'s colour functions on one pixel, with a uniform level
/// image, a uniform grab and a constant noise texture (so the distortion, which only
/// moves where they are read, drops out).
struct Pixel {
    /// Level texel (0..255 RGB) everywhere.
    level: [u8; 3],
    /// Grab texel (0..255 RGB, alpha 255) everywhere.
    grab: [u8; 3],
    noise: f32,
    rain: f32,
    /// `_waterDepth * 31`.
    depth: f32,
    palette: RoomPalette,
}

trait Mix {
    fn mix(self, to: Self, t: f32) -> Self;
}

impl Mix for f32 {
    fn mix(self, to: f32, t: f32) -> f32 {
        self + (to - self) * t
    }
}

impl Mix for [f32; 4] {
    fn mix(self, to: Self, t: f32) -> Self {
        std::array::from_fn(|i| self[i].mix(to[i], t))
    }
}

impl Pixel {
    /// The shaders' `3.14 * 2`, not 2π.
    #[allow(clippy::approx_constant, reason = "the game's shader uses 3.14")]
    const GAME_TAU: f32 = 6.28;

    fn sky(&self) -> bool {
        self.level == [255, 255, 255]
    }

    fn fmod(x: f32, y: f32) -> f32 {
        x - y * (x / y).trunc()
    }

    fn drawn(&self) -> bool {
        self.grab[0] > 1 || self.grab[1] != 0 || self.grab[2] != 0
    }

    fn depth_unclamped(&self) -> f32 {
        if self.sky() {
            60.0
        } else {
            Self::fmod(f32::from(self.level[0]) - 1.0, 30.0).min(60.0)
        }
    }

    fn depth_clamped(&self) -> f32 {
        let mut red = i32::from(self.level[0]);
        if red > 90 {
            red -= 90;
        }
        let d = if self.sky() {
            30.0
        } else {
            Self::fmod(red as f32 - 1.0, 30.0)
        };
        d.min(60.0)
    }

    /// `fs_surface`: `None` when transparent.
    fn surface(&self, uv_y: f32, color: [f32; 4]) -> Option<[f32; 4]> {
        let offset = 0.02_f32.mix(-0.075, 1.0 - self.depth);
        if self.depth_unclamped() / 30.0 < uv_y + offset {
            return None;
        }
        if uv_y + offset > 6.0 / 30.0 && self.drawn() {
            return None;
        }
        Some([color[0], color[1], color[2], 1.0])
    }

    /// `fs_deep` at screen uv `scr` (0..1) on a `screen`-sized screen; `None` when
    /// transparent.
    fn deep(&self, uv: f32, scr: [f32; 2], screen: [f32; 2], clr: [f32; 4]) -> Option<[f32; 4]> {
        let rain = self.rain;
        let mut light = (((scr[0] - clr[0]) * screen[0] / screen[1]).hypot(scr[1] - clr[1])
            * 21.0_f32.mix(8.0, clr[2]))
        .clamp(0.0, 1.0);
        let mut grad = Self::fmod(f32::from(self.level[0]) - 1.0, 30.0) / 30.0;
        let mut grab =
            [self.grab[0], self.grab[1], self.grab[2], 255].map(|c| f32::from(c) / 255.0);
        let mut drawn = self.drawn();
        light = (light + (1.0 - grad).powi(3)).clamp(0.0, 1.0);
        if self.sky() {
            grad = 1.0;
        }
        if self.grab == [0, 2, 0] {
            grad = 1.0;
        } else if grad > 6.0 / 30.0 {
            if drawn {
                if self.grab == [0, 1, 0] {
                    grad = 1.0;
                    grab = [0.0; 4];
                    drawn = false;
                } else {
                    grad = 6.0 / 30.0;
                    let tint = self.palette.water_color2.mix([1.0; 4], 0.75);
                    grab = std::array::from_fn(|i| grab[i] * tint[i]);
                    light = 1.0;
                }
            }
        } else {
            drawn = false;
        }
        if self.depth_clamped() < self.depth {
            return None;
        }
        grad = grad.powf((1.0 - uv.powf(10.0)).clamp(0.5, 1.0)) * uv;
        let phase = rain * 6.0 + self.noise * 7.0;
        let c1 = (phase * Self::GAME_TAU).sin() * 0.5 + 0.5;
        let phase = rain * 2.7 + self.noise * 6.33;
        let c2 = (phase * Self::GAME_TAU).sin() * 0.5 + 0.5;
        if c1.max(c2).powf(47.0) >= 0.8 {
            grad = (grad + 0.1 * (1.0 - (0.5 - grad).abs() * 2.0) * uv).min(1.0);
        }
        light = light.powi(3);
        grad = grad.mix(grad.powf(0.2_f32.mix(1.0, light)), clr[2]);
        let p = &self.palette;
        let colour = p.water_color2.mix(p.water_color1, grad);
        if drawn {
            Some(colour.mix(grab, ((uv - 0.9) * 10.0).clamp(0.0, 1.0).powi(3)))
        } else {
            Some(colour)
        }
    }
}

fn pixel(level: [u8; 3]) -> Pixel {
    Pixel {
        level,
        grab: [0, 0, 0],
        noise: 0.0,
        rain: 0.0,
        depth: 0.0,
        palette: palette(),
    }
}

#[test]
fn deep_water_reference_colours() {
    let p = palette();
    let screen = [1366.0, 768.0];
    let none = [0.0; 4];
    let mid = [0.5, 0.5];
    // Depth 10 at the surface: grad = (10/30)^0.5 = 0.57735 between waterColor2 and 1.
    let c = pixel([11, 0, 0]).deep(1.0, mid, screen, none).unwrap();
    assert!(
        close4(c, p.water_color2.mix(p.water_color1, 0.577_350_3)),
        "{c:?}"
    );
    // Half way down: (1/3)^(1 - 0.5^10) * 0.5.
    let g = (1.0_f32 / 3.0).powf(1.0 - 0.5_f32.powi(10)) * 0.5;
    let c = pixel([11, 0, 0]).deep(0.5, mid, screen, none).unwrap();
    assert!(close4(c, p.water_color2.mix(p.water_color1, g)));
    assert!(close(g, 0.166_845));
    // Sky at the surface: waterColor1; the sun flag (+90) does not change the depth.
    let c = pixel([255, 255, 255]).deep(1.0, mid, screen, none).unwrap();
    assert!(close4(c, p.water_color1));
    let c = pixel([101, 0, 0]).deep(1.0, mid, screen, none).unwrap();
    assert!(close4(c, p.water_color2.mix(p.water_color1, 0.577_350_3)));
    // Behind terrain (_waterDepth 1/31): depth-0 pixels stay uncovered, depth 1 not.
    let mut behind = pixel([1, 0, 0]);
    behind.depth = 1.0;
    assert_eq!(behind.deep(1.0, mid, screen, none), None);
    behind.level = [2, 0, 0];
    assert!(behind.deep(1.0, mid, screen, none).is_some());
    // In front: depth 0 is covered, near colour (grad 0).
    let c = pixel([1, 0, 0]).deep(1.0, mid, screen, none).unwrap();
    assert!(close4(c, p.water_color2));
}

#[test]
fn deep_water_reference_sprites_and_light() {
    let p = palette();
    let screen = [1366.0, 768.0];
    let mid = [0.5, 0.5];
    // A sprite behind far level (depth 10) just under the surface (uv 0.95): grad 6/30,
    // 0.2^0.5 * 0.95; the tinted sprite mixed in by ((0.95 - 0.9) * 10)^3 = 0.125.
    let mut px = pixel([11, 0, 0]);
    px.grab = [204, 153, 102];
    let c = px.deep(0.95, mid, screen, [0.0; 4]).unwrap();
    let tint = p.water_color2.mix([1.0; 4], 0.75);
    let sprite = [0.8 * tint[0], 0.6 * tint[1], 0.4 * tint[2], 1.0];
    let water = p.water_color2.mix(p.water_color1, 0.2_f32.sqrt() * 0.95);
    assert!(close4(c, water.mix(sprite, 0.125)), "{c:?}");
    // Deeper the sprite is a flat silhouette.
    let c = px.deep(0.5, mid, screen, [0.0; 4]).unwrap();
    let g = 0.2_f32.powf(1.0 - 0.5_f32.powi(10)) * 0.5;
    assert!(close4(c, p.water_color2.mix(p.water_color1, g)));
    // In front of near level (depth 3) the sprite is ignored.
    px.level = [4, 0, 0];
    let c = px.deep(1.0, mid, screen, [0.0; 4]).unwrap();
    assert!(close4(
        c,
        p.water_color2.mix(p.water_color1, 0.1_f32.sqrt())
    ));
    // Player at the pixel with full light: light = (2/3)^3, cubed = 0.026012;
    // grad = 0.57735^lerp(0.2, 1, 0.026012).
    let c = pixel([11, 0, 0])
        .deep(1.0, mid, screen, [0.5, 0.5, 1.0, 1.0])
        .unwrap();
    let want = 0.577_350_3_f32.powf(0.2 + 0.8 * (8.0_f32 / 27.0).powi(3));
    assert!(close(want, 0.885_776));
    assert!(close4(c, p.water_color2.mix(p.water_color1, want)), "{c:?}");
}

#[test]
fn surface_band_reference() {
    let color = [0.3, 0.5, 0.7, 0.5];
    // In front of terrain (offset -0.075): depth 10 hides the band beyond uv 0.408.
    let px = pixel([11, 0, 0]);
    assert_eq!(px.surface(0.3, color), Some([0.3, 0.5, 0.7, 1.0]));
    assert_eq!(px.surface(0.5, color), None);
    // Sky never hides it.
    assert!(pixel([255, 255, 255]).surface(1.0, color).is_some());
    // Behind terrain (offset +0.02): depth 10 hides it beyond uv 0.313.
    let mut behind = pixel([11, 0, 0]);
    behind.depth = 1.0;
    assert!(behind.surface(0.3, color).is_some());
    assert_eq!(behind.surface(0.32, color), None);
    // A sprite behind the level hides the far band only (uv + offset > 6/30).
    let mut sprite = pixel([255, 255, 255]);
    sprite.grab = [200, 10, 10];
    assert!(sprite.surface(0.27, color).is_some());
    assert_eq!(sprite.surface(0.28, color), None);
}
