//! `PlayerGuideState` (in `miscWorldSaveData`): what the guide overseer knows of the
//! player, kept in the save state across cycles. The save text is `rw-save`'s.

use rw_assets::{AbstractObjectType, CreatureType};

/// `PlayerGuideState`; a new game's values. The type lists keep the `ExtEnum` values as
/// read, so a value no mod registers stays as it is.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerGuideState {
    /// The saved integers behind the properties below.
    pub integers: [i32; 7],
    /// `itemTypes`: the food the player has learnt about.
    pub item_types: Vec<String>,
    /// `creatureTypes`: the creatures the player has been warned of.
    pub creature_types: Vec<String>,
    pub likes_player: f32,
    pub hand_holding: f32,
    /// Rooms whose picture the guide has shown.
    pub images_shown: Vec<String>,
    /// `forcedDirectionsGiven`: room index and how often.
    pub forced_directions_given: Vec<(i32, i32)>,
    /// `increaseLikeOnSave`: the save raises the like (a weapon thrown at the guide
    /// clears it). Not saved, like the per-cycle fields below.
    pub increase_like_on_save: bool,
    pub has_been_to_a_swarm_room_this_cycle: bool,
    pub want_food_hand_holding_this_cycle: f32,
    pub want_shelter_hand_holding_this_cycle: f32,
    pub want_direction_hand_holding_this_cycle: f32,
}

impl Default for PlayerGuideState {
    fn default() -> Self {
        Self {
            integers: [0; 7],
            item_types: Vec::new(),
            creature_types: Vec::new(),
            likes_player: 0.3,
            hand_holding: 0.8,
            images_shown: Vec::new(),
            forced_directions_given: Vec::new(),
            increase_like_on_save: true,
            has_been_to_a_swarm_room_this_cycle: false,
            want_food_hand_holding_this_cycle: 0.65,
            want_shelter_hand_holding_this_cycle: 1.0,
            want_direction_hand_holding_this_cycle: 0.6,
        }
    }
}

impl PlayerGuideState {
    /// `playerHasVisitedMoon` (read by nothing in the game).
    #[must_use]
    pub fn player_has_visited_moon(&self) -> bool {
        self.integers[0] == 1
    }

    #[must_use]
    pub fn super_jumps_shown(&self) -> i32 {
        self.integers[1]
    }

    pub fn set_super_jumps_shown(&mut self, n: i32) {
        self.integers[1] = n;
    }

    #[must_use]
    pub fn pickup_objects_shown(&self) -> i32 {
        self.integers[2]
    }

    pub fn set_pickup_objects_shown(&mut self, n: i32) {
        self.integers[2] = n;
    }

    #[must_use]
    pub fn scav_trade_instruction_completed(&self) -> bool {
        self.integers[3] > 0
    }

    pub fn set_scav_trade_instruction_completed(&mut self, value: bool) {
        self.integers[3] = i32::from(value);
    }

    #[must_use]
    pub fn angry_with_player(&self) -> bool {
        self.integers[4] > 0
    }

    pub fn set_angry_with_player(&mut self, value: bool) {
        self.integers[4] = i32::from(value);
    }

    #[must_use]
    pub fn displayed_anger(&self) -> bool {
        self.integers[5] > 0
    }

    pub fn set_displayed_anger(&mut self, value: bool) {
        self.integers[5] = i32::from(value);
    }

    /// Which guidance symbol the direction pointer shows (1 Moon, 2 neuron).
    #[must_use]
    pub fn guide_symbol(&self) -> i32 {
        self.integers[6]
    }

    pub fn set_guide_symbol(&mut self, symbol: i32) {
        self.integers[6] = symbol;
    }

    /// `InfluenceLike`.
    pub fn influence_like(&mut self, influence: f32) {
        self.likes_player = (self.likes_player + influence).clamp(-1.0, 1.0);
    }

    /// `InfluenceHandHolding`.
    pub fn influence_hand_holding(&mut self, influence: f32) {
        self.hand_holding = (self.hand_holding + influence).clamp(0.0, 1.0);
    }

    /// `ImageShownInRoom`.
    pub fn image_shown_in_room(&mut self, room: &str) {
        if !self.has_image_been_shown_in_room(room) {
            self.images_shown.push(room.to_owned());
        }
    }

    #[must_use]
    pub fn has_image_been_shown_in_room(&self, room: &str) -> bool {
        self.images_shown.iter().any(|r| r == room)
    }

    /// `HowManyTimesHasForcedDirectionBeenGiven`.
    #[must_use]
    pub fn how_many_times_has_forced_direction_been_given(&self, room: i32) -> i32 {
        self.forced_directions_given
            .iter()
            .find(|(r, _)| *r == room)
            .map_or(0, |&(_, n)| n)
    }

    /// `IncrementTimesForcedDirectionHasBeenGiven`.
    pub fn increment_times_forced_direction_has_been_given(&mut self, room: i32) {
        match self
            .forced_directions_given
            .iter_mut()
            .find(|(r, _)| *r == room)
        {
            Some((_, n)) => *n += 1,
            None => self.forced_directions_given.push((room, 1)),
        }
    }

    /// `itemTypes.Contains(type)`.
    #[must_use]
    pub fn knows_item_type(&self, kind: AbstractObjectType) -> bool {
        self.item_types.iter().any(|t| t == kind.value())
    }

    /// `itemTypes.Add(type)`.
    pub fn learn_item_type(&mut self, kind: AbstractObjectType) {
        self.item_types.push(kind.value().to_owned());
    }

    /// `creatureTypes.Contains(type)`.
    #[must_use]
    pub fn knows_creature_type(&self, kind: CreatureType) -> bool {
        self.creature_types.iter().any(|t| t == kind.value())
    }

    /// `OverseerGuidanceSymbol(guideSymbol)` (base game; More Slugcats adds Pebbles and
    /// the energy cell).
    #[must_use]
    pub fn guidance_symbol(selector: i32) -> &'static str {
        match selector {
            1 => "GuidanceMoon",
            2 => "GuidanceNeuron",
            _ => "GuidanceSlugcat",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forced_directions_count_per_room() {
        let mut g = PlayerGuideState::default();
        assert_eq!(g.how_many_times_has_forced_direction_been_given(7), 0);
        g.increment_times_forced_direction_has_been_given(7);
        g.increment_times_forced_direction_has_been_given(3);
        g.increment_times_forced_direction_has_been_given(7);
        assert_eq!(g.forced_directions_given, [(7, 2), (3, 1)]);
        assert_eq!(g.how_many_times_has_forced_direction_been_given(7), 2);
    }

    #[test]
    fn properties_read_the_integers() {
        let mut g = PlayerGuideState {
            integers: [1, 4, 1, 2, 3, 0, 2],
            ..PlayerGuideState::default()
        };
        assert!(g.player_has_visited_moon());
        assert_eq!((g.super_jumps_shown(), g.pickup_objects_shown()), (4, 1));
        assert!(g.scav_trade_instruction_completed() && g.angry_with_player());
        assert!(!g.displayed_anger());
        assert_eq!(g.guide_symbol(), 2);
        g.set_angry_with_player(false);
        g.set_displayed_anger(true);
        assert_eq!(g.integers, [1, 4, 1, 2, 0, 1, 2]);
        g.item_types = vec!["DangleFruit".to_owned()];
        assert!(g.knows_item_type(AbstractObjectType::DangleFruit));
        assert!(!g.knows_item_type(AbstractObjectType::SeedCob));
    }
}
