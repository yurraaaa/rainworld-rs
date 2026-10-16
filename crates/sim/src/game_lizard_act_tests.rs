//! One green lizard moving in a small room. Expected values from `Lizard.Act`,
//! `FollowConnection`, `MovementAnimation`, `GetFrameSpeed`, `Creature.Update`'s
//! shortcut entry, `ShortcutHandler` and `AImapper` (`Lizard.cs`, `Creature.cs`,
//! `ShortcutHandler.cs`, `AImapper.cs`).

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, CreatureType, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::creature::lizard::{AnimationOutcome, LizardEmotion, LizardVoice, MovementAnimation};
use crate::creature::{CreatureId, CreatureTemplate, WorldCoordinate};
use crate::tests::room;
use crate::world::{AiMap, AiMapper, MovementConnection, MovementType, World};
use crate::{EmitterId, Input, RainWorldGame, SoundEvent, Sounds, Start, Vec2};

/// A Survivor story in a room of `rows`, the player at `player`.
fn game(rows: &[&str], player: (i32, i32)) -> RainWorldGame {
    game_with_sounds(rows, player, Sounds::default())
}

/// As [`game`], with the green lizard's voice in the sound table.
fn game_with_voice(rows: &[&str], player: (i32, i32)) -> RainWorldGame {
    let text =
        "Volume: 6\nVolume exponent: 1.8\nLizard_Voice_Green_A : LizardVoiceGreenA/vol=0.3\n";
    let names = rw_assets::sound_table::ClipNames::new(["lizardvoicegreena".to_owned()], None);
    let table = Arc::new(rw_assets::SoundTable::parse(text, &names).unwrap());
    game_with_sounds(rows, player, Sounds::new(table))
}

fn game_with_sounds(rows: &[&str], player: (i32, i32), sounds: Sounds) -> RainWorldGame {
    let room = room(rows);
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let campaign = &SlugcatCampaign::SURVIVOR;
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let mut g = RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some(player),
        },
        UnityRandom::new(5),
        sounds,
        &mut (),
    );
    for _ in 0..20 {
        g.tick(Input::NONE, &mut ());
    }
    g
}

/// A 30 x 6 room with a floor at y 1.
const HALL: [&str; 6] = [
    "##############################",
    "#............................#",
    "#............................#",
    "#............................#",
    "#............................#",
    "##############################",
];

/// Two halves split by a wall, joined by a shortcut under the floor between the
/// entrances at (2, 2) and (21, 2).
const SPLIT: [&str; 9] = [
    "########################",
    "#...........#..........#",
    "#...........#..........#",
    "#...........#..........#",
    "#...........#..........#",
    "#...........#..........#",
    "##E##################E##",
    "##xxxxxxxxxxxxxxxxxxxx##",
    "########################",
];

impl RainWorldGame {
    /// A realized green lizard placed at `tile` of room 0 (`PlaceInRoom`), its body
    /// laid along the floor facing `facing`, then left to map its room.
    fn act_lizard(&mut self, tile: (i32, i32), facing: f32) -> CreatureId {
        let session = self.story_session();
        let template = CreatureTemplate::of(CreatureType::GreenLizard, ActiveMods::NONE);
        let id = self.ids.new_id(-1);
        let at = WorldCoordinate::new(0, tile.0, tile.1, -1);
        let c = self
            .world
            .new_creature(template, at, id, &session, &mut self.ids, &mut self.rng);
        let l = self.world.insert_creature(c);
        self.world.add_entity(l);
        assert!(self.world.realize_lizard(l, &mut self.rng));
        let mut cx = crate::world::LizardRoomCx {
            rng: &mut self.rng,
            observer: &mut (),
            camera_room: -1,
        };
        self.world.place_lizard(l, 0, &mut cx);
        let mid = Vec2::new(tile.0 as f32 * 20.0 + 10.0, tile.1 as f32 * 20.0 + 9.6);
        for (i, c) in self.world.lizard_at(l).body.chunks.iter_mut().enumerate() {
            c.hard_set_position(mid + Vec2::new(facing * 18.7 * (1.0 - i as f32), 0.0));
            c.vel = Vec2::ZERO;
        }
        for _ in 0..40 {
            self.tick(Input::NONE, &mut ());
        }
        l
    }

    /// One tick; the `Stop` events it made.
    fn tick_stops(&mut self) -> Vec<EmitterId> {
        self.sounds.take_events();
        self.tick(Input::NONE, &mut ());
        self.sounds
            .take_events()
            .into_iter()
            .filter_map(|e| match e {
                SoundEvent::Stop(id) => Some(id),
                _ => None,
            })
            .collect()
    }

    fn voice(&self, l: CreatureId) -> &LizardVoice {
        self.world.lizard_ref(l).voice.as_ref().expect("voice")
    }

    fn head_tile(&self, l: CreatureId) -> (i32, i32) {
        self.world.lizard_ref(l).head().pos.tile()
    }
}

#[test]
fn a_lizard_walks_to_its_destination() {
    let mut g = game(&HALL, (2, 1));
    let l = g.act_lizard((8, 1), 1.0);
    assert!(g.dev_set_lizard_destination(l, WorldCoordinate::new(0, 24, 1, -1), Some(0.5)));
    let mut arrived = None;
    for t in 0..400 {
        g.tick(Input::NONE, &mut ());
        if (g.head_tile(l).0 - 24).abs() <= 1 {
            arrived = Some(t);
            break;
        }
    }
    assert!(arrived.is_some(), "head at {:?}", g.head_tile(l));
    let lizard = g.world.lizard_ref(l);
    assert!(!lizard.apply_gravity, "on its feet");
    assert_eq!(lizard.legs_gripping(), 2);
    let pf = g.world.path_finder(l).expect("path finder");
    assert_eq!(pf.get_destination().tile, (24, 1));
}

#[test]
fn a_lizard_facing_away_from_its_path_turns() {
    // `FollowPath` makes a Standard move against the body's direction on a floor
    // Unwanted, so the turn (allowed) is taken: a `LizardTurn` movement animation.
    let mut g = game(&HALL, (26, 1));
    let l = g.act_lizard((14, 1), 1.0);
    g.dev_set_lizard_destination(l, WorldCoordinate::new(0, 3, 1, -1), Some(0.5));
    let mut turned = false;
    for _ in 0..120 {
        g.tick(Input::NONE, &mut ());
        let lizard = g.world.lizard_ref(l);
        turned |= lizard
            .movement_animation
            .as_ref()
            .is_some_and(|a| a.connection.kind == MovementType::LizardTurn);
    }
    assert!(turned);
    assert!(g.head_tile(l).0 < 14, "heading left: {:?}", g.head_tile(l));
}

#[test]
fn a_lizard_takes_the_shortcut_to_the_other_half() {
    let mut g = game(&SPLIT, (5, 3));
    let l = g.act_lizard((8, 3), -1.0);
    g.dev_set_lizard_destination(l, WorldCoordinate::new(0, 17, 3, -1), Some(0.5));
    let mut vessel = None;
    for _ in 0..800 {
        g.tick(Input::NONE, &mut ());
        if vessel.is_none()
            && let Some(v) = g.transport_vessels().flatten().find(|v| v.creature == l)
        {
            vessel = Some(v);
            // A lizard (3 shortcut segments) trails two last positions.
            assert_eq!(v.positions().1.len(), 2);
            assert!(g.world.lizard_ref(l).in_shortcut);
            assert_eq!(g.world.lizard_ref(l).room, None);
        }
        if vessel.is_some() && g.world.lizard_ref(l).room == Some(0) && g.head_tile(l).0 > 12 {
            return;
        }
    }
    panic!(
        "vessel {:?}, lizard room {:?} head {:?}",
        vessel.is_some(),
        g.world.lizard_ref(l).room,
        g.head_tile(l)
    );
}

#[test]
fn a_lizard_reaches_over_a_gap() {
    // A pit three tiles wide: its edge tiles are floor (a solid corner below), the
    // middle air; the pit cannot be climbed out of, so the ReachOverGap is the way.
    let pit = [
        "####################",
        "#..................#",
        "#..................#",
        "#..................#",
        "#########...########",
        "#########...########",
        "####################",
    ];
    let mut g = game(&pit, (2, 3));
    let l = g.act_lizard((5, 3), 1.0);
    g.dev_set_lizard_destination(l, WorldCoordinate::new(0, 15, 3, -1), Some(0.5));
    let mut reached = false;
    for _ in 0..400 {
        g.tick(Input::NONE, &mut ());
        reached |= g
            .world
            .lizard_ref(l)
            .movement_animation
            .as_ref()
            .is_some_and(|a| {
                a.connection.kind == MovementType::ReachOverGap && a.connection.dest.tile == (11, 3)
            });
        if g.head_tile(l).0 >= 14 {
            break;
        }
    }
    assert!(reached);
    assert!(g.head_tile(l).0 >= 14, "head {:?}", g.head_tile(l));
    assert_eq!(g.head_tile(l).1, 3, "not in the pit");
}

#[test]
fn define_node_takes_the_nearest_exit_in_a_ready_room() {
    // Two dens at the ends of a corridor. Ready for AI, a tile some exit is reachable
    // from takes the node with the least exit distance, after the random relevant node
    // (one draw).
    let dens = [
        "##############################",
        "DE..........................ED",
        "##############################",
    ];
    let g = game(&dens, (14, 1));
    let t = CreatureTemplate::of(CreatureType::GreenLizard, ActiveMods::NONE);
    let mut rng = g.rng.clone();
    let mut expected = rng.clone();
    expected.range_i32(0, 2);
    let left = g
        .world
        .define_node(WorldCoordinate::new(0, 4, 1, -1), &t, &mut rng);
    assert_eq!(rng, expected);
    let right = g
        .world
        .define_node(WorldCoordinate::new(0, 25, 1, -1), &t, &mut rng);
    assert_eq!(left.tile, (4, 1));
    let mut r = UnityRandom::new(1);
    assert_eq!(
        g.world.local_coordinate_of_node(0, left.node, &mut r),
        (1, 1)
    );
    assert_eq!(
        g.world.local_coordinate_of_node(0, right.node, &mut r),
        (28, 1)
    );
}

#[test]
fn the_shelter_rebuild_resets_the_path_finders_of_the_room() {
    // Once the mapper is done the room's AI map is replaced and every path finder in
    // the room starts mapping again (`UpdatePathfindingCreatures`).
    let mut g = game(&HALL, (2, 1));
    let l = g.act_lizard((8, 1), 1.0);
    assert!(!g.world.path_finder_mapping(l));
    let before = Arc::clone(
        g.world
            .realized(0)
            .and_then(|r| r.ai_map.as_ref())
            .expect("map"),
    );
    let mut door = crate::shelter::ShelterDoor::new(
        0,
        &g.world.realized(0).expect("room").room.as_ref().clone(),
        &RoomSettings::ANCESTOR,
        ActiveMods::NONE,
        10_000,
        false,
        &mut UnityRandom::new(1),
    );
    door.ai_mapper = Some(AiMapper::default());
    g.shelter_doors.push(door);
    // 30 x 6 with 28 x 4 open: 2 * 180 + 112 + 4 = 476 steps, 5 ticks; the sixth sees
    // it done.
    for _ in 0..5 {
        g.run_shelter_ai_mapper(0, 0);
    }
    assert!(
        g.shelter_doors[0]
            .ai_mapper
            .as_ref()
            .is_some_and(AiMapper::done)
    );
    g.run_shelter_ai_mapper(0, 0);
    assert!(g.shelter_doors[0].ai_mapper.is_none());
    let after = g
        .world
        .realized(0)
        .and_then(|r| r.ai_map.as_ref())
        .expect("map");
    assert!(!Arc::ptr_eq(&before, after));
    assert!(g.world.path_finder_mapping(l), "reset: mapping again");
}

#[test]
fn a_movement_animation_step_draws_three_times_per_destination() {
    let mut g = game(&HALL, (2, 1));
    let l = g.act_lizard((8, 1), 1.0);
    let start = WorldCoordinate::new(0, 8, 1, -1);
    let dest = WorldCoordinate::new(0, 8, 3, -1);
    let conn = MovementConnection::new(MovementType::ReachUp, start, dest, 2);
    let lizard = g.world.lizard_at(l);
    let chunks = [
        lizard.body.chunks[0].pos,
        lizard.body.chunks[1].pos,
        lizard.body.chunks[2].pos,
    ];
    // ReachUp's first step has one destination (the head), its second three.
    let first = MovementAnimation::new(chunks, conn, 1, 0.99, 0.75);
    assert_eq!(first.destinations.len(), 1);
    let second = MovementAnimation::new(chunks, conn, 2, 0.99, 0.75);
    assert_eq!(second.destinations.len(), 3);
    lizard.movement_animation = Some(second);
    let mut expected = g.rng.clone();
    for _ in 0..9 {
        expected.value();
    }
    let out = g
        .world
        .lizard_at(l)
        .movement_animation_update(0.5, 0.5, &mut g.rng);
    assert_eq!(g.rng, expected);
    assert!(!matches!(out, AnimationOutcome::NextStep(_)), "step 2 of 2");
}

#[test]
fn the_frame_speed_draws_once() {
    let mut g = game(&HALL, (2, 1));
    let l = g.act_lizard((8, 1), 1.0);
    let r = g.act_room(0).expect("room 0");
    let start = WorldCoordinate::new(0, 8, 1, -1);
    let dest = WorldCoordinate::new(0, 9, 1, -1);
    g.world.lizard_at(l).following_connection = Some(MovementConnection::new(
        MovementType::Standard,
        start,
        dest,
        1,
    ));
    let mut expected = g.rng.clone();
    expected.value();
    let speed = g.lizard_frame_speed(&r, l, 0.5);
    assert_eq!(g.rng, expected);
    assert!(speed > 0.0);
}

#[test]
fn the_shelter_mapper_takes_two_passes_and_the_proximity_steps() {
    // 20 x 8 walled: 18 x 6 open tiles, every one queued by the terrain proximity
    // mapper. Steps: 2 * 160 + 108 + 4 = 432, 100 a tick.
    let r = room(&[
        "####################",
        "#..................#",
        "#..................#",
        "#..................#",
        "#..................#",
        "#..................#",
        "#..................#",
        "####################",
    ]);
    let old = AiMap::new(&r, 0, true, false);
    let mut m = AiMapper::default();
    for _ in 0..4 {
        m.update(100, &old, &r);
        assert!(!m.done());
    }
    assert_eq!(m.total, Some(432));
    m.update(100, &old, &r);
    assert!(m.done());
    let map = m.take_map().expect("done");
    // The baked visibility does not survive; the tiles are the same classes.
    for x in 0..20 {
        for y in 0..8 {
            assert_eq!(map.tile(x, y).visibility, 0);
            assert_eq!(map.tile(x, y).acc, old.tile(x, y).acc);
        }
    }
}

#[test]
fn excitement_settles_with_nothing_tracked() {
    // `LizardAI.Update`: `excitement` lerps 0.1 toward max(hunger, CombinedFear), 0
    // while the trackers track nothing.
    let mut g = game(&HALL, (2, 1));
    let l = g.act_lizard((8, 1), 1.0);
    g.world.lizard_at(l).ai.excitement = 0.5;
    g.tick(Input::NONE, &mut ());
    assert!((g.world.lizard_ref(l).ai.excitement - 0.45).abs() < 1e-6);
}

/// `CreatureVoice.Update` lets `soundA` go once the call is over (`soundID` None)
/// without touching the emitter, which updates before the lizard (`Room.Update` runs
/// the list backwards and the emitter joined after it). On the drop's tick it spends
/// the `alive` the last upkeep gave it; on the next its update finds no upkeep and
/// `Destroy`s it, and the microphone's `ObjectSound` (`Done` once its controller is
/// slated, no `fadeOutOnDestroy`) cuts the sound.
#[test]
fn a_dropped_voice_emitter_dies_at_its_next_update() {
    let mut g = game_with_voice(&HALL, (2, 1));
    let l = g.act_lizard((14, 1), 1.0);
    g.lizard_voice_with(l, LizardEmotion::Fear, 1.0);
    g.tick(Input::NONE, &mut ());
    g.tick(Input::NONE, &mut ());
    let emitter = g.voice(l).sound_a.as_ref().expect("emitter").emitter.sound;
    let v = g.world.lizard_at(l).voice.as_mut().expect("voice");
    v.sound_id = None;
    v.articulation = None;
    assert_eq!(g.tick_stops(), vec![], "the drop's tick");
    let v = g.voice(l);
    assert!(v.sound_a.is_none());
    assert_eq!(v.released.len(), 1);
    assert!(!v.released[0].alive && !v.released[0].slated);
    assert_eq!(g.tick_stops(), vec![emitter], "its next update");
    assert!(g.voice(l).released.is_empty());
    assert_eq!(g.tick_stops(), vec![]);
}

/// `Creature.SuckedIntoShortCut` takes the lizard out of the room (`RemoveObject`:
/// `room = null`). On that tick its emitter updated before it and the voice's upkeep
/// came before `Act`; at its next update `ChunkSoundEmitter.Update` sees
/// `chunk.owner.room != room` and destroys it.
#[test]
fn a_voice_emitter_dies_when_its_lizard_enters_a_shortcut() {
    let mut g = game_with_voice(&SPLIT, (5, 3));
    let l = g.act_lizard((8, 3), -1.0);
    g.dev_set_lizard_destination(l, WorldCoordinate::new(0, 17, 3, -1), Some(0.5));
    let mut emitter = None;
    for _ in 0..800 {
        if !g.voice(l).making_a_sound() {
            g.lizard_voice_with(l, LizardEmotion::Fear, 1.0);
        }
        emitter = g.voice(l).sound_a.as_ref().map(|s| s.emitter.sound);
        let stops = g.tick_stops();
        if g.world.lizard_ref(l).in_shortcut {
            assert_eq!(stops, vec![], "the tick it went in");
            break;
        }
    }
    let emitter = emitter.expect("an emitter when it went in");
    let held = g.voice(l).sound_a.as_ref().expect("still held").emitter;
    assert_eq!(held.sound, emitter);
    assert!(
        held.alive && !held.slated,
        "the voice kept it alive before Act sucked it in"
    );
    assert_eq!(g.tick_stops(), vec![emitter], "its next update");
    assert!(g.voice(l).sound_a.as_ref().unwrap().emitter.slated);
    assert_eq!(g.tick_stops(), vec![]);
}
