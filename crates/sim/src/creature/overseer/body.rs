//! The realized `Overseer`'s state: one body chunk hovering at `hoverTile`, rooted in a
//! wall at `rootTile`, and the zip path it travels through walls between them.

use rw_random::UnityRandom;

use crate::creature::ai::SeededRandom;
use crate::creature::{AbstractCreature, CreatureId};
use crate::{BodyChunk, FloatExt, PhysicalObject, PhysicsParams, Vec2};

/// `Overseer.Mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverseerMode {
    #[default]
    Watching,
    Withdrawing,
    Zipping,
    Emerging,
    SittingInWall,
    Conversing,
    Projecting,
}

/// The constructor's physics: no gravity, air friction 0.8, no terrain collision.
const PARAMS: PhysicsParams = PhysicsParams {
    gravity: 0.0,
    air_friction: 0.8,
    bounce: 0.0,
    surface_friction: 1.0,
    impact_threshold: 1.0,
    buoyancy: 0.0,
    water_friction: 1.0,
    water_retardation_immunity: 0.0,
};

/// `appendages[0]`'s segment count.
pub const APPENDAGE_SEGMENTS: usize = 4;

/// The realized `Overseer`.
#[derive(Debug, Clone, PartialEq)]
pub struct OverseerBody {
    /// One chunk (radius 3, mass 0.4).
    pub body: PhysicalObject,
    pub hover_tile: (i32, i32),
    pub next_hover_tile: (i32, i32),
    pub root_tile: (i32, i32),
    pub root_pos: Vec2,
    pub last_root_pos: Vec2,
    pub root_dir: Vec2,
    pub last_root_dir: Vec2,
    pub conversation_partner: Option<CreatureId>,
    pub last_conversation_partner: Option<CreatureId>,
    pub zip_path: Vec<(i32, i32)>,
    /// `zipPathCount`: the path's length, or fewer after a failed pathing's ray trace
    /// wrote over its start.
    pub zip_path_count: usize,
    pub mode: OverseerMode,
    pub after_withdraw_mode: OverseerMode,
    pub zip_prog_speed: f32,
    pub zip_progs: [f32; 5],
    pub extended: f32,
    pub last_extended: f32,
    /// 0.6 for the player guide, else 0.5..1 from the ID's seed.
    pub size: f32,
    pub stationary_counter: i32,
    pub zip_mesh_direction: bool,
    pub conversation_delay: i32,
    pub leave_room_on_zip_completion: bool,
    /// `inShortcut`: withdrawn (or riding a teleport vessel).
    pub in_shortcut: bool,
    /// `room != null`: placed in its room (not in the shortcut handler's lobby).
    pub in_room: bool,
    /// `appendages[0].segments` as `AppendagePosition` gives them without a graphics
    /// module (with one the game reads its drawn tentacle; only weapons hit-test it).
    pub appendage: [Vec2; APPENDAGE_SEGMENTS],
    /// Position in its room's update list (higher updates first).
    pub update_order: u64,
}

impl OverseerBody {
    /// The `Overseer` constructor: no global draws (`size` comes from the ID's seed).
    pub(crate) fn new(parent: &AbstractCreature, guide: bool, rng: &mut UnityRandom) -> Self {
        let mut size = 0.5.lerp(1.0, rng.seeded_value(parent.id.random_seed()));
        if guide {
            size = 0.6;
        }
        let mut chunk = BodyChunk::new(Vec2::ZERO, 3.0, 0.4);
        chunk.collide_with_terrain = false;
        chunk.go_through_floors = true;
        Self {
            body: PhysicalObject::new(vec![chunk], PARAMS),
            hover_tile: (0, 0),
            next_hover_tile: (0, 0),
            root_tile: (0, 0),
            root_pos: Vec2::ZERO,
            last_root_pos: Vec2::ZERO,
            root_dir: Vec2::ZERO,
            last_root_dir: Vec2::ZERO,
            conversation_partner: None,
            last_conversation_partner: None,
            zip_path: vec![parent.pos.tile],
            zip_path_count: 1,
            mode: OverseerMode::Watching,
            after_withdraw_mode: OverseerMode::Watching,
            zip_prog_speed: 0.0,
            zip_progs: [0.0; 5],
            extended: 1.0,
            last_extended: 1.0,
            size,
            stationary_counter: 0,
            zip_mesh_direction: false,
            conversation_delay: 0,
            leave_room_on_zip_completion: false,
            in_shortcut: false,
            in_room: false,
            appendage: [Vec2::ZERO; APPENDAGE_SEGMENTS],
            update_order: 0,
        }
    }

    /// `mainBodyChunk`.
    #[must_use]
    pub fn chunk(&self) -> &BodyChunk {
        &self.body.chunks[0]
    }

    pub fn chunk_mut(&mut self) -> &mut BodyChunk {
        &mut self.body.chunks[0]
    }

    /// `SwitchModes(Watching)` called by its conversation partner; `delay` is the
    /// `Range(50, 120)` drawn when it was conversing (-1 when it was not).
    pub(crate) fn switch_to_watching(&mut self, delay: i32) {
        if self.mode == OverseerMode::Conversing {
            self.conversation_delay = delay;
            self.last_conversation_partner = self.conversation_partner;
            self.conversation_partner = None;
        }
        self.mode = OverseerMode::Watching;
    }

    /// `Appendage.Update` with `AppendagePosition` as without graphics: hidden far away
    /// when withdrawn, else evenly from the root to the body.
    pub(crate) fn update_appendage(&mut self) {
        let pos = self.chunk().pos;
        for (i, seg) in self.appendage.iter_mut().enumerate() {
            *seg = if self.extended < 0.2 {
                Vec2::new(-10000.0, -10000.0)
            } else {
                let t = i as f32 / (APPENDAGE_SEGMENTS - 1) as f32;
                self.root_pos.lerp(pos, t.clamp(0.0, 1.0))
            };
        }
    }
}
