//! `LizardLimb` (decompile `LizardLimb.cs`): a leg that steps ahead of its chunk,
//! reaches for terrain and holds it. Its grip counters are the body's `LegsGripping`.
//! Its `Update` draws on the shared stream (the health rolls and the foot sounds).

use rw_assets::{CreatureType, Room};
use rw_random::UnityRandom;
use rw_sim::creature::lizard::LizardView;
use rw_sim::{BodyChunk, FloatExt, SoundId, Sounds, Vec2};

use crate::body_part::{BodyPart, PointConnection};
use crate::limb::{GripSearch, Limb, LimbMode};
use crate::math::CustomVec;

/// One lizard leg.
#[derive(Debug, Clone, PartialEq)]
pub struct LizardLimb {
    pub limb: Limb,
    /// The chunk it hangs on (0 for the front pair, 2 for the hind pair).
    pub chunk: usize,
    /// `otherLimbInPair`: the even limb before an odd one.
    pub pair: Option<usize>,
    pub reaching_for_terrain: bool,
    pub grip_counter: i32,
    pub currently_disabled: bool,
    pub joint_dist: f32,
    pub grab_pos: Vec2,
    pub flip: f32,
    pub extra_long_step: bool,
    pub sound_bool: bool,
    pub grab_sound: SoundId,
    pub release_sound: SoundId,
}

/// What a limb's `Update` reads of the lizard and the room.
pub struct LimbTick<'a, 'v> {
    pub view: &'a LizardView<'v>,
    pub room: &'a Room,
    pub rng: &'a mut UnityRandom,
    pub sounds: &'a mut Sounds,
}

impl LimbTick<'_, '_> {
    fn chunk(&self, i: usize) -> &BodyChunk {
        &self.view.lizard.body.chunks[i]
    }

    /// `connection.rotationChunk`: chunk 0's is 2, the others' 2 or 0.
    fn rotation_chunk(&self, i: usize) -> &BodyChunk {
        self.chunk(if i == 2 { 0 } else { 2 })
    }
}

impl LizardLimb {
    /// The constructor: `Limb`'s, whose `Reset` (overridden: `retract` set) places it
    /// on the chunk (a draw); retracted, with the breed's foot sounds.
    #[must_use]
    pub fn new(view: &LizardView<'_>, number: usize, rng: &mut UnityRandom) -> Self {
        let params = view.lizard.params;
        let chunk = if number >= 2 { 2 } else { 0 };
        let mut limb = Limb::new(
            number,
            BodyPart::new(2.5, 0.7, 0.99),
            params.limb_speed,
            params.limb_quickness,
        );
        limb.retract = true;
        limb.part.reset(view.lizard.body.chunks[chunk].pos, rng);
        limb.absolute_hunt_pos = limb.part.pos;
        limb.mode = LimbMode::Retracted;
        let (grab_sound, release_sound) = match view.kind {
            CreatureType::GreenLizard => (
                SoundId::LizardGreenFootGrab,
                SoundId::LizardGreenFootRelease,
            ),
            CreatureType::BlueLizard | CreatureType::WhiteLizard => (
                SoundId::LizardBlueWhiteFootGrab,
                SoundId::LizardBlueWhiteFootRelease,
            ),
            _ => (
                SoundId::LizardPinkYellowRedFootGrab,
                SoundId::LizardPinkYellowRedFootRelease,
            ),
        };
        Self {
            limb,
            chunk,
            pair: (number % 2 == 1).then(|| number - 1),
            reaching_for_terrain: false,
            grip_counter: 0,
            currently_disabled: false,
            joint_dist: 25.0 * ((params.body_size_fac + 1.0) / 2.0),
            grab_pos: Vec2::ZERO,
            flip: 0.0,
            extra_long_step: false,
            sound_bool: false,
            grab_sound,
            release_sound,
        }
    }

    /// `LizardLimb.Reset`: `retract`, then `BodyPart.Reset` (a draw).
    pub fn reset(&mut self, point: Vec2, rng: &mut UnityRandom) {
        self.limb.retract = true;
        self.limb.part.reset(point, rng);
    }

    /// `health`: the limb's, 0 while unconscious.
    fn health(&self, view: &LizardView<'_>) -> f32 {
        if view.lizard.conscious() {
            view.limb_health[self.limb.limb_number]
        } else {
            0.0
        }
    }

    /// `StepLength`.
    fn step_length(&self, view: &LizardView<'_>) -> f32 {
        (-0.5f32).lerp(0.5, view.lizard.params.step_length * self.health(view))
    }

    /// `LizardLimb.Update` of `limbs[i]`.
    pub fn update(limbs: &mut [Self], i: usize, cx: &mut LimbTick<'_, '_>) {
        let view = cx.view;
        let lizard = view.lizard;
        let params = lizard.params;
        let health = limbs[i].health(view);
        let me = &mut limbs[i];
        let disable = !me.currently_disabled && cx.rng.value() > health && cx.rng.value() < 0.02;
        if disable || lizard.stunned() {
            me.currently_disabled = true;
        } else if (me.currently_disabled && cx.rng.value() < health && cx.rng.value() < 0.02)
            || health == 1.0
        {
            me.currently_disabled = false;
        }
        me.limb.hunt_speed = params.limb_speed * 0.5f32.lerp(1.0, health);
        me.limb.quickness = 0.1f32.lerp(params.limb_quickness, health);
        let connection = cx.chunk(me.chunk).clone();
        let rotation = cx.rotation_chunk(me.chunk).pos;
        if me.currently_disabled || lizard.swim > 0.5 {
            me.limb.mode = LimbMode::Dangle;
            me.limb.part.vel.y -= 0.9;
            me.limb.retract = false;
            me.grip_counter = 0;
            me.reaching_for_terrain = false;
        } else {
            Self::step(limbs, i, cx, &connection, rotation);
        }
        let me = &mut limbs[i];
        if me.limb.mode == LimbMode::Dangle {
            let side = 30.0 - 60.0 * (me.limb.limb_number % 2) as f32;
            let target =
                connection.pos + Vec2::new(side, 10.0).rotated(rotation.aim_to(connection.pos));
            me.limb.part.vel += me.limb.part.pos.direction_to(target) * 0.6;
        }
        if me.limb.retract && me.limb.reached_snap_position {
            me.limb.mode = LimbMode::HuntAbsolutePosition;
            me.reaching_for_terrain = false;
            me.limb.retract = false;
        }
        if let Some(p) = me.pair {
            let rad = me.limb.part.rad;
            let other = limbs[p].limb.part.pos;
            let me = &mut limbs[i];
            if me.limb.part.pos.dist_less(other, rad * 3.0) {
                let dist = me.limb.part.pos.distance(other);
                let v = me.limb.part.pos.direction_to(other) * ((rad * 3.0 - dist) * 0.5);
                me.limb.part.vel -= v;
                limbs[p].limb.part.vel += v;
            }
        }
        let me = &mut limbs[i];
        me.limb.update(cx.room, &connection, rotation);
        me.limb.part.connect_to_point(PointConnection {
            point: connection.pos,
            rad: me.joint_dist,
            push: false,
            elastic: 0.0,
            host_vel: connection.vel,
            adapt_vel: 0.0,
            exaggerate_vel: 0.0,
        });
        let gripping = me.limb.mode == LimbMode::HuntAbsolutePosition
            && me.reaching_for_terrain
            && (me.limb.reached_snap_position
                || (me.limb.overlapping_hunt_pos() && me.limb.part.terrain_contact));
        if gripping {
            me.grip_counter += 1;
        } else {
            me.grip_counter = 0;
        }
    }

    /// The stepping branch of `Update` (not disabled, not swimming).
    fn step(
        limbs: &mut [Self],
        i: usize,
        cx: &mut LimbTick<'_, '_>,
        connection: &BodyChunk,
        rotation: Vec2,
    ) {
        let view = cx.view;
        let lizard = view.lizard;
        let params = lizard.params;
        let step_length = limbs[i].step_length(view);
        let pair = limbs[i]
            .pair
            .map(|p| (limbs[p].currently_disabled, limbs[p].grip_counter));
        let all_counters: Vec<i32> = limbs.iter().map(|l| l.grip_counter).collect();
        let me = &mut limbs[i];
        if me.limb.mode == LimbMode::HuntAbsolutePosition {
            me.limb.hunt_speed = me.limb.default_hunt_speed() + connection.vel.length();
        }
        let mut a = rotation.direction_to(connection.pos);
        if me.chunk == 2 {
            a *= -1.0;
        }
        a = a
            .lerp(connection.pos.direction_to(lizard.limbs_aim_for), 0.4)
            .normalized();
        let num = me
            .limb
            .part
            .pos
            .distance_to_line(connection.pos, connection.pos + a.perpendicular());
        if !me.reaching_for_terrain {
            me.limb.mode = LimbMode::HuntAbsolutePosition;
            me.limb.absolute_hunt_pos = me
                .limb
                .part
                .pos
                .lerp_clamped(connection.pos, params.lift_feet)
                + a * (me.joint_dist + 1.0);
            if me.extra_long_step
                && let Some((disabled, counter)) = pair
            {
                if disabled {
                    me.extra_long_step = false;
                }
                if counter > params.limb_grip_delay {
                    me.extra_long_step = false;
                }
            }
            if !me.extra_long_step && num < me.joint_dist * -step_length {
                me.reaching_for_terrain = true;
            }
            return;
        }
        if !me.limb.overlapping_hunt_pos() {
            let n = me.limb.limb_number;
            a += a.perpendicular() * ((6.0 - 12.0 * (n % 2) as f32) * 0.2);
            a.y -= 0.3 * params.feet_down;
            a.x += if n.is_multiple_of(2) { -1.0 } else { 1.0 }
                * params.leg_pair_displacement
                * me.flip;
            let wall_climber = matches!(
                view.kind,
                CreatureType::BlueLizard | CreatureType::WhiteLizard
            );
            if me.limb.find_grip(
                cx.room,
                GripSearch {
                    attached: connection.pos,
                    search_from: connection.pos,
                    max_radius: me.joint_dist - 1.0,
                    goal: connection.pos + a * 50.0,
                    forbidden_x: 2,
                    forbidden_y: 2,
                    behind_walls: wall_climber,
                },
            ) {
                // `GrabbedTerrain`.
                me.grab_pos = me.limb.absolute_hunt_pos;
                me.reaching_for_terrain = true;
            }
        } else {
            if me.sound_bool {
                cx.sounds
                    .play_at(me.grab_sound, me.limb.part.pos, 1.0, 1.0, cx.rng);
                me.sound_bool = false;
            }
            let pos = me.limb.part.pos;
            if num > me.joint_dist * -0.5 * (params.step_length + 0.1)
                && !pos.dist_less(connection.pos, me.joint_dist - 1.0)
                && !me
                    .limb
                    .absolute_hunt_pos
                    .dist_less(connection.pos, me.joint_dist)
            {
                me.extra_long_step = pair.is_some()
                    && params.smoothen_leg_movement
                    && pair.is_some_and(|(_, c)| c < 1);
                me.reaching_for_terrain = false;
            }
            if lizard.ai.run_speed > 0.1 && params.smoothen_leg_movement {
                let mine = me.grip_counter;
                let last = all_counters.iter().all(|&c| c <= mine && c != 0);
                if last {
                    me.reaching_for_terrain = false;
                }
            }
            if !me.reaching_for_terrain && !me.sound_bool {
                cx.sounds
                    .play_at(me.release_sound, me.limb.part.pos, 1.0, 1.0, cx.rng);
                me.sound_bool = true;
            }
        }
        if me.limb.mode == LimbMode::Dangle {
            me.reaching_for_terrain = false;
        }
    }
}
