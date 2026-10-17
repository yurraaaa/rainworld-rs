//! Items in the hands, one test room.

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, Room, RoomSettings};
use rw_random::UnityRandom;

use super::Grabability;
use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::creature::fly::FlyRoomTick;
use crate::creature::{CreatureId, CreatureTemplate, EntityId, WorldCoordinate};
use crate::item::{AbstractObject, Entity, ObjectId, WeaponMode};
use crate::player::StomachObject;
use crate::tests::room;
use crate::world::World;
use crate::{Input, RainWorldGame, SoundEvent, SoundId, Sounds, Start, Vec2};
use rw_assets::CreatureType;

const PICKUP: Input = Input {
    x: 0,
    y: 0,
    jump: false,
    map: false,
    pickup: true,
    throw: false,
};

/// A 10 x 8 room: walls, a floor at row 1, air above.
fn floor_room() -> Room {
    let mut rows = vec!["##########"];
    rows.extend(["#........#"; 6]);
    rows.push("##########");
    room(&rows)
}

/// A wall at x = 4 with a pipe through it: entrances (3,1) and (5,1).
fn pipe_room() -> Room {
    room(&[
        "###########",
        "#...#.....#",
        "#...#.....#",
        "#..ExE....#",
        "###########",
    ])
}

/// A Survivor story in `room` with the player at `tile` and `objects` (as rocks or
/// spears at their tiles) in it, stood up (10 ticks of up, 10 of
/// nothing). The room makes no random item.
fn game(
    room: Room,
    tile: (i32, i32),
    objects: Vec<AbstractObject>,
) -> (RainWorldGame, Vec<ObjectId>) {
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let campaign = &SlugcatCampaign::SURVIVOR;
    let mut world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let ids: Vec<ObjectId> = objects.into_iter().map(|o| world.add_object(o)).collect();
    let mut g = RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some(tile),
        },
        UnityRandom::new(5),
        Sounds::default(),
        &mut (),
    );
    for _ in 0..10 {
        g.tick(Input::new(0, 1, false), &mut ());
    }
    for _ in 0..10 {
        g.tick(Input::NONE, &mut ());
    }
    let placed: Vec<ObjectId> = g.world.room_objects(0).collect();
    assert_eq!(placed, ids, "only the test's objects");
    (g, ids)
}

fn rock(n: i32, tile: (i32, i32)) -> AbstractObject {
    AbstractObject::new_rock(
        EntityId::new(0, n),
        WorldCoordinate::new(0, tile.0, tile.1, -1),
    )
}

fn spear(n: i32, tile: (i32, i32)) -> AbstractObject {
    AbstractObject::new_spear(
        EntityId::new(0, n),
        WorldCoordinate::new(0, tile.0, tile.1, -1),
        false,
    )
}

impl RainWorldGame {
    /// A realized fly in room 0, out of its shortcut delay.
    fn test_fly(&mut self, tile: (i32, i32)) -> CreatureId {
        let session = self.story_session();
        let template = CreatureTemplate::of(CreatureType::Fly, ActiveMods::NONE);
        let id = self.ids.new_id(-1);
        let pos = WorldCoordinate::new(0, tile.0, tile.1, -1);
        let c = self
            .world
            .new_creature(template, pos, id, &session, &mut self.ids, &mut self.rng);
        let fly = self.world.insert_creature(c);
        self.world.add_entity(fly);
        let mut cx = FlyRoomTick {
            rng: &mut self.rng,
            observer: &mut (),
            camera_room: self.camera_room,
        };
        self.world.realize_fly_in_room(fly, 0, &mut cx);
        if let Some(f) = self.world.creature_mut(fly).fly_mut() {
            f.shortcut_delay = 0;
        }
        fly
    }

    /// Every chunk of `e` set still at `pos`.
    fn put(&mut self, e: Entity, pos: Vec2) {
        for c in self.world.body_chunks_mut(e).expect("realized") {
            c.hard_set_position(pos);
            c.vel = Vec2::ZERO;
        }
    }

    fn candidate(&self) -> Option<Entity> {
        let room = self.world.realized(0).expect("room 0").room.clone();
        self.pickup_candidate(0, &room, 20.0)
    }

    fn mode(&self, id: ObjectId) -> WeaponMode {
        self.world
            .object(id)
            .realized
            .as_deref()
            .expect("realized")
            .mode
    }
}

#[test]
fn a_tap_picks_up_the_rock_in_reach() {
    // `GrabUpdate`: the press sets `wantToPickUp` 5, the candidate goes to the first free
    // hand (`SlugcatGrab`), `Weapon.Grabbed` changes the rock to Carried.
    let (mut g, ids) = game(floor_room(), (4, 1), vec![rock(1, (5, 1))]);
    let rock = Entity::Object(ids[0]);
    assert_eq!(g.player_ref().grab_state().pick_up_candidate, Some(rock));
    g.tick(PICKUP, &mut ());
    assert_eq!(g.player_grasps(), [Some(rock), None]);
    assert_eq!(g.mode(ids[0]), WeaponMode::Carried);
    assert_eq!(g.grabability(rock), Grabability::OneHand);
    assert_eq!(g.player_ref().grab_state().want_to_pick_up, 0);
    // Held, it is no candidate (`CanIPickThisUp`: already in a hand).
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.player_ref().grab_state().pick_up_candidate, None);
}

#[test]
fn stuck_and_forbidden_weapons_cannot_be_picked_up() {
    // `CanIPickThisUp`: a weapon StuckInWall or Thrown, or forbidden to the player, stays.
    let (mut g, ids) = game(floor_room(), (4, 1), vec![spear(1, (5, 1))]);
    let s = Entity::Object(ids[0]);
    assert!(g.can_i_pick_this_up(s));
    assert_eq!(g.grabability(s), Grabability::BigOneHand);
    for mode in [WeaponMode::StuckInWall, WeaponMode::Thrown] {
        g.world
            .object_mut(ids[0])
            .realized
            .as_deref_mut()
            .expect("spear")
            .mode = mode;
        assert!(!g.can_i_pick_this_up(s), "{mode:?}");
    }
    let w = g
        .world
        .object_mut(ids[0])
        .realized
        .as_deref_mut()
        .expect("spear");
    w.mode = WeaponMode::Free;
    w.forbidden_to_player = 1;
    assert!(!g.can_i_pick_this_up(s));
}

#[test]
fn spears_are_favoured_over_a_nearer_fly_and_rocks_are_not() {
    // `PickupCandidate(20)`: distance from the head, a spear 20 nearer, the facing side
    // 10 nearer (all three on the same side here).
    let (mut g, ids) = game(
        floor_room(),
        (4, 1),
        vec![spear(1, (6, 3)), rock(2, (6, 4))],
    );
    let fly = Entity::Creature(g.test_fly((5, 3)));
    let head = g.player_ref().head().pos;
    let (s, r) = (Entity::Object(ids[0]), Entity::Object(ids[1]));
    g.put(fly, head + Vec2::new(25.0, 0.0));
    g.put(s, head + Vec2::new(40.0, 0.0));
    g.put(r, head + Vec2::new(0.0, 200.0));
    // Fly 25, spear 40 - 20 = 20.
    assert_eq!(g.candidate(), Some(s));
    g.put(s, head + Vec2::new(0.0, 200.0));
    g.put(r, head + Vec2::new(40.0, 0.0));
    // Fly 25, rock 40.
    assert_eq!(g.candidate(), Some(fly));
    g.put(r, head + Vec2::new(20.0, 0.0));
    assert_eq!(g.candidate(), Some(r));
}

#[test]
fn a_double_tap_switches_hands_on_the_sixth_tick() {
    // Press 1 sets `switchHandsCounter` 15; press 2 within it starts the process at
    // 0.01, which grows 1/12 a tick from that tick on: the grasps swap when it passes
    // 0.5 (6 steps), and it ends at 1 (12 steps).
    let (mut g, ids) = game(floor_room(), (4, 1), vec![rock(1, (5, 1))]);
    let rock = Entity::Object(ids[0]);
    g.slugcat_grab(rock, 0, &mut ());
    g.tick(PICKUP, &mut ());
    assert_eq!(g.player_ref().grab_state().switch_hands_counter, 14);
    g.tick(Input::NONE, &mut ());
    g.tick(PICKUP, &mut ());
    for _ in 1..5 {
        assert_eq!(g.player_grasps(), [Some(rock), None]);
        g.tick(Input::NONE, &mut ());
    }
    assert_eq!(g.player_grasps(), [Some(rock), None]);
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.player_grasps(), [None, Some(rock)]);
    for _ in 6..11 {
        assert!(g.player_ref().grab_state().switch_hands_process > 0.0);
        g.tick(Input::NONE, &mut ());
    }
    assert!(g.player_ref().grab_state().switch_hands_process > 0.0);
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.player_ref().grab_state().switch_hands_process, 0.0);
}

#[test]
fn holding_pickup_swallows_a_rock_and_coughs_it_back_into_a_hand() {
    // `GrabUpdate`'s `flag2`: the counter passes 90 on the 91st held tick and the rock
    // goes down; with it in the stomach the counter starts again and passes 110 on the
    // 111th, and standing upright the slugcat catches it (`Regurgitate`).
    let (mut g, ids) = game(floor_room(), (4, 1), vec![rock(1, (5, 1))]);
    let rock = Entity::Object(ids[0]);
    g.slugcat_grab(rock, 0, &mut ());
    for _ in 0..90 {
        g.tick(PICKUP, &mut ());
    }
    assert_eq!(
        g.player_ref().grab_state().swallow_and_regurgitate_counter,
        90
    );
    assert_eq!(g.player_grasps(), [Some(rock), None]);
    g.tick(PICKUP, &mut ());
    assert_eq!(g.player_grasps(), [None, None]);
    assert_eq!(
        g.player_ref().object_in_stomach(),
        Some(&StomachObject::Object(ids[0]))
    );
    assert!(g.world.object(ids[0]).realized.is_none());
    assert_eq!(g.world.room_objects(0).count(), 0);
    assert_eq!(
        g.swallowed_item_save_string(),
        Some(
            g.world
                .object(ids[0])
                .save_string(&g.world.coordinate_save_string(g.pos()))
        )
    );
    for _ in 0..110 {
        g.tick(PICKUP, &mut ());
    }
    assert!(g.player_ref().object_in_stomach().is_some());
    g.tick(PICKUP, &mut ());
    assert_eq!(g.player_ref().object_in_stomach(), None);
    assert_eq!(g.player_grasps(), [Some(rock), None]);
    assert_eq!(g.mode(ids[0]), WeaponMode::Carried);
    assert_eq!(g.world.room_objects(0).collect::<Vec<_>>(), ids);
}

#[test]
fn regurgitating_without_graphics_draws_twenty_two_times_and_a_sound() {
    // `RealizeInRoom` (rotation, spin), the velocity's RNV and value, three drips of
    // RNV, value, RNV, value, value and `WaterDrip`'s Range(10, 120); the head shake's
    // two draws come only with graphics. Then `Slugcat_Regurgitate_Item`; the grab's
    // `ChangeMode(Carried)` draws nothing.
    let (mut g, ids) = game(floor_room(), (4, 1), vec![rock(1, (5, 1))]);
    g.slugcat_grab(Entity::Object(ids[0]), 0, &mut ());
    g.swallow_object(0, 0);
    let mut expect = g.rng.clone();
    for _ in 0..22 {
        expect.value();
    }
    Sounds::default().play_on_chunk(
        SoundId::SlugcatRegurgitateItem,
        0,
        false,
        1.0,
        1.0,
        &mut expect,
    );
    g.regurgitate(0, &mut ());
    assert_eq!(g.rng.state(), expect.state());
    assert_eq!(g.player_grasps()[0], Some(Entity::Object(ids[0])));
}

#[test]
fn coughed_up_lying_down_the_rock_falls_free() {
    // `Regurgitate`: only with the head above the hips more than beside them does it
    // land in a free hand.
    let (mut g, ids) = game(floor_room(), (4, 1), vec![rock(1, (5, 1))]);
    g.slugcat_grab(Entity::Object(ids[0]), 0, &mut ());
    for _ in 0..20 {
        g.tick(Input::new(0, -1, false), &mut ());
    }
    g.swallow_object(0, 0);
    g.regurgitate(0, &mut ());
    assert_eq!(g.player_grasps(), [None, None]);
    assert_eq!(g.mode(ids[0]), WeaponMode::Free);
}

#[test]
fn a_held_rock_goes_through_a_shortcut_with_the_slugcat() {
    // `SuckedIntoShortCut` takes every connected object out of the room; `SpitOutOfShortCut`
    // puts them back at the exit, still in the hand.
    let (mut g, ids) = game(pipe_room(), (2, 1), vec![rock(1, (1, 1))]);
    let rock = Entity::Object(ids[0]);
    g.slugcat_grab(rock, 0, &mut ());
    let down = Input::new(0, -1, false);
    let right = Input::new(1, 0, false);
    for _ in 0..20 {
        g.tick(down, &mut ());
    }
    let mut left_room = false;
    for _ in 0..120 {
        g.tick(right, &mut ());
        left_room |= g.world.realized_weapon(ids[0], 0).is_none();
    }
    assert!(left_room, "the rock left the room with the slugcat");
    assert!(g.player_ref().head().pos.x > 100.0, "out of the far end");
    assert_eq!(g.player_grasps(), [Some(rock), None]);
    let w = g
        .world
        .realized_weapon(ids[0], 0)
        .expect("back in the room");
    assert_eq!(w.mode, WeaponMode::Carried);
    assert!(w.chunk().pos.x > 100.0);
}

#[test]
fn a_saved_swallowed_rock_comes_back_up() {
    // The `Player` constructor reads `swallowedItems[0]` into `objectInStomach`; a
    // creature the port does not type stays text and is never coughed up.
    let (mut g, ids) = game(floor_room(), (4, 1), vec![rock(1, (5, 1))]);
    let at = g.world.coordinate_save_string(g.pos());
    let saved = g.world.object(ids[0]).save_string(&at);
    g.story.swallowed_item = Some(saved.clone());
    let Some(StomachObject::Object(id)) = g.stomach_from_save() else {
        panic!("a typed rock");
    };
    assert_eq!(g.world.object(id).save_string(&at), saved);
    g.player_mut()
        .set_object_in_stomach(Some(StomachObject::Object(id)));
    g.regurgitate(0, &mut ());
    assert_eq!(g.player_grasps()[0], Some(Entity::Object(id)));

    g.story.swallowed_item = Some("ID.-1.5<cA>Fly<cA>SU_A42.-1.0.0<cA>0".to_owned());
    assert!(matches!(
        g.stomach_from_save(),
        Some(StomachObject::Saved(_))
    ));
    g.story.swallowed_item = Some("0".to_owned());
    assert_eq!(g.stomach_from_save(), None);
}

#[test]
fn a_thrown_rock_leaves_the_hand_at_forty_and_kicks_the_slugcat() {
    // `ThrowObject`: `Thrown` along `ThrowDirection` (the stick, right); the head gets
    // 8 along the throw, the hips 4 against it.
    let (mut g, ids) = game(floor_room(), (4, 1), vec![rock(1, (5, 1))]);
    g.slugcat_grab(Entity::Object(ids[0]), 0, &mut ());
    g.tick(Input::new(1, 0, false), &mut ());
    let (head, hips) = (g.player_ref().head().vel, g.player_ref().hips().vel);
    g.throw_object(0, 0, &mut ());
    assert_eq!(g.player_grasps(), [None, None]);
    assert_eq!(g.mode(ids[0]), WeaponMode::Thrown);
    let w = g.world.weapon_mut(ids[0]).expect("rock");
    assert_eq!(w.throw_dir, (1, 0));
    assert!(w.chunk().vel.x > 39.0, "{:?}", w.chunk().vel);
    assert_eq!(g.player_ref().head().vel.x, head.x + 8.0);
    assert_eq!(g.player_ref().hips().vel.x, hips.x - 4.0);
}

#[test]
fn a_monk_spear_draws_once_for_its_damage_and_flies_slower() {
    // `ThrownSpear` with throwingSkill 0: spearDamageBonus 0.6 + 0.3 value^4, 18 throw
    // frames, vel.x * 0.77.
    let (mut g, ids) = game(floor_room(), (4, 1), vec![spear(1, (5, 1))]);
    g.character_stats.throwing_skill = 0;
    let w = g.world.weapon_mut(ids[0]).expect("spear");
    w.chunk_mut().vel = Vec2::new(40.0, 0.0);
    let mut expect = g.rng.clone();
    let bonus = 0.6 + 0.3 * expect.value().powf(4.0);
    g.thrown_spear(ids[0]);
    assert_eq!(g.rng.state(), expect.state());
    let w = g.world.weapon_mut(ids[0]).expect("spear");
    assert_eq!(w.spear().expect("spear").spear_damage_bonus, bonus);
    assert_eq!(w.throw_mode_frames, 18);
    assert_eq!(w.chunk().vel.x, 40.0 * 0.77);
    // The Survivor (skill 1) draws nothing and keeps the speed.
    let (mut g, ids) = game(floor_room(), (4, 1), vec![spear(1, (5, 1))]);
    g.world.weapon_mut(ids[0]).expect("spear").chunk_mut().vel = Vec2::new(40.0, 0.0);
    let expect = g.rng.clone();
    g.thrown_spear(ids[0]);
    assert_eq!(g.rng.state(), expect.state());
    assert_eq!(
        g.world.weapon_mut(ids[0]).expect("spear").chunk().vel.x,
        40.0
    );
}

#[test]
fn a_forward_belly_slide_throw_skims_the_spear_along_the_floor() {
    // `ThrowObject` (93175): rollCounter 9..14, thrown along the slide with skill: +15
    // px a tick, 30 floor bounce frames, always sticking, 5 down, not through floors.
    let (mut g, ids) = game(floor_room(), (2, 1), vec![spear(1, (3, 1))]);
    g.slugcat_grab(Entity::Object(ids[0]), 0, &mut ());
    g.player_mut().set_belly_slide(10, 1, 1);
    g.throw_object(0, 0, &mut ());
    let w = g.world.weapon_mut(ids[0]).expect("spear");
    assert_eq!(w.floor_bounce_frames, 30);
    assert!(w.spear().expect("spear").always_stick_in_walls);
    assert!(!w.chunk().go_through_floors);
    assert!(w.chunk().vel.x > 54.0, "{:?}", w.chunk().vel);
    assert!(!g.player_ref().long_belly_slide());
}

#[test]
fn a_backward_belly_slide_throw_starts_a_long_slide_with_spikes() {
    // Thrown against the slide: up 5 (a rock), the long slide, `ExplosionSpikes` 40 px
    // behind the hips, `Slugcat_Rocket_Jump`. A second one in the long slide does not.
    let (mut g, ids) = game(floor_room(), (5, 1), vec![rock(1, (5, 1)), rock(2, (5, 1))]);
    g.slugcat_grab(Entity::Object(ids[0]), 0, &mut ());
    g.slugcat_grab(Entity::Object(ids[1]), 1, &mut ());
    g.player_mut().set_belly_slide(12, 1, -1);
    g.throw_object(0, 0, &mut ());
    assert!(g.player_ref().long_belly_slide());
    assert_eq!(g.player_ref().roll_counter(), 8);
    assert_eq!(g.world.room_cosmetics(0).len(), 1);
    g.player_mut().set_belly_slide(12, 1, -1);
    g.throw_object(0, 1, &mut ());
    assert_eq!(g.player_ref().roll_counter(), 12);
    assert_eq!(g.world.room_cosmetics(0).len(), 1);
}

/// Hands a fixed offset from the head, as a graphics module would report them.
struct OffsetHands {
    head: Vec2,
}

impl crate::PlayerObserver for OffsetHands {
    fn created(&mut self, _: &crate::Player, _: &mut UnityRandom) {}
    fn updated(
        &mut self,
        player: &crate::Player,
        _: &Room,
        _: Option<&crate::Water>,
        _: &crate::PlayerSurroundings<'_>,
        _: &mut UnityRandom,
    ) {
        self.head = player.head().pos;
    }
    fn spat_out(&mut self, _: &crate::Player, _: &mut UnityRandom) {}
    fn player_hands(&self) -> Option<crate::PlayerHands> {
        let hand = (self.head + Vec2::new(-12.0, -8.0), Vec2::new(1.0, 0.0));
        Some(crate::PlayerHands {
            hands: [hand, hand],
            ..crate::PlayerHands::default()
        })
    }
}

#[test]
fn a_held_rock_stays_in_the_hand_while_walking() {
    // `GraphicsModuleUpdated`: the held chunk takes the hand's velocity and
    // `MoveFromOutsideMyUpdate(eu, hand.pos)`, which compares the realized rock's own
    // `evenUpdate` (the `eu` of its last update), so after every tick the rock is at the
    // hand. Comparing the abstract entity's flag (flipped by the abstract room's
    // updates) lagged it a tick on alternate ticks.
    use crate::PlayerObserver;
    let (mut g, ids) = game(floor_room(), (3, 1), vec![rock(1, (4, 1))]);
    let mut hands = OffsetHands { head: Vec2::ZERO };
    g.tick(PICKUP, &mut hands);
    assert_eq!(g.player_grasps()[0], Some(Entity::Object(ids[0])));
    for _ in 0..30 {
        g.tick(Input::new(1, 0, false), &mut hands);
        let hand = hands.player_hands().expect("hands").hands[0].0;
        let w = g
            .world
            .object(ids[0])
            .realized
            .as_deref()
            .expect("realized");
        assert_eq!(w.chunk().pos, hand);
    }
}

/// `DynamicSoundLoop.Stop` only sets `emitter.alive = false` and drops it. A rock that
/// stops skidding lets its loop go; the emitter joined the room after the rock, so it
/// updated first that tick, and at its next update it finds no upkeep and `Destroy`s
/// itself, which cuts the sound.
#[test]
fn a_stopped_rock_loop_dies_at_its_next_update() {
    let (mut g, ids) = game(floor_room(), (2, 1), vec![rock(1, (6, 1))]);
    let text = "Volume: 6\nVolume exponent: 1.8\nRock_Skidding_On_Ground_LOOP : clip\n";
    let names = rw_assets::ClipNames::new(["clip".to_owned()], None);
    let table = rw_assets::SoundTable::parse(text, &names).unwrap();
    g.sounds = Sounds::new(Arc::new(table));
    g.world
        .weapon_mut(ids[0])
        .expect("realized")
        .chunk_mut()
        .vel = Vec2::new(12.0, 0.0);
    g.tick(Input::NONE, &mut ());
    let held = g.weapon_loop(ids[0]).emitter.expect("the skid loop").sound;
    g.world
        .weapon_mut(ids[0])
        .expect("realized")
        .chunk_mut()
        .vel = Vec2::ZERO;
    g.sounds.take_events();
    g.tick(Input::NONE, &mut ());
    let events = g.sounds.take_events();
    assert!(
        !events.iter().any(|e| matches!(e, SoundEvent::Stop(_))),
        "the tick it was let go: {events:?}"
    );
    let released = &g.weapon_loop(ids[0]).released;
    assert!(g.weapon_loop(ids[0]).emitter.is_none());
    assert_eq!(released.len(), 1);
    assert!(!released[0].alive && !released[0].slated);
    g.tick(Input::NONE, &mut ());
    let stops: Vec<_> = g
        .sounds
        .take_events()
        .into_iter()
        .filter(|e| matches!(e, SoundEvent::Stop(_)))
        .collect();
    assert_eq!(stops, vec![SoundEvent::Stop(held)], "its next update");
    assert!(g.weapon_loop(ids[0]).released.is_empty());
}

/// `Player.SwallowObject` calls `RemoveFromRoom` on the rock, which only nulls its
/// `room`; the loop's `ChunkSoundEmitter` stays in the update list and finds
/// `chunk.owner.room != room` at its next update (`alive = false`, then `Destroy`). It
/// joined after the player, so `Room.Update`'s backward walk runs it first next tick.
#[test]
fn a_swallowed_rocks_loop_emitter_dies_at_its_next_update() {
    let (mut g, ids) = game(floor_room(), (4, 1), vec![rock(1, (5, 1))]);
    g.slugcat_grab(Entity::Object(ids[0]), 0, &mut ());
    let sound = crate::EmitterId(900);
    let order = g.world.next_update_seq();
    g.world.weapon_mut(ids[0]).expect("rock").sound_loop.emitter = Some(crate::LoopEmitter {
        sound,
        room: 0,
        update_order: order,
        alive: true,
        slated: false,
        still_playing: true,
        last_playing_tick: g.tick,
        playing: true,
    });
    g.sounds.take_events();
    g.swallow_object(0, 0);
    assert_eq!(g.stops(), vec![], "the swallow");
    assert_eq!(g.world.orphan_loop_count(), 1);
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.stops(), vec![sound], "its next update");
    assert_eq!(g.world.orphan_loop_count(), 0);
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.stops(), vec![]);
}

impl RainWorldGame {
    /// The `Stop`s since the last call.
    fn stops(&mut self) -> Vec<crate::EmitterId> {
        self.sounds
            .take_events()
            .into_iter()
            .filter_map(|e| match e {
                SoundEvent::Stop(id) => Some(id),
                _ => None,
            })
            .collect()
    }

    fn weapon_loop(&self, id: ObjectId) -> &crate::item::ChunkDynamicSoundLoop {
        &self
            .world
            .object(id)
            .realized
            .as_deref()
            .expect("realized")
            .sound_loop
    }
}
