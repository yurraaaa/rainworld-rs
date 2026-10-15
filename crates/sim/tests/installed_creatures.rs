//! A new story game's world load (population, overseers, `FliesWorldAI`) and the
//! overseers' abstract movement, in the installed game's worlds (read-only). Set `RW_DIR`
//! to the game install directory; skipped if absent.
//!
//! The expected values come from an independent model of the decompiled game (a Python
//! UnityRandom and a step-by-step reading of `RainWorldGame`'s constructor,
//! `SaveState`, `World`, `WaterLevelCycle`, `WorldLoader.GeneratePopulation`,
//! `OverseerSpawnConditions`, `FliesWorldAI`, `AbstractRoom.Update`,
//! `AbstractCreature.InDenUpdate` and `OverseerAbstractAI.AbstractBehavior`/`Roam`), not
//! from the port. Seed 42 throughout.

use std::path::PathBuf;
use std::sync::Arc;

use rw_assets::world::{GameRooms, LineFilter, Regions, StoryConditions, WorldData, WorldRequest};
use rw_assets::{ActiveMods, CreatureType, StreamingAssets};
use rw_random::UnityRandom;
use rw_sim::campaign::SlugcatCampaign;
use rw_sim::creature::{CreatureId, EntityId, PlayerGuideState};
use rw_sim::world::{AbstractTick, RainCycle, World};
use rw_sim::{RainWorldGame, Sounds, Start, WorldCoordinate};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";
const SEED: i32 = 42;

struct Installed(StreamingAssets);

/// What the model expects of one campaign's world load.
struct Expected {
    room: &'static str,
    campaign: &'static SlugcatCampaign,
    next_issued_id: i32,
    cycle_length: i32,
    sun_down: i32,
    guide_counter: Option<i32>,
    region_overseers: usize,
    state: [u32; 4],
}

/// One overseer's room change: tick, overseer (spawn order), from, to.
type Move<'a> = (u32, usize, &'a str, &'a str);

impl Installed {
    fn open() -> Option<Self> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let sa = root.join("RainWorld_Data").join("StreamingAssets");
        if sa.join("world").join("regions.txt").is_file() {
            Some(Self(StreamingAssets::new(sa)))
        } else {
            eprintln!("skipping: no world under {}", sa.display());
            None
        }
    }

    /// The region world of `room` as `campaign` loads it, before the population.
    fn world(&self, room: &str, campaign: &'static SlugcatCampaign) -> World {
        let filter = LineFilter {
            timeline: Some(campaign.timeline),
            mods: ActiveMods::NONE,
            conditions: StoryConditions::new_story(),
        };
        let regions = Regions::load(&self.0).expect("regions");
        let request = WorldRequest::for_first_room(&self.0, room, filter);
        let data = WorldData::load(&self.0, &regions, &request).expect("world loads");
        let rooms = Arc::new(GameRooms {
            assets: self.0.clone(),
            region: Some(request.name.to_lowercase()),
            timeline: campaign.timeline,
        });
        World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true)
    }
}

impl Expected {
    const ALL: [Self; 3] = [
        Self {
            room: "SU_C04",
            campaign: &SlugcatCampaign::SURVIVOR,
            next_issued_id: 6731,
            cycle_length: 26821,
            sun_down: 27549,
            guide_counter: Some(313),
            region_overseers: 0,
            state: [3_832_289_982, 3_255_041_036, 2_108_798_842, 3_730_246_898],
        },
        Self {
            room: "SU_C04",
            campaign: &SlugcatCampaign::MONK,
            next_issued_id: 6722,
            cycle_length: 28104,
            sun_down: 28788,
            guide_counter: Some(399),
            region_overseers: 0,
            state: [613_541_491, 3_707_364_485, 2_024_634_953, 3_684_096_284],
        },
        Self {
            room: "LF_H01",
            campaign: &SlugcatCampaign::HUNTER,
            next_issued_id: 6794,
            cycle_length: 26821,
            sun_down: 27549,
            guide_counter: None,
            region_overseers: 0,
            state: [3_932_484_434, 3_991_853_763, 3_277_511_479, 2_118_726_954],
        },
    ];

    fn check(&self, installed: &Installed) {
        let name = self.campaign.name;
        let mut world = installed.world(self.room, self.campaign);
        let mut rng = UnityRandom::new(SEED);
        let new = RainWorldGame::new_game_world(&mut world, self.campaign, &mut rng);
        assert_eq!(
            rng.state(),
            self.state,
            "{name:?}: state after FliesWorldAI"
        );
        assert_eq!(new.ids.next_issued_id, self.next_issued_id, "{name:?}");
        assert_eq!(new.save_state_seed, 9962, "{name:?}");
        assert_eq!(world.rain_cycle.cycle_length, self.cycle_length, "{name:?}");
        assert_eq!(
            world.rain_cycle.sun_down_start_time, self.sun_down,
            "{name:?}"
        );
        assert_eq!(world.rain_cycle.rainbow_seed, 3394, "{name:?}");

        let guide = world
            .creatures()
            .find(|(_, c)| c.id == EntityId::new(-1, 5));
        match (self.guide_counter, guide) {
            (Some(counter), Some((id, c))) => {
                let room = world.room_index("SU_C04").expect("SU_C04");
                assert_eq!(c.pos, WorldCoordinate::new(room, 137, 17, 0));
                assert_eq!(c.kind(), CreatureType::OVERSEER);
                assert!(!c.in_den && world.room_creatures(room).contains(&id));
                let ai = c.overseer_ai().expect("overseer AI");
                assert!(ai.is_player_guide);
                assert_eq!(ai.player_guide_counter, counter, "{name:?}");
            }
            (None, None) => {}
            (want, got) => panic!("{name:?}: guide {want:?}, found {:?}", got.map(|g| g.1.pos)),
        }
        let den = world.offscreen_den_index();
        let region_overseers = world
            .room_dens(den)
            .iter()
            .filter(|&&c| world.creature(c).kind() == CreatureType::OVERSEER)
            .count();
        assert_eq!(region_overseers, self.region_overseers, "{name:?}");
    }
}

#[test]
fn story_world_load_draws_match_the_model() {
    let Some(installed) = Installed::open() else {
        return;
    };
    // Common to all three: nextIssuedId, SaveState.seed, then the World constructor's
    // six draws (cycle length, sundown, rainbow seed, WaterLevelCycle's three).
    for campaign in [&SlugcatCampaign::SURVIVOR, &SlugcatCampaign::HUNTER] {
        let mut rng = UnityRandom::new(SEED);
        assert_eq!(rng.range_i32(1000, 10000), 6683);
        assert_eq!(
            rng.state(),
            [3_107_752_595, 1_225_807_616, 4_076_950_273, 4_076_870_683]
        );
        assert_eq!(rng.range_i32(0, 10000), 9962);
        RainCycle::new_story(campaign.name, &mut rng);
        assert_eq!(
            rng.state(),
            [1_864_723_394, 1_792_162_932, 3_150_331_535, 2_503_639_016]
        );
    }
    for expected in &Expected::ALL {
        expected.check(&installed);
    }
}

/// The overseers after `rounds` rounds of `RainWorldGame.Update`'s abstract room rotation
/// (`updateAbstractRoom`, time `NumberOfRooms`), run after a new Survivor game's world
/// load with no player and the rain timer at 0 (no room is realized, the rain cycle
/// draws nothing).
struct AbstractRun {
    world: World,
    rng: UnityRandom,
    moves: Vec<(u32, usize, String, String)>,
}

/// An overseer's state at the end: room, destination room, `lastRooms`,
/// `playerGuideCounter`.
type Final<'a> = (&'a str, &'a str, &'a [&'a str], i32);

impl AbstractRun {
    /// Every overseer in spawn order: the guide (if any), then the region overseers.
    fn overseers(world: &World) -> Vec<CreatureId> {
        world
            .creatures()
            .filter(|(_, c)| c.kind() == CreatureType::OVERSEER)
            .map(|(id, _)| id)
            .collect()
    }

    fn new(installed: &Installed, room: &str, rounds: u32) -> Self {
        let campaign = &SlugcatCampaign::SURVIVOR;
        let mut world = installed.world(room, campaign);
        // The model takes `lastRoom`'s default (global room 0) to be outside the world.
        assert_ne!(world.first_room_index(), 0);
        let mut rng = UnityRandom::new(SEED);
        RainWorldGame::new_game_world(&mut world, campaign, &mut rng);
        let ids = Self::overseers(&world);
        let guide = PlayerGuideState::default();
        let count = world.number_of_rooms();
        let time = i32::try_from(count).expect("room count");
        let mut index = 0;
        let mut moves = Vec::new();
        for tick in 1..=rounds * u32::try_from(count).expect("room count") {
            index += 1;
            if index >= count {
                index = 0;
            }
            let before: Vec<i32> = ids.iter().map(|&c| world.creature(c).pos.room).collect();
            let mut cx = AbstractTick {
                rng: &mut rng,
                guide: &guide,
                player: None,
                player_karma: 0,
                cycle: 0,
            };
            let target = world.first_room_index() + i32::try_from(index).expect("index");
            world.update_abstract_room(target, time, &mut cx);
            for (n, &c) in ids.iter().enumerate() {
                let now = world.creature(c).pos.room;
                if now != before[n] {
                    moves.push((
                        tick,
                        n,
                        Self::name(&world, before[n]),
                        Self::name(&world, now),
                    ));
                }
            }
        }
        Self { world, rng, moves }
    }

    fn name(world: &World, room: i32) -> String {
        world.abstract_room(room).expect("room").name.clone()
    }

    fn assert_moves(&self, expected: &[Move<'_>]) {
        let got: Vec<Move<'_>> = self
            .moves
            .iter()
            .map(|(t, n, a, b)| (*t, *n, a.as_str(), b.as_str()))
            .collect();
        assert_eq!(got, expected);
    }

    fn assert_finals(&self, expected: &[Final<'_>]) {
        let ids = Self::overseers(&self.world);
        assert_eq!(ids.len(), expected.len());
        for (&id, &(room, dest, last, counter)) in ids.iter().zip(expected) {
            let c = self.world.creature(id);
            assert_eq!(Self::name(&self.world, c.pos.room), room);
            let destination = c.ai.as_ref().expect("AI").destination.room;
            assert_eq!(Self::name(&self.world, destination), dest);
            let ai = c.overseer_ai().expect("overseer AI");
            let rooms: Vec<String> = ai
                .last_rooms
                .iter()
                .map(|&r| Self::name(&self.world, r))
                .collect();
            assert_eq!(rooms, last);
            assert_eq!(ai.player_guide_counter, counter);
        }
    }
}

#[test]
fn su_guide_roams_as_the_model() {
    let Some(installed) = Installed::open() else {
        return;
    };
    let run = AbstractRun::new(&installed, "SU_C04", 200);
    assert_eq!(run.world.number_of_rooms(), 55);
    run.assert_moves(&[
        (627, 0, "SU_C04", "SU_A41"),
        (1673, 0, "SU_A41", "SU_A44"),
        (3326, 0, "SU_A44", "SU_A41"),
        (6458, 0, "SU_A41", "SU_A43"),
        (6955, 0, "SU_A43", "SU_A22"),
        (7666, 0, "SU_A22", "SU_A23"),
        (8380, 0, "SU_A23", "SU_A24"),
        (8709, 0, "SU_A24", "SU_A25"),
        (8763, 0, "SU_A25", "SU_A38"),
        (9862, 0, "SU_A38", "SU_A53"),
        (10025, 0, "SU_A53", "SU_A39"),
        (10741, 0, "SU_A39", "SU_A37"),
    ]);
    run.assert_finals(&[(
        "SU_A37",
        "SU_A37",
        &[
            "SU_A37", "SU_A39", "SU_A53", "SU_A38", "SU_A25", "SU_A24", "SU_A23", "SU_A22",
            "SU_A43", "SU_A41",
        ],
        313,
    )]);
    assert_eq!(
        run.rng.state(),
        [308_215_501, 286_354_149, 3_885_124_770, 2_032_524_891]
    );
}

#[test]
fn uw_region_overseers_roam_as_the_model() {
    let Some(installed) = Installed::open() else {
        return;
    };
    let run = AbstractRun::new(&installed, "UW_A01", 200);
    assert_eq!(run.world.number_of_rooms(), 51);
    run.assert_moves(&[
        (254, 1, "OffScreenDen_UW", "UW_A13"),
        (520, 1, "UW_A13", "UW_E01"),
        (917, 3, "OffScreenDen_UW", "UW_A01"),
        (1631, 0, "OffScreenDen_UW", "UW_D01"),
        (1733, 2, "OffScreenDen_UW", "UW_F01"),
        (1899, 1, "UW_E01", "UW_A02"),
        (2056, 0, "UW_D01", "UW_D07"),
        (2508, 2, "UW_F01", "UW_A12"),
        (2711, 2, "UW_A12", "UW_A14"),
        (2761, 2, "UW_A14", "UW_A12"),
        (3119, 2, "UW_A12", "UW_H01"),
        (3168, 2, "UW_H01", "UW_A12"),
        (3177, 3, "UW_A01", "UW_J01"),
        (3230, 0, "UW_D07", "UW_C05"),
        (3323, 2, "UW_A12", "UW_F01"),
        (3946, 0, "UW_C05", "UW_D06"),
        (4094, 3, "UW_J01", "UW_A02"),
        (4508, 0, "UW_D06", "UW_C04"),
        (4701, 2, "UW_F01", "UW_A13"),
        (4816, 0, "UW_C04", "UW_D05"),
        (5470, 1, "UW_A02", "UW_J01"),
        (5722, 2, "UW_A13", "UW_E01"),
        (5939, 0, "UW_D05", "UW_A04"),
        (6501, 0, "UW_A04", "UW_C02"),
        (6655, 0, "UW_C02", "UW_A05"),
        (6962, 0, "UW_A05", "UW_C01"),
        (7728, 0, "UW_C01", "UW_A03"),
        (7968, 2, "UW_E01", "UW_A02"),
        (8632, 2, "UW_A02", "UW_J01"),
        (8647, 0, "UW_A03", "UW_J02"),
        (8852, 0, "UW_J02", "UW_A06"),
        (8939, 1, "UW_J01", "UW_A01"),
        (9092, 2, "UW_J01", "UW_A01"),
        (9144, 1, "UW_A01", "UW_D01"),
        (9604, 1, "UW_D01", "UW_D07"),
        (9809, 1, "UW_D07", "UW_C05"),
        (10015, 1, "UW_C05", "UW_D06"),
        (10132, 0, "UW_A06", "UW_E04"),
    ]);
    let den = "OffScreenDen_UW";
    run.assert_finals(&[
        (
            "UW_E04",
            "UW_E04",
            &[
                "UW_E04", "UW_A06", "UW_J02", "UW_A03", "UW_C01", "UW_A05", "UW_C02", "UW_A04",
                "UW_D05", "UW_C04",
            ],
            493,
        ),
        (
            "UW_D06",
            "UW_D06",
            &[
                "UW_D06", "UW_C05", "UW_D07", "UW_D01", "UW_A01", "UW_J01", "UW_A02", "UW_E01",
                "UW_A13", den,
            ],
            221,
        ),
        (
            "UW_A01",
            "UW_A01",
            &[
                "UW_A01", "UW_J01", "UW_A02", "UW_E01", "UW_A13", "UW_F01", "UW_A12", "UW_H01",
                "UW_A12", "UW_A14",
            ],
            209,
        ),
        (
            "UW_A02",
            "UW_A02",
            &["UW_A02", "UW_J01", "UW_A01", den, den],
            458,
        ),
    ]);
    assert_eq!(
        run.rng.state(),
        [998_820_865, 3_135_890_852, 4_140_135_978, 2_205_092_606]
    );
}

/// `Overseer.PlaceInRoom` when SU_C04 loads at the start: the guide is realized on an
/// open tile next to a solid one (`HardSetTile(tile, tile + eightDirectionsAndZero[j])`).
#[test]
fn guide_is_realized_in_the_start_room() {
    let Some(installed) = Installed::open() else {
        return;
    };
    let campaign = &SlugcatCampaign::SURVIVOR;
    let world = installed.world("SU_C04", campaign);
    let room = world.room_index("SU_C04").expect("SU_C04");
    let start = Start { room, tile: None };
    let game = RainWorldGame::new(
        world,
        campaign,
        start,
        UnityRandom::new(SEED),
        Sounds::default(),
        &mut (),
    );
    let world = game.world();
    let (_, guide) = world
        .creatures()
        .find(|(_, c)| c.id == EntityId::new(-1, 5))
        .expect("guide");
    let body = guide.overseer_body().expect("realized");
    assert!((body.size - 0.6).abs() < f32::EPSILON);
    let tiles = &world.realized(room).expect("realized room").room;
    let (hover, root) = (body.hover_tile, body.root_tile);
    assert!(!tiles.tile(hover.0, hover.1).is_solid());
    assert!(tiles.tile(root.0, root.1).is_solid());
    assert!((hover.0 - root.0).abs() <= 1 && (hover.1 - root.1).abs() <= 1 && hover != root);
}
