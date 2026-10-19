//! What the slugcat keeps for its hands: the `GrabUpdate` counters, `ThrowDirection`,
//! the belly-slide push of `Collide`, the grasps `Stun` and `Die` drop, the objects
//! carried into a shortcut and the drips its impacts throw. The grasps themselves live
//! on its `AbstractCreature`; [`crate::RainWorldGame`] runs the catch and `GrabUpdate`.
//!
//! Mirrors: Player.cs (GrabUpdate, ThrowDirection, Collide, Stun, Die)

use rw_random::UnityRandom;

use super::{Animation, BodyMode, HEAD, HIPS, Player};
use crate::item::{Entity, ObjectId};
use crate::{TerrainImpact, Vec2, WaterDrip};

/// The `Player` fields `GrabUpdate` reads and sets.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GrabState {
    pub eat_counter: i32,
    pub want_to_pick_up: i32,
    pub no_pick_up_on_release: i32,
    pub dont_grab_stuff: i32,
    pub switch_hands_counter: i32,
    pub switch_hands_process: f32,
    pub want_to_throw: i32,
    pub swallow_and_regurgitate_counter: i32,
    pub pick_up_candidate: Option<Entity>,
}

/// `objectInStomach`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StomachObject {
    /// A rock or spear of the world's arena, in no room.
    Object(ObjectId),
    /// A saved object or creature the port does not type, kept as its save string
    /// (docs/divergences/ D16).
    Saved(String),
}

impl Player {
    /// `objectInStomach`.
    #[must_use]
    pub fn object_in_stomach(&self) -> Option<&StomachObject> {
        self.stomach.as_ref()
    }

    pub(crate) fn set_object_in_stomach(&mut self, object: Option<StomachObject>) {
        self.stomach = object;
    }

    #[must_use]
    pub fn grab_state(&self) -> &GrabState {
        &self.grab
    }

    /// `ThrowDirection`: the held direction, else the facing, while airborne or lying
    /// short; otherwise the way the body points.
    #[must_use]
    pub fn throw_direction(&self) -> i32 {
        let (head, hips) = (self.body.chunks[HEAD].pos, self.body.chunks[HIPS].pos);
        if self.body_mode == BodyMode::Default || (head.x - hips.x).abs() < 15.0 {
            if self.input_x() == 0 {
                return self.flip_direction;
            }
            return self.input_x();
        }
        // `Mathf.Sign` is 1 at 0.
        if head.x - hips.x >= 0.0 { 1 } else { -1 }
    }

    /// `TotalMass`.
    #[must_use]
    pub fn total_mass(&self) -> f32 {
        self.body.chunks.iter().map(|c| c.mass).sum()
    }

    /// `mainBodyChunk.submersion`.
    #[must_use]
    pub fn submersion(&self, water: Option<&crate::Water>) -> f32 {
        self.body.chunks[HEAD].submersion(water)
    }

    /// The belly-slide part of `Player.Collide` with a creature: a short slide caps the
    /// roll counter at 11, and the head is pushed on. Returns the creature's stun while
    /// the player slides (2 ticks, 4 on a long slide). A sleeping slugcat wakes.
    pub(crate) fn collide_with_creature(&mut self) -> Option<i32> {
        let mut stun = None;
        if self.animation == Animation::BellySlide {
            stun = Some(if self.long_belly_slide { 4 } else { 2 });
            if !self.long_belly_slide && self.roll_counter > 11 {
                self.roll_counter = 11;
            }
            self.body.chunks[HEAD].vel.x += self.roll_direction as f32 * 3.0;
        }
        if self.sleeping() {
            self.sleep_counter = 0;
        }
        stun
    }

    #[must_use]
    pub fn roll_counter(&self) -> i32 {
        self.roll_counter
    }

    #[must_use]
    pub fn roll_direction(&self) -> i32 {
        self.roll_direction
    }

    pub(crate) fn grab_mut(&mut self) -> &mut GrabState {
        &mut self.grab
    }

    /// Whether `Stun` or `Die` let go of the grasps since the last call.
    pub(crate) fn take_release_grasps(&mut self) -> bool {
        std::mem::take(&mut self.release_grasps)
    }

    pub(crate) fn request_release_grasps(&mut self) {
        self.release_grasps = true;
    }

    /// `MovementUpdate` ran this tick: `GrabUpdate` is due.
    #[must_use]
    pub fn movement_ran(&self) -> bool {
        self.movement_ran
    }

    /// The shortcut pull this tick (target, snap radius), for the carried objects.
    pub(crate) fn shortcut_pull(&self) -> Option<(Vec2, f32)> {
        self.shortcut_pull
    }

    /// The body when this tick's shortcut pull began (what the graphics' extra update in
    /// `Creature.Update` reads), if it was pulled.
    #[must_use]
    pub fn before_pull(&self) -> Option<&Player> {
        self.before_pull.as_deref()
    }

    pub(crate) fn set_shortcut_pull(&mut self, target: Vec2, radius: f32) {
        self.shortcut_pull = Some((target, radius));
    }

    pub(crate) fn set_ceiling_drips(&mut self, ceiling_drips: f32) {
        self.ceiling_drips = ceiling_drips;
    }

    /// The drips this tick's impacts threw.
    pub(crate) fn take_drips(&mut self) -> Vec<WaterDrip> {
        std::mem::take(&mut self.drips)
    }

    /// The base `Creature.TerrainImpact` the slugcat's ends with: a first contact over a
    /// wet ceiling's threshold throws drips.
    pub(super) fn impact_drips(
        &mut self,
        pos: Vec2,
        rad: f32,
        mass: f32,
        impact: TerrainImpact,
        rng: &mut UnityRandom,
    ) {
        if !impact.first_contact {
            return;
        }
        let drips = WaterDrip::from_impact(&impact, pos, rad, mass, self.ceiling_drips, rng);
        self.drips.extend(drips);
    }
}
