//! `OverseerGraphics`: the overseer's cosmetic simulation (its mycelia, its lens, the
//! look direction, culling) and its sprites. Behaviour: `rainworld-re/notes/overseer.md`.
//!
//! Mirrors: OverseerGraphics.cs

use rw_assets::Room;
use rw_random::UnityRandom;
use rw_sim::creature::EntityId;
use rw_sim::creature::overseer::{OverseerBody, OverseerColor, OverseerMode, OverseerView};
use rw_sim::{
    CameraView, FloatExt, MyceliaConnection, MyceliaCx, MyceliumId, MyceliumPool, OwnerKey,
    PointOwner, SystemId, Vec2,
};

use crate::draw::Color;
use crate::hologram_matrix::HologramMatrix;
use crate::math::{Custom, Mathf, Vec3};

/// An overseer's graphics module.
#[derive(Debug, Clone, PartialEq)]
pub struct OverseerGraphics {
    pub id: EntityId,
    /// The `IOwnMycelia` identity of this module (the pool compares owners by it).
    pub owner: OwnerKey,
    /// Its strands, in the world's pool.
    pub mycelia: Vec<MyceliumId>,
    pub holo: HologramMatrix,
    /// Per movement: now, from, to, progress, speed.
    pub mycelia_movements: [[f32; 5]; 2],
    pub mycelia_con_rad: f32,
    /// `myceliaStuckAt`: where along the body the mycelia attach.
    pub mycelia_stuck_at: f32,
    pub color_mycelia_from: f32,
    pub holo_lens_up: f32,
    pub last_holo_lens_up: f32,
    pub cosmetic_look_at: Vec2,
    pub last_cosmetic_look_at: Vec2,
    pub convo_mode: f32,
    pub last_convo_mode: f32,
    pub culled: bool,
    pub last_culled: bool,
    /// `totalLength`: the body mesh's length when it was last drawn.
    pub total_length: f32,
    /// `UpdateDrawPositions(1)` of the last update.
    pub at: DrawPositions,
}

/// `UpdateDrawPositions`' fields.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DrawPositions {
    pub use_pos: Vec2,
    pub use_look_at: Vec2,
    pub use_dir: Vec3,
    pub use_root_pos: Vec2,
    pub use_root_dir: Vec2,
    pub z_start: f32,
    pub z_end: f32,
}

impl OverseerGraphics {
    /// `MainColor`.
    #[must_use]
    pub fn main_color(color: OverseerColor) -> Color {
        let [r, g, b] = color.rgb();
        Color::rgb(r, g, b)
    }
}

impl DrawPositions {
    /// `UpdateDrawPositions(t)`.
    #[must_use]
    pub fn new(g: &OverseerGraphics, b: &OverseerBody, t: f32) -> Self {
        let chunk = b.chunk();
        let use_pos = chunk.last_pos.lerp(chunk.pos, t);
        let use_look_at = g.last_cosmetic_look_at.lerp(g.cosmetic_look_at, t);
        let toward = FloatExt::inverse_lerp(use_pos.distance(use_look_at), 600.0, 0.0);
        let convo = g.last_convo_mode.lerp(g.convo_mode, t);
        let ext = b.last_extended.lerp(b.extended, t);
        let use_dir = Vec3::from_xy(use_pos.direction_to(use_look_at))
            .slerp(Vec3::FORWARD, toward * (1.0 - convo))
            .scale(ext);
        let num = 0.5 / b.zip_path_count as f32;
        let p = &b.zip_progs;
        Self {
            use_pos,
            use_look_at,
            use_dir,
            use_root_pos: b.last_root_pos.lerp(b.root_pos, t),
            use_root_dir: Vec3::from_xy(b.last_root_dir)
                .slerp(Vec3::from_xy(b.root_dir), t)
                .xy(),
            z_start: (1.0 - num * 3.0).min(p[4].lerp(p[3], t).s_curve(0.5)),
            z_end: num.max(p[1].lerp(p[0], t).s_curve(0.5)),
        }
    }

    /// `DrawPosOfSegment(f, t)`: along the body from the eye (0) to the root (1),
    /// between its zip path and its bezier as it extends.
    #[must_use]
    pub fn segment(&self, b: &OverseerBody, f: f32, t: f32) -> Vec2 {
        let ext = b.last_extended.lerp(b.extended, t);
        let reach = (40.0 * b.size).max(self.use_pos.distance(self.use_root_pos) * 0.85) * ext;
        let bezier = self.use_pos.bezier(
            self.use_pos - self.use_dir.xy() * (15.0 * b.size * ext),
            self.use_root_pos,
            self.use_root_pos + self.use_root_dir * reach,
            f,
        );
        let along = if b.zip_mesh_direction { f } else { 1.0 - f };
        let zip = Self::zip_position(b, self.z_end.lerp(self.z_start, along));
        zip.lerp(bezier, Self::extension(b, f, t).clamp(0.0, 1.0))
    }

    /// `ExtensionOfSegment`.
    #[must_use]
    pub fn extension(b: &OverseerBody, f: f32, t: f32) -> f32 {
        b.last_extended
            .lerp(b.extended, t)
            .max(0.0)
            .mathf_pow(0.1.lerp(4.0, f))
    }

    /// `PositionOfZipProg`.
    fn zip_position(b: &OverseerBody, f: f32) -> Vec2 {
        let f = 1.0 - f;
        let count = i32::try_from(b.zip_path_count).unwrap_or(1);
        let at = f * count as f32 - 1.0;
        let i = (at as i32).clamp(0, count - 1);
        let j = (at as i32 + 1).clamp(0, count - 1);
        let a = Vec2::middle_of_tile(b.zip_path[usize::try_from(i).unwrap_or(0)]);
        let c = Vec2::middle_of_tile(b.zip_path[usize::try_from(j).unwrap_or(0)]);
        a.lerp(c, FloatExt::inverse_lerp(at, i as f32, j as f32))
    }
}

impl OverseerGraphics {
    /// The constructor: from the ID's seed (the global state saved and restored) the
    /// mycelium count, the mycelia (each `Reset` once), the lens's shape and the mycelia
    /// movements; then the lens state and `Reset`.
    pub fn new(view: &OverseerView<'_>, mx: &mut MyceliaCx<'_>, rng: &mut UnityRandom) -> Self {
        let size = view.body.size;
        let saved = rng.state();
        rng.init_state(view.id.random_seed());
        let a = rng.value();
        let b = rng.value().mathf_pow(0.5);
        let inner = 2.0.lerp(6.0, a.lerp(FloatExt::inverse_lerp(size, 0.5, 1.0), b));
        let count = inner.lerp(4.0, rng.value()) as usize;
        let length = size.lerp_map(0.5, 1.0, 55.0, 80.0);
        let start = view.body.chunk().pos;
        let owner = mx.pool.new_owner();
        let mycelia: Vec<MyceliumId> = (0..count)
            .map(|i| {
                let id = mx.pool.add(
                    view.neuron_system,
                    (owner, &PointOwner(start)),
                    i,
                    length,
                    start,
                    rng,
                );
                if let Some(m) = mx.pool.get_mut(id) {
                    m.move_away_from_walls = false;
                    m.use_static_culling = false;
                }
                id
            })
            .collect();
        let holo = HologramMatrix::new(mycelia.len(), size, rng);
        let mut mycelia_movements = [[0.0; 5]; 2];
        for m in &mut mycelia_movements {
            m[0] = rng.value();
            m[3] = 1.0;
        }
        rng.set_state(saved);
        let lens = if view.bring_up_lens < 0.5 { 0.0 } else { 1.0 };
        let con_rad = mycelia
            .first()
            .and_then(|&id| mx.pool.get(id))
            .map_or(0.0, |m| m.con_rad / 2.0);
        let mut g = Self {
            id: view.id,
            owner,
            mycelia_con_rad: con_rad,
            mycelia,
            holo,
            mycelia_movements,
            mycelia_stuck_at: size.lerp_map(0.5, 1.0, 0.5, 0.2),
            color_mycelia_from: size.lerp_map(0.5, 1.0, 0.5, 0.3),
            holo_lens_up: lens,
            last_holo_lens_up: lens,
            cosmetic_look_at: Vec2::ZERO,
            last_cosmetic_look_at: Vec2::ZERO,
            convo_mode: 0.0,
            last_convo_mode: 0.0,
            culled: true,
            last_culled: true,
            total_length: 1.0,
            at: DrawPositions::default(),
        };
        g.reset(view.body, mx.pool, rng);
        g
    }

    /// `Reset`: every mycelium restarts at the eye (`3 + 2 * points` draws each).
    pub fn reset(&mut self, body: &OverseerBody, pool: &mut MyceliumPool, rng: &mut UnityRandom) {
        self.at = DrawPositions::new(self, body, 1.0);
        let eye = self.at.segment(body, 0.0, 1.0);
        for &id in &self.mycelia {
            pool.reset(id, eye, &PointOwner(eye), rng);
        }
    }

    /// `Emerge`: the mycelia restart at the eye as last placed. (The game uses the
    /// positions of the last drawn frame; the port the last update's.)
    pub fn emerge(&mut self, body: &OverseerBody, pool: &mut MyceliumPool, rng: &mut UnityRandom) {
        let eye = self.at.segment(body, 0.0, 1.0);
        for &id in &self.mycelia {
            pool.reset(id, eye, &PointOwner(eye), rng);
        }
    }

    /// `UpdateCoralNeuronSystem` (from `Overseer.NewRoom`): out of the registry of the
    /// system the overseer had (`old`) and, if the strand's own system is not `new`, into
    /// the new one's.
    pub fn new_room(&self, pool: &mut MyceliumPool, old: Option<SystemId>, new: Option<SystemId>) {
        if let Some(old) = old {
            for &id in &self.mycelia {
                pool.unregister(old, id);
            }
        }
        for &id in &self.mycelia {
            pool.move_to_system(id, new);
        }
    }

    /// `ConnectionPos`: where the mycelia attach.
    #[must_use]
    pub fn connection_pos(&self, body: &OverseerBody, t: f32) -> Vec2 {
        self.at.segment(body, self.mycelia_stuck_at, t)
    }

    /// `ShouldBeCulled` for camera 0 showing `room` (`cullRange` 200, widescreen).
    #[must_use]
    pub fn should_be_culled(&self, body: &OverseerBody, room: &Room, camera: &CameraView) -> bool {
        let pos = body.chunk().pos;
        let margin = 200.0 + if self.culled { 0.0 } else { 100.0 };
        if room.cameras().len() > 2 && !camera.position_currently_visible(room, pos, margin, true) {
            let next = if self.culled { 50.0 } else { 100.0 };
            return !camera.position_visible_in_next_screen(room, pos, next, true);
        }
        false
    }

    /// `Update`, after the overseer's. `partner` is the conversation partner's graphics
    /// module, if it has one.
    pub fn update(
        &mut self,
        view: &OverseerView<'_>,
        mx: &mut MyceliaCx<'_>,
        partner: Option<&Self>,
        rng: &mut UnityRandom,
    ) {
        let (room, camera) = (mx.env.room, mx.env.camera);
        let body = view.body;
        self.at = DrawPositions::new(self, body, 1.0);
        self.last_cosmetic_look_at = self.cosmetic_look_at;
        self.cosmetic_look_at = view.cosmetic_look_at;
        self.last_convo_mode = self.convo_mode;
        self.convo_mode = if body.mode == OverseerMode::Conversing {
            (self.convo_mode + 1.0 / 30.0).min(1.0)
        } else {
            (self.convo_mode - 1.0 / 30.0).max(0.0)
        };
        self.update_lens(view);
        // Dying, it looks away from its root (its effects and sparks are the sim's).
        if body.dying > 0.0 {
            let pos = body.chunk().pos;
            self.cosmetic_look_at = pos + body.root_pos.direction_to(pos) * 1000.0;
        }
        // `GraphicsModule.Update`: back in view, it resets; the lens moves only then.
        self.last_culled = self.culled;
        self.culled = self.should_be_culled(body, room, &camera);
        if !self.culled && self.last_culled {
            self.reset(body, mx.pool, rng);
        }
        if !self.culled && (self.holo_lens_up != 0.0 || self.last_holo_lens_up != 0.0) {
            self.holo.update(rng);
        }
        // The zip loop (`Overseer_Zip_LOOP`) is commented out of `sounds.txt`.
        self.update_mycelia_movements(rng);
        let pi = std::f32::consts::PI;
        let ur_x = -self.at.use_dir.y / (pi * 2.0);
        let ur_y = self.at.use_dir.x / (pi * 2.0);
        for l in 0..self.mycelia.len() {
            let base = PointOwner(self.connection_pos(body, 1.0));
            mx.pool.update(self.mycelia[l], &base, &mx.env, rng);
            self.pull_mycelium(l, body, ur_x, ur_y, mx.pool);
            self.link_mycelium(l, body, partner, mx.pool, rng);
        }
    }

    /// `holoLensUp` follows `bringUpLens` with hysteresis, down while conversing, and
    /// snaps while zipping.
    fn update_lens(&mut self, view: &OverseerView<'_>) {
        self.last_holo_lens_up = self.holo_lens_up;
        let lens = view.bring_up_lens;
        let up = (self.holo_lens_up + 1.0 / 30.0).min(1.0);
        let down = (self.holo_lens_up - 1.0 / 30.0).max(0.0);
        self.holo_lens_up = if lens > 0.7 {
            up
        } else if lens < 0.3 || self.holo_lens_up <= 0.5 {
            down
        } else {
            up
        };
        self.holo_lens_up = self
            .holo_lens_up
            .min(1.0 - self.convo_mode)
            .min(1.0 - view.body.dying);
        if view.body.mode == OverseerMode::Zipping {
            self.holo_lens_up = if lens < 0.5 { 0.0 } else { 1.0 };
        }
    }

    /// The two mycelia movements ease toward random targets.
    fn update_mycelia_movements(&mut self, rng: &mut UnityRandom) {
        for (k, m) in self.mycelia_movements.iter_mut().enumerate() {
            m[0] = m[1].lerp(m[2], m[3].s_curve(0.5));
            m[3] = (m[3] + m[4]).min(1.0);
            let chance = 1.0 / if k == 0 { 80.0 } else { 20.0 };
            if rng.value() < chance && m[3] == 1.0 {
                m[1] = m[0];
                m[3] = 0.0;
                if k == 0 {
                    let a = rng.value().mathf_pow(3.0) * 0.8;
                    let sign = if rng.value() < 0.5 { -1.0 } else { 1.0 };
                    m[2] = m[0] + a * sign;
                    m[4] = 0.01 / (m[1] - m[2]).abs();
                } else {
                    m[2] = rng.value();
                    m[4] = 0.02 / (m[1] - m[2]).abs();
                }
            }
        }
    }

    /// The pull of each mycelium point toward its place around the eye.
    fn pull_mycelium(
        &mut self,
        l: usize,
        body: &OverseerBody,
        ur_x: f32,
        ur_y: f32,
        pool: &mut MyceliumPool,
    ) {
        let Some(m) = pool.get_mut(self.mycelia[l]) else {
            return;
        };
        let mm = self.mycelia_movements;
        let ext = body.extended;
        let count = self.mycelia.len() as f32;
        m.con_rad = self.mycelia_con_rad * ext * 1.0.lerp(2.0 - body.size, self.convo_mode);
        let around =
            Vec2::from_degrees(l as f32 / count * 360.0) * (25.0.lerp(15.0, mm[1][0]) * body.size);
        let depth = mm[1][0] * -25.0 * body.size - body.size.lerp_map(0.5, 1.0, 5.0, 0.0);
        let chunk = body.chunk();
        let spot = chunk.pos
            + HologramMatrix::rotate(
                Vec3::new(around.x, around.y, depth).scale(ext),
                ur_x,
                ur_y,
                mm[0][0],
            );
        let out = chunk.pos.direction_to(spot);
        let back = Vec3::from_xy(-self.at.use_dir.xy())
            .slerp(Vec3::from_xy(out), 0.5)
            .xy();
        let base = self.connection_pos(body, 1.0);
        let use_dir = self.at.use_dir.xy();
        let last = (m.points.len() - 1) as f32;
        for (i, p) in m.points.iter_mut().enumerate() {
            let num = i as f32 / last;
            p[2] -= chunk.vel;
            p[2] *= 0.9 * ext;
            p[2] += chunk.vel;
            p[2] += back * (1.0 - num);
            p[2] += (spot - p[0]) * (0.05 * (num * std::f32::consts::PI).mathf_sin());
            p[2] += use_dir * (num.mathf_pow(2.0) * 0.5);
            p[0] = p[0].lerp(base, (1.0 - ext).clamp(0.0, 1.0));
        }
    }

    /// While conversing, an unlinked mycelium may link to a random one of the
    /// partner's; out of the conversation, links break.
    fn link_mycelium(
        &self,
        l: usize,
        body: &OverseerBody,
        partner: Option<&Self>,
        pool: &mut MyceliumPool,
        rng: &mut UnityRandom,
    ) {
        let id = self.mycelia[l];
        let conversing = body.mode == OverseerMode::Conversing;
        let Some(me) = pool.get(id) else {
            return;
        };
        match me.connection {
            None => {
                let Some(p) = partner.filter(|_| conversing && body.conversation_partner.is_some())
                else {
                    return;
                };
                let len = i32::try_from(p.mycelia.len()).unwrap_or(0);
                let k = usize::try_from(rng.range_i32(0, len)).unwrap_or(0);
                let Some(&candidate) = p.mycelia.get(k) else {
                    return;
                };
                let Some(other) = pool.get(candidate) else {
                    return;
                };
                if candidate != id
                    && other.owner != me.owner
                    && other.connection.is_none()
                    && me
                        .base()
                        .dist_less(other.base(), (me.length + other.length) * 0.75)
                {
                    let connection = MyceliaConnection {
                        a: id,
                        b: candidate,
                    };
                    pool.set_connection(id, Some(connection));
                    pool.set_connection(candidate, Some(connection));
                }
            }
            Some(connection) => {
                let other = connection.other(id);
                let linked_back = pool
                    .get(other)
                    .is_some_and(|o| o.connection == Some(connection));
                if !conversing || !linked_back {
                    pool.set_connection(other, None);
                    pool.set_connection(id, None);
                }
            }
        }
    }
}
