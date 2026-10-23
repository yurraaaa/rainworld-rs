//! The player on climbable vines: the grab in `MovementUpdate` and the `VineGrab`
//! animation (`vineGrabDelay`, `vinePos`, `vineClimbCursor`).
//!
//! Behaviour: `rainworld-re/notes/climbable-vines.md`; divergences D39. The room's
//! `climbableVines` and the live vine owners come through the update's [`NoiseSink`]
//! ([`NoiseSink::vines`]). The vine code itself makes no `UnityEngine.Random` call (the
//! draws in the original's are behind the More Slugcats `ClimbableVine`); an owner's
//! `BeingClimbedOn` may.
//!
//! Mirrors: Player.cs (MovementUpdate: vine grab, UpdateAnimation: VineGrab)

use rw_random::UnityRandom;

use super::{Animation, BodyMode, HEAD, HIPS, Player};
use crate::climbable_vines::{VineClimber, VineHost, VineHostMut, VineKind, VinePosition};
use crate::float_ext::FloatExt;
use crate::{NoiseSink, SoundId, Sounds, Vec2};

impl Player {
    /// `vinePos`.
    #[must_use]
    pub fn vine_pos(&self) -> Option<VinePosition> {
        self.vine_pos
    }

    /// `vineClimbCursor`.
    #[must_use]
    pub fn vine_climb_cursor(&self) -> Vec2 {
        self.vine_climb_cursor
    }

    /// `vineGrabDelay`.
    #[must_use]
    pub fn vine_grab_delay(&self) -> i32 {
        self.vine_grab_delay
    }

    /// `wantToGrab`, as the last `MovementUpdate` left it.
    #[must_use]
    pub fn want_to_grab(&self) -> i32 {
        self.want_to_grab
    }

    /// `MovementUpdate`, after the zero-gravity switch: `vineGrabDelay` counts down; with
    /// gravity an up press (this tick's or last tick's `wantToGrab`) grabs the first vine
    /// along the main chunk's last-to-current path (up to 10 samples), at gravity 0 any
    /// direction grabs a vine under the chunk; a vine that left the room's list lets go.
    pub(super) fn vine_grab(&mut self, noises: &mut dyn NoiseSink) {
        if self.vine_grab_delay > 0 {
            self.vine_grab_delay -= 1;
        }
        let mut access = noises.vines();
        if self.animation != Animation::VineGrab
            && self.vine_grab_delay == 0
            && let Some(vines) = access.as_mut()
        {
            let gravity = self.effective_room_gravity();
            let (x, y) = self.zero_g_int_vec(0);
            let head = &self.body.chunks[HEAD];
            let (last, now, rad) = (head.last_pos, head.pos, head.rad);
            let mut found = None;
            if gravity > 0.0 && (self.want_to_grab > 0 || self.input[0].y > 0) {
                let samples = (last.distance(now) / 5.0).to_int_cs().clamp(1, 10);
                for m in 0..samples {
                    let t = if samples > 1 {
                        m as f32 / (samples - 1) as f32
                    } else {
                        0.0
                    };
                    found = vines
                        .system
                        .vine_overlap(&vines.host, last.lerp(now, t), rad);
                    if found.is_some() {
                        break;
                    }
                }
            } else if (x != 0 || y != 0) && gravity == 0.0 {
                found = vines.system.vine_overlap(&vines.host, now, rad);
            }
            if let Some(found) = found {
                let kind = vines.host.kind(found.vine);
                self.grab_vine(kind, found);
            }
        }
        if self.animation == Animation::VineGrab
            && !access
                .as_ref()
                .zip(self.vine_pos)
                .is_some_and(|(vines, pos)| vines.system.contains(pos.vine))
        {
            self.animation = Animation::None;
        }
    }

    /// The grab itself: the vine's class picks the sound (the More Slugcats `Leaves` sound of
    /// its `ClimbableVine` is not ported, D39).
    fn grab_vine(&mut self, kind: VineKind, found: VinePosition) {
        match kind {
            VineKind::CoralNeuron => self.play_sound(SoundId::GrabNeuron, HEAD, 1.0, 1.0),
            VineKind::CoralStem => self.play_sound(SoundId::GrabCoralStem, HEAD, 1.0, 1.0),
            VineKind::ClimbableCorruptionTube => {
                self.play_sound(SoundId::GrabCorruptionTube, HEAD, 1.0, 1.0);
            }
            _ => {}
        }
        self.animation = Animation::VineGrab;
        self.vine_pos = Some(found);
        self.want_to_grab = 0;
    }

    /// `AnimationIndex.VineGrab`: the held direction moves a cursor (at most 30 px from the
    /// main chunk) that sets the climbing speed along the vine and leans the body, a
    /// crossing vine under the cursor takes over, the chunk is held to the vine, a jump
    /// press lets go (flung along the held direction, the vine kicked back, regrab delay
    /// 10), and at gravity 0 the hips are drawn toward the nearer point 20 px along the vine.
    pub(super) fn vine_grab_animation(
        &mut self,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
        noises: &mut dyn NoiseSink,
    ) {
        self.dynamic_run_speed = [0.0, 0.0];
        self.body_mode = BodyMode::Default;
        let (Some(access), Some(mut pos)) = (noises.vines(), self.vine_pos) else {
            self.animation = Animation::None;
            return;
        };
        let (vines, mut host) = (access.system, access.host);
        let host = &mut host;
        let dir = self.swim_dir();
        if host.draws_when_climbed(pos.vine) {
            self.flush_sounds(rng, sounds);
        }
        vines.vine_being_climbed_on(
            host,
            &pos,
            &VineClimber {
                main_chunk_pos: self.body.chunks[HEAD].pos,
            },
            rng,
            sounds,
        );
        if dir.length() > 0.0 {
            let aligned = dir.dot(self.vine_climb_cursor.normalized());
            self.vine_climb_cursor = (self.vine_climb_cursor
                + dir * aligned.lerp_map(-1.0, 1.0, 10.0, 3.0))
            .clamp_magnitude(30.0);
            let before = vines.on_vine_pos(&*host, &pos);
            let goal = self.body.chunks[HEAD].pos + self.vine_climb_cursor;
            pos.float_pos += vines.climb_on_vine_speed(&*host, &pos, goal)
                * 2.1_f32.lerp(1.5, self.effective_room_gravity())
                / vines.total_length(&*host, pos.vine);
            pos.float_pos = pos.float_pos.clamp(0.0, 1.0);
            let after = vines.on_vine_pos(&*host, &pos);
            vines.push_at_vine(host, &pos, (before - after) * 0.05);
            if self.vine_grab_delay == 0 {
                let goal = self.body.chunks[HEAD].pos + self.vine_climb_cursor;
                if let Some(switched) =
                    vines.vine_switch(&*host, &pos, goal, self.body.chunks[HEAD].rad)
                {
                    pos = switched;
                    self.vine_grab_delay = 10;
                }
            }
            self.animation_frame += 1;
            if self.animation_frame > 30 {
                self.animation_frame = 0;
            }
        } else {
            self.vine_climb_cursor *= 0.8;
        }
        let lean = self.vine_climb_cursor / 190.0;
        self.body.chunks[HEAD].vel += lean;
        self.body.chunks[HIPS].vel -= lean;
        let on_vine = vines.on_vine_pos(&*host, &pos);
        let (x, y) = self.zero_g_int_vec(0);
        if x != 0 {
            self.zero_g_pole_grab_dir.0 = x;
        }
        if y != 0 {
            self.zero_g_pole_grab_dir.1 = y;
        }
        let mut let_go = false;
        if self.input[0].jump && !self.input[1].jump {
            let_go = true;
            if dir.length() > 0.0 {
                let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
                head.vel += dir.normalized() * 4.0;
                hips.vel += dir.normalized() * 3.5;
                head.vel = head.vel.lerp(head.vel.clamp_magnitude(5.0), 0.5);
                hips.vel = hips.vel.lerp(hips.vel.clamp_magnitude(5.0), 0.5);
                vines.push_at_vine(host, &pos, -dir.normalized() * 15.0);
                self.vine_grab_delay = 10;
            }
        } else if !vines.vine_currently_climbable(&*host, &pos) {
            let_go = true;
            self.vine_grab_delay = 10;
        }
        let rad = if let_go {
            0.0
        } else {
            vines.vine_rad(&*host, &pos)
        };
        if !let_go && self.body.chunks[HEAD].pos.dist_less(on_vine, 40.0 + rad) {
            vines.connect_chunk_to_vine(host, &mut self.body.chunks[HEAD], &pos, rad);
            let across = vines.vine_dir(&*host, &pos).perpendicular();
            let side = if across.x.abs() > across.y.abs() {
                self.zero_g_pole_grab_dir.0
            } else {
                self.zero_g_pole_grab_dir.1
            };
            self.body.chunks[HEAD].vel += across * (0.2 * side as f32);
            if self.effective_room_gravity() == 0.0 {
                let length = vines.total_length(&*host, pos.vine);
                let behind = vines.on_vine_pos(
                    &*host,
                    &VinePosition::new(pos.vine, pos.float_pos - 20.0 / length),
                );
                let ahead = vines.on_vine_pos(
                    &*host,
                    &VinePosition::new(pos.vine, pos.float_pos + 20.0 / length),
                );
                let hips = self.body.chunks[HIPS].pos;
                let nearer = if hips.distance(behind) < hips.distance(ahead) {
                    behind
                } else {
                    ahead
                };
                let pull = (nearer - hips).clamp_magnitude(5.0) / 20.0;
                self.body.chunks[HEAD].vel -= pull;
                self.body.chunks[HIPS].vel += pull;
            }
        } else {
            self.animation = Animation::None;
        }
        self.vine_pos = Some(pos);
    }
}
