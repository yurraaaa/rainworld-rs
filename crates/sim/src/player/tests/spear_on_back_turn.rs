//! The Hunter's spear on the back, one test room.

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, ClipNames, Room, RoomSettings, SoundTable};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::creature::{EntityId, WorldCoordinate};
use crate::item::{AbstractObject, Entity, ObjectId, WeaponMode};
use crate::tests::room;
use crate::world::World;
use crate::{Input, RainWorldGame, SoundEvent, SoundId, Sounds, Start, Vec2};

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

/// A story of `campaign` in the floor room with the player at (4, 1) and `objects`,
/// stood up (10 ticks of up, 10 of nothing).
fn game(
    campaign: &'static SlugcatCampaign,
    objects: Vec<AbstractObject>,
) -> (RainWorldGame, Vec<ObjectId>) {
    let room = floor_room();
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let mut world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let ids: Vec<ObjectId> = objects.into_iter().map(|o| world.add_object(o)).collect();
    let mut g = RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some((4, 1)),
        },
        UnityRandom::new(5),
        Sounds::default(),
    );
    for _ in 0..10 {
        g.tick(Input::new(0, 1, false), &mut ());
    }
    for _ in 0..10 {
        g.tick(Input::NONE, &mut ());
    }
    let text = "Volume: 6\nVolume exponent: 1.8\nSlugcat_Stash_Spear_On_Back : clip\n\
                Slugcat_Pick_Up_Spear : clip\nSlugcat_Switch_Hands_Init : clip\n\
                Spear_Dislodged_From_Creature : clip\n";
    let names = ClipNames::new(["clip".to_owned()], None);
    g.sounds = Sounds::new(Arc::new(SoundTable::parse(text, &names).unwrap()));
    (g, ids)
}

fn spear(n: i32, tile: (i32, i32)) -> AbstractObject {
    AbstractObject::new_spear(
        EntityId::new(0, n),
        WorldCoordinate::new(0, tile.0, tile.1, -1),
        false,
    )
}

fn rock(n: i32, tile: (i32, i32)) -> AbstractObject {
    AbstractObject::new_rock(
        EntityId::new(0, n),
        WorldCoordinate::new(0, tile.0, tile.1, -1),
    )
}

impl RainWorldGame {
    fn mode_of(&self, id: ObjectId) -> WeaponMode {
        self.world
            .object(id)
            .realized
            .as_deref()
            .expect("realized")
            .mode
    }

    fn back_spear(&self) -> Option<ObjectId> {
        self.spear_on_back().and_then(|s| s.spear)
    }

    /// Holds pick-up (neutral) until `done`, at most `max` ticks; the tick it happened.
    fn hold_pickup_until(&mut self, max: usize, done: impl Fn(&Self) -> bool) -> Option<usize> {
        (1..=max).find(|_| {
            self.tick(PICKUP, &mut ());
            done(self)
        })
    }

    /// The Hunter holding spear `id` in hand 0, then a tick without input.
    fn hold_spear(&mut self, id: ObjectId) {
        self.slugcat_grab(Entity::Object(id), 0, &mut ());
        self.tick(Input::NONE, &mut ());
        assert_eq!(self.mode_of(id), WeaponMode::Carried);
    }

    fn played(&mut self, id: SoundId) -> bool {
        self.sounds
            .take_events()
            .iter()
            .any(|e| matches!(e, SoundEvent::Play(p) if p.id == id))
    }
}

#[test]
fn only_the_hunter_has_a_spear_on_the_back() {
    // The `Player` constructor: `SlugCatClass == Red` (not a ghost).
    for (campaign, has) in [
        (&SlugcatCampaign::SURVIVOR, false),
        (&SlugcatCampaign::MONK, false),
        (&SlugcatCampaign::HUNTER, true),
    ] {
        let (g, _) = game(campaign, vec![]);
        assert_eq!(g.spear_on_back().is_some(), has, "{:?}", campaign.name);
    }
}

#[test]
fn holding_pickup_stashes_the_spear_on_the_22nd_tick_and_takes_it_back() {
    // `GrabUpdate` sets `increment` after `SpearOnBack.Update` has run, so the counter
    // starts on the second held tick and passes 20 on the 22nd: `SpearToBack` (the hand
    // lets go, `OnBack`, locked, `Slugcat_Stash_Spear_On_Back`, an `AbstractOnBackStick`).
    let (mut g, ids) = game(&SlugcatCampaign::HUNTER, vec![spear(1, (6, 1))]);
    g.hold_spear(ids[0]);
    let at = g.hold_pickup_until(40, |g| g.back_spear().is_some());
    assert_eq!(at, Some(22));
    assert!(g.played(SoundId::SlugcatStashSpearOnBack));
    assert_eq!(g.player_grasps(), [None, None]);
    assert_eq!(g.mode_of(ids[0]), WeaponMode::OnBack);
    let s = g.spear_on_back().expect("hunter");
    assert!(s.interaction_locked);
    assert_eq!(s.counter, 0);
    assert_eq!(g.world.on_back_sticks().len(), 1);
    assert!(
        g.world
            .connected_objects(Entity::Creature(g.player_id))
            .contains(&Entity::Object(ids[0]))
    );
    assert_eq!(g.player_ref().grab_state().no_pick_up_on_release, 19);
    // Still held, the lock keeps it there; let go, then hold again: `SpearToHand` on the
    // 22nd tick into the first free hand, `Slugcat_Pick_Up_Spear`, the stick ended.
    for _ in 0..30 {
        g.tick(PICKUP, &mut ());
    }
    assert_eq!(g.back_spear(), Some(ids[0]));
    g.tick(Input::NONE, &mut ());
    assert!(!g.spear_on_back().expect("hunter").interaction_locked);
    g.sounds.take_events();
    let at = g.hold_pickup_until(40, |g| g.back_spear().is_none());
    assert_eq!(at, Some(22));
    assert!(g.played(SoundId::SlugcatPickUpSpear));
    assert_eq!(g.player_grasps(), [Some(Entity::Object(ids[0])), None]);
    assert_eq!(g.mode_of(ids[0]), WeaponMode::Carried);
    assert!(g.world.on_back_sticks().is_empty());
}

#[test]
fn the_survivor_keeps_the_spear_in_hand() {
    // No `spearOnBack`: holding pick-up with a spear (which cannot be swallowed) does
    // nothing.
    let (mut g, ids) = game(&SlugcatCampaign::SURVIVOR, vec![spear(1, (6, 1))]);
    g.hold_spear(ids[0]);
    assert_eq!(
        g.hold_pickup_until(60, |g| g.player_grasps()[0].is_none()),
        None
    );
    assert_eq!(g.mode_of(ids[0]), WeaponMode::Carried);
}

#[test]
fn a_spear_in_reach_goes_straight_to_the_back_with_both_hands_full() {
    // The pick-up branch: a spear candidate, `CanPutSpearToBack`, both hands full:
    // `Slugcat_Switch_Hands_Init` and `SpearToBack`.
    let (mut g, ids) = game(
        &SlugcatCampaign::HUNTER,
        vec![rock(1, (4, 1)), rock(2, (4, 1)), spear(3, (5, 1))],
    );
    g.slugcat_grab(Entity::Object(ids[0]), 0, &mut ());
    g.slugcat_grab(Entity::Object(ids[1]), 1, &mut ());
    for _ in 0..25 {
        g.tick(Input::NONE, &mut ());
    }
    assert_eq!(
        g.player_ref().grab_state().pick_up_candidate,
        Some(Entity::Object(ids[2]))
    );
    g.tick(PICKUP, &mut ());
    assert!(g.played(SoundId::SlugcatSwitchHandsInit));
    assert_eq!(g.back_spear(), Some(ids[2]));
    assert_eq!(g.mode_of(ids[2]), WeaponMode::OnBack);
    assert_eq!(g.player_ref().grab_state().want_to_pick_up, 0);
}

impl RainWorldGame {
    /// The Hunter with spear `id` stashed and the lock released.
    fn stash(&mut self, id: ObjectId) {
        self.hold_spear(id);
        self.hold_pickup_until(40, |g| g.back_spear().is_some())
            .expect("stashed");
        self.tick(Input::NONE, &mut ());
    }
}

#[test]
fn a_stun_over_the_second_draw_drops_the_spear_with_four_more_draws() {
    // `Stun`: `Range(5, 12)`, `Range(40, 80)` (always drawn), then above it `DropSpear`:
    // `RNV()` and `value` for the velocity, `ChangeMode(Free)`'s spin (two); the
    // stun's own `UI_Slugcat_Stunned_Init` follows.
    let (mut g, ids) = game(&SlugcatCampaign::HUNTER, vec![spear(1, (6, 1))]);
    g.stash(ids[0]);
    let main_vel = g.player_ref().head().vel;
    let mut expect = g.rng.clone();
    expect.range_i32(5, 12);
    expect.range_i32(40, 80);
    let dir = Vec2::random_unit(&mut expect);
    let vel = main_vel + dir * (3.0 * expect.value());
    let (sign, speed) = (expect.value(), expect.value());
    Sounds::default().play_on_chunk(
        SoundId::UiSlugcatStunnedInit,
        0,
        false,
        1.0,
        1.0,
        &mut expect,
    );
    let mut sounds = Sounds::default();
    let crate::RainWorldGame { player, rng, .. } = &mut g;
    player.as_mut().expect("player").stun(100, rng, &mut sounds);
    assert_eq!(g.rng.state(), expect.state());
    g.apply_player_releases();
    assert_eq!(g.back_spear(), None);
    assert_eq!(g.mode_of(ids[0]), WeaponMode::Free);
    assert!(g.world.on_back_sticks().is_empty());
    let w = g.world.object(ids[0]).realized.as_deref().expect("spear");
    assert_eq!(w.chunk().vel, vel);
    // `SetRandomSpin` of a spear (room gravity 1).
    let turn = if sign < 0.5 { -1.0 } else { 1.0 };
    assert_eq!(w.rotation_speed, turn * (50.0 + 100.0 * speed));
}

#[test]
fn a_short_stun_keeps_the_spear() {
    // 30 ticks never pass `Range(40, 80)`.
    let (mut g, ids) = game(&SlugcatCampaign::HUNTER, vec![spear(1, (6, 1))]);
    g.stash(ids[0]);
    let mut sounds = Sounds::default();
    let crate::RainWorldGame { player, rng, .. } = &mut g;
    player.as_mut().expect("player").stun(30, rng, &mut sounds);
    g.apply_player_releases();
    assert_eq!(g.back_spear(), Some(ids[0]));
}

#[test]
fn death_drops_the_spear() {
    // `Die`: `DropSpear` first.
    let (mut g, ids) = game(&SlugcatCampaign::HUNTER, vec![spear(1, (6, 1))]);
    g.stash(ids[0]);
    g.kill_player();
    assert_eq!(g.back_spear(), None);
    assert_eq!(g.mode_of(ids[0]), WeaponMode::Free);
    assert!(g.world.on_back_sticks().is_empty());
}

#[test]
fn a_danger_grasp_pickup_takes_the_back_spear_into_the_first_free_hand() {
    // `DangerGraspPickup`: the last free hand only says one is free; `SpearToHand`
    // takes the first.
    let (mut g, ids) = game(&SlugcatCampaign::HUNTER, vec![spear(1, (6, 1))]);
    g.stash(ids[0]);
    let room = g.world.realized(0).expect("room").room.clone();
    g.danger_grasp_pickup(0, &room, &mut ());
    assert_eq!(g.player_grasps(), [Some(Entity::Object(ids[0])), None]);
    assert_eq!(g.back_spear(), None);
}

#[test]
fn down_and_pickup_on_the_ground_lays_the_back_spear_down() {
    // `wantToPickUp` with down held 5 ticks, empty hands, the main chunk on the floor:
    // `DropSpear`.
    let (mut g, ids) = game(&SlugcatCampaign::HUNTER, vec![spear(1, (6, 1))]);
    g.stash(ids[0]);
    for _ in 0..30 {
        g.tick(Input::new(0, -1, false), &mut ());
    }
    assert!(g.player_ref().head().contact().y < 0);
    g.tick(
        Input {
            pickup: true,
            ..Input::new(0, -1, false)
        },
        &mut (),
    );
    assert_eq!(g.back_spear(), None);
    assert_eq!(g.mode_of(ids[0]), WeaponMode::Free);
    // `CreaturePutItemOnGround(spearOnBack.spear, this)` before `DropSpear`.
    let owned = &g.world.realized(0).expect("room").social_events;
    let o = owned.item_ownership(Entity::Object(ids[0])).expect("owned");
    assert_eq!(o.owner, g.player_id);
}

#[test]
fn a_held_rock_laid_down_is_owned() {
    // `ReleaseObject`: `CreaturePutItemOnGround(grasps[grasp].grabbed, this)`.
    let (mut g, ids) = game(&SlugcatCampaign::SURVIVOR, vec![rock(1, (5, 1))]);
    g.slugcat_grab(Entity::Object(ids[0]), 0, &mut ());
    for _ in 0..30 {
        g.tick(Input::new(0, -1, false), &mut ());
    }
    g.tick(
        Input {
            pickup: true,
            ..Input::new(0, -1, false)
        },
        &mut (),
    );
    assert_eq!(g.player_grasps(), [None, None]);
    let owned = &g.world.realized(0).expect("room").social_events;
    let o = owned.item_ownership(Entity::Object(ids[0])).expect("owned");
    assert_eq!((o.owner, o.age), (g.player_id, 0));
}

#[test]
fn the_back_spear_rides_between_hips_and_head() {
    // `GraphicsModuleUpdated` without graphics: between the main chunk and the hips,
    // turned along the body less the lean, carried at the main chunk's velocity.
    let (mut g, ids) = game(&SlugcatCampaign::HUNTER, vec![spear(1, (6, 1))]);
    g.stash(ids[0]);
    g.tick(Input::NONE, &mut ());
    let flip = g.spear_on_back().expect("hunter").flip;
    let p = g.player_ref();
    let (v, v2) = (p.head().pos, p.hips().pos);
    let w = g.world.object(ids[0]).realized.as_deref().expect("spear");
    let to = v2.lerp(v, 0.6) - (v2 - v).perpendicular() * (7.5 * flip);
    assert!(
        w.chunk().pos.distance(to) < 1.0,
        "{:?} {:?}",
        w.chunk().pos,
        to
    );
    assert_eq!(w.rotation_speed, 0.0);
}

/// Floating in a room without gravity the lean eases to 0 by `LerpAndTick(flip, 0, 0.15,
/// 1/7)` whatever the body does (with gravity: `0.15`, `1/6` toward the body's lean).
#[test]
fn the_back_spear_lean_eases_off_in_zero_gravity() {
    let (mut g, ids) = game(&SlugcatCampaign::HUNTER, vec![spear(1, (6, 1))]);
    g.stash(ids[0]);
    g.tick(Input::NONE, &mut ());
    g.spear_on_back_mut().expect("hunter").flip = 0.5;
    g.world.realized_mut(0).expect("room").gravity = 0.0;
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.player_ref().body_mode(), crate::BodyMode::ZeroG);
    let flip = g.spear_on_back().expect("hunter").flip;
    assert!((flip - (0.5 * 0.85 - 1.0 / 7.0)).abs() < 1e-5, "{flip}");
}

#[test]
fn holding_one_spear_another_in_reach_goes_straight_to_the_back() {
    // `CanIPickThisUp`'s Hunter return: a free spear is a candidate while the back is
    // empty, past the two-hand check (a held spear is `BigOneHand`); the pick-up branch
    // then sends it to the back since a hand holds something `BigOneHand`.
    let (mut g, ids) = game(
        &SlugcatCampaign::HUNTER,
        vec![spear(1, (4, 1)), spear(2, (5, 1))],
    );
    g.hold_spear(ids[0]);
    for _ in 0..25 {
        g.tick(Input::NONE, &mut ());
    }
    assert_eq!(
        g.player_ref().grab_state().pick_up_candidate,
        Some(Entity::Object(ids[1]))
    );
    g.tick(PICKUP, &mut ());
    assert!(g.played(SoundId::SlugcatSwitchHandsInit));
    assert_eq!(g.back_spear(), Some(ids[1]));
    assert_eq!(g.player_grasps(), [Some(Entity::Object(ids[0])), None]);
}

#[test]
fn the_survivor_holding_a_spear_cannot_take_a_second() {
    // No back: the two-hand check refuses a second `BigOneHand`.
    let (mut g, ids) = game(
        &SlugcatCampaign::SURVIVOR,
        vec![spear(1, (4, 1)), spear(2, (5, 1))],
    );
    g.hold_spear(ids[0]);
    assert!(!g.can_i_pick_this_up(Entity::Object(ids[1])));
}

#[test]
fn a_lodged_spear_dislodges_before_the_stash_sound() {
    // `SpearToBack` -> `ChangeMode(OnBack)` out of `StuckInCreature` plays
    // `Spear_Dislodged_From_Creature` inside `ChangeMode`, then
    // `Slugcat_Stash_Spear_On_Back`.
    let (mut g, ids) = game(&SlugcatCampaign::HUNTER, vec![spear(1, (6, 1))]);
    g.world
        .object_mut(ids[0])
        .realized
        .as_deref_mut()
        .expect("spear")
        .mode = WeaponMode::StuckInCreature;
    g.sounds.take_events();
    g.spear_to_back(0, ids[0]);
    let played: Vec<SoundId> = g
        .sounds
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            SoundEvent::Play(p) => Some(p.id),
            _ => None,
        })
        .collect();
    assert_eq!(
        played,
        vec![
            SoundId::SpearDislodgedFromCreature,
            SoundId::SlugcatStashSpearOnBack
        ]
    );
}

#[test]
fn connected_objects_follow_stick_creation_order() {
    // One `stuckObjects` list in creation order: a spear stashed, then a rock picked
    // up, gives [player, spear, rock] (a shortcut spits them out in that order).
    let (mut g, ids) = game(
        &SlugcatCampaign::HUNTER,
        vec![spear(1, (6, 1)), rock(2, (8, 1))],
    );
    g.stash(ids[0]);
    g.slugcat_grab(Entity::Object(ids[1]), 0, &mut ());
    assert_eq!(
        g.world.connected_objects(Entity::Creature(g.player_id)),
        vec![
            Entity::Creature(g.player_id),
            Entity::Object(ids[0]),
            Entity::Object(ids[1])
        ]
    );
}
