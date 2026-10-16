//! `OverseerHolograms`: the pictures an overseer projects for the player. A hologram is
//! a room object of its own (it updates in the room's update list, newest first), owned
//! here by the overseer that made it ([`super::OverseerBody::holograms`]); the overseer
//! points at its current one. Its parts hold what the update decides (fades, offsets,
//! key presses); the line geometry and drawing are `rw-graphics`'. Behaviour:
//! `rainworld-re/notes/overseer.md` §7.

mod add;
mod instruction;
mod parts;
mod update;
mod world_queries;

pub use instruction::{
    Controller, InputInstruction, InstructionDevice, InstructionKind, WaitMeter,
};
pub use parts::{ButtonState, HoloColor, HoloImage, HologramPart, PartColor, PartKind, SymbolPart};
pub(crate) use update::HologramUpdate;

use rw_assets::IggySymbol;

use crate::Vec2;

/// `OverseerHologram.Message` (base game).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HologramMessage {
    None,
    Bats,
    DangerousCreature,
    FoodObject,
    Shelter,
    ProgressionDirection,
    GateScene,
    InWorldSuperJump,
    PickupObject,
    ScavengerTrade,
    Angry,
    ForcedDirection,
    TutorialFood,
    GetUpOnFirstBox,
    ClimbPole,
    EatInstruction,
    SuperJump,
}

impl HologramMessage {
    /// The messages `TryAddHologram` answers with an input instruction.
    #[must_use]
    pub fn is_instruction(self) -> bool {
        matches!(
            self,
            Self::GetUpOnFirstBox
                | Self::ClimbPole
                | Self::SuperJump
                | Self::InWorldSuperJump
                | Self::EatInstruction
                | Self::PickupObject
                | Self::ScavengerTrade
        )
    }
}

/// `Options.ControlSetup.Preset` as `TryAddHologram` sorts it: which instruction picture
/// is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlPreset {
    #[default]
    Keyboard,
    XBox,
    Ps4DualShock,
    Ps5DualSense,
    /// Any of the Switch presets.
    Switch,
}

/// A `DoorPointer`'s state.
#[derive(Debug, Clone, PartialEq)]
pub struct DoorPointer {
    /// `d`: the room connection pointed at.
    pub door: i32,
    pub door_pos: Vec2,
    pub report_player_exit_entrance: bool,
    pub pointer_necessary: bool,
    /// Whether a `CreatureDoorDistanceTracker` was made (it needs the Fly exit-distance
    /// maps, not loaded: docs/DIVERGENCES.md D14).
    pub tracker: bool,
    /// The pointer's `lastRoom`.
    pub last_room: i32,
}

/// What a pointing hologram points at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerKind {
    /// `BatPointer` (`Bats` and `TutorialFood`).
    Bat {
        currently_pointing_at_swarm_room: i32,
    },
    /// `ShelterPointer`.
    Shelter {
        /// `FORCEPOINT`: the tutorial's fixed door, -1 for none.
        forcepoint: i32,
        showing_a_shelter_door: bool,
        currently_showing_to_shelter: i32,
    },
    /// `DirectionPointer`.
    Direction { cycle: i32 },
    /// `ForcedDirectionPointer`: the guide's `forcedDirectionToGive` (a
    /// `ReliableIggyDirection` room object) when it was made.
    Forced {
        direction: u64,
        symbol: Option<IggySymbol>,
        cycle: i32,
    },
}

/// What an `ObjectPointer` points at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ObjectPointerKind {
    /// `CreaturePointer`.
    Creature {
        has_warned_counter_visible: i32,
        has_warned_counter_off_screen: i32,
        cycle: i32,
    },
    /// `FoodPointer`.
    Food { has_told_counter: i32, cycle: i32 },
}

/// The hologram's class and its own state. Part indices refer to
/// [`OverseerHologram::parts`].
#[derive(Debug, Clone, PartialEq)]
pub enum HologramKind {
    /// A `DoorPointer`: parts arrow, light, symbol.
    Door {
        pointer: PointerKind,
        door: DoorPointer,
    },
    /// An `ObjectPointer`: parts arrow, light, its own arrow, symbol. Nothing it can point
    /// at is realized in the port.
    Object { pointer: ObjectPointerKind },
    /// `OverseerImage`.
    Image(ImageHologram),
    /// `AngryHologram`: parts red cross, image, frame.
    Angry { counter: i32, show_time: i32 },
    /// An input instruction (`KeyBoardInstruction`, `GamePadInstruction`,
    /// `SwitchInstruction`).
    Instruction(Box<InputInstruction>),
}

/// `OverseerImage.ImageID` (base game), in the enum's order (`Index` picks the cell of
/// the projection texture).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageId {
    MoonFullFigure,
    MoonDoubleSize,
    MoonPortrait,
    Slugcat1,
    Slugcat2,
    Slugcat3,
    Slugcat4,
    Slugcat5,
    Slugcat6,
    Slugcat7,
    Clue1,
    Clue2,
    Clue3,
    Clue4,
    ScavOutpost,
    ScavAndPearls,
    ScavSlugcatTrade,
    Swarmers,
    MoonAndSwarmers,
    DeadSlugcatA,
    DeadSlugcatB,
    MoonFantasy,
    SlugcatEating,
    SlugcatSleeping,
    Undefined,
}

impl ImageId {
    /// `ExtEnum.Index`.
    #[must_use]
    pub fn index(self) -> i32 {
        self as i32
    }
}

/// `OverseerImage`: parts image, frame.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageHologram {
    pub images: Vec<ImageId>,
    /// The room's `ProjectedImagePosition` placed object, if any.
    pub show_pos: Option<Vec2>,
    pub counter: i32,
    pub curr_image: usize,
    pub time_on_each_image: i32,
    pub show_time: i32,
    pub logged_as_seen: bool,
}

/// One `OverseerHologram`.
#[derive(Debug, Clone, PartialEq)]
pub struct OverseerHologram {
    /// Unique per game, for the graphics.
    pub id: u64,
    pub message: HologramMessage,
    pub kind: HologramKind,
    /// The room it was added to.
    pub room: i32,
    /// Position in the room's update list (higher updates first).
    pub update_order: u64,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub look_at_communication_creature: bool,
    pub look_at_comm_crit_counter: i32,
    pub parts: Vec<HologramPart>,
    pub display_tile: (i32, i32),
    pub temp_display_tile: (i32, i32),
    pub fade: f32,
    pub last_fade: f32,
    pub still_relevant: bool,
    pub importance: f32,
    pub push_around_pos: Vec2,
    pub overseer_sit_still: bool,
    /// `slatedForDeletetion`.
    pub slated: bool,
}

impl OverseerHologram {
    fn new(id: u64, message: HologramMessage, importance: f32, room: i32) -> Self {
        Self {
            id,
            message,
            kind: HologramKind::Angry {
                counter: 0,
                show_time: 0,
            },
            room,
            update_order: 0,
            pos: Vec2::ZERO,
            last_pos: Vec2::ZERO,
            look_at_communication_creature: false,
            look_at_comm_crit_counter: 0,
            parts: Vec::new(),
            display_tile: (0, 0),
            temp_display_tile: (0, 0),
            fade: 0.0,
            last_fade: 0.0,
            still_relevant: true,
            importance,
            push_around_pos: Vec2::ZERO,
            overseer_sit_still: false,
            slated: false,
        }
    }

    /// `AddPart`: returns the part's index.
    fn add_part(&mut self, part: HologramPart) -> usize {
        self.parts.push(part);
        self.parts.len() - 1
    }

    /// The instruction state, for an input instruction.
    #[must_use]
    pub fn instruction(&self) -> Option<&InputInstruction> {
        match &self.kind {
            HologramKind::Instruction(i) => Some(i),
            _ => None,
        }
    }

    /// The colour `hologram.color` resolves to now (the pointers that flash override it).
    #[must_use]
    pub fn color(&self) -> HoloColor {
        match &self.kind {
            HologramKind::Door {
                pointer: PointerKind::Direction { cycle },
                ..
            } if cycle % 30 >= 15 => HoloColor::White,
            HologramKind::Door {
                pointer: PointerKind::Forced { symbol, cycle, .. },
                ..
            } => match symbol {
                Some(IggySymbol::Food) if cycle % 60 >= 30 => HoloColor::Mint,
                Some(IggySymbol::SlugcatFace | IggySymbol::DynamicDirection)
                    if cycle % 30 >= 15 =>
                {
                    HoloColor::White
                }
                Some(IggySymbol::Danger) if cycle % 30 >= 15 => HoloColor::Red,
                _ => HoloColor::Base,
            },
            HologramKind::Object {
                pointer: ObjectPointerKind::Creature { cycle, .. },
            } if cycle % 30 >= 15 => HoloColor::Red,
            HologramKind::Object {
                pointer: ObjectPointerKind::Food { cycle, .. },
            } if cycle % 60 >= 30 => HoloColor::Mint,
            _ => HoloColor::Base,
        }
    }
}
