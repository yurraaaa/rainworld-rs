//! The HUD a sleep or death screen owns (`HUD.InitSleepHud`): a food meter and the
//! region map, drawn into the screen's own stage and sounding through its menu
//! microphone. The screen (`SleepAndDeathScreen`) is not here; it builds this with
//! its HUD containers and feeds it a [`ScreenTick`] each tick.

use rw_menu::{AtlasManager, MenuMic, NodeId, StageTree};
use rw_random::UnityRandom;
use rw_sim::{FloatExt, MapDiscovery, Vec2};

use crate::{
    FadeCircle, FoodMeter, HudFx, HudOwnerType, HudSounds, Map, MapData, MapOwner, ScreenTick,
};

/// `HUD` with owner `SleepScreen` or `DeathScreen`.
pub struct SleepHud {
    owner: HudOwnerType,
    /// `fContainers`: the map's, then the food meter's (the screen's second and first
    /// HUD `MenuContainer`).
    containers: [NodeId; 2],
    screen_size: [f32; 2],
    pub food_meter: FoodMeter,
    /// `None` without map data.
    pub map: Option<Map>,
    /// The karma ladder adds its fade circles here too.
    pub fade_circles: Vec<FadeCircle>,
}

/// What `InitSleepHud` reads.
pub struct SleepHudSetup {
    /// `SleepScreen` or `DeathScreen`.
    pub owner: HudOwnerType,
    /// `fContainers`, as the screen passes them: its second HUD container, then its
    /// first.
    pub containers: [NodeId; 2],
    pub screen_size: [f32; 2],
    /// The character's `maxFood` and `foodToHibernate`.
    pub max_food: i32,
    pub food_to_hibernate: i32,
    /// The screen's `CurrentFood` (the package's food).
    pub current_food: i32,
    pub map: Option<MapData>,
}

impl SleepHud {
    /// `InitSleepHud`: the food meter, then the map (its in-front container goes on
    /// top of `stage`), then the meter at `FoodMeterXPos` and y 0.
    #[must_use]
    pub fn new(setup: SleepHudSetup, stage: &mut StageTree, atlases: &mut AtlasManager) -> Self {
        let mut food_meter = FoodMeter::new_for_screen(
            stage,
            setup.containers[1],
            setup.max_food,
            setup.food_to_hibernate,
            setup.owner,
            setup.current_food,
        );
        let map = setup.map.map(|data| {
            let in_front = stage.add_container(StageTree::ROOT);
            Map::new(
                data,
                setup.owner,
                stage,
                setup.containers[0],
                in_front,
                atlases,
                setup.screen_size,
                None,
            )
        });
        let x = Self::food_meter_x_for(setup.screen_size);
        food_meter.pos = Vec2::new(x, 0.0);
        food_meter.last_pos = food_meter.pos;
        Self {
            owner: setup.owner,
            containers: setup.containers,
            screen_size: setup.screen_size,
            food_meter,
            map,
            fade_circles: Vec::new(),
        }
    }

    #[must_use]
    pub fn owner(&self) -> HudOwnerType {
        self.owner
    }

    /// `SleepAndDeathScreen.FoodMeterXPos` without the Watcher's warp map (the
    /// argument is unused then).
    #[must_use]
    pub fn food_meter_x(&self) -> f32 {
        Self::food_meter_x_for(self.screen_size)
    }

    fn food_meter_x_for(screen_size: [f32; 2]) -> f32 {
        screen_size[0].lerp_map(1024.0, 1366.0, screen_size[0] / 2.0 - 110.0, 540.0)
    }

    /// `HUD.Update`: the parts newest first (the map, then the food meter), then the
    /// fade circles. `reveal_map` is the screen's `RevealMap`; `map` its map owner
    /// fields (`MapDiscoveryActive` is false). Returns whether the food meter called
    /// `FoodCountDownDone`, which the screen then runs.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        stage: &mut StageTree,
        screen: &ScreenTick,
        reveal_map: bool,
        map: &MapOwner,
        mic: &mut MenuMic,
        rng: &mut UnityRandom,
        discovery: &mut MapDiscovery,
    ) -> bool {
        if let Some(m) = &mut self.map {
            m.update(map, reveal_map, false, stage, discovery, rng);
        }
        let mut fx = HudFx {
            stage,
            container: self.containers[1],
            fade_circles: &mut self.fade_circles,
            sounds: HudSounds::Menu(mic),
            rng,
        };
        let done = self.food_meter.update_screen(screen, &mut fx);
        for i in (0..self.fade_circles.len()).rev() {
            if self.fade_circles[i].life <= 0.0 {
                let gone = self.fade_circles.remove(i);
                stage.remove_from_container(gone.circle.sprite);
            } else {
                self.fade_circles[i].update();
            }
        }
        done
    }

    /// `HUD.Draw`: the food meter, the map, then the fade circles last first.
    pub fn draw(&mut self, stage: &mut StageTree, t: f32) {
        self.food_meter.draw(stage, 0, t);
        if let Some(m) = &mut self.map {
            m.draw(stage, t);
        }
        for c in self.fade_circles.iter().rev() {
            c.circle.draw(stage, t);
        }
    }

    /// Hands the map's reveal texture to `atlases` (see [`crate::Hud::sync_textures`]).
    pub fn sync_textures(&mut self, atlases: &mut AtlasManager) {
        if let Some(m) = &mut self.map {
            m.sync_textures(atlases);
        }
    }

    /// The map shaders' globals, if there is a map.
    #[must_use]
    pub fn map_globals(&self, t: f32) -> Option<rw_render::MapGlobals> {
        self.map.as_ref().map(|m| m.globals(t))
    }
}

#[cfg(test)]
mod tests;
