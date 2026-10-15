//! What `RainWorldGame.CommunicateWithUpcomingProcess` reads of the game for the sleep,
//! dream, death and starve screens' data package.

use rw_assets::StreamingAssets;
use rw_hud::{
    GameOverPrompt, Hud, PlayerAt, RealizedAt, ScreenPackageSource, SleepDeathScreenDataPackage,
};
use rw_menu::ProcessId;
use rw_save::PlayerProgression;
use rw_sim::{HEAD, Vec2};

use crate::frontend::AppProcess;
use crate::game::Game;
use crate::hibernation::SessionRecordSource;

impl Game {
    /// The package for screen `next`, from the save state in memory, the world, the
    /// player and camera 0's HUD (its map data and prompt); `None` without a save
    /// state. The prompt's `foodInStomach` is not stored yet, so it is 0.
    #[must_use]
    pub fn screen_package(
        &self,
        next: ProcessId,
        saves: &PlayerProgression,
        assets: &StreamingAssets,
        hud: Option<&Hud>,
    ) -> Option<SleepDeathScreenDataPackage> {
        let save = saves.current_save_state.as_ref()?;
        let sim = self.sim();
        let campaign = sim.campaign();
        let source = ScreenPackageSource {
            next,
            save,
            world: sim.world(),
            assets,
            map_data: hud.map(|h| h.map.data.clone()),
            player: Some(self.screen_player_at()),
            prompt: hud.map(|h| GameOverPrompt {
                death_room: h.text_prompt.death_room,
                death_pos: Vec2::from(h.text_prompt.death_pos),
                food_in_stomach: 0,
            }),
            slugcat: campaign.name,
            max_food: campaign.stats.max_food,
            food_to_hibernate: campaign.stats.food_to_hibernate,
            session_record: sim.session_record(),
            discovery: sim.map_discovery().clone(),
        };
        Some(SleepDeathScreenDataPackage::from_game(source))
    }

    /// The player's abstract room and tile, and its main chunk and room once realized
    /// (no room while in a shortcut).
    fn screen_player_at(&self) -> PlayerAt {
        let sim = self.sim();
        let pos = sim.player_pos();
        let realized = sim.player().map(|p| RealizedAt {
            main_chunk: p.chunks()[HEAD].pos,
            room: (!p.in_shortcut())
                .then(|| sim.world().abstract_room(pos.room))
                .flatten()
                .map(|r| {
                    let size = Vec2::new(r.size.0 as f32, r.size.1 as f32) * 20.0;
                    (pos.room, size)
                }),
        });
        PlayerAt {
            room: pos.room,
            tile: pos.tile,
            realized,
        }
    }
}

impl AppProcess {
    /// The package a game process that just ended hands to screen `next`
    /// (`CommunicateWithUpcomingProcess`): for the sleep, dream, death and starve
    /// screens only.
    #[must_use]
    pub fn screen_package(
        &self,
        next: ProcessId,
        saves: &PlayerProgression,
        assets: &StreamingAssets,
    ) -> Option<SleepDeathScreenDataPackage> {
        let screen = matches!(
            next,
            ProcessId::SleepScreen
                | ProcessId::Dream
                | ProcessId::DeathScreen
                | ProcessId::StarveScreen
        );
        let AppProcess::Game(g) = self else {
            return None;
        };
        if !screen {
            return None;
        }
        g.scene
            .game()
            .screen_package(next, saves, assets, g.scene.hud())
    }
}
