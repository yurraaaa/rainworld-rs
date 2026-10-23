//! The player in a room with gravity 0: the `ZeroG` body mode, the `ZeroGSwim` and
//! `ZeroGPoleGrab` animations, and the entry and exit checks of `MovementUpdate`.
//!
//! Behaviour: `rainworld-re/surveys/zero-g-player.md` (Player.cs 8541-8815 and 12257,
//! 12327). The keyboard's `analogueDir` is the direction keys clamped to length 1, so
//! `ZeroGGamePadIntVec` is `x, y` and `SwimDir(false)` is `SwimDir(true)` (D34); the coral
//! circuit's resistance and jump meter are 0 without circuits.
//!
//! Mirrors: Player.cs (MovementUpdate, UpdateAnimation, UpdateBodyMode: ZeroG)

use rw_assets::Room;
use rw_random::UnityRandom;

use super::{Animation, BodyMode, HEAD, HIPS, Player};
use crate::float_ext::FloatExt;
use crate::room_ext::RoomExt;
use crate::{SoundId, Sounds, Vec2};

/// `Custom.eightDirectionsAndZero`.
const EIGHT_DIRECTIONS_AND_ZERO: [(i32, i32); 9] = [
    (0, 0),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
];

/// `Custom.eightDirectionsDiagonalsLast`.
const EIGHT_DIRECTIONS_DIAGONALS_LAST: [(i32, i32); 8] = [
    (-1, 0),
    (0, -1),
    (1, 0),
    (0, 1),
    (-1, -1),
    (1, -1),
    (1, 1),
    (-1, 1),
];

impl Player {
    /// `input[i].ZeroGGamePadIntVec`.
    pub(super) fn zero_g_int_vec(&self, i: usize) -> (i32, i32) {
        (i32::from(self.input[i].x), i32::from(self.input[i].y))
    }

    /// `room.GetTile(pos)` has a horizontal or vertical beam (`AnyBeam`).
    fn beam_at(room: &Room, pos: Vec2) -> bool {
        room.tile_at_pos(pos).any_pole()
    }

    /// `room.readyForAI && room.aimap.getAItile(pos).narrowSpace`.
    fn narrow_space_at(&self, pos: Vec2) -> bool {
        let (x, y) = pos.tile();
        self.ai_map.as_ref().is_some_and(|m| m.narrow_space(x, y))
    }

    /// `MovementUpdate`: gravity came back while on a zero-gravity pole, so the player
    /// hangs from or climbs the pole.
    pub(super) fn leave_zero_g_pole(&mut self, room: &Room) {
        if self.effective_room_gravity() > 0.0 && self.animation == Animation::ZeroGPoleGrab {
            self.body_mode = BodyMode::ClimbingOnBeam;
            self.animation = if room
                .tile_at_pos(self.body.chunks[HEAD].pos)
                .horizontal_pole()
            {
                Animation::HangFromBeam
            } else {
                Animation::ClimbOnBeam
            };
        }
    }

    /// `MovementUpdate`, after the water switch: at gravity 0 (and not in a corridor)
    /// the body mode is `ZeroG`, and the animation a pole grab on a beam tile, else a
    /// swim.
    pub(super) fn enter_zero_g(&mut self, room: &Room) {
        if self.effective_room_gravity() == 0.0
            && self.body_mode != BodyMode::CorridorClimb
            && self.animation != Animation::VineGrab
        {
            self.body_mode = BodyMode::ZeroG;
            if !matches!(
                self.animation,
                Animation::ZeroGSwim | Animation::ZeroGPoleGrab
            ) {
                self.animation = if Self::beam_at(room, self.body.chunks[HEAD].pos) {
                    Animation::ZeroGPoleGrab
                } else {
                    Animation::ZeroGSwim
                };
            }
        }
    }

    /// `UpdateBodyMode`'s `ZeroG` branch: drawn toward a one-tile opening the input
    /// points at, floors are no obstacle, and gravity ends the mode.
    pub(super) fn zero_g_mode(&mut self, room: &Room) {
        self.gravitate_to_opening(room);
        self.body.set_go_through_floors(true);
        if self.effective_room_gravity() > 0.0 {
            self.body_mode = BodyMode::Default;
            self.animation = Animation::None;
        }
    }

    /// `AnimationIndex.ZeroGSwim`: thrust along the held direction (one draw), a push off
    /// a solid neighbour on a jump press, and a grab of a beam the body passes.
    pub(super) fn zero_g_swim(&mut self, room: &Room, rng: &mut UnityRandom, sounds: &mut Sounds) {
        self.dynamic_run_speed = [0.0, 0.0];
        self.body_mode = BodyMode::ZeroG;
        self.standing = false;
        for i in 0..2 {
            let c = &self.body.chunks[i];
            let touching = c.contact().x != 0 || c.contact().y != 0;
            let changed = c.last_contact() != c.contact();
            if touching {
                self.can_jump = 12;
                if changed {
                    self.blink(5);
                    self.play_sound(SoundId::SlugcatRegainFooting, HEAD, 1.0, 1.0);
                }
            }
        }
        let main = self.body.chunks[HEAD].pos;
        let mut braced = self.can_jump > 0 || Self::beam_at(room, main);
        for (dx, dy) in EIGHT_DIRECTIONS_AND_ZERO {
            if braced {
                break;
            }
            braced = room.is_solid_at(main + Vec2::new(dx as f32, dy as f32) * 10.0);
        }
        self.swim_cycle += 4.0
            / self.body.chunks[HEAD]
                .vel
                .length()
                .lerp_map(0.0, 2.0, 120.0, 60.0);
        let (zx, zy) = self.zero_g_int_vec(0);
        if zx != 0 || zy != 0 {
            // The sounds queued so far draw first.
            self.flush_sounds(rng, sounds);
            self.swim_cycle += 1.0 / 2.0_f32.lerp(6.0, rng.value());
            let dir = self.swim_dir();
            self.zero_g_thrust(room, dir, braced);
            if self.can_jump > 0 && self.want_to_jump > 0 {
                self.zero_g_push_off(room, dir);
            } else {
                self.zero_g_grab_beam(room);
            }
        }
        if self.swim_cycle > 4.0 {
            self.swim_cycle = 0.0;
        }
    }

    /// The thrust of `ZeroGSwim`: the main chunk accelerates along `dir` (0.2 braced, else
    /// by how far apart the chunks move), the hips are pushed back, and a beam under the
    /// main chunk slows it first.
    fn zero_g_thrust(&mut self, room: &Room, dir: Vec2, braced: bool) {
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        if braced {
            head.vel += dir * 0.2;
        } else {
            let apart = head.vel.distance(hips.vel);
            let fast = (head.vel + hips.vel).length().lerp_map(4.0, 8.0, 0.15, 0.1);
            head.vel += dir * apart.lerp_map(1.0, 4.0, 0.1, fast);
        }
        hips.vel -= dir * 0.1;
        // `Custom.fourDirectionsAndZero[0]` is (0, 0): the main chunk's own tile.
        if room.tile_at_pos(head.pos).any_pole() {
            head.vel *= 0.8;
            head.vel += dir * 0.2;
        }
    }

    /// A jump press off a brace: the solid neighbour (chunk 1's tiles, then chunk 0's)
    /// nearest to the held direction pushes the body away from it.
    fn zero_g_push_off(&mut self, room: &Room, dir: Vec2) {
        let mut wall = Vec2::ZERO;
        for i in [HIPS, HEAD] {
            if wall.x != 0.0 || wall.y != 0.0 {
                break;
            }
            let (tx, ty) = self.body.chunks[i].pos.tile();
            for (dx, dy) in EIGHT_DIRECTIONS_DIAGONALS_LAST {
                let d = Vec2::new(dx as f32, dy as f32);
                if room.tile(tx - dx, ty - dy).is_solid()
                    && ((wall.x == 0.0 && wall.y == 0.0) || dir.distance(d) < wall.distance(dir))
                {
                    wall = d;
                }
            }
        }
        if wall.x == 0.0 && wall.y == 0.0 {
            return;
        }
        let wall = wall.normalized();
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        let away = dir
            .lerp(wall, 0.5)
            .lerp(hips.pos.direction_to(head.pos), 0.25);
        head.vel = (head.vel + away * 5.4).clamp_magnitude(5.4);
        hips.vel = (hips.vel + away * 5.0).clamp_magnitude(5.0);
        head.vel += wall;
        self.play_sound(SoundId::SlugcatNormalJump, HEAD, 1.0, 1.0);
        self.can_jump = 0;
        self.want_to_jump = 0;
    }

    /// A held `axis` along the beam, or the other held against the motion across it.
    fn passes_beam(axis: i32, other: i32, vel: f32) -> bool {
        axis != 0 || (other != 0 && (other as f32).unity_sign() != vel.unity_sign())
    }

    /// Thrusting through a beam tile the body has just come to grabs it
    /// (`ZeroGPoleGrab`), horizontal beams first.
    fn zero_g_grab_beam(&mut self, room: &Room) {
        let (zx, zy) = self.zero_g_int_vec(0);
        let (px, py) = self.zero_g_int_vec(1);
        let head = &self.body.chunks[HEAD];
        let here = room.tile_at_pos(head.pos);
        let before = room.tile_at_pos(head.last_pos);
        let horizontal = Self::passes_beam(zx, zy, head.vel.y)
            && here.horizontal_pole()
            && (px == 0 || !before.horizontal_pole());
        let vertical = Self::passes_beam(zy, zx, head.vel.x)
            && here.vertical_pole()
            && (py == 0 || !before.vertical_pole());
        if horizontal || vertical {
            self.play_sound(SoundId::SlugcatGrabBeam, HEAD, 1.0, 1.0);
            self.animation = Animation::ZeroGPoleGrab;
            self.standing = !horizontal;
        }
    }

    /// `AnimationIndex.ZeroGPoleGrab`: holding a beam, pulled to its line and along it by
    /// the held direction; a jump press lets go (with a push if a direction is held).
    pub(super) fn zero_g_pole_grab(&mut self, room: &Room) {
        self.dynamic_run_speed = [0.0, 0.0];
        self.body_mode = BodyMode::ZeroG;
        let head = &mut self.body.chunks[HEAD];
        head.vel *= head.vel.length().lerp_map(2.0, 5.0, 0.7, 0.3);
        let (zx, zy) = self.zero_g_int_vec(0);
        if zx != 0 {
            self.zero_g_pole_grab_dir.0 = zx;
        }
        if zy != 0 {
            self.zero_g_pole_grab_dir.1 = zy;
        }
        let main = self.body.chunks[HEAD].pos;
        let tile = room.tile_at_pos(main);
        if !tile.horizontal_pole() && !tile.vertical_pole() {
            self.standing = false;
            self.animation = Animation::ZeroGSwim;
            return;
        } else if !tile.horizontal_pole() && tile.vertical_pole() {
            self.standing = true;
        } else if (tile.horizontal_pole() && !tile.vertical_pole()) || (zx != 0 && zy == 0) {
            // A horizontal beam only, or both beams with only x held.
            self.standing = false;
        } else if zx == 0 && zy != 0 {
            self.standing = true;
        }
        let blocked = self.zero_g_pole_along(room, (zx, zy));
        if self.input[0].jump && !self.input[1].jump {
            self.zero_g_pole_let_go(room, (zx, zy), blocked);
        }
        if self.narrow_space_at(self.body.chunks[HEAD].pos)
            && self.narrow_space_at(
                self.body.chunks[HEAD].pos + Vec2::new(zx as f32 * 20.0, zy as f32 * 20.0),
            )
        {
            self.body_mode = BodyMode::CorridorClimb;
            self.animation = Animation::None;
        }
    }

    /// The pull along the beam's line and the climb of `ZeroGPoleGrab`; whether the
    /// held direction left the beam (`flag6`).
    fn zero_g_pole_along(&mut self, room: &Room, (zx, zy): (i32, i32)) -> bool {
        let vertical = self.standing;
        let main = self.body.chunks[HEAD].pos;
        // The held axis along the beam.
        let across_held = if vertical { zy } else { zx };
        let ahead = if vertical {
            Vec2::new(0.0, zy as f32 * 20.0)
        } else {
            Vec2::new(zx as f32 * 20.0, 0.0)
        };
        let narrow = self.narrow_space_at(main + ahead);
        let dir = self.zero_g_pole_grab_dir;
        let mid = main.tile_middle();
        let mut left_beam = false;
        let head = &mut self.body.chunks[HEAD];
        if vertical {
            head.vel.x += if narrow {
                (mid.x - head.pos.x) * 0.1
            } else {
                (mid.x + 5.0 * dir.0 as f32 - head.pos.x) * 0.1
            };
        } else {
            head.vel.y += if narrow {
                (mid.y - head.pos.y) * 0.1
            } else {
                (mid.y + 5.0 * dir.1 as f32 - head.pos.y) * 0.1
            };
        }
        if across_held != 0 {
            let probe = if vertical {
                Vec2::new(0.0, zy as f32 * 10.0)
            } else {
                Vec2::new(zx as f32 * 10.0, 0.0)
            };
            let on_beam = {
                let t = room.tile_at_pos(head.pos + probe);
                if vertical {
                    t.vertical_pole()
                } else {
                    t.horizontal_pole()
                }
            };
            if on_beam {
                let push = across_held as f32 * 1.05 * self.stats.pole_climb_speed_fac;
                if vertical {
                    head.vel.y += push;
                } else {
                    head.vel.x += push;
                }
                self.animation_frame += 1;
                if self.animation_frame > 20 {
                    self.animation_frame = 0;
                    self.play_sound(SoundId::SlugcatClimbUpVerticalBeam, HEAD, 1.0, 1.0);
                }
            } else if zx != 0 || zy != 0 {
                left_beam = true;
            }
        }
        let hips = self.body.chunks[HIPS].pos;
        let hips_tile = room.tile_at_pos(hips);
        let hips_on_beam = if vertical {
            hips_tile.vertical_pole()
        } else {
            hips_tile.horizontal_pole()
        };
        if !left_beam && hips_on_beam {
            let hips_mid = hips.tile_middle();
            let hips = &mut self.body.chunks[HIPS];
            hips.vel *= 0.7;
            if vertical {
                hips.vel.x += (hips_mid.x - 5.0 * dir.0 as f32 - hips.pos.x) * 0.1;
            } else {
                hips.vel.y += (hips_mid.y - 5.0 * dir.1 as f32 - hips.pos.y) * 0.1;
            }
        }
        left_beam
    }

    /// The jump press of `ZeroGPoleGrab`: with a direction held the body is flung along
    /// it (less across the beam's line), without one only the sound; either way it swims.
    fn zero_g_pole_let_go(&mut self, room: &Room, (zx, zy): (i32, i32), left_beam: bool) {
        if zx != 0 || zy != 0 {
            let mut dir = self.swim_dir();
            let tile = room.tile_at_pos(self.body.chunks[HEAD].pos);
            if !left_beam && (!tile.horizontal_pole() || !tile.vertical_pole()) {
                if self.standing && zx == 0 {
                    dir.y *= 0.1;
                } else if !self.standing && zy == 0 {
                    dir.x *= 0.1;
                }
            }
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            head.vel = (head.vel + dir * 5.4).clamp_magnitude(5.4);
            hips.vel = (hips.vel + dir * 5.0).clamp_magnitude(5.0);
            self.play_sound(SoundId::SlugcatFromHorizontalPoleJump, HEAD, 1.0, 1.0);
        } else {
            self.play_sound(SoundId::SlugcatClimbAlongHorizontalBeam, HEAD, 1.0, 1.0);
        }
        self.standing = false;
        self.animation = Animation::ZeroGSwim;
    }
}
