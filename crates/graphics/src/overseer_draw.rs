//! `OverseerGraphics.DrawSprites`: the overseer's glow, eye bulb, body meshes, mycelia
//! and lens as a [`DrawList`].
//!
//! Mirrors: OverseerGraphics.cs (DrawSprites)

use std::borrow::Cow;

use rw_random::UnityRandom;
use rw_sim::creature::overseer::{OverseerBody, OverseerColor, OverseerView};
use rw_sim::{FloatExt, MyceliumPool, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, Sprite};
use crate::hologram_matrix::LensFrame;
use crate::math::{Mathf, VecToDeg};
use crate::mycelium::{LongMesh, MyceliumSprites};
use crate::overseer_graphics::{DrawPositions, OverseerGraphics};

/// `segments` of the body meshes.
const SEGMENTS: usize = 10;

/// The sprite indices (`BkgMeshSprite` .. `BulbSprite`, then the mycelia).
const BKG_MESH: usize = 0;
const BKG_BULB: usize = 1;
const GLOW: usize = 2;
const MESH: usize = 3;
const BULB: usize = 4;
const FIRST_MYCELIUM: usize = 5;

/// What one frame of drawing reads besides the module.
struct Frame<'a> {
    body: &'a OverseerBody,
    at: DrawPositions,
    main: Color,
    earth: Color,
    t: f32,
}

impl Frame<'_> {
    /// `ColorOfSegment`: dying, it fades to its clear main colour.
    fn color_of_segment(&self, f: f32) -> Color {
        let living = Self::segment_color(
            self.main,
            self.earth,
            f,
            DrawPositions::extension(self.body, f, self.t),
        );
        living.lerp(self.main.with_alpha(0.0), self.dying())
    }

    /// `Lerp(lastDying, dying, t)`.
    fn dying(&self) -> f32 {
        self.body.last_dying.lerp(self.body.dying, self.t)
    }

    /// `ColorOfSegment` (not dying) at extension `ext` of segment `f`.
    fn segment_color(main: Color, earth: Color, f: f32, ext: f32) -> Color {
        let neutral = (Color::WHITE + earth).scale(0.5);
        let deep = (main + Color::rgb(0.0, 0.0, 1.0) + earth.scale(8.0))
            .scale(0.1)
            .with_alpha(0.5);
        let lit = main.lerp(neutral.lerp(earth, f.mathf_pow(2.0)), 0.5);
        deep.lerp(lit, ext)
    }

    /// `RadOfSegment` with the body mesh's last length.
    fn rad_of_segment(&self, f: f32, total_length: f32) -> f32 {
        let size = self.body.size;
        let k = total_length.lerp_map(20.0 * size, 90.0 * size, 1.75, 0.35);
        let tip = 5.0.lerp(0.0, f.mathf_pow(k));
        (1.0 + (f * std::f32::consts::PI).mathf_sin())
            .lerp(tip, DrawPositions::extension(self.body, f, self.t))
            * (1.0 - self.dying()).mathf_pow(0.2)
    }

    fn segment(&self, f: f32) -> Vec2 {
        self.at.segment(self.body, f, self.t)
    }
}

impl OverseerGraphics {
    /// `OverseerHologram.color` while the overseer has a graphics module:
    /// `Color.Lerp(MainColor, ColorOfSegment(0, 0), 0.5)` (at `timeStacker` 0, so from
    /// `lastExtended`), in the palette's `earth` colour.
    #[must_use]
    pub fn hologram_color(view: &OverseerView<'_>, earth: Color) -> Color {
        Self::hologram_color_of(view.color, view.body.last_extended, earth)
    }

    /// [`OverseerGraphics::hologram_color`] of an overseer of `color` at `last_extended`.
    #[must_use]
    pub fn hologram_color_of(color: OverseerColor, last_extended: f32, earth: Color) -> Color {
        let main = Self::main_color(color);
        let ext = last_extended.max(0.0).mathf_pow(0.1);
        main.lerp(Frame::segment_color(main, earth, 0.0, ext), 0.5)
    }

    /// The eye the overseer's holograms pop out of: `DrawPosOfSegment(0, t)`.
    #[must_use]
    pub fn eye(&self, body: &OverseerBody, t: f32) -> Vec2 {
        DrawPositions::new(self, body, t).segment(body, 0.0, t)
    }

    /// `DrawSprites` at `t` of the way from the last update to this one, in the room
    /// palette's earth colour (`GetPixel(0, 2)`). `rng` is a cosmetic stream for the
    /// lens's per-frame draws (the game draws them from the global stream).
    pub fn draw(
        &mut self,
        view: &OverseerView<'_>,
        earth: Color,
        t: f32,
        pool: &MyceliumPool,
        rng: &mut UnityRandom,
    ) -> DrawList {
        let body = view.body;
        let mut items = Vec::new();
        if !body.in_room {
            return DrawList { items };
        }
        let fr = Frame {
            body,
            at: DrawPositions::new(self, body, t),
            main: Self::main_color(view.color),
            earth,
            t,
        };
        let ext = body.last_extended.lerp(body.extended, t);
        if !self.culled {
            self.body_items(&fr, ext, &mut items);
        }
        if ext > 0.0 {
            let from = fr.segment(self.mycelia_stuck_at);
            let color = fr.color_of_segment(self.mycelia_stuck_at);
            for (l, m) in self
                .mycelia
                .iter()
                .enumerate()
                .filter_map(|(l, &id)| Some((l, pool.get(id)?)))
                .filter(|(_, m)| !m.culled)
            {
                for index in [FIRST_MYCELIUM + l * 2, FIRST_MYCELIUM + l * 2 + 1] {
                    let layer = if index > FIRST_MYCELIUM && index % 2 == 1 {
                        Layer::Items
                    } else {
                        Layer::Foreground
                    };
                    let mesh = m.mesh(index, from, t, color, self.color_mycelia_from, layer);
                    items.push(DrawItem::Mesh(mesh));
                }
            }
        }
        let lens_up = self.holo_lens_up != 0.0 || self.last_holo_lens_up != 0.0;
        if !self.culled && lens_up {
            let fold = 1.0 - ext;
            let lens = self.last_holo_lens_up.lerp(self.holo_lens_up, t);
            let frame = LensFrame {
                use_pos: fr.at.use_pos,
                use_dir: fr.at.use_dir,
                fold,
                hide: fold.max(1.0 - lens),
                color: fr.main.lerp(fr.color_of_segment(0.0), rng.value()),
                t,
            };
            let first = 8 + self.mycelia.len() * 2;
            for s in self.holo.sprites(first, &frame, rng) {
                items.push(DrawItem::Sprite(s));
            }
        }
        items.sort_by_key(DrawItem::index);
        DrawList { items }
    }

    /// The glow, the eye's bulbs and white, the pupil and the body meshes.
    fn body_items(&mut self, fr: &Frame<'_>, ext: f32, items: &mut Vec<DrawItem>) {
        let body = fr.body;
        let dir = fr.at.use_dir;
        let dir2 = dir.xy();
        let mut p = fr.segment(0.0);
        if ext < 1.0 {
            p += p.direction_to(fr.segment(0.001)) * (5.0 * body.size * (1.0 - ext));
        }
        let rad = fr.rad_of_segment(0.0, self.total_length);
        let rotation = dir2.vec_to_deg();
        let white = self.mycelia.len() * 2 + FIRST_MYCELIUM;
        let bulb_scale = Vec2::new(rad / 10.0, rad * 1.2.lerp(1.0, dir.z) / 10.0);
        let eye_scale = Vec2::new(
            rad * 0.7 * ext / 10.0,
            rad * 0.35.lerp(0.7, dir.z) * ext / 10.0,
        );
        let bulb_color = fr.color_of_segment(0.0);
        let pupil = p
            + dir2 * (2.0 * ext)
            + dir2.normalized() * ((dir2.length() * std::f32::consts::PI).mathf_sin() * 2.0);
        let sprites = [
            (
                GLOW,
                "Futile_White",
                p,
                0.0,
                Vec2::new(3.125, 3.125),
                fr.main.with_alpha(0.25 * ext),
                Shader::FlatLight,
                Layer::Foreground,
            ),
            (
                BULB,
                "Circle20",
                p,
                rotation,
                bulb_scale,
                bulb_color,
                Shader::Basic,
                Layer::Foreground,
            ),
            (
                BKG_BULB,
                "Circle20",
                p,
                rotation,
                bulb_scale,
                bulb_color,
                Shader::Basic,
                Layer::Items,
            ),
            (
                white,
                "Circle20",
                p + dir2 * (4.0 * ext),
                rotation,
                eye_scale,
                fr.color_of_segment(0.75)
                    .lerp(Color::rgb(0.0, 0.0, 1.0), 0.5),
                Shader::Basic,
                Layer::Foreground,
            ),
            (
                white + 1,
                "Circle20",
                p + dir2 * (2.5 * ext),
                rotation,
                eye_scale * 0.75,
                Color::WHITE,
                Shader::Basic,
                Layer::Foreground,
            ),
            (
                white + 2,
                "pixel",
                pupil,
                0.0,
                Vec2::new(1.0, 1.0),
                Color::BLACK.with_alpha(0.5),
                Shader::Basic,
                Layer::Foreground,
            ),
        ];
        for (index, element, pos, rotation, scale, color, shader, layer) in sprites {
            items.push(DrawItem::Sprite(Sprite {
                index,
                element,
                pos,
                rotation,
                scale,
                anchor: Vec2::new(0.5, 0.5),
                color,
                shader,
                layer,
            }));
        }
        let (vertices, length) = self.body_vertices(fr);
        self.total_length = length;
        let last = (SEGMENTS * 4 - 1) as f32;
        let vertex_colors: Vec<Color> = (0..SEGMENTS * 4)
            .map(|k| fr.color_of_segment(1.0 - k as f32 / last))
            .collect();
        for (index, layer) in [(MESH, Layer::Foreground), (BKG_MESH, Layer::Items)] {
            items.push(DrawItem::Mesh(Mesh {
                index,
                vertices: vertices.clone(),
                uvs: LongMesh::uvs(SEGMENTS),
                triangles: Cow::Owned(LongMesh::triangles(SEGMENTS)),
                color: Color::WHITE,
                vertex_colors: vertex_colors.clone(),
                shader: Shader::OverseerZip,
                layer,
            }));
        }
    }

    /// The body mesh from the root (its first vertices) to the eye, with its length.
    fn body_vertices(&self, fr: &Frame<'_>) -> (Vec<Vec2>, f32) {
        let mut rad_prev = fr.rad_of_segment(1.0, self.total_length);
        let root = fr.segment(1.0);
        let mut prev = root + fr.segment(0.999).direction_to(root);
        let mut total = 0.0;
        let mut vertices = vec![Vec2::ZERO; SEGMENTS * 4];
        for i in 0..SEGMENTS {
            let f = 1.0 - i as f32 / (SEGMENTS - 1) as f32;
            let pos = fr.segment(f);
            let rad = fr.rad_of_segment(f, self.total_length);
            let n = (prev - pos).normalized();
            let perp = n.perpendicular();
            let dist = prev.distance(pos);
            total += dist;
            let overlap = dist / 5.0;
            let wide = perp * ((rad + rad_prev) * 0.5);
            vertices[i * 4] = prev - n * overlap - wide;
            vertices[i * 4 + 1] = prev - n * overlap + wide;
            vertices[i * 4 + 2] = pos + n * overlap - perp * rad;
            vertices[i * 4 + 3] = pos + n * overlap + perp * rad;
            prev = pos;
            rad_prev = rad;
        }
        (vertices, total)
    }
}
