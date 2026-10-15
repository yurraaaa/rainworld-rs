//! `OverseerAI`'s state and its zip pathing matrix. The update itself is in
//! [`super::update`], where the body, the AI and the room meet.

use rw_random::UnityRandom;

use super::TutorialBehavior;
use crate::Vec2;
use crate::creature::CreatureId;

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
/// needs. The holograms it would show are overseer step 3.
#[derive(Debug, Clone, Copy, PartialEq)]
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
        }
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
