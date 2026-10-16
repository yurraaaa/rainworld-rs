//! `Overseer.TryAddHologram` and the constructors' room-dependent parts.

use super::{
    HologramMessage, HologramUpdate, ImageHologram, ImageId, ObjectPointerKind, OverseerHologram,
    PointerKind,
};
use rw_assets::{IggySymbol, PlacedObjectType};

use crate::Vec2;
use crate::creature::overseer::PlayerGuideState;
use crate::creature::overseer::update::OverseerUpdate;

impl OverseerUpdate<'_, '_> {
    /// The `Update` of hologram `id`, which this overseer owns.
    pub(crate) fn update_hologram(&mut self, id: u64) {
        let Some(i) = self.body.holograms.iter().position(|h| h.id == id) else {
            return;
        };
        let mut holo = self.body.holograms.remove(i);
        HologramUpdate {
            holo: &mut holo,
            body: self.body,
            ai: self.ai,
            abs: self.abs,
            base: self.base,
            cx: self.cx,
        }
        .update();
        self.body.holograms.insert(i, holo);
    }

    /// `TryAddHologram(message, player, importance)`: a new hologram replaces the current
    /// one unless that shows the same message or matters at least as much (an importance
    /// of `f32::MAX` always replaces). The replaced one fades out on its own.
    pub(crate) fn try_add_hologram(&mut self, message: HologramMessage, importance: f32) {
        let tutorial = self.ai.tutorial.is_some();
        self.add_hologram(message, importance, tutorial);
    }

    /// `TryAddHologram` from the tutorial's update (its behaviour is out of the AI then).
    pub(crate) fn tutorial_hologram(&mut self, message: HologramMessage, importance: f32) {
        self.add_hologram(message, importance, true);
    }

    fn add_hologram(&mut self, message: HologramMessage, importance: f32, tutorial: bool) {
        if let Some(current) = self.body.current_hologram_mut() {
            if current.message == message
                || (current.importance >= importance && importance != f32::MAX)
            {
                return;
            }
            current.still_relevant = false;
        }
        self.body.hologram = None;
        if !self.body.in_room {
            return;
        }
        *self.cx.hologram_ids += 1;
        let id = *self.cx.hologram_ids;
        let room = self.cx.room_index;
        let me = self.body.chunk().pos;
        let holo = if message.is_instruction() {
            OverseerHologram::new_instruction(
                id,
                message,
                importance,
                room,
                self.cx.control_preset,
                tutorial,
                me,
                self.cx.rng,
            )
        } else {
            match message {
                HologramMessage::Bats | HologramMessage::TutorialFood => {
                    log::debug!("new bat hologram");
                    let pointer = PointerKind::Bat {
                        currently_pointing_at_swarm_room: -1,
                    };
                    OverseerHologram::new_door_pointer(
                        id,
                        message,
                        importance,
                        room,
                        pointer,
                        "batSymbol",
                        me,
                    )
                }
                HologramMessage::Shelter => {
                    let pointer = PointerKind::Shelter {
                        forcepoint: -1,
                        showing_a_shelter_door: false,
                        currently_showing_to_shelter: -1,
                    };
                    OverseerHologram::new_door_pointer(
                        id,
                        message,
                        importance,
                        room,
                        pointer,
                        "shelterSymbol",
                        me,
                    )
                }
                HologramMessage::ProgressionDirection => {
                    let symbol = PlayerGuideState::guidance_symbol(self.cx.guide.guide_symbol());
                    let pointer = PointerKind::Direction { cycle: 0 };
                    OverseerHologram::new_door_pointer(
                        id, message, importance, room, pointer, symbol, me,
                    )
                }
                HologramMessage::DangerousCreature => OverseerHologram::new_object_pointer(
                    id,
                    message,
                    importance,
                    room,
                    ObjectPointerKind::Creature {
                        has_warned_counter_visible: 0,
                        has_warned_counter_off_screen: 0,
                        cycle: 0,
                    },
                ),
                HologramMessage::FoodObject => OverseerHologram::new_object_pointer(
                    id,
                    message,
                    importance,
                    room,
                    ObjectPointerKind::Food {
                        has_told_counter: 0,
                        cycle: 0,
                    },
                ),
                HologramMessage::GateScene => {
                    let image = self.room_image();
                    OverseerHologram::new_image(id, importance, room, image, self.cx.rng)
                }
                HologramMessage::Angry => {
                    OverseerHologram::new_angry(id, importance, room, self.cx.rng)
                }
                HologramMessage::ForcedDirection => {
                    let Some(direction) = self
                        .ai
                        .communication
                        .as_ref()
                        .and_then(|c| c.forced_direction_to_give)
                        .and_then(|id| self.forced_direction(id))
                    else {
                        // The game's constructor dereferences a null direction here.
                        log::warn!("forced direction hologram without a direction");
                        return;
                    };
                    let element = match direction.symbol {
                        Some(IggySymbol::Shelter) => "shelterSymbol",
                        Some(IggySymbol::Food) => "foodSymbol",
                        Some(IggySymbol::DynamicDirection) => {
                            PlayerGuideState::guidance_symbol(self.cx.guide.guide_symbol())
                        }
                        Some(IggySymbol::Danger) => "miscDangerSymbol",
                        Some(IggySymbol::Bat) => "batSymbol",
                        Some(IggySymbol::SlugcatFace) | None => "GuidanceSlugcat",
                    };
                    log::info!("forced dir");
                    let pointer = PointerKind::Forced {
                        direction: direction.id,
                        symbol: direction.symbol,
                        cycle: 0,
                    };
                    OverseerHologram::new_door_pointer(
                        id, message, importance, room, pointer, element, me,
                    )
                }
                _ => {
                    log::warn!("no hologram for {message:?}");
                    return;
                }
            }
        };
        self.body.hologram = Some(id);
        self.body.holograms.push(holo);
    }

    /// The `OverseerImage` constructor's pictures for this room (the gate scenes and
    /// the placed scenes), which can also pick the guide's symbol, and the room's first
    /// `ProjectedImagePosition` (every placed object, active or not).
    fn room_image(&mut self) -> ImageHologram {
        use ImageId::{
            Clue1, Clue2, Clue3, Clue4, MoonAndSwarmers, MoonDoubleSize, MoonFullFigure,
            MoonPortrait, ScavAndPearls, ScavSlugcatTrade, Slugcat1, Slugcat2, Slugcat3, Slugcat4,
            Slugcat5, Slugcat6, Slugcat7, Swarmers,
        };
        let mods = self.cx.world.mods();
        let show_pos = self.cx.world.realized(self.cx.room_index).and_then(|r| {
            r.settings
                .placed_objects
                .iter()
                .find(|o| o.object_type(mods) == Some(PlacedObjectType::ProjectedImagePosition))
                .map(|o| Vec2::from(o.pos))
        });
        let mut image = ImageHologram {
            images: Vec::new(),
            show_pos,
            counter: 0,
            curr_image: 0,
            time_on_each_image: 0,
            show_time: 0,
            logged_as_seen: false,
        };
        if self.ai.communication.is_none() {
            return image;
        }
        let world = self.cx.world;
        let name = world
            .abstract_room(self.cx.room_index)
            .map_or("", |r| r.name.as_str());
        let guide = &mut *self.cx.guide;
        let (images, each, show): (&[ImageId], i32, i32) = match name {
            // The Remix gate-stand tutorial's pictures are an MMF branch.
            "GATE_SU_HI" => (
                &[
                    Slugcat1, Slugcat2, Slugcat3, Slugcat4, Slugcat5, Slugcat7, Slugcat6,
                ],
                60,
                900,
            ),
            "GATE_SU_DS" => (&[], 0, 0),
            "GATE_HI_GW" => {
                guide.set_guide_symbol(1);
                (&[MoonFullFigure, MoonDoubleSize, MoonPortrait], 40, 900)
            }
            // `GATE_SL_VS` shows them only with More Slugcats.
            "GATE_GW_SL" | "GATE_SH_SL" => {
                guide.set_guide_symbol(1);
                (
                    &[
                        Clue1, Clue1, Clue1, Clue1, Clue1, Clue2, Clue3, Clue4, Clue4, Slugcat3,
                        Clue4, Slugcat5, Clue4, Slugcat6, Clue4,
                    ],
                    30,
                    1400,
                )
            }
            "GATE_SL_VS" => (&[], 0, 0),
            "SU_A22" => (&[Slugcat1], 0, 90),
            "SU_A37" => (&[Slugcat7, Slugcat5], 30, 120),
            "SU_A33" => (&[Slugcat2, Slugcat5], 20, 40),
            "SU_A30" => (&[Slugcat3, Slugcat6], 40, 60),
            "SU_A12" => (&[Slugcat4], 0, 60),
            "HI_A24" => (&[Slugcat3, MoonFullFigure], 30, 90),
            "HI_D01" => (&[MoonFullFigure, Slugcat5], 40, 80),
            "HI_A21" => (&[Slugcat5, Slugcat2], 30, 80),
            "SL_A08" => (&[MoonFullFigure, MoonDoubleSize, Clue1, Clue4], 20, 120),
            "SL_C11" => (&[MoonPortrait], 0, 10),
            "SL_A03" => (&[MoonFullFigure, Clue4], 20, 40),
            "SL_C09" => (&[MoonFullFigure], 0, 20),
            "SL_I01" => (
                &[
                    MoonFullFigure,
                    MoonDoubleSize,
                    MoonPortrait,
                    Clue4,
                    MoonPortrait,
                    Clue4,
                    MoonPortrait,
                ],
                15,
                110,
            ),
            "SL_A15" => {
                guide.set_guide_symbol(2);
                (
                    &[Swarmers, MoonAndSwarmers, Swarmers, MoonAndSwarmers, Clue2],
                    40,
                    320,
                )
            }
            // Remix's Moon pictures in `SL_AI` are an MMF branch.
            "SL_AI" => (&[], 0, 0),
            "SH_C13" => (&[ScavAndPearls, ScavSlugcatTrade], 25, 150),
            _ => (&[MoonDoubleSize], 0, 0),
        };
        image.images = images.to_vec();
        image.time_on_each_image = each;
        image.show_time = show;
        if guide.guide_symbol() == 0
            && image.images.iter().any(|i| {
                matches!(
                    i,
                    MoonAndSwarmers | MoonDoubleSize | MoonFullFigure | MoonPortrait
                )
            })
        {
            guide.set_guide_symbol(1);
        }
        image
    }
}
