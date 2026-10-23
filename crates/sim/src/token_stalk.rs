//! `CollectToken.TokenStalk` without a token: the dead stalk `Room.Loaded` makes for a
//! `DeadTokenStalk` placed object. An arm on a base, a head on a rope of 3 px segments;
//! its lamp fades out over 120 ticks. The constructor's draws run under `InitState` with
//! the global state restored; every `Update` makes two shared-stream draws (four more
//! when it picks a new pose, about one tick in 300). Sprites: `rw-graphics`
//! `token_stalk.rs` (its lamp flicker draws on the cosmetic stream, D1).
//!
//! Mirrors: CollectToken.cs (TokenStalk), Room.cs (Loaded: DeadTokenStalk), RWCustom/Custom.cs (InverseKinematic, Bezier)

use rw_assets::Room;
use rw_random::UnityRandom;

use crate::float_ext::{MathfSin, SCurve};
use crate::shared_physics::{ExactRayTrace, TerrainCollisionData};
use crate::{FloatExt, Vec2};

const COORD_SEG: f32 = 3.0;

/// One rope segment (`coord[i, 0..3]`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StalkCoord {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub vel: Vec2,
}

/// One of `curveLerps`' rows: an eased value from `from` to `to`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurveLerp {
    pub progress: f32,
    pub last_progress: f32,
    pub from: f32,
    pub to: f32,
    pub speed: f32,
}

impl CurveLerp {
    /// `NewCurveLerp`: only once the last one has finished.
    fn start(&mut self, to: f32, speed: f32) {
        if self.progress < 1.0 || self.last_progress < 1.0 {
            return;
        }
        self.from = self.to;
        self.to = to;
        self.speed = speed / (self.from - self.to).abs();
        self.progress = 0.0;
        self.last_progress = 0.0;
    }

    /// `GetCurveLerp`.
    #[must_use]
    pub fn get(&self, s_curve_k: f32, t: f32) -> f32 {
        let p = self.last_progress.lerp(self.progress, t).s_curve(s_curve_k);
        self.from.lerp(self.to, p)
    }
}

/// A `TokenStalk` with `token == null`.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenStalk {
    /// Its place in the room's update list (`AddObject`).
    pub update_order: u64,
    pub hover_pos: Vec2,
    pub base_pos: Vec2,
    pub main_dir: Vec2,
    pub flip: f32,
    pub arm_pos: Vec2,
    pub last_arm_pos: Vec2,
    pub arm_vel: Vec2,
    pub arm_get_to_pos: Vec2,
    pub head: Vec2,
    pub last_head: Vec2,
    pub head_vel: Vec2,
    pub head_dir: Vec2,
    pub last_head_dir: Vec2,
    pub head_dist: f32,
    pub arm_length: f32,
    pub coord: Vec<StalkCoord>,
    pub coord_length: f32,
    pub curve_lerps: [CurveLerp; 2],
    pub keep_distance: f32,
    pub sin_counter: f32,
    pub last_sin_counter: f32,
    pub lamp_power: f32,
    pub last_lamp_power: f32,
}

impl TokenStalk {
    /// The constructor (`token` null, `blue` false), drawing from
    /// `InitState((int)(hoverPos.x * 10) + (int)(hoverPos.y * 10))`, `rng`'s state
    /// restored.
    #[must_use]
    pub fn new(update_order: u64, hover_pos: Vec2, base_pos: Vec2, rng: &mut UnityRandom) -> Self {
        let saved = rng.state();
        rng.init_state(
            (hover_pos.x * 10.0)
                .to_int_cs()
                .wrapping_add((hover_pos.y * 10.0).to_int_cs()),
        );
        let mut curve_lerps = [CurveLerp {
            progress: 1.0,
            last_progress: 1.0,
            from: 0.0,
            to: 0.0,
            speed: 0.0,
        }; 2];
        curve_lerps[0].to = rng.value() * 360.0;
        curve_lerps[1].to = 10.0_f32.lerp(20.0, rng.value());
        let flip = if rng.value() < 0.5 { -1.0 } else { 1.0 };
        rng.set_state(saved);
        let main_dir = base_pos.direction_to(hover_pos);
        let coord_length = base_pos.distance(hover_pos) * 0.6;
        let arm_length = base_pos.distance(hover_pos) / 2.0;
        let arm_pos = base_pos + main_dir * arm_length;
        let segments = (coord_length / COORD_SEG).to_int_cs().max(0) as usize;
        let head_dist = 15.0;
        let head = hover_pos - main_dir * head_dist;
        Self {
            update_order,
            hover_pos,
            base_pos,
            main_dir,
            flip,
            arm_pos,
            last_arm_pos: arm_pos,
            arm_vel: Vec2::ZERO,
            arm_get_to_pos: arm_pos,
            head,
            last_head: head,
            head_vel: Vec2::ZERO,
            head_dir: Vec2::ZERO,
            last_head_dir: Vec2::ZERO,
            head_dist,
            arm_length,
            coord: vec![
                StalkCoord {
                    pos: arm_pos,
                    last_pos: arm_pos,
                    vel: Vec2::ZERO,
                };
                segments
            ],
            coord_length,
            curve_lerps,
            keep_distance: 0.0,
            sin_counter: 0.0,
            last_sin_counter: 0.0,
            lamp_power: 0.0,
            last_lamp_power: 0.0,
        }
    }

    /// `Update` with no token (`alive` 0): two draws, four more on a new pose.
    pub fn update(&mut self, room: &Room, rng: &mut UnityRandom) {
        self.last_arm_pos = self.arm_pos;
        self.arm_pos += self.arm_vel;
        self.arm_pos = self
            .arm_pos
            .move_towards(self.arm_get_to_pos, (0.8 + self.arm_length / 150.0) / 2.0);
        self.arm_vel *= 0.8;
        self.arm_vel += (self.arm_get_to_pos - self.arm_pos).clamp_magnitude(4.0) / 11.0;
        self.last_head = self.head;
        self.head += self.head_vel;
        self.head_vel *= 0.8;
        self.last_lamp_power = self.lamp_power;
        self.last_sin_counter = self.sin_counter;
        self.sin_counter += rng.value() * self.lamp_power;
        self.lamp_power = (self.lamp_power - 1.0 / 120.0).max(0.0);
        if !self.head.dist_less(self.arm_pos, self.coord_length) {
            let pull = self.arm_pos.direction_to(self.head)
                * ((self.arm_pos.distance(self.head) - self.coord_length) * 0.8);
            self.head_vel -= pull;
            self.head -= pull;
        }
        let up = Vec2::from_degrees(self.curve_lerps[0].get(0.5, 1.0));
        self.head_vel += up.slerp(Vec2::new(0.0, 1.0), 0.4) * 0.4;
        self.last_head_dir = self.head_dir;
        let target = self.hover_pos;
        let pull =
            target.direction_to(self.head) * ((target.distance(self.head) - self.head_dist) * 0.8);
        self.head_vel -= pull;
        self.head -= pull;
        self.head_dir = self.head.direction_to(target);
        // `alive` is 0: one chance in `Lerp(300, 60, 0)`.
        if rng.value() < 1.0 / 300.0 {
            self.new_pose(room, rng);
        }
        self.head_dist = self.curve_lerps[1].get(0.5, 1.0);
        self.head_dist = self.head_dist.lerp(50.0, self.keep_distance.mathf_pow(0.5));
        let elbow = self.elbow(self.arm_pos);
        let arm_dir = elbow.direction_to(self.arm_pos);
        self.move_coords(arm_dir);
        self.connect_coords();
        self.connect_coords();
        for c in &mut self.coord {
            let cd = TerrainCollisionData::new(c.pos, c.last_pos, c.vel, 2.0, true)
                .horizontal(room)
                .vertical(room);
            c.pos = cd.pos;
            c.vel = cd.vel;
        }
        for l in &mut self.curve_lerps {
            l.last_progress = l.progress;
            l.progress = (l.progress + l.speed).min(1.0);
        }
    }

    /// The tick's new pose: the arm's goal (if the tiles to it are clear) and both
    /// curves, `alive` 0.
    fn new_pose(&mut self, room: &Room, rng: &mut UnityRandom) {
        let dir = Vec2::random_unit(rng);
        let goal = self.base_pos
            + self.main_dir * (self.arm_length * 0.7)
            + dir * (rng.value() * self.arm_length * 0.1);
        if room.ray_trace_tiles_for_terrain_exact(self.arm_get_to_pos, goal) {
            self.arm_get_to_pos = goal;
        }
        let angle = self.curve_lerps[0].to + (-180.0_f32).lerp(180.0, rng.value());
        self.curve_lerps[0].start(angle, 1.0);
        let dist = 10.0_f32.lerp(20.0, rng.value().mathf_pow(0.75));
        self.curve_lerps[1].start(dist, 0.4);
    }

    /// `Custom.InverseKinematic(basePos, arm, armLength * 0.65, armLength * 0.35,
    /// flip)`: the elbow.
    #[must_use]
    pub fn elbow(&self, arm: Vec2) -> Vec2 {
        let (a, b) = (self.arm_length * 0.65, self.arm_length * 0.35);
        let d = self.base_pos.distance(arm);
        let cos = ((d * d + a * a - b * b) / (2.0 * d * a)).clamp(0.2, 0.98);
        let bend = (f64::from(cos).acos() as f32) * (self.flip * 180.0 / std::f32::consts::PI);
        self.base_pos + Vec2::from_degrees((arm - self.base_pos).to_degrees() + bend) * a
    }

    /// The rope's pull toward its Bézier from the arm to the head.
    fn move_coords(&mut self, arm_dir: Vec2) {
        let n = self.coord.len();
        let half = self.coord_length * 0.5;
        for i in 0..n {
            let f = (i as f32).inverse_lerp(-1.0, n as f32);
            let goal = self.arm_pos.bezier(
                self.arm_pos + arm_dir * half,
                self.head,
                self.head - self.head_dir * half,
                f,
            );
            let pull = 0.0_f32.lerp(0.25, (f * std::f32::consts::PI).mathf_sin());
            let c = &mut self.coord[i];
            c.last_pos = c.pos;
            c.pos += c.vel;
            c.vel *= 0.8;
            c.vel += (goal - c.pos) * pull;
            c.pos += (goal - c.pos) * pull;
            if i > 2 {
                let d = self.coord[i - 2].pos.direction_to(self.coord[i].pos);
                self.coord[i].vel += d;
                self.coord[i - 2].vel -= d;
            }
            if i > 3 {
                let d = self.coord[i - 3].pos.direction_to(self.coord[i].pos) * 0.5;
                self.coord[i].vel += d;
                self.coord[i - 3].vel -= d;
            }
            if f < 0.5 {
                self.coord[i].vel +=
                    arm_dir * (f.inverse_lerp(0.5, 0.0) * (i as f32).inverse_lerp(5.0, 0.0));
            } else {
                self.coord[i].vel -= self.head_dir * f.inverse_lerp(0.5, 1.0);
            }
        }
    }

    /// `ConnectCoord`: the rope's ends to the arm and the head, segments `coordSeg`
    /// apart at most.
    fn connect_coords(&mut self) {
        let Some(last) = self.coord.len().checked_sub(1) else {
            return;
        };
        let first = &mut self.coord[0];
        let pull =
            self.arm_pos.direction_to(first.pos) * (self.arm_pos.distance(first.pos) - COORD_SEG);
        first.vel -= pull;
        first.pos -= pull;
        for i in 1..self.coord.len() {
            let (a, b) = (self.coord[i - 1].pos, self.coord[i].pos);
            if a.dist_less(b, COORD_SEG) {
                continue;
            }
            let d = b.direction_to(a) * (a.distance(b) - COORD_SEG);
            self.coord[i].vel += d * 0.5;
            self.coord[i].pos += d * 0.5;
            self.coord[i - 1].vel -= d * 0.5;
            self.coord[i - 1].pos -= d * 0.5;
        }
        let end = &mut self.coord[last];
        let pull = self.head.direction_to(end.pos) * (self.head.distance(end.pos) - COORD_SEG);
        end.vel -= pull;
        end.pos -= pull;
    }
}
