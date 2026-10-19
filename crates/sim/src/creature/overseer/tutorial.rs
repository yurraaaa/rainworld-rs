//! `OverseerTutorialBehavior`'s state: the guide's lessons in Survivor's and Monk's
//! first cycle in Outskirts. Its update is [`super::OverseerUpdate::tutorial_update`].
//!
//! Mirrors: OverseerTutorialBehavior.cs

/// `OverseerTutorialBehavior`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TutorialBehavior {
    pub encounter_counter: i32,
    pub player_has_jumped_over_box: bool,
    pub player_has_approached_box: bool,
    pub box_trouble: i32,
    pub player_has_climbed_pole: bool,
    pub player_has_made_super_jump: bool,
    pub super_jump_trouble: i32,
    pub pole_trouble: i32,
    pub bat_trouble: i32,
    pub fruit_trouble: i32,
    pub last_food_in_stomach: i32,
    pub messages_displayed: [bool; 7],
    pub bring_in_rain: bool,
    pub pause_rain: bool,
    pub display_run_message: bool,
    pub bats_in_bat_room: i32,
    pub available_fruit_in_fruit_room: i32,
    pub super_jump_attempt_time: i32,
}

impl Default for TutorialBehavior {
    fn default() -> Self {
        Self {
            encounter_counter: 40,
            player_has_jumped_over_box: false,
            player_has_approached_box: false,
            box_trouble: 0,
            player_has_climbed_pole: false,
            player_has_made_super_jump: false,
            super_jump_trouble: 0,
            pole_trouble: 0,
            bat_trouble: 0,
            fruit_trouble: 0,
            last_food_in_stomach: 0,
            messages_displayed: [false; 7],
            bring_in_rain: false,
            pause_rain: true,
            display_run_message: false,
            bats_in_bat_room: 5,
            available_fruit_in_fruit_room: 3,
            super_jump_attempt_time: 0,
        }
    }
}

/// `TutorialText`: a line for the HUD's text prompt (`AddMessage(Translate(text),
/// wait, time, darken: true, hideHud)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TutorialMessage {
    /// The English text; the HUD translates it.
    pub text: &'static str,
    pub wait: i32,
    pub time: i32,
    pub hide_hud: bool,
}

impl TutorialBehavior {
    /// `GetUpOnFirstBox.InZone`.
    #[must_use]
    pub fn in_box_zone(x: f32) -> bool {
        x > 1680.0 && x < 1836.0
    }

    /// `InfluenceHoverScoreOfTile`: the guide keeps right of x = 120 while it waits.
    #[must_use]
    pub fn influence_hover_score(&self, tile: (i32, i32), score: f32) -> f32 {
        if self.encounter_counter > 0 && tile.0 < 120 {
            f32::MAX
        } else {
            score
        }
    }

    /// `EatInstructionRelevant` in room `room` (by name) with `food` pips and `edibles`
    /// `IPlayerEdible`s in the room: SU_A42 until the stomach holds enough to hibernate,
    /// SU_A44 until it is full; both only after the player's trouble.
    #[must_use]
    pub fn eat_instruction_relevant(
        &self,
        room: &str,
        food: i32,
        food_to_hibernate: i32,
        max_food: i32,
        edibles: usize,
    ) -> bool {
        let a44 = room == "SU_A44";
        if a44 {
            if food >= max_food || self.fruit_trouble < 200 {
                return false;
            }
        } else if room != "SU_A42" || food >= food_to_hibernate || self.bat_trouble < 100 {
            return false;
        }
        edibles >= if a44 { 2 } else { 1 }
    }
}
