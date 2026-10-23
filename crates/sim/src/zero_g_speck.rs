//! `GenericZeroGSpeck` and its two subclasses: `BlinkSpeck` (a `ZeroGSpecks` effect's
//! blinking pixel) and `CorruptionSpore` (a `CorruptionSpores` effect's spinning
//! disc). In zero gravity they float around camera 0's window, wrapping at its edges;
//! with gravity they fall as dust and wait to be placed again on a solid tile's open
//! side. The room's gravity is read from [`SpeckCx::gravity`] (`Room.gravity`, which
//! `AntiGravity` sets and the port does not port yet: always 1). Every draw is on the
//! shared stream; sprites are `rw-graphics`' `zero_g_speck.rs`. Watcher's calcified
//! and sentient spores are not ported (D29).
//!
//! Mirrors: GenericZeroGSpeck.cs, BlinkSpeck.cs, CorruptionSpore.cs, Room.cs (NowViewed: ZeroGSpecks, CorruptionSpores)

use rw_assets::Room;
use rw_assets::shortcut::FOUR_DIRECTIONS;
use rw_random::UnityRandom;

use crate::world::AiMap;
use crate::{FloatExt, LightSource, RoomExt, Vec2};

/// The window `GenericZeroGSpeck` wraps in: `1024 x 768` without the Remix screen size.
const SCREEN: Vec2 = Vec2::new(1024.0, 768.0);

/// Which subclass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeckType {
    /// `BlinkSpeck`, from `ZeroGSpecks`.
    Blink,
    /// `CorruptionSpore`, from `CorruptionSpores`.
    CorruptionSpore,
}

impl SpeckType {
    /// How many `Room.NowViewed` makes for an effect of `amount`: the `k` with `k <
    /// 1000 * amount` for specks, `k < 200 * amount` for spores.
    #[must_use]
    pub fn count(self, amount: f32) -> usize {
        let per = match self {
            Self::Blink => 1000.0,
            Self::CorruptionSpore => 200.0,
        };
        (per * amount).ceil().max(0.0) as usize
    }
}

/// What a speck's `Update` reads of its room and camera 0.
#[derive(Debug, Clone, Copy)]
pub struct SpeckCx<'a> {
    pub room: &'a Room,
    /// `Room.gravity`.
    pub gravity: f32,
    /// `readyForAI`; `ai_map` is the room's then.
    pub ready_for_ai: bool,
    pub ai_map: Option<&'a AiMap>,
    /// `cameras[0].pos`.
    pub camera_pos: Vec2,
    /// `cameras[0].currentCameraPosition`.
    pub current_camera: i32,
    /// `Room.BeingViewed`.
    pub being_viewed: bool,
    /// `Room.Darkness(pos)`.
    pub darkness: f32,
    pub lights: &'a [LightSource],
}

/// A `BlinkSpeck`'s own state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BlinkState {
    pub room_darkness: f32,
    pub blink: f32,
    pub last_blink: f32,
    pub col: [f32; 4],
    pub last_col: [f32; 4],
}

/// A `CorruptionSpore`'s own state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SporeState {
    pub scale: f32,
    pub rot: f32,
    pub last_rot: f32,
    pub y_rot: f32,
    pub last_y_rot: f32,
    pub rot_speed: f32,
    pub y_rot_speed: f32,
    pub col: f32,
}

/// The subclass and its state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpeckKind {
    Blink(BlinkState),
    CorruptionSpore(SporeState),
}

/// A `GenericZeroGSpeck`.
#[derive(Debug, Clone, PartialEq)]
pub struct ZeroGSpeck {
    pub update_order: u64,
    pub slated: bool,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub vel: Vec2,
    pub my_float_speed: f32,
    pub saved_cam_pos: i32,
    pub reset: bool,
    pub kind: SpeckKind,
}

impl ZeroGSpeck {
    /// The constructor: at the origin until its first update places it; `ResetMe`'s
    /// draws (2 for a speck, 8 for a spore).
    #[must_use]
    pub fn new(kind: SpeckType, rng: &mut UnityRandom) -> Self {
        let mut speck = Self {
            update_order: 0,
            slated: false,
            pos: Vec2::ZERO,
            last_pos: Vec2::ZERO,
            vel: Vec2::ZERO,
            my_float_speed: 0.0,
            saved_cam_pos: -1,
            reset: false,
            kind: match kind {
                SpeckType::Blink => SpeckKind::Blink(BlinkState::default()),
                SpeckType::CorruptionSpore => SpeckKind::CorruptionSpore(SporeState::default()),
            },
        };
        speck.reset_me(rng);
        speck
    }

    /// `ResetMe`.
    fn reset_me(&mut self, rng: &mut UnityRandom) {
        self.my_float_speed = 0.1_f32.lerp(3.0, rng.value());
        self.vel = Vec2::random_unit(rng) * self.my_float_speed;
        if let SpeckKind::CorruptionSpore(s) = &mut self.kind {
            s.col = rng.value();
            s.scale = rng.value();
            s.rot = rng.value() * 360.0;
            s.last_rot = s.rot;
            s.rot_speed = (-10.0_f32).lerp(10.0, rng.value());
            s.y_rot = rng.value() * std::f32::consts::PI;
            s.last_y_rot = s.y_rot;
            s.y_rot_speed = (-1.0_f32).lerp(1.0, rng.value()) * 0.05;
        }
    }

    /// `Update`: the base class, then the subclass.
    pub fn update(&mut self, cx: &SpeckCx, rng: &mut UnityRandom) {
        self.update_generic(cx, rng);
        match &mut self.kind {
            SpeckKind::Blink(b) => {
                b.last_blink = b.blink;
                if rng.value() < 1.0 / 3.0 {
                    b.blink = rng.value();
                }
                b.room_darkness = cx.darkness;
                if b.room_darkness > 0.0 {
                    b.last_col = b.col;
                    let [r, g, bl] = LightSource::color_over(cx.lights, self.pos);
                    b.col = [r, g, bl, 1.0];
                }
            }
            SpeckKind::CorruptionSpore(s) => {
                s.last_rot = s.rot;
                s.rot += s.rot_speed;
                s.rot_speed =
                    (s.rot_speed + (-1.0_f32).lerp(1.0, rng.value()) / 30.0).clamp(-10.0, 10.0);
                s.last_y_rot = s.y_rot;
                s.y_rot += s.y_rot_speed;
                s.y_rot_speed =
                    (s.y_rot_speed + (-1.0_f32).lerp(1.0, rng.value()) / 320.0).clamp(-0.05, 0.05);
            }
        }
    }

    /// `GenericZeroGSpeck.Update`.
    fn update_generic(&mut self, cx: &SpeckCx, rng: &mut UnityRandom) {
        if self.reset {
            self.try_replace(cx, rng);
            return;
        }
        self.last_pos = self.pos;
        self.pos += self.vel;
        if cx.gravity == 0.0 {
            self.float(cx, rng);
        } else {
            self.vel.y -= cx.gravity * 0.2;
            self.vel += Vec2::from_degrees(180.0 + (-45.0_f32).lerp(45.0, rng.value()))
                * (cx.gravity * 0.1);
            if cx.room.is_solid_at(self.pos) && cx.room.is_solid_at(self.last_pos) {
                self.reset = true;
            }
        }
        self.wrap_in_window(cx);
        if cx.current_camera != self.saved_cam_pos && cx.gravity == 0.0 {
            self.pos = cx.camera_pos + Self::random_in_window(rng);
            self.last_pos = self.pos;
            self.saved_cam_pos = cx.current_camera;
        }
        if !cx.being_viewed {
            self.slated = true;
        }
    }

    /// A random point of the window grown by 200 px (two draws).
    fn random_in_window(rng: &mut UnityRandom) -> Vec2 {
        Vec2::new(
            (-200.0_f32).lerp(SCREEN.x + 200.0, rng.value()),
            (-200.0_f32).lerp(968.0, rng.value()),
        )
    }

    /// A fallen speck, once gravity is 0: 2.5 % a tick it picks a point of the window
    /// and, if that is a solid tile (once `readyForAI`) with open neighbours, starts
    /// again there (a `ResetMe` for each open neighbour).
    fn try_replace(&mut self, cx: &SpeckCx, rng: &mut UnityRandom) {
        if cx.gravity != 0.0 || rng.value() >= 0.025 {
            return;
        }
        let tile = (cx.camera_pos + Self::random_in_window(rng)).tile();
        if !cx.ready_for_ai || !cx.room.is_solid_tile(tile.0, tile.1) {
            return;
        }
        for (dx, dy) in FOUR_DIRECTIONS {
            if !cx.room.is_solid_tile(tile.0 + dx, tile.1 + dy) {
                self.pos = Vec2::middle_of_tile(tile);
                self.last_pos = self.pos;
                self.reset_me(rng);
                self.reset = false;
            }
        }
    }

    /// The zero-gravity drift: pushed off walls within 2 tiles of them, else eased to
    /// its speed, then a random nudge (2 draws).
    fn float(&mut self, cx: &SpeckCx, rng: &mut UnityRandom) {
        match cx.ai_map.filter(|_| cx.ready_for_ai) {
            Some(map) if map.terrain_proximity(self.pos.tile().0, self.pos.tile().1) < 2 => {
                let push = Self::wall_push(map, self.pos.tile()).normalized();
                self.vel += push * 0.2;
                self.vel = self.vel.lerp(push * self.my_float_speed, 0.02);
            }
            _ => {
                self.vel = self
                    .vel
                    .lerp(self.vel.normalized() * self.my_float_speed, 0.01);
            }
        }
        let direction = Vec2::random_unit(rng);
        self.vel += direction * (rng.value() * 0.05);
    }

    /// The four directions weighted by the terrain proximity around each.
    fn wall_push(map: &AiMap, tile: (i32, i32)) -> Vec2 {
        let mut push = Vec2::ZERO;
        for (jx, jy) in FOUR_DIRECTIONS {
            let mut weight = 0.0;
            for (kx, ky) in FOUR_DIRECTIONS {
                weight += map.terrain_proximity(tile.0 + jx + kx, tile.1 + jy + ky) as f32;
            }
            push += Vec2::new(jx as f32, jy as f32) * weight;
        }
        push
    }

    /// Past 200 px outside the window a speck wraps (zero gravity) or falls out (`reset`).
    fn wrap_in_window(&mut self, cx: &SpeckCx) {
        let cam = cx.camera_pos;
        let zero_g = cx.gravity == 0.0;
        if self.pos.x < cam.x - 200.0 {
            if zero_g {
                self.pos.x = cam.x + SCREEN.x + 200.0;
            } else {
                self.reset = true;
            }
        }
        if self.pos.x > cam.x + SCREEN.x + 200.0 {
            if zero_g {
                self.pos.x = cam.x - 200.0;
            } else {
                self.reset = true;
            }
        }
        if self.pos.y < cam.y - 200.0 {
            if zero_g {
                self.pos.y = cam.y + SCREEN.y + 200.0;
            } else {
                self.reset = true;
            }
        }
        if self.pos.y > cam.y + SCREEN.y + 200.0 {
            if zero_g {
                self.pos.y = cam.y - 200.0;
            } else {
                self.reset = true;
            }
        }
    }
}
