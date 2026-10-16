//! A saved region state applied to the installed game's SU world (read-only): the
//! `RegionState` constructor, `AdaptWorldToRegionState`, `GeneratePopulation(false)` and
//! `AdaptRegionStateToWorld`, with expectations from reading the decompiled game. Set
//! `RW_DIR` to the game install directory; skipped if absent.

use std::path::PathBuf;
use std::sync::Arc;

use rw_assets::creatures::CreatureSpawner;
use rw_assets::world::{GameRooms, LineFilter, Regions, StoryConditions, WorldData, WorldRequest};
use rw_assets::{ActiveMods, CreatureType, StreamingAssets};
use rw_random::UnityRandom;
use rw_sim::RainWorldGame;
use rw_sim::campaign::{SlugcatCampaign, StoryStart};
use rw_sim::creature::{CreatureTemplate, EntityId};
use rw_sim::world::World;

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

/// A Survivor's SU state as cycle 2 left it: an unknown (More Slugcats) creature, a rock
/// in shelter SU_S04, a dangle fruit (kept as text), two spears stuck in SU_A13's walls
/// for 2 and 1 more cycles and a rock there, a stick, an unknown entry, one swarm room
/// depleted for 4 more cycles.
const SAVED: &str = "REGIONNAME<rgB>SU<rgA>LASTCYCLEUPDATED<rgB>2<rgA>\
    SWARMROOMS<rgB>SU_A24:4,SU_A40:0<rgA>LINEAGES<rgB>SU_A30.-1.-1.2:1<rgA>\
    OBJECTS<rgB>ID.-239.10152<oA>Rock<oA>SU_S04.22.24.0<rgC>\
    ID.-1.300<oB>0<oA>DangleFruit<oA>SU_S04.3.3.0<oA>0<oA>2<rgC>\
    ID.-12.10200<oB>0<oA>Spear<oA>SU_A13.30.20.-1<oA>2<oA>0<rgC>\
    ID.-12.10201<oB>0<oA>Spear<oA>SU_A13.31.20.-1<oA>-1<oA>0<rgC>\
    ID.-12.10202<oB>0<oA>Rock<oA>SU_A13.5.5.-1<rgC><rgA>\
    POPULATION<rgB>GreenLizard<cA>ID.5028.9955<cA>SU_B12.2<cA>\
    Social<cC>REL<rA>ID.-1.0<rA>L<rB>0.4<smA>REL<rA>ID.-1.5<rA>L<rB>0.2<cB><rgC>\
    PinkLizard<cA>ID.5014.9942<cA>SU_A20.6<cA>SpawnData<cC>{X}<cB><cB>Health<cC>0.5<rgC>\
    SlugNPC<cA>ID.-1.77<cA>SU_S01.0<cA><rgC><rgA>\
    STICKS<rgB>254<stkA>gripStk<stkA>ID.-1.1<stkA>ID.-1.2<stkA>0<stkA>1<rgC><rgA>\
    CONSUMEDITEMS<rgB>SU_A02.3.5<rgA>ROOMSVISITED<rgB>SU_C04,SU_A41,SU_S01<rgA>\
    MODKEY<rgB>x<rgA>";

fn world() -> Option<World> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let sa = root.join("RainWorld_Data").join("StreamingAssets");
    if !sa.join("world").join("regions.txt").is_file() {
        eprintln!("skipping: no world under {}", sa.display());
        return None;
    }
    let assets = StreamingAssets::new(sa);
    let campaign = &SlugcatCampaign::SURVIVOR;
    let filter = LineFilter {
        timeline: Some(campaign.timeline),
        mods: ActiveMods::NONE,
        conditions: StoryConditions::new_story(),
    };
    let regions = Regions::load(&assets).expect("regions");
    let request = WorldRequest::for_first_room(&assets, "SU_S01", filter);
    let data = WorldData::load(&assets, &regions, &request).expect("world loads");
    let rooms = Arc::new(GameRooms {
        assets,
        region: Some("su".to_owned()),
        timeline: campaign.timeline,
    });
    Some(World::new(
        data,
        rooms,
        campaign.timeline,
        ActiveMods::NONE,
        true,
    ))
}

#[test]
fn continue_restores_the_saved_population_and_keeps_the_rest() {
    let Some(mut world) = world() else { return };
    // A simple spawner whose creature is put up for respawn.
    let (index, kind) = world
        .data()
        .spawners
        .iter()
        .enumerate()
        .find_map(|(i, s)| match s {
            CreatureSpawner::Simple(s)
                if !CreatureTemplate::of(s.creature, ActiveMods::NONE).quantified
                    && CreatureTemplate::of(s.creature, ActiveMods::NONE).save_creature =>
            {
                Some((i, s.creature))
            }
            _ => None,
        })
        .expect("a simple spawner");
    let region = world.data().region.as_ref().unwrap().region_number;
    let respawner = region * 1000 + i32::try_from(index).unwrap();
    let campaign = &SlugcatCampaign::SURVIVOR;
    let story = StoryStart {
        den: "SU_S01".to_owned(),
        cycle_number: 3,
        region_state: Some(SAVED.to_owned()),
        respawn_creatures: vec![respawner],
        next_issued_id: Some(20000),
        ..StoryStart::new_game(campaign)
    };
    let mut rng = UnityRandom::new(42);
    let new = RainWorldGame::story_world(&mut world, campaign, &story, &mut rng);
    // The respawn was taken.
    assert!(new.respawn_creatures.is_empty());

    let state = world.region_state.as_ref().unwrap();
    assert_eq!(state.rooms_visited, ["SU_C04", "SU_A41", "SU_S01"]);
    // `RainCycleTick(3 - 2, 0)`: the depleted room counts down, the item waits less.
    assert_eq!(state.swarm_room_counters[0], ("SU_A24".to_owned(), 3));
    assert_eq!(state.consumed_items[0].wait_cycles, 4);
    assert_eq!(state.last_cycle_updated, 3);
    assert_eq!(state.unrecognized_population.len(), 1);
    // `AdaptWorldToRegionState` placed the rocks and spears; `RainCycleTick(1)` freed
    // the spear stuck for 1 more cycle.
    let objects: Vec<_> = world
        .objects()
        .map(|(_, o)| {
            let room = world.abstract_room(o.pos.room).unwrap().name.clone();
            (o.id.number, room, o.spear().map(|s| s.stuck_in_wall_cycles))
        })
        .collect();
    assert_eq!(
        objects,
        [
            (10152, "SU_S04".to_owned(), None),
            (10200, "SU_A13".to_owned(), Some(1)),
            (10201, "SU_A13".to_owned(), Some(0)),
            (10202, "SU_A13".to_owned(), None),
        ]
    );
    assert_eq!(state.unrecognized_saved_objects.len(), 1);

    for (id, kind) in [
        (EntityId::new(5028, 9955), CreatureType::GreenLizard),
        (EntityId::new(5014, 9942), CreatureType::PinkLizard),
    ] {
        let found = world
            .creatures()
            .find(|(_, c)| c.id == id)
            .map(|(_, c)| (c.kind(), c.in_den, c.state.alive));
        assert_eq!(found, Some((kind, true, true)));
    }
    let respawned: Vec<_> = world
        .creatures()
        .filter(|(_, c)| c.id.spawner == respawner)
        .map(|(_, c)| c.kind())
        .collect();
    assert_eq!(respawned, [kind]);

    let shelter = world.room_index("SU_S01").unwrap();
    let mut respawn = Vec::new();
    let saved = world
        .adapt_region_state_to_world(shelter, &mut respawn)
        .unwrap();
    assert!(respawn.is_empty());
    assert!(saved.starts_with(
        "REGIONNAME<rgB>SU<rgA>LASTCYCLEUPDATED<rgB>3<rgA>SWARMROOMS<rgB>SU_A24:3,SU_A40:0,"
    ));
    assert!(saved.contains("LINEAGES<rgB>SU_A30.-1.-1.2:1,"));
    // The text kept first, then room by room (SU_S04 comes before SU_A13): the shelter's
    // rock (written with its ripple layer) and the spear still stuck; the free spear and
    // the rock outside a shelter are not saved.
    assert!(saved.contains(
        "OBJECTS<rgB>ID.-1.300<oB>0<oA>DangleFruit<oA>SU_S04.3.3.0<oA>0<oA>2<rgC>\
         ID.-239.10152<oB>0<oA>Rock<oA>SU_S04.22.24.0<rgC>\
         ID.-12.10200<oB>0<oA>Spear<oA>SU_A13.30.20.-1<oA>1<oA>0<rgC><rgA>"
    ));
    // Rewritten with the ID's ripple layer; the states after one `CycleTick`: the like
    // of 0.4 faded x0.97, the one of 0.2 forgotten; health up a quarter.
    assert!(saved.contains(
        "<rgC>GreenLizard<cA>ID.5028.9955<cB>0<cA>SU_B12.2<cA>\
         Social<cC>REL<rA>ID.-1.0<rA>L<rB>0.388<cB><rgC>"
    ));
    assert!(saved.contains(
        "PinkLizard<cA>ID.5014.9942<cB>0<cA>SU_A20.6<cA>SpawnData<cC>{X}<cB><cB>Health<cC>0.75<rgC>"
    ));
    // `AdaptRegionStateToWorld` puts the unknown creatures first.
    assert!(saved.contains("<rgA>POPULATION<rgB>SlugNPC<cA>ID.-1.77<cA>SU_S01.0<cA><rgC>"));
    // The rest as read; REGIONNAME, which the constructor does not read, once more.
    assert!(saved.ends_with(
        "<cA><rgC><rgA>STICKS<rgB>254<stkA>gripStk<stkA>ID.-1.1<stkA>ID.-1.2<stkA>0<stkA>1<rgC><rgA>\
         CONSUMEDITEMS<rgB>SU_A02.3.4<rgA>ROOMSVISITED<rgB>SU_C04,SU_A41,SU_S01<rgA>\
         REGIONNAME<rgB>SU<rgA>MODKEY<rgB>x<rgA>"
    ));
}

#[test]
fn new_game_region_state_saves_its_fresh_population() {
    let Some(mut world) = world() else { return };
    let campaign = &SlugcatCampaign::SURVIVOR;
    let mut rng = UnityRandom::new(42);
    RainWorldGame::new_game_world(&mut world, campaign, &mut rng);
    let shelter = world.room_index("SU_S01").unwrap();
    let mut respawn = Vec::new();
    let saved = world
        .adapt_region_state_to_world(shelter, &mut respawn)
        .unwrap();
    // Every swarm room active, lineages at their first stage or past a NONE.
    assert!(saved.starts_with("REGIONNAME<rgB>SU<rgA>LASTCYCLEUPDATED<rgB>0<rgA>SWARMROOMS<rgB>"));
    assert!(saved.contains("<rgA>LINEAGES<rgB>"));
    // Overseers (not saved) and flies (quantified) are not in the population.
    assert!(!saved.contains("Overseer<cA>"));
    assert!(!saved.contains("Fly<cA>"));
    assert!(saved.contains("<rgA>POPULATION<rgB>"));
    assert!(saved.ends_with("ROOMSVISITED<rgB><rgA>"));
}
