//! `KarmaLadderScreen.SleepDeathScreenDataPackage` and how `RainWorldGame.
//! CommunicateWithUpcomingProcess` fills it for the sleep, dream, death and starve
//! screens (Remix, More Slugcats and Watcher branches left out).
//!
//! Mirrors: Menu/KarmaLadderScreen.cs (SleepDeathScreenDataPackage), RainWorldGame.cs (CommunicateWithUpcomingProcess)

use rw_assets::{SlugcatName, StreamingAssets};
use rw_menu::ProcessId;
use rw_save::{PlayerSessionRecord, SaveState};
use rw_sim::world::World;
use rw_sim::{MapDiscovery, Vec2};

use crate::{CustomInt, MapData};

/// The game's handover to a karma ladder screen.
#[derive(Debug, Clone)]
pub struct SleepDeathScreenDataPackage {
    /// The save state's food on the sleep and dream screens, the prompt's
    /// `foodInStomach` on the death and starve screens.
    pub food: i32,
    /// Karma and karma cap.
    pub karma: (i32, i32),
    pub karma_reinforced: bool,
    pub player_room: i32,
    /// The constructor's `playerPos.y = Max(playerPos.y, -50)` assigns its parameter,
    /// so the position is kept as given.
    pub player_pos: Vec2,
    /// `cameras[0].hud.map.mapData` after `UpdateData`; `None` without a HUD.
    pub map_data: Option<MapData>,
    /// `characterStats`: its name, `maxFood` and `foodToHibernate`.
    pub slugcat: SlugcatName,
    pub max_food: i32,
    pub food_to_hibernate: i32,
    /// `playerSessionRecords[0]`.
    pub session_record: PlayerSessionRecord,
    /// `saveState.lastMalnourished` and `malnourished`.
    pub start_malnourished: bool,
    pub goal_malnourished: bool,
    /// Not in the game's package: the map's discovery textures, which the game keeps
    /// in the progression and the port in the sim (D16).
    pub discovery: MapDiscovery,
}

/// Where the first alive (else any) player is: its abstract room and tile, and its
/// realized main chunk and room.
#[derive(Debug, Clone, Copy)]
pub struct PlayerAt {
    pub room: i32,
    pub tile: (i32, i32),
    pub realized: Option<RealizedAt>,
}

/// A realized player's `mainBodyChunk.pos` and, while in a room, the room's index and
/// pixel size (`RoomRect`).
#[derive(Debug, Clone, Copy)]
pub struct RealizedAt {
    pub main_chunk: Vec2,
    pub room: Option<(i32, Vec2)>,
}

/// What the text prompt's `EnterGameOverMode` stored (all 0 when it was not entered,
/// as after a starve).
#[derive(Debug, Clone, Copy, Default)]
pub struct GameOverPrompt {
    pub death_room: i32,
    pub death_pos: Vec2,
    pub food_in_stomach: i32,
}

/// What the game process hands over (`CommunicateWithUpcomingProcess`).
pub struct ScreenPackageSource<'a> {
    /// The screen being switched to: `SleepScreen`, `Dream`, `DeathScreen` or
    /// `StarveScreen`.
    pub next: ProcessId,
    /// `GetStorySession.saveState` (after `Win` or `SessionEnded`).
    pub save: &'a SaveState,
    pub world: &'a World,
    /// To load the map image again for the screen's map.
    pub assets: &'a StreamingAssets,
    /// `cameras[0].hud.map.mapData`, if the camera has a HUD.
    pub map_data: Option<MapData>,
    /// `FirstAlivePlayer ?? FirstAnyPlayer`.
    pub player: Option<PlayerAt>,
    /// `cameras[0].hud.textPrompt`; `None` without a HUD.
    pub prompt: Option<GameOverPrompt>,
    pub slugcat: SlugcatName,
    pub max_food: i32,
    pub food_to_hibernate: i32,
    pub session_record: PlayerSessionRecord,
    pub discovery: MapDiscovery,
}

impl SleepDeathScreenDataPackage {
    /// `CommunicateWithUpcomingProcess` for a karma ladder or dream screen: the map
    /// updated with the next cycle's karma (one lower on the death screen unless
    /// reinforced), `1 + foodReplenishBonus` food ticks and the save's flower; the
    /// player's room and position (the prompt's death room and position on the death
    /// screen; kept within 50 px of the room it is realized in). The map image, which
    /// the game's map took, is loaded again for the screen's map.
    #[must_use]
    pub fn from_game(src: ScreenPackageSource<'_>) -> Self {
        let dp = &src.save.death_persistent;
        let karma = dp.karma;
        let death_screen = src.next == ProcessId::DeathScreen;
        let map_karma = if death_screen && !dp.reinforced_karma {
            (karma - 1).int_clamp(0, dp.karma_cap)
        } else {
            karma
        };
        let mut map_data = src.map_data;
        if let Some(map) = &mut map_data {
            let flower = dp.karma_flower_coordinate();
            map.update_data(
                src.world,
                1 + dp.food_replenish_bonus,
                map_karma,
                flower.as_ref(),
            );
            map.reload_image(src.assets, src.slugcat.value());
        }
        let death_prompt = src.prompt.filter(|_| death_screen);
        let (player_room, player_pos) = src
            .player
            .map_or((-1, Vec2::ZERO), |p| Self::player_place(p, death_prompt));
        let food = if matches!(src.next, ProcessId::SleepScreen | ProcessId::Dream) {
            src.save.food
        } else {
            src.prompt.map_or(0, |p| p.food_in_stomach)
        };
        Self {
            food,
            karma: (karma, dp.karma_cap),
            karma_reinforced: dp.reinforced_karma,
            player_room,
            player_pos,
            map_data,
            slugcat: src.slugcat,
            max_food: src.max_food,
            food_to_hibernate: src.food_to_hibernate,
            session_record: src.session_record,
            start_malnourished: src.save.last_malnourished,
            goal_malnourished: src.save.malnourished,
            discovery: src.discovery,
        }
    }

    /// The player's room and position; `death` is the prompt on the death screen.
    fn player_place(p: PlayerAt, death: Option<GameOverPrompt>) -> (i32, Vec2) {
        let mut room = p.room;
        let mut pos = Vec2::new(p.tile.0 as f32, p.tile.1 as f32) * 20.0;
        if let Some(d) = death {
            room = d.death_room;
            pos = d.death_pos;
        } else if let Some(r) = p.realized {
            pos = r.main_chunk;
        }
        if let Some((index, size)) = p.realized.and_then(|r| r.room)
            && index == room
        {
            pos = Vec2::new(
                pos.x.clamp(-50.0, size.x + 50.0),
                pos.y.clamp(-50.0, size.y + 50.0),
            );
        }
        (room, pos)
    }
}
