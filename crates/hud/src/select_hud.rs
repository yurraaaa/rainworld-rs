//! The HUD a slugcat select continue page owns (`SlugcatPageContinue` as `IOwnAHUD`,
//! owner `CharacterSelect`): the karma meter, then the food meter, both in the page's
//! second HUD container. The page places and fades them after each update.

use rw_menu::{MenuMic, NodeId, StageTree};
use rw_random::UnityRandom;
use rw_sim::Vec2;

use crate::{FadeCircle, FoodMeter, HudFx, HudSounds, KarmaMeter, KarmaNeighbours};

/// `HUD` with owner `CharacterSelect`.
pub struct SelectHud {
    /// `hudContainers[1]`, where both meters are.
    container: NodeId,
    pub karma_meter: KarmaMeter,
    pub food_meter: FoodMeter,
    pub fade_circles: Vec<FadeCircle>,
    /// `CurrentFood`: the page's save data's food.
    current_food: i32,
}

/// What the page builds its HUD from.
#[derive(Debug, Clone, Copy)]
pub struct SelectHudSetup {
    pub container: NodeId,
    /// The `KarmaMeter`'s `displayKarma` (karma, cap) and `showAsReinforced`.
    pub karma: (i32, i32),
    pub karma_reinforced: bool,
    /// `SlugcatStats.SlugcatFoodMeter`: max food and food to hibernate.
    pub food_meter: (i32, i32),
    pub current_food: i32,
}

impl SelectHud {
    /// The page's `AddPart`s: the karma meter, then the food meter.
    #[must_use]
    pub fn new(setup: SelectHudSetup, stage: &mut StageTree) -> Self {
        let karma_meter = KarmaMeter::new(
            stage,
            setup.container,
            setup.karma.0,
            setup.karma.1,
            setup.karma_reinforced,
        );
        let food_meter = FoodMeter::new_for_screen(
            stage,
            setup.container,
            setup.food_meter.0,
            setup.food_meter.1,
            crate::HudOwnerType::CharacterSelect,
            setup.current_food,
        );
        Self {
            container: setup.container,
            karma_meter,
            food_meter,
            fade_circles: Vec::new(),
            current_food: setup.current_food,
        }
    }

    /// `KarmaMeter.Radius`.
    #[must_use]
    pub fn karma_radius(&self) -> f32 {
        KarmaMeter::radius_of(
            self.karma_meter.rad,
            self.karma_meter.show_as_reinforced,
            self.food_meter.force_sleep,
        )
    }

    /// `HUD.Update`: the parts newest first (the food meter, then the karma meter),
    /// then the fade circles.
    pub fn update(&mut self, stage: &mut StageTree, mic: &mut MenuMic, rng: &mut UnityRandom) {
        let mut fx = HudFx {
            stage,
            container: self.container,
            fade_circles: &mut self.fade_circles,
            sounds: HudSounds::Menu(mic),
            rng,
        };
        self.food_meter
            .update_character_select(self.current_food, self.karma_meter.fade, &mut fx);
        let (karma, cap) = self.karma_meter.display_karma;
        let n = KarmaNeighbours {
            food_down_in_corner: self.food_meter.down_in_corner,
            food_fade: self.food_meter.fade,
            map_fade: 0.0,
            text_food_visible_mode_zero: false,
            text_lower_border: 0.0,
            reveal_map: false,
            show_karma_food_rain: false,
            hide_general_hud: false,
            character_select: true,
            save_karma: (karma, cap),
            save_reinforced: self.karma_meter.show_as_reinforced,
        };
        self.karma_meter.update(&n, &mut fx);
        for i in (0..self.fade_circles.len()).rev() {
            if self.fade_circles[i].life <= 0.0 {
                let gone = self.fade_circles.remove(i);
                stage.remove_from_container(gone.circle.sprite);
            } else {
                self.fade_circles[i].update();
            }
        }
    }

    /// The page's placing after the update: both meters' fade, `initPlopCircle` reset
    /// while hidden, the karma symbol at `karma_pos` and the food meter to its right.
    pub fn place(&mut self, fade: f32, karma_pos: Vec2) {
        self.food_meter.fade = fade;
        if self.food_meter.fade == 0.0 {
            self.food_meter.init_plop_circle = -1;
            self.food_meter.init_plop_delay = 0;
        }
        self.karma_meter.fade = fade;
        self.karma_meter.pos = karma_pos + Vec2::new(0.01, 0.01);
        self.food_meter.pos = karma_pos + Vec2::new(self.karma_radius() + 22.0 + 0.01, 0.01);
    }

    /// `HUD.Draw`: the karma meter, the food meter, then the fade circles last first.
    pub fn draw(&mut self, stage: &mut StageTree, t: f32) {
        let force_sleep = self.food_meter.force_sleep;
        self.karma_meter.draw(stage, self.container, force_sleep, t);
        self.food_meter.draw(stage, 0, t);
        for c in self.fade_circles.iter().rev() {
            c.circle.draw(stage, t);
        }
    }
}

#[cfg(test)]
mod tests;
