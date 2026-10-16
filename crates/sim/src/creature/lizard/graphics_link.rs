//! What a lizard's graphics module (`LizardGraphics`, built in `rw-graphics` while its
//! room is viewed) shares with the body: the grip its legs report (`LegsGripping`,
//! `NoGripCounter`), the flickers, the head and body parts the bubbles and a lodged
//! spear read, its head light and its belly-drag loop; what it reads of the body
//! ([`LizardView`]) and of the room ([`LizardGraphicsTick`]), and the objects it adds
//! to the room.

use rw_assets::{CreatureType, Room, SoundTable};
use rw_random::UnityRandom;

use super::{Lizard, LizardBreedParams};
use crate::creature::{CreatureId, EntityId};
use crate::item::{ChunkDynamicSoundLoop, LoopCx};
use crate::{CameraView, Sounds, Vec2, Water, WaterDrip};

/// The graphics module's state the body and the room's objects read or set. The body
/// parts' positions are a copy taken after each of the module's updates (they change
/// nowhere else); velocity pushes wait for its next update.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LizardGraphicsLink {
    /// The module's serial, set by the observer that builds it (0 for none); the
    /// bubbles keep the module that made them, even after it is dropped.
    pub module: u64,
    /// `legsGrabbing`: the limbs holding on (`LegsGripping`).
    pub legs_grabbing: i32,
    /// `noGripCounter` (`NoGripCounter`).
    pub no_grip_counter: i32,
    /// `whiteFlicker`.
    pub white_flicker: i32,
    /// How many times `WhiteFlicker` was called (each sets `everySecondDraw` false).
    pub white_flicker_calls: u32,
    /// `flicker` (`Stun`).
    pub flicker: i32,
    /// `lightSource`: the id of its light in the room.
    pub light: Option<u64>,
    /// `soundLoop` (`ChunkDynamicSoundLoop` on the main chunk).
    pub sound_loop: ChunkDynamicSoundLoop,
    /// The rooms whose `drawableObjects` hold the module (`NoLongerViewed` of any of
    /// them removes it).
    pub drawn_in: Vec<i32>,
    /// The head as the module last left it.
    pub head: LizardHead,
    /// `bodyParts[i].pos` (limbs, tail, head) as the module last left them.
    pub parts: Vec<Vec2>,
    /// Velocity added to `bodyParts[i]` since the module's last update.
    pub pushes: Vec<(usize, Vec2)>,
    /// The module's voice reading as of its last update.
    pub voice: LizardVoiceView,
}

/// `voiceVisualizationIntensity` and `voiceVisualization` of a lizard's graphics.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LizardVoiceView {
    pub intensity: f32,
    pub visualization: f32,
}

impl LizardGraphicsLink {
    /// `LizardGraphics.WhiteFlicker(fl)`.
    pub fn white_flicker(&mut self, fl: i32) {
        if fl > self.white_flicker {
            self.white_flicker = fl;
        }
        self.white_flicker_calls += 1;
    }

    /// `LizardGraphics.Stun(st)`.
    pub fn stun(&mut self, st: i32) {
        self.flicker = (st / 3).clamp(3, 15);
    }

    /// `bodyParts[index].vel += dv`, returning the part's position.
    pub fn push_part(&mut self, index: usize, dv: Vec2) -> Option<Vec2> {
        let pos = *self.parts.get(index)?;
        self.pushes.push((index, dv));
        Some(pos)
    }
}

/// What `LizardGraphics` reads of a realized lizard.
pub struct LizardView<'a> {
    pub id: EntityId,
    pub creature: CreatureId,
    pub kind: CreatureType,
    pub lizard: &'a Lizard,
    /// `LizardState.health` and `limbHealth`.
    pub health: f32,
    pub limb_health: [f32; 4],
    /// `grasps[0] != null`.
    pub holding: bool,
    /// The grabbed chunk's position while `grasps[0]` holds a creature.
    pub grabbed_creature: Option<Vec2>,
}

impl LizardView<'_> {
    /// `JawOpen`.
    #[must_use]
    pub fn jaw_open(&self) -> f32 {
        self.lizard.jaw_open(self.holding)
    }
}

/// The graphics' head as the bubbles read it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LizardHead {
    pub pos: Vec2,
    pub vel: Vec2,
    /// `AimFromOneVectorToAnother(drawPositions[0, 0], head.pos)`.
    pub aim: f32,
    /// `headDepthRotation`.
    pub depth_rotation: f32,
}

impl LizardHead {
    /// `HeadRotation(1)` for a lizard of `params` whose `JawOpen` is `jaw`.
    #[must_use]
    pub fn rotation(&self, params: &LizardBreedParams, jaw: f32) -> f32 {
        self.aim
            + params.jaw_open_angle
                * (1.0 - params.jaw_open_lower_jaw_fac)
                * jaw.clamp(0.0, 1.0)
                * self.depth_rotation
    }

    /// What a bubble reads of the head now.
    #[must_use]
    pub fn for_bubble(&self, params: &LizardBreedParams, jaw: f32) -> BubbleHead {
        BubbleHead {
            pos: self.pos,
            vel: self.vel,
            rotation: self.rotation(params, jaw),
        }
    }
}

/// `head.pos`, `head.vel` and `HeadRotation(1)` as a `LizardBubble` reads them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BubbleHead {
    pub pos: Vec2,
    pub vel: Vec2,
    /// Degrees.
    pub rotation: f32,
}

/// An object the graphics add to their room (`Room.AddObject`), in its place in the
/// update list.
#[derive(Debug, Clone, PartialEq)]
pub enum LizardSpawn {
    Drip(WaterDrip),
    Bubble(super::LizardBubble),
    /// `new LightSource(head.pos, false, effectColor, lizard)` with `requireUpKeep`.
    Light {
        pos: Vec2,
        color: [f32; 3],
    },
}

/// What the graphics set on their light each update.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightUpkeep {
    /// The light's id.
    pub id: u64,
    pub pos: Vec2,
    pub rad: f32,
    pub alpha: f32,
    pub color: [f32; 3],
}

/// What the graphics' `Update` reads of the room and the game, and what it adds to
/// the room.
pub struct LizardGraphicsTick<'a> {
    pub room_index: i32,
    pub room: &'a Room,
    pub water: Option<&'a Water>,
    /// The room's camera count (culling needs more than two).
    pub cameras: usize,
    pub camera_room: &'a Room,
    pub camera: &'a CameraView,
    pub rng: &'a mut UnityRandom,
    pub sounds: &'a mut Sounds,
    pub triggers: &'a SoundTable,
    pub tick: u64,
    /// The world's update-list counter, written back after the update.
    pub update_seq: u64,
    /// `Room.Darkness(pos)`: the camera's palette darkness (the room is viewed).
    pub darkness: f32,
    /// Whether the link's light is slated for deletion.
    pub light_slated: bool,
    /// The objects added, with their places in the update list.
    pub added: Vec<(u64, LizardSpawn)>,
    pub light_upkeep: Option<LightUpkeep>,
}

impl LizardGraphicsTick<'_> {
    /// `Room.AddObject`: the object takes the next place in the update list.
    pub fn add(&mut self, spawn: LizardSpawn) {
        self.update_seq += 1;
        self.added.push((self.update_seq, spawn));
    }

    /// What the belly-drag loop reads.
    pub fn loop_cx(&mut self) -> LoopCx<'_> {
        LoopCx {
            room_index: self.room_index,
            sounds: self.sounds,
            rng: self.rng,
            triggers: self.triggers,
            camera_room: self.camera.room,
            tick: self.tick,
            update_seq: &mut self.update_seq,
        }
    }

    /// `Room.PointSubmerged`.
    #[must_use]
    pub fn point_submerged(&self, pos: Vec2) -> bool {
        self.water.is_some_and(|w| pos.y < w.level_at(pos.x))
    }
}
