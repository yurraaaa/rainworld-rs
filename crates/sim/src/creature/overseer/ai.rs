//! `OverseerAI`'s state and its zip pathing matrix. The update itself is in
//! [`super::update`], where the body, the AI and the room meet.

use rw_assets::{CreatureType, ShowProjectedImageEvent};
use rw_random::UnityRandom;

use super::hologram::HologramMessage;
use super::{PlayerGuideState, TutorialBehavior};
use crate::Vec2;
use crate::creature::{CreatureId, EntityId};
use crate::item::{Entity, ObjectId};
use crate::trigger::InstructionViewKind;
use crate::world::World;

/// `zipPathingMatrix` and its offset: path costs from the centre tile through walls,
/// `f32::MAX` where not reached yet.
#[derive(Debug, Clone, PartialEq)]
pub struct ZipMatrix {
    width: i32,
    height: i32,
    cells: Vec<f32>,
    /// `zipPathMatrixOffset`.
    offset: (i32, i32),
}

impl ZipMatrix {
    /// `ResetZipPathingMatrix`'s grid: a 61-tile square around `center` cut at the room
    /// edges; its last row and column are left out (the game sizes it `max(1, x1 - x0)`).
    #[must_use]
    pub fn new(center: (i32, i32), room_size: (i32, i32)) -> Self {
        let x0 = (center.0 - 30).max(0);
        let y0 = (center.1 - 30).max(0);
        let x1 = (room_size.0 - 1).min(center.0 + 30);
        let y1 = (room_size.1 - 1).min(center.1 + 30);
        let (width, height) = ((x1 - x0).max(1), (y1 - y0).max(1));
        Self {
            width,
            height,
            cells: vec![f32::MAX; usize::try_from(width * height).unwrap_or(0)],
            offset: (-x0, -y0),
        }
    }

    fn index(&self, (x, y): (i32, i32), room_size: (i32, i32)) -> Option<usize> {
        if x < 0 || x >= room_size.0 || y < 0 || y >= room_size.1 {
            return None;
        }
        let (mx, my) = (x + self.offset.0, y + self.offset.1);
        if mx < 0 || mx >= self.width || my < 0 || my >= self.height {
            return None;
        }
        usize::try_from(mx * self.height + my).ok()
    }

    /// `GetZipPathMatrixCell`: -1 outside the room or the grid.
    #[must_use]
    pub fn get(&self, tile: (i32, i32), room_size: (i32, i32)) -> f32 {
        self.index(tile, room_size).map_or(-1.0, |i| self.cells[i])
    }

    /// `SetZipPathMatrixCell` (ignored outside).
    pub fn set(&mut self, tile: (i32, i32), value: f32, room_size: (i32, i32)) {
        if let Some(i) = self.index(tile, room_size) {
            self.cells[i] = value;
        }
    }
}

/// `OverseerCommunicationModule.PlayerConcern` (base game).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayerConcern {
    #[default]
    None,
    Bats,
    Shelter,
    DangerousCreature,
    Progression,
    ShowGateScene,
    ShowPlacedScene,
    ForcedDirection,
    FoodItemInRoom,
    InputInstruction,
    Anger,
}

/// The player guide's `OverseerCommunicationModule`: what the guide thinks the player
/// needs, and the holograms that answer it.
#[derive(Debug, Clone, PartialEq)]
pub struct CommunicationModule {
    pub current_concern: PlayerConcern,
    pub current_concern_weight: f32,
    pub player_room: i32,
    pub last_player_room: i32,
    pub general_hand_holding: f32,
    pub progression_show_tendency: f32,
    /// Cycles 1 to 3.
    pub first_few_cycles: bool,
    pub showed_image_time: i32,
    pub has_already_showed_player_to_a_shelter: bool,
    pub most_dangerous_creature_danger: f32,
    pub most_delicous_food_delicious: f32,
    pub no_bat_direction_from_room: i32,
    pub no_shelter_direction_from_room: i32,
    pub no_progression_direction_from_room: i32,
    pub follow_given_direction_streak: i32,
    pub free_movement_streak: i32,
    pub un_wanted_swarm_room: i32,
    pub un_wanted_shelter: i32,
    /// `imageToShow`, set by a `ShowProjectedImageEvent` trigger.
    pub image_to_show: Option<ShowProjectedImageEvent>,
    pub show_image_room: i32,
    /// `inputInstruction`: the id of the input instruction room object.
    pub input_instruction: Option<u64>,
    /// `forcedDirectionToGive`: the id of a `ReliableIggyDirection` room object.
    pub forced_direction_to_give: Option<u64>,
    /// `objectsAlreadyTalkedAbout`: creatures warned of and food shown.
    pub objects_already_talked_about: Vec<EntityId>,
    pub most_dangerous_creature_in_room: Option<CreatureId>,
    pub most_delicious_food_in_room: Option<Entity>,
    /// `holoLight`: the id of the `HologramLight` it projects.
    pub holo_light: Option<u64>,
}

impl CommunicationModule {
    #[must_use]
    pub fn new(cycle: i32) -> Self {
        Self {
            current_concern: PlayerConcern::None,
            current_concern_weight: 0.0,
            player_room: -1,
            last_player_room: -1,
            general_hand_holding: 0.5,
            progression_show_tendency: 0.0,
            first_few_cycles: cycle > 0 && cycle < 4,
            showed_image_time: 0,
            has_already_showed_player_to_a_shelter: false,
            most_dangerous_creature_danger: 0.0,
            most_delicous_food_delicious: 0.0,
            no_bat_direction_from_room: -1,
            no_shelter_direction_from_room: -1,
            no_progression_direction_from_room: -1,
            follow_given_direction_streak: 0,
            free_movement_streak: 0,
            un_wanted_swarm_room: -1,
            un_wanted_shelter: -1,
            image_to_show: None,
            show_image_room: -1,
            input_instruction: None,
            forced_direction_to_give: None,
            objects_already_talked_about: Vec::new(),
            most_dangerous_creature_in_room: None,
            most_delicious_food_in_room: None,
            holo_light: None,
        }
    }

    /// `PlayerHasNowBeenWarnedOfCreature`: remembers the creature (at most 21), and its
    /// type when it is now handled.
    pub fn player_has_now_been_warned_of_creature(
        &mut self,
        id: EntityId,
        kind: CreatureType,
        creature_type_now_handled: bool,
        guide: &mut PlayerGuideState,
    ) {
        log::info!("creature talked about: {kind:?} {creature_type_now_handled}");
        if creature_type_now_handled && !guide.knows_creature_type(kind) {
            guide.creature_types.push(kind.value().to_owned());
        }
        if self.objects_already_talked_about.contains(&id) {
            return;
        }
        if self.objects_already_talked_about.len() > 20 {
            self.objects_already_talked_about.remove(0);
        }
        self.objects_already_talked_about.push(id);
    }

    /// `InWorldInputInstructionCompleted`: counts the current instruction's kind.
    pub fn in_world_input_instruction_completed(
        &self,
        kind: Option<InstructionViewKind>,
        guide: &mut PlayerGuideState,
    ) {
        log::info!("input instruction completed");
        match kind {
            Some(InstructionViewKind::SuperJump { .. }) => {
                guide.set_super_jumps_shown(guide.super_jumps_shown() + 1);
            }
            Some(InstructionViewKind::Pickup { .. }) => {
                guide.set_pickup_objects_shown(guide.pickup_objects_shown() + 1);
            }
            Some(InstructionViewKind::ScavengerTrade { .. }) => {
                guide.set_scav_trade_instruction_completed(true);
            }
            None => {}
        }
    }

    /// `AnyProgressionDirection(player)`: `player_room` is the realized player's room
    /// (`None` in a shortcut).
    #[must_use]
    pub fn any_progression_direction(
        &self,
        world: &World,
        player_room: Option<i32>,
        player_karma: i32,
    ) -> bool {
        if self.first_few_cycles {
            return false;
        }
        let Some(finder) = world.guide_direction_finder() else {
            return false;
        };
        if player_room.is_some_and(|r| {
            r == self.no_progression_direction_from_room || r == finder.show_to_room
        }) {
            return false;
        }
        if player_karma < finder.min_karma {
            return !finder.destination_room_visited;
        }
        true
    }

    /// `PlayerIsFollowingADirection`.
    pub fn player_is_following_a_direction(
        &mut self,
        message: HologramMessage,
        guide: &mut PlayerGuideState,
    ) {
        self.follow_given_direction_streak += 1;
        self.free_movement_streak = 0;
        log::info!("player IS following a direction");
        if self.follow_given_direction_streak > 2 {
            guide.influence_hand_holding(0.1);
        }
        let want = match message {
            HologramMessage::Bats => &mut guide.want_food_hand_holding_this_cycle,
            HologramMessage::Shelter => &mut guide.want_shelter_hand_holding_this_cycle,
            HologramMessage::ProgressionDirection => {
                &mut guide.want_direction_hand_holding_this_cycle
            }
            _ => return,
        };
        *want = (*want + 0.1).clamp(0.0, 1.0);
    }

    /// `PlayerNOTfollowingADirection`.
    pub fn player_not_following_a_direction(
        &mut self,
        message: HologramMessage,
        explicitly_visible: bool,
        guide: &mut PlayerGuideState,
    ) {
        self.free_movement_streak += 1;
        self.follow_given_direction_streak = 0;
        log::info!("player is NOT following a direction");
        guide.influence_hand_holding(if explicitly_visible { -0.05 } else { -0.025 });
        let want = match message {
            HologramMessage::Bats => &mut guide.want_food_hand_holding_this_cycle,
            HologramMessage::Shelter => &mut guide.want_shelter_hand_holding_this_cycle,
            HologramMessage::ProgressionDirection => {
                &mut guide.want_direction_hand_holding_this_cycle
            }
            _ => return,
        };
        let less = if explicitly_visible { 0.1 } else { 0.05 };
        *want = (*want - less).clamp(0.0, 1.0);
    }
}

/// `OverseerAI`.
#[derive(Debug, Clone, PartialEq)]
pub struct OverseerAi {
    pub look_at: Vec2,
    pub look_at_add: Vec2,
    pub slow_look_at: Vec2,
    pub last_look_at_air_positions: Vec<Vec2>,
    pub look_at_same_air_pos_counter: i32,
    pub casual_interest_creature: Option<CreatureId>,
    pub casual_interest_bonus: f32,
    pub temp_hover_tile: (i32, i32),
    pub target_stationary: f32,
    /// `zipPathingMatrix`, `None` until first reset.
    pub zip_matrix: Option<ZipMatrix>,
    pub add_to_pathing_list: Vec<(i32, i32)>,
    pub avoid_positions: Vec<(i32, i32)>,
    pub last_target_pos: Vec2,
    pub bring_up_lens: f32,
    pub random_bring_up_lens_bonus: f32,
    pub tutorial: Option<TutorialBehavior>,
    /// The player guide's.
    pub communication: Option<CommunicationModule>,
    /// `ArtificialIntelligence.timeInRoom` and `lastRoom`.
    pub time_in_room: i32,
    pub last_room: i32,
    /// `lookAtFlyingWeapon`.
    pub look_at_flying_weapon: Option<ObjectId>,
}

impl OverseerAi {
    /// The constructor: `randomBringUpLensBonus` (one draw); the guide gets its
    /// communication module.
    pub fn new(guide: bool, cycle: i32, rng: &mut UnityRandom) -> Self {
        Self {
            look_at: Vec2::ZERO,
            look_at_add: Vec2::ZERO,
            slow_look_at: Vec2::ZERO,
            last_look_at_air_positions: Vec::new(),
            look_at_same_air_pos_counter: 0,
            casual_interest_creature: None,
            casual_interest_bonus: 0.0,
            temp_hover_tile: (16, 21),
            target_stationary: 0.0,
            zip_matrix: None,
            add_to_pathing_list: Vec::new(),
            avoid_positions: Vec::new(),
            last_target_pos: Vec2::ZERO,
            bring_up_lens: 0.0,
            random_bring_up_lens_bonus: rng.value(),
            tutorial: None,
            communication: guide.then(|| CommunicationModule::new(cycle)),
            time_in_room: 0,
            last_room: -1,
            look_at_flying_weapon: None,
        }
    }

    /// `CosmeticLookAt`, given the body's position.
    #[must_use]
    pub fn cosmetic_look_at(&self, body_pos: Vec2) -> Vec2 {
        self.slow_look_at + self.look_at_add * (body_pos.distance(self.look_at) * 0.2)
    }

    /// `ResetZipPathingMatrix(center)`.
    pub fn reset_zip_matrix(&mut self, center: (i32, i32), room_size: (i32, i32)) {
        let mut matrix = ZipMatrix::new(center, room_size);
        matrix.set(center, 0.0, room_size);
        self.zip_matrix = Some(matrix);
        self.add_to_pathing_list = vec![center];
    }

    /// Queues `tile` (cost `total`) in `addToPathingList`, kept in cost order.
    pub(crate) fn insert_by_cost(&mut self, tile: (i32, i32), total: f32, room_size: (i32, i32)) {
        let list = &self.add_to_pathing_list;
        let last = list.last().map(|&l| self.zip_cell(l, room_size));
        match last {
            Some(l) if total <= l => {
                let mut i = list.len() - 1;
                while i > 0 && total < self.zip_cell(list[i - 1], room_size) {
                    i -= 1;
                }
                self.add_to_pathing_list.insert(i, tile);
            }
            _ => self.add_to_pathing_list.push(tile),
        }
    }

    /// `GetZipPathMatrixCell` (-1 before any reset).
    #[must_use]
    pub fn zip_cell(&self, tile: (i32, i32), room_size: (i32, i32)) -> f32 {
        self.zip_matrix
            .as_ref()
            .map_or(-1.0, |m| m.get(tile, room_size))
    }
}
