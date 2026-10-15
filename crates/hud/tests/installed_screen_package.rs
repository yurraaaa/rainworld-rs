//! `RainWorldGame.CommunicateWithUpcomingProcess` (58013-58140) and
//! `MapData.UpdateData` (478115) on the installed game's SU world (read-only). Set
//! `RW_DIR` to override the default install; tests skip when it is absent.

use std::path::PathBuf;
use std::sync::Arc;

use rw_assets::world::{GameRooms, LineFilter, Regions, StoryConditions, WorldData, WorldRequest};
use rw_assets::{ActiveMods, SlugcatName, StreamingAssets};
use rw_hud::{
    GameOverPrompt, MapData, PlayerAt, RealizedAt, ScreenPackageSource, SleepDeathScreenDataPackage,
};
use rw_menu::ProcessId;
use rw_save::{PlayerSessionRecord, SaveState};
use rw_sim::campaign::SlugcatCampaign;
use rw_sim::world::World;
use rw_sim::{MapDiscovery, Vec2};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

fn assets() -> Option<StreamingAssets> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let sa = root.join("RainWorld_Data").join("StreamingAssets");
    if !sa.join("world/regions.txt").is_file() {
        eprintln!("skipping: no game under {}", root.display());
        return None;
    }
    Some(StreamingAssets::new(sa))
}

/// A new Survivor game's SU world.
fn su_world(assets: &StreamingAssets) -> World {
    let campaign = &SlugcatCampaign::SURVIVOR;
    let filter = LineFilter {
        timeline: Some(campaign.timeline),
        mods: ActiveMods::NONE,
        conditions: StoryConditions::new_story(),
    };
    let regions = Regions::load(assets).unwrap();
    let request = WorldRequest::for_first_room(assets, "SU_C04", filter);
    let data = WorldData::load(assets, &regions, &request).unwrap();
    let rooms = Arc::new(GameRooms {
        assets: assets.clone(),
        region: Some(request.name.to_lowercase()),
        timeline: campaign.timeline,
    });
    World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true)
}

struct Fixture {
    assets: StreamingAssets,
    world: World,
    save: SaveState,
}

impl Fixture {
    fn new() -> Option<Self> {
        let assets = assets()?;
        let world = su_world(&assets);
        let mut save = SaveState::new(SlugcatName::White, 0);
        save.food = 2;
        save.death_persistent.karma = 2;
        save.death_persistent.karma_cap = 5;
        Some(Self {
            assets,
            world,
            save,
        })
    }

    fn package(&self, next: ProcessId, player: PlayerAt) -> SleepDeathScreenDataPackage {
        let map = MapData::build(&self.world, &self.assets, "White", 0, None);
        SleepDeathScreenDataPackage::from_game(ScreenPackageSource {
            next,
            save: &self.save,
            world: &self.world,
            assets: &self.assets,
            map_data: map,
            player: Some(player),
            prompt: Some(GameOverPrompt {
                death_room: 7,
                death_pos: Vec2::new(-300.0, 40.0),
                food_in_stomach: 3,
            }),
            slugcat: SlugcatName::White,
            max_food: 7,
            food_to_hibernate: 4,
            session_record: PlayerSessionRecord::default(),
            discovery: MapDiscovery::default(),
        })
    }

    fn room(&self, name: &str) -> i32 {
        self.world.room_index(name).unwrap()
    }
}

#[test]
fn map_karma_food_and_player_place_per_screen() {
    let Some(mut fx) = Fixture::new() else { return };
    let c04 = fx.room("SU_C04");
    let size = fx.world.abstract_room(c04).unwrap().size;
    let room_px = Vec2::new(size.0 as f32 * 20.0, size.1 as f32 * 20.0);
    let player = PlayerAt {
        room: c04,
        tile: (3, 4),
        realized: Some(RealizedAt {
            main_chunk: Vec2::new(room_px.x + 80.0, 30.0),
            room: Some((c04, room_px)),
        }),
    };
    // Sleep: the save's food and karma; the realized main chunk, kept 50 px from the
    // room.
    let p = fx.package(ProcessId::SleepScreen, player);
    assert_eq!((p.food, p.karma, p.player_room), (2, (2, 5), c04));
    assert_eq!(p.player_pos, Vec2::new(room_px.x + 50.0, 30.0));
    assert_eq!(p.map_data.as_ref().unwrap().current_karma, 2);
    assert!(p.map_data.as_ref().unwrap().image.is_some());
    // Death: the prompt's food, room and position (not in the realized room, so not
    // restricted); the map shows the karma one lower.
    let p = fx.package(ProcessId::DeathScreen, player);
    assert_eq!((p.food, p.player_room), (3, 7));
    assert_eq!(p.player_pos, Vec2::new(-300.0, 40.0));
    assert_eq!(p.map_data.as_ref().unwrap().current_karma, 1);
    // Reinforced: the flower keeps it.
    fx.save.death_persistent.reinforced_karma = true;
    let p = fx.package(ProcessId::DeathScreen, player);
    assert_eq!(p.map_data.as_ref().unwrap().current_karma, 2);
    assert!(p.karma_reinforced);
    // Starve: the prompt's food (0 when game over was never entered), the player's
    // abstract tile when not realized.
    let p = fx.package(
        ProcessId::StarveScreen,
        PlayerAt {
            realized: None,
            ..player
        },
    );
    assert_eq!(p.food, 3);
    assert_eq!(p.player_pos, Vec2::new(60.0, 80.0));
    // The save's malnourished flags.
    fx.save.last_malnourished = true;
    let p = fx.package(ProcessId::SleepScreen, player);
    assert!(p.start_malnourished && !p.goal_malnourished);
}

#[test]
fn update_data_places_the_flower_and_counts_swarm_rooms() {
    let Some(mut fx) = Fixture::new() else { return };
    let player = PlayerAt {
        room: fx.room("SU_C04"),
        tile: (0, 0),
        realized: None,
    };
    // A flower in this region is placed by its room's index; one elsewhere is not.
    fx.save.death_persistent.karma_flower_position = Some("SU_A41.12.5.-1".to_owned());
    let p = fx.package(ProcessId::DeathScreen, player);
    let flower = p.map_data.unwrap().karma_flower_pos.unwrap();
    assert_eq!((flower.room, flower.tile), (fx.room("SU_A41"), (12, 5)));
    fx.save.death_persistent.karma_flower_position = Some("HI_A01.1.1.-1".to_owned());
    let p = fx.package(ProcessId::DeathScreen, player);
    assert!(p.map_data.unwrap().karma_flower_pos.is_none());
    // Swarm rooms: active when the counter is within `1 + foodReplenishBonus`.
    let swarm = fx.world.data().swarm_rooms.clone();
    assert!(swarm.len() >= 2, "SU has swarm rooms");
    let names: Vec<String> = swarm
        .iter()
        .map(|&r| fx.world.abstract_room(r).unwrap().name.clone())
        .collect();
    let state = fx.world.region_state.as_mut().unwrap();
    state.set_swarm_room_counter(&names[0], 1);
    state.set_swarm_room_counter(&names[1], 2);
    let p = fx.package(ProcessId::SleepScreen, player);
    let rooms = p.map_data.unwrap().swarm_rooms;
    assert!(rooms[0].active);
    assert!(!rooms[1].active);
    fx.save.death_persistent.food_replenish_bonus = 1;
    let p = fx.package(ProcessId::SleepScreen, player);
    assert!(p.map_data.unwrap().swarm_rooms[1].active);
}
