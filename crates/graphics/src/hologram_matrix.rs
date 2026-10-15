//! `OverseerGraphics.HologramMatrix`: the overseer's lens, a wireframe of lines in 3D
//! that folds out in front of its eye while `bringUpLens` is high, turning and
//! morphing between two shapes. Behaviour: `rainworld-re/notes/overseer.md`.

use rw_random::UnityRandom;
use rw_sim::{FloatExt, Vec2};

use crate::draw::{Color, Layer, Shader, Sprite};
use crate::math::{Custom, CustomVec, Mathf, Vec3};

/// One lens line: its ends in each of the two shapes, which rotation and which
/// movement morph it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoloLine {
    pub a: Vec3,
    pub b: Vec3,
    pub alt_a: Vec3,
    pub alt_b: Vec3,
    pub rotation: usize,
    pub movement: usize,
}

/// What the lens is drawn from in one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LensFrame {
    pub use_pos: Vec2,
    pub use_dir: Vec3,
    /// `1 - extended`, interpolated.
    pub fold: f32,
    /// `max(fold, 1 - holoLensUp)`, interpolated.
    pub hide: f32,
    pub color: Color,
    pub t: f32,
}

/// The lens.
#[derive(Debug, Clone, PartialEq)]
pub struct HologramMatrix {
    pub lines: Vec<HoloLine>,
    /// Per rotation: now, last, from, to, progress, speed.
    pub rotations: [[f32; 6]; 2],
    /// Per movement: now, last, from, to, progress, speed.
    pub movements: Vec<[f32; 6]>,
}

impl HoloLine {
    fn new(a: Vec3, alt_a: Vec3, b: Vec3, alt_b: Vec3, rotation: usize, movement: usize) -> Self {
        Self {
            a,
            b,
            alt_a,
            alt_b,
            rotation,
            movement,
        }
    }
}

impl Vec3 {
    /// `Custom.DegToVec(deg) * r` with `z` set.
    fn ring(deg: f32, r: f32, z: f32) -> Self {
        let v = Vec2::from_degrees(deg) * r;
        Self::new(v.x, v.y, z)
    }
}

/// The draws of `GenerateShape`, read in the game's order.
struct ShapeRng<'a>(&'a mut UnityRandom);

impl ShapeRng<'_> {
    fn v(&mut self) -> f32 {
        self.0.value()
    }

    /// `Mathf.Lerp(a, b, value)`.
    fn lerp(&mut self, a: f32, b: f32) -> f32 {
        a.lerp(b, self.v())
    }

    /// `Mathf.Lerp(a, b, Pow(value, p))`.
    fn lerp_pow(&mut self, a: f32, b: f32, p: f32) -> f32 {
        a.lerp(b, self.v().mathf_pow(p))
    }

    /// `Pow(value, 3) * (value < 0.85 ? 5 : 10) * (value < 0.85 ? -1 : 1)`.
    fn wobble(&mut self) -> f32 {
        let a = self.v().mathf_pow(3.0);
        let b = if self.v() < 0.85 { 5.0 } else { 10.0 };
        let c = if self.v() < 0.85 { -1.0 } else { 1.0 };
        a * b * c
    }
}

impl HologramMatrix {
    /// The constructor (inside the graphics constructor's seeded block):
    /// `GenerateShape`, then the two rotations.
    pub fn new(mycelia: usize, size: f32, rng: &mut UnityRandom) -> Self {
        let mut m = Self {
            lines: Vec::new(),
            rotations: [[0.0; 6]; 2],
            movements: Vec::new(),
        };
        m.generate_shape(mycelia, size, rng);
        for r in &mut m.rotations {
            r[0] = rng.value();
            r[1] = r[0];
            r[4] = 1.0;
        }
        m
    }

    #[allow(clippy::too_many_lines)]
    fn generate_shape(&mut self, mycelia: usize, size: f32, rng: &mut UnityRandom) {
        let mut r = ShapeRng(rng);
        let mut sides = r.0.range_i32(3, 7);
        if r.v() < 0.5 && mycelia != 4 && mycelia > 2 {
            sides = i32::try_from(mycelia).unwrap_or(sides);
        }
        let num2 = r.lerp_pow(4.0, 8.0, 0.5);
        let mut num3 = r.lerp(12.0, 20.0);
        let num4 = num2.min(r.lerp(3.0, 8.0));
        let z = num3 + r.lerp(5.0, 12.0);
        let num5 = (num2 * 0.5).lerp(4.0, 0.5 + r.v() * 0.5);
        let mut num6 = r.lerp(12.0, 20.0);
        let num7 = (num4 * 0.5).lerp(3.0, 0.5 + r.v() * 0.5);
        let z2 = num6 + r.lerp_pow(10.0, 20.0, 0.5);
        let n = sides as f32;
        for i in 1..=sides {
            let (d6, d7) = (i as f32 * 360.0 / n, (i + 1) as f32 * 360.0 / n);
            self.lines.push(HoloLine::new(
                Vec3::ring(d6, num2, num3),
                Vec3::ring(d6, num5, num6),
                Vec3::ring(d7, num2, num3),
                Vec3::ring(d7, num5, num6),
                0,
                0,
            ));
            self.lines.push(HoloLine::new(
                Vec3::ring(d6, num4, z),
                Vec3::ring(d6, num7, z2),
                Vec3::ring(d7, num4, z),
                Vec3::ring(d7, num7, z2),
                0,
                0,
            ));
            self.lines.push(HoloLine::new(
                Vec3::ring(d6, num2, num3),
                Vec3::ring(d6, num5, num6),
                Vec3::ring(d6, num4, z),
                Vec3::ring(d6, num7, z2),
                0,
                0,
            ));
        }
        let mut spokes = r.0.range_i32(3, 9);
        if sides == 3 {
            spokes = 6;
        }
        if r.v() < 0.5 {
            let hi = r.0.range_i32(3, 9);
            spokes = r.0.range_i32(3, hi);
        }
        if r.v() < 0.5 {
            spokes = sides;
        }
        num3 = r.lerp(7.0, 16.0);
        let num9 = r.lerp(7.0, 25.0);
        let mut num10 = r.lerp_pow(0.2, 0.6, 1.5);
        let num11 = num10 * r.lerp_pow(1.0, 0.25, 1.5);
        let num12 = r.lerp(num2 + 2.0, 11.0);
        let k = (spokes as f32).lerp_map(3.0, 8.0, 1.0, 5.0);
        let mut num13 = r.lerp_pow(num12 + 5.0, 18.0, k);
        let mut kind = r.0.range_i32(0, 5);
        if r.v() < 1.0 / 3.0 {
            kind = 2;
        }
        if kind == 0 {
            num13 = r.lerp(num13, 18.0);
            num10 = r.lerp(num10, 0.6);
        }
        let t = r.lerp_pow(0.2, 0.8, if kind == 3 { 0.5 } else { 1.0 });
        let num15 = num12.lerp(num13, t);
        let z3 = num3.lerp(num9, t) + r.wobble();
        let near = r.lerp(7.0, 16.0);
        num6 = num3.lerp(near, r.v());
        let far = r.lerp(11.0, 20.0) * if r.v() < 0.15 { -1.0 } else { 1.0 };
        let wide = r.lerp(11.0, 35.0);
        let num16 = (num9 + far).lerp(wide, r.v());
        let num17 = r.lerp_pow(0.2, 0.6, 1.5);
        let num18 = num17 * r.lerp_pow(1.0, 0.25, 1.5);
        let num19 = r.lerp(num2, 16.0);
        let num20 = r.lerp_pow(num19 + 5.0, 18.0, k) * num16.abs().lerp_map(0.0, 25.0, 1.5, 0.5);
        let num21 = num19.lerp(num20, t);
        let z4 = num6.lerp(num16, t) + r.wobble();
        let s = spokes as f32;
        for j in 0..spokes {
            let deg = j as f32 / s * 360.0;
            let mv = 2 + usize::try_from(j).unwrap_or(0);
            let half = 180.0 / s;
            let pts: Vec<(Vec3, Vec3)> = match kind {
                0 => vec![
                    (
                        Vec3::ring(deg - half * num10, num12, num3),
                        Vec3::ring(deg - half * num17, num19, num6),
                    ),
                    (
                        Vec3::ring(deg + half * num10, num12, num3),
                        Vec3::ring(deg + half * num17, num19, num6),
                    ),
                    (Vec3::ring(deg, num13, num9), Vec3::ring(deg, num20, num16)),
                ],
                1 => vec![
                    (Vec3::ring(deg, num12, num3), Vec3::ring(deg, num19, num6)),
                    (
                        Vec3::ring(deg - half * num10, num13, num9),
                        Vec3::ring(deg - half * num17, num20, num16),
                    ),
                    (
                        Vec3::ring(deg + half * num10, num13, num9),
                        Vec3::ring(deg + half * num17, num20, num16),
                    ),
                ],
                2 => vec![
                    (
                        Vec3::ring(deg - half * num10, num12, num3),
                        Vec3::ring(deg - half * num17, num19, num6),
                    ),
                    (
                        Vec3::ring(deg - half * num11, num13, num9),
                        Vec3::ring(deg - half * num18, num20, num16),
                    ),
                    (
                        Vec3::ring(deg + half * num11, num13, num9),
                        Vec3::ring(deg + half * num18, num20, num16),
                    ),
                    (
                        Vec3::ring(deg + half * num10, num12, num3),
                        Vec3::ring(deg + half * num17, num19, num6),
                    ),
                ],
                3 => vec![
                    (
                        Vec3::ring(deg - half * num10, num12, num3),
                        Vec3::ring(deg - half * num17, num19, num6),
                    ),
                    (
                        Vec3::ring(deg - half * num11, num15, z3),
                        Vec3::ring(deg - half * num18, num21, z4),
                    ),
                    (Vec3::ring(deg, num13, num9), Vec3::ring(deg, num20, num16)),
                    (
                        Vec3::ring(deg + half * num11, num15, z3),
                        Vec3::ring(deg + half * num18, num21, z4),
                    ),
                    (
                        Vec3::ring(deg + half * num10, num12, num3),
                        Vec3::ring(deg + half * num17, num19, num6),
                    ),
                ],
                _ => vec![
                    (Vec3::ring(deg, num12, num3), Vec3::ring(deg, num19, num6)),
                    (
                        Vec3::ring(deg - half * num10, num15, z3),
                        Vec3::ring(deg - half * num17, num21, z4),
                    ),
                    (
                        Vec3::ring(deg - half * num11, num13, num9),
                        Vec3::ring(deg - half * num18, num20, num16),
                    ),
                    (
                        Vec3::ring(deg + half * num11, num13, num9),
                        Vec3::ring(deg + half * num18, num20, num16),
                    ),
                    (
                        Vec3::ring(deg + half * num10, num15, z3),
                        Vec3::ring(deg + half * num17, num21, z4),
                    ),
                ],
            };
            for p in 0..pts.len() {
                let (a, alt_a) = pts[p];
                let (b, alt_b) = pts[(p + 1) % pts.len()];
                self.lines.push(HoloLine::new(a, alt_a, b, alt_b, 1, mv));
            }
        }
        let scale = 1.0.lerp(size, r.v() * 0.5);
        for line in &mut self.lines {
            line.a = line.a.scale(scale);
            line.alt_a = line.alt_a.scale(scale);
            line.b = line.b.scale(scale);
            line.alt_b = line.alt_b.scale(scale);
        }
        let rows = 3 + usize::try_from(spokes).unwrap_or(0);
        self.movements = (0..rows)
            .map(|_| {
                let v = r.v();
                [v, v, 0.0, 0.0, 1.0, 0.0]
            })
            .collect();
    }

    /// `Update` (only while the lens is up or coming down): the rotations and
    /// movements ease toward random targets.
    pub fn update(&mut self, rng: &mut UnityRandom) {
        for (i, r) in self.rotations.iter_mut().enumerate() {
            r[1] = r[0];
            r[0] = r[2].lerp(r[3], r[4].s_curve(0.5));
            r[4] = (r[4] + r[5]).min(1.0);
            let chance = 1.0 / if i == 0 { 40.0 } else { 70.0 };
            if rng.value() < chance && r[4] == 1.0 {
                r[4] = 0.0;
                r[2] = r[0];
                let a = rng.value().mathf_pow(3.0) * 0.8;
                let sign = if rng.value() < 0.5 { -1.0 } else { 1.0 };
                r[3] = r[0] + a * sign;
                r[5] = 0.02 / (r[2] - r[3]).abs();
            }
        }
        for (j, m) in self.movements.iter_mut().enumerate() {
            m[1] = m[0];
            m[0] = m[2].lerp(m[3], m[4].s_curve(0.5));
            m[4] = (m[4] + m[5]).min(1.0);
            if rng.value() < 0.05 && m[4] == 1.0 {
                m[4] = 0.0;
                m[2] = m[0];
                m[3] = match j {
                    0 => rng.value(),
                    2 => {
                        if rng.value() < 0.5 {
                            0.0
                        } else {
                            1.0
                        }
                    }
                    _ => {
                        if rng.value() < 0.5 {
                            0.0
                        } else {
                            rng.value()
                        }
                    }
                };
                m[5] = 0.02 / (m[2] - m[3]).abs();
            }
        }
        if rng.value() < 0.025 {
            for m in self.movements.iter_mut().skip(1) {
                m[4] = 0.0;
                m[2] = m[0];
                m[3] = 0.0;
                m[5] = 0.02 / (m[2] - m[3]).abs();
            }
        }
    }

    /// `OnPlanePos`: folds the point flat by `fold`, then turns it about z, x and y.
    #[must_use]
    pub fn on_plane_pos(p: Vec3, ur_x: f32, ur_y: f32, ur_z: f32, fold: f32) -> Vec2 {
        let p = Vec3::new(
            p.x * (1.0 - fold),
            p.y * (1.0 - fold),
            p.z * (1.0 + fold * 5.0),
        );
        Self::rotate(p, ur_x, ur_y, ur_z)
    }

    /// The rotation `OnPlanePos` and `MyceliaPosTo2D` share: about z by `ur_z`, x by
    /// `ur_x`, then y by `ur_y` (in turns), projected on the screen.
    #[must_use]
    pub fn rotate(mut p: Vec3, ur_x: f32, ur_y: f32, ur_z: f32) -> Vec2 {
        let pi = std::f32::consts::PI;
        let (sz, cz) = ((ur_z * pi * 2.0).mathf_sin(), (ur_z * pi * 2.0).mathf_cos());
        let x = p.x * cz - p.y * sz;
        p.y = p.x * sz + p.y * cz;
        p.x = x;
        let (sx, cx) = ((ur_x * pi * 2.0).mathf_sin(), (ur_x * pi * 2.0).mathf_cos());
        let y = p.y * cx - p.z * sx;
        p.z = p.y * sx + p.z * cx;
        p.y = y;
        let (sy, cy) = ((ur_y * pi * 2.0).mathf_sin(), (ur_y * pi * 2.0).mathf_cos());
        p.x = p.z * sy + p.x * cy;
        Vec2::new(p.x, p.y)
    }

    /// `DrawSprites`: a `pixel` line per lens line from its folded, turned ends, some
    /// hidden or snapped to the eye by chance. `rng` is a cosmetic stream (the game
    /// draws from the global one each rendered frame).
    pub fn sprites(&self, first: usize, f: &LensFrame, rng: &mut UnityRandom) -> Vec<Sprite> {
        let tau = std::f32::consts::PI * 2.0;
        let pos = f.use_pos + f.use_dir.xy() * (f.fold * 30.0);
        let ur_x = -f.use_dir.y / tau;
        let ur_y = f.use_dir.x / tau;
        let mut out = Vec::new();
        for (j, line) in self.lines.iter().enumerate() {
            if rng.value() < f.hide {
                continue;
            }
            let m = &self.movements[line.movement];
            let mut morph = m[1].lerp(m[0], f.t);
            if line.movement > 0 {
                let m1 = self.movements[1][1].lerp(self.movements[1][0], f.t);
                let m2 = self.movements[2][1].lerp(self.movements[2][0], f.t);
                morph = m1.lerp(morph, m2);
            }
            let r = &self.rotations[line.rotation];
            let ur_z = r[1].lerp(r[0], f.t);
            let mut a =
                pos + Self::on_plane_pos(line.a.slerp(line.alt_a, morph), ur_x, ur_y, ur_z, f.fold);
            let mut b =
                pos + Self::on_plane_pos(line.b.slerp(line.alt_b, morph), ur_x, ur_y, ur_z, f.fold);
            if rng.value() < 0.5 {
                if rng.value() < f.hide {
                    a = pos;
                }
                if rng.value() < f.hide {
                    b = pos;
                }
            }
            out.push(Sprite {
                index: first + j,
                element: "pixel",
                pos: a,
                rotation: a.aim_to(b),
                scale: Vec2::new(1.0, a.distance(b)),
                anchor: Vec2::new(0.5, 0.0),
                color: f.color.with_alpha(f.color.a * (1.0 - f.hide)),
                shader: Shader::Hologram,
                layer: Layer::Foreground,
            });
        }
        out
    }
}
