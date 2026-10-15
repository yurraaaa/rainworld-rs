//! `Map.OnMapConnection`: a dashed bezier line between two rooms' exits that grows out
//! from the end the reveal reaches first.

use rw_assets::shortcut::FOUR_DIRECTIONS;
use rw_graphics::Vec2Bezier;
use rw_menu::{AtlasManager, NodeId, Sprite, StageColor, StageTree, TriangleStrip};
use rw_render::StageShader;
use rw_sim::{FloatExt, Vec2};

/// What a connection draws with: its two ends on screen and the alphas of their layers.
pub struct ConnectionView {
    pub pos_a: Vec2,
    pub pos_b: Vec2,
    /// `Alpha(LayerOfRoom(room), t, true)` of each end.
    pub alpha_a: f32,
    pub alpha_b: f32,
    pub map_color: StageColor,
}

#[derive(Debug, Clone)]
pub struct MapConnection {
    pub room_a: i32,
    pub room_b: i32,
    /// Tiles.
    pub pos_a: (i32, i32),
    pub pos_b: (i32, i32),
    pub dir_a: usize,
    pub dir_b: usize,
    pub last_reveal_a: f32,
    pub reveal_a: f32,
    pub last_reveal_b: f32,
    pub reveal_b: f32,
    /// Ticks until the end starts revealing; -1 before the reveal reached it.
    pub start_reveal_a: i32,
    pub start_reveal_b: i32,
    pub direction: f32,
    pub segments: usize,
    length_fac: f32,
    line: Option<NodeId>,
    dots: Option<[NodeId; 2]>,
}

impl MapConnection {
    #[must_use]
    pub fn new(rooms: [i32; 2], pos: [(i32, i32); 2], dirs: [i32; 2]) -> Self {
        Self {
            room_a: rooms[0],
            room_b: rooms[1],
            pos_a: pos[0],
            pos_b: pos[1],
            dir_a: dirs[0].rem_euclid(4) as usize,
            dir_b: dirs[1].rem_euclid(4) as usize,
            last_reveal_a: 0.0,
            reveal_a: 0.0,
            last_reveal_b: 0.0,
            reveal_b: 0.0,
            start_reveal_a: -1,
            start_reveal_b: -1,
            direction: 0.0,
            segments: 2,
            length_fac: 1.0,
            line: None,
            dots: None,
        }
    }

    /// The ends in room pixels.
    #[must_use]
    pub fn room_pos_a(&self) -> Vec2 {
        Vec2::new(self.pos_a.0 as f32, self.pos_a.1 as f32) * 20.0
    }

    #[must_use]
    pub fn room_pos_b(&self) -> Vec2 {
        Vec2::new(self.pos_b.0 as f32, self.pos_b.1 as f32) * 20.0
    }

    /// `Update`: each end counts down its start, then reveals over
    /// `Lerp(30, pending reveal pixels, 0.5)` ticks.
    pub fn update(&mut self, pending: usize) {
        let step = 1.0 / 30.0_f32.lerp(pending as f32, 0.5);
        self.last_reveal_a = self.reveal_a;
        if self.start_reveal_a > 0 {
            self.start_reveal_a -= 1;
        } else if self.start_reveal_a == 0 && self.reveal_a < 1.0 {
            self.reveal_a = (self.reveal_a + step).min(1.0);
        }
        self.last_reveal_b = self.reveal_b;
        if self.start_reveal_b > 0 {
            self.start_reveal_b -= 1;
        } else if self.start_reveal_b == 0 && self.reveal_b < 1.0 {
            self.reveal_b = (self.reveal_b + step).min(1.0);
        }
    }

    /// `ResetReveal`'s part.
    pub fn reset(&mut self) {
        self.reveal_a = 0.0;
        self.reveal_b = 0.0;
        self.last_reveal_a = 0.0;
        self.last_reveal_b = 0.0;
        self.start_reveal_a = -1;
        self.start_reveal_b = -1;
    }

    /// `InitiateSprites`: `distance` is between the ends on the map texture; four
    /// segments more between layers.
    pub fn initiate_sprites(
        &mut self,
        stage: &mut StageTree,
        container: NodeId,
        distance: f32,
        across_layers: bool,
    ) {
        self.segments = ((distance / 2.0) as i32).clamp(2, 100) as usize;
        if across_layers {
            self.segments += 4;
        }
        self.length_fac = 2.0 / self.segments as f32;
        let mut strip = TriangleStrip::long_mesh(self.segments, AtlasManager::WHITE);
        strip.shader = StageShader::MapShortcut;
        self.line = Some(stage.add_strip(container, strip));
        let dot = Sprite::new("deerEyeB");
        self.dots = Some([
            stage.add_sprite(container, dot.clone()),
            stage.add_sprite(container, dot),
        ]);
    }

    /// `Map.Draw`'s visibility of the line and dots.
    pub fn set_visible(&self, stage: &mut StageTree, visible: bool) {
        if let (Some(line), Some(dots)) = (self.line, self.dots) {
            for id in [line, dots[0], dots[1]] {
                stage.transform_mut(id).visible = visible;
            }
        }
    }

    /// `DrawSprites`.
    pub fn draw(&self, stage: &mut StageTree, view: &ConnectionView, t: f32) {
        let (Some(line), Some([dot_a, dot_b])) = (self.line, self.dots) else {
            return;
        };
        let ra = self.last_reveal_a.lerp(self.reveal_a, t);
        let rb = self.last_reveal_b.lerp(self.reveal_b, t);
        stage.transform_mut(dot_a).visible = ra > 0.0;
        stage.transform_mut(dot_b).visible = rb > 0.0;
        if ra == 0.0 && rb == 0.0 {
            stage.transform_mut(line).visible = false;
            return;
        }
        stage.transform_mut(line).visible = true;
        let (pos, pos2) = (view.pos_a, view.pos_b);
        let mut dist = pos.distance(pos2);
        if dist > 300.0 {
            dist = dist.lerp(300.0, 0.5);
        }
        let a_alpha = view.alpha_a * ra;
        let b_alpha = view.alpha_b * rb;
        let wa = rb.inverse_lerp(ra, ra + 0.75);
        let wb = ra.inverse_lerp(rb, rb + 0.75);
        let white = StageColor::WHITE;
        for (id, p, w, a) in [(dot_a, pos, wa, a_alpha), (dot_b, pos2, wb, b_alpha)] {
            let s = stage.sprite_mut(id);
            s.transform.x = p.x;
            s.transform.y = p.y;
            s.color = StageColor {
                a,
                ..white.lerp(view.map_color, w)
            };
        }
        let (da, db) = (FOUR_DIRECTIONS[self.dir_a], FOUR_DIRECTIONS[self.dir_b]);
        let dist = if da.0 != -db.0 && da.1 != -db.1 {
            dist / 1.5
        } else {
            dist / 3.0
        };
        let ca = pos - Vec2::new(da.0 as f32, da.1 as f32) * dist;
        let cb = pos2 - Vec2::new(db.0 as f32, db.1 as f32) * dist;
        let strip = stage.strip_mut(line);
        let mut prev = pos;
        let n = self.segments as f32;
        for i in 0..self.segments {
            let f = (i + 1) as f32 / n;
            let t0 = (i as f32 - 0.5) / n;
            let p = pos.bezier(ca, pos2, cb, f);
            let perp = (prev - p).normalized().perpendicular();
            let c0 = StageColor {
                r: wa.lerp(wb, t0),
                g: self.length_fac,
                b: self.direction,
                a: a_alpha.lerp(b_alpha, t0),
            };
            let c1 = StageColor {
                r: wa.lerp(wb, f),
                a: a_alpha.lerp(b_alpha, f),
                ..c0
            };
            strip.colors[i * 4..i * 4 + 4].copy_from_slice(&[c0, c0, c1, c1]);
            let mid = prev.lerp(p, 0.5);
            strip.vertices[i * 4] = (mid - perp).to_array();
            strip.vertices[i * 4 + 1] = (mid + perp).to_array();
            strip.vertices[i * 4 + 2] = (p - perp).to_array();
            strip.vertices[i * 4 + 3] = (p + perp).to_array();
            prev = p;
        }
    }
}
