//! Thrown weapons meeting flies and overseers, in one test room. Expected values from
//! `Weapon.Update`'s layer-0 walk (81606), `HitSomethingWithoutStopping` (81784),
//! `Spear.TryImpaleSmallCreature` (271955), `Overseer.Die` (228937) and
//! `HitByWeapon` (228965), `Creature.Die` (77465) and `SaveState.AddCreatureToRespawn`
//! (48297).

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, CreatureType, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::creature::fly::FlyRoomTick;
use crate::creature::{
    Community, CreatureId, CreatureTemplate, DamageType, EntityId, RealizedCreature,
    ViolenceTemplate, WorldCoordinate,
};
use crate::game::OverseerCall;
use crate::item::{AbstractObject, Entity, ObjectId, SpearStickKind, WeaponMode};
use crate::tests::room;
use crate::world::World;
use crate::{Input, RainWorldGame, Sounds, Start, Vec2};

/// A Survivor story in a 20 x 8 room with walls round it and `objects`, the player
/// stood at (2, 1).
fn game(objects: Vec<AbstractObject>) -> (RainWorldGame, Vec<ObjectId>) {
    game_with_placed(objects, "")
}

/// [`game`] with the room's placed objects.
fn game_with_placed(objects: Vec<AbstractObject>, placed: &str) -> (RainWorldGame, Vec<ObjectId>) {
    let mut rows = vec!["####################"];
    rows.extend(["#..................#"; 6]);
    rows.push("####################");
    let room = room(&rows);
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        placed_objects: rw_assets::PlacedObject::parse_list(placed),
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
            tile: Some((2, 1)),
        },
        UnityRandom::new(5),
        Sounds::default(),
    );
    for _ in 0..20 {
        g.tick(Input::NONE, &mut ());
    }
    (g, ids)
}

fn rock(n: i32) -> AbstractObject {
    AbstractObject::new_rock(EntityId::new(0, n), WorldCoordinate::new(0, 10, 4, -1))
}

fn spear(n: i32) -> AbstractObject {
    AbstractObject::new_spear(
        EntityId::new(0, n),
        WorldCoordinate::new(0, 10, 4, -1),
        false,
    )
}

/// The first seed whose first `value` is below `limit` (or not, with `below` false).
fn seed_with_first_value(limit: f32, below: bool) -> i32 {
    (0..)
        .find(|&s| (UnityRandom::new(s).value() < limit) == below)
        .expect("a seed")
}

impl RainWorldGame {
    /// A realized fly still at `pos` in room 0.
    fn hit_test_fly(&mut self, pos: Vec2, spawner: i32) -> CreatureId {
        let session = self.story_session();
        let template = CreatureTemplate::of(CreatureType::Fly, ActiveMods::NONE);
        let id = self.ids.new_id(spawner);
        let at = WorldCoordinate::new(0, 10, 4, -1);
        let c = self
            .world
            .new_creature(template, at, id, &session, &mut self.ids, &mut self.rng);
        let fly = self.world.insert_creature(c);
        self.world.add_entity(fly);
        let mut cx = FlyRoomTick {
            rng: &mut self.rng,
            observer: &mut (),
            camera_room: self.camera_room,
        };
        self.world.realize_fly_in_room(fly, 0, &mut cx);
        let f = self.world.fly_at(fly);
        f.chunk_mut().hard_set_position(pos);
        f.chunk_mut().vel = Vec2::ZERO;
        fly
    }

    /// A realized overseer in room 0 with its body still at `pos` (the guide when
    /// `guide`).
    fn hit_test_overseer(&mut self, pos: Vec2, guide: bool) -> CreatureId {
        let session = self.story_session();
        let template = CreatureTemplate::of(CreatureType::Overseer, ActiveMods::NONE);
        let id = if guide {
            EntityId::new(-1, 5)
        } else {
            self.ids.new_id(-1)
        };
        let at = WorldCoordinate::new(0, 10, 4, -1);
        let c = self
            .world
            .new_creature(template, at, id, &session, &mut self.ids, &mut self.rng);
        let o = self.world.insert_creature(c);
        self.world.add_entity(o);
        if guide && let Some(a) = self.world.creature_mut(o).overseer_ai_mut() {
            a.set_as_player_guide(1);
        }
        assert!(self.world.realize_overseer(o, 0, &mut self.rng));
        self.run_overseer(o, OverseerCall::Place, &mut ());
        let Some(RealizedCreature::Overseer(b)) = &mut self.world.creature_mut(o).realized else {
            panic!("realized");
        };
        b.chunk_mut().hard_set_position(pos);
        b.chunk_mut().vel = Vec2::ZERO;
        o
    }

    /// Weapon `id` thrown by the player, its last move from `from` to `to`.
    fn fly_through(&mut self, id: ObjectId, from: Vec2, to: Vec2) {
        let player = self.player_id;
        let w = self.world.weapon_mut(id).expect("realized");
        w.mode = WeaponMode::Thrown;
        w.thrown_by = Some(player);
        w.throw_dir = (1, 0);
        w.rotation = Vec2::new(1.0, 0.0);
        let c = w.chunk_mut();
        c.last_pos = from;
        c.pos = to;
        c.vel = to - from;
    }

    fn weapon_mode(&self, id: ObjectId) -> WeaponMode {
        self.world
            .object(id)
            .realized
            .as_deref()
            .expect("realized")
            .mode
    }

    fn fly_ref_dead(&self, id: CreatureId) -> bool {
        self.world.creature(id).fly().expect("fly").dead
    }
}

#[test]
fn a_thrown_rock_kills_a_fly_on_a_roll_below_0_8() {
    // `HitSomethingWithoutStopping`: a rock and a small creature, `Random.value < 0.8`
    // for a fly: `Die`. The fly weighs 0.05, so the rock flies on; one draw in all.
    let (mut g, ids) = game(vec![rock(1)]);
    let fly = g.hit_test_fly(Vec2::new(200.0, 90.0), -1);
    g.fly_through(ids[0], Vec2::new(170.0, 90.0), Vec2::new(230.0, 90.0));
    g.rng = UnityRandom::new(seed_with_first_value(0.8, true));
    let mut expected = g.rng.clone();
    expected.value();
    g.thrown_through_layer_zero(0, ids[0], &mut ());
    assert!(g.fly_ref_dead(fly));
    assert!(!g.world.creature(fly).state.alive);
    assert_eq!(g.world.creature(fly).kill_tag, Some(g.player_id));
    assert_eq!(g.rng, expected, "one draw");
    assert_eq!(
        g.weapon_mode(ids[0]),
        WeaponMode::Thrown,
        "0.05 is not enough to stop it"
    );
    // `Killing` by the slugcat: a kill record (a fly does not end the peace and has no
    // community).
    let kills = &g.world.session.kills;
    assert_eq!(kills.len(), 1);
    assert_eq!(kills[0].kind, CreatureType::Fly);
    assert!(g.world.session.peaceful);
    assert!(g.world.session.influences.is_empty());
}

#[test]
fn a_thrown_rock_stuns_a_fly_on_a_roll_of_0_8_or_more() {
    let (mut g, ids) = game(vec![rock(1)]);
    let fly = g.hit_test_fly(Vec2::new(200.0, 90.0), -1);
    g.fly_through(ids[0], Vec2::new(170.0, 90.0), Vec2::new(230.0, 90.0));
    g.rng = UnityRandom::new(seed_with_first_value(0.8, false));
    let mut expected = g.rng.clone();
    expected.value();
    g.thrown_through_layer_zero(0, ids[0], &mut ());
    assert!(!g.fly_ref_dead(fly));
    assert_eq!(g.world.creature(fly).fly().expect("fly").stun, 80);
    assert_eq!(g.rng, expected, "one draw");
    assert!(g.world.session.kills.is_empty());
}

#[test]
fn a_rock_that_misses_the_fly_does_nothing() {
    let (mut g, ids) = game(vec![rock(1)]);
    let fly = g.hit_test_fly(Vec2::new(200.0, 140.0), -1);
    g.fly_through(ids[0], Vec2::new(170.0, 90.0), Vec2::new(230.0, 90.0));
    let expected = g.rng.clone();
    g.thrown_through_layer_zero(0, ids[0], &mut ());
    assert!(!g.fly_ref_dead(fly));
    assert_eq!(g.world.creature(fly).kill_tag, None);
    assert_eq!(g.rng, expected);
}

#[test]
fn a_thrown_spear_kills_and_impales_flies_and_carries_them() {
    // A spear kills without a draw and impales each fly in turn (places 0, 1); the end
    // of `Spear.Update` holds them on the shaft at LerpMap(place, 0, 4, 15, -15).
    let (mut g, ids) = game(vec![spear(1)]);
    let a = g.hit_test_fly(Vec2::new(190.0, 90.0), -1);
    let b = g.hit_test_fly(Vec2::new(215.0, 90.0), -1);
    g.fly_through(ids[0], Vec2::new(170.0, 90.0), Vec2::new(240.0, 90.0));
    let expected = g.rng.clone();
    g.thrown_through_layer_zero(0, ids[0], &mut ());
    assert_eq!(g.rng, expected, "no draws");
    assert!(g.fly_ref_dead(a) && g.fly_ref_dead(b));
    let places: Vec<(CreatureId, SpearStickKind)> = g
        .world
        .spear_sticks_of(Entity::Object(ids[0]))
        .iter()
        .map(|s| (s.creature, s.kind))
        .collect();
    assert_eq!(
        places,
        [
            (
                a,
                SpearStickKind::Impaled {
                    chunk: 0,
                    on_spear_position: 0
                }
            ),
            (
                b,
                SpearStickKind::Impaled {
                    chunk: 0,
                    on_spear_position: 1
                }
            ),
        ]
    );
    // Held on the shaft: the fly's update has the same parity as the spear's.
    let eu = g.world.creature(a).fly().expect("fly").even_update;
    g.world.hold_impaled(0, ids[0], eu);
    let spear_pos = g.world.weapon_mut(ids[0]).expect("spear").chunk().pos;
    let at = g.world.creature(a).fly().expect("fly").chunk().pos;
    assert_eq!(at, spear_pos + Vec2::new(15.0, 0.0));
    let at = g.world.creature(b).fly().expect("fly").chunk().pos;
    assert_eq!(at, spear_pos + Vec2::new(7.5, 0.0));
    // The flies go where the spear goes (`GetAllConnectedObjects`).
    let connected = g.world.connected_objects(Entity::Object(ids[0]));
    assert!(connected.contains(&Entity::Creature(a)));
    assert!(connected.contains(&Entity::Creature(b)));
}

#[test]
fn a_held_fly_comes_off_the_spear() {
    let (mut g, ids) = game(vec![spear(1)]);
    let fly = g.hit_test_fly(Vec2::new(200.0, 90.0), -1);
    g.fly_through(ids[0], Vec2::new(170.0, 90.0), Vec2::new(230.0, 90.0));
    g.thrown_through_layer_zero(0, ids[0], &mut ());
    assert_eq!(g.world.spear_sticks_of(Entity::Creature(fly)).len(), 1);
    g.world
        .creature_mut(fly)
        .grabbed_by
        .push(crate::creature::Grasp {
            grabber: g.player_id,
            grabbed: Entity::Creature(fly),
            grasp_used: 0,
            chunk_grabbed: 0,
            shareability: crate::creature::Shareability::CanNotShare,
            dominance: 0.5,
            pacifying: true,
            serial: 0,
        });
    g.world.hold_impaled(0, ids[0], false);
    assert!(g.world.spear_sticks_of(Entity::Creature(fly)).is_empty());
}

#[test]
fn a_rock_stuns_an_overseer_80_and_the_guide_likes_the_player_less() {
    // Not a small creature: no roll, `Stun(80)`; the overseer weighs 0.4, so the rock
    // goes on and also crosses its appendage, hitting it again. Each `HitByWeapon` of
    // the guide by the slugcat: like -0.1, no like raise at the save.
    let (mut g, ids) = game(vec![rock(1)]);
    let o = g.hit_test_overseer(Vec2::new(200.0, 90.0), true);
    let root = g.world.creature(o).overseer_body().expect("body").root_pos;
    g.fly_through(ids[0], Vec2::new(170.0, 90.0), Vec2::new(230.0, 90.0));
    let crosses = g
        .world
        .creature(o)
        .overseer_body()
        .expect("body")
        .appendage_line_cross(Vec2::new(170.0, 90.0), Vec2::new(230.0, 90.0));
    let like = g.guide.likes_player;
    g.thrown_through_layer_zero(0, ids[0], &mut ());
    let b = g.world.creature(o).overseer_body().expect("body");
    assert_eq!(b.stun, 80);
    assert!(!b.dead);
    let hits = if crosses { 2.0 } else { 1.0 };
    assert!(
        (g.guide.likes_player - (like - 0.1 * hits)).abs() < 1e-6,
        "{root:?}"
    );
    assert!(!g.guide.increase_like_on_save);
}

#[test]
fn a_spear_kills_the_guide_out_of_view_and_a_new_one_waits_in_the_den() {
    let (mut g, ids) = game(vec![spear(1)]);
    let o = g.hit_test_overseer(Vec2::new(200.0, 90.0), true);
    let before = g.world.creatures().count();
    // Out of the camera's view (`ViewedByAnyCamera(pos, 0)` false).
    g.world.set_kill_tag(o, g.player_id);
    if let Some(RealizedCreature::Overseer(b)) = &mut g.world.creature_mut(o).realized {
        b.chunk_mut().pos = Vec2::new(-5000.0, -5000.0);
    }
    g.overseer_die(o);
    assert!(g.world.realized_dead(o));
    assert!(!g.world.creature(o).state.alive);
    assert_eq!(g.world.creatures().count(), before + 1);
    let (new, c) = g.world.creatures().last().expect("the new guide");
    assert_eq!(c.id, EntityId::new(-1, 5));
    assert_eq!(c.pos.room, g.world.offscreen_den_index());
    assert!(c.overseer_ai().expect("overseer").is_player_guide);
    assert_eq!(
        g.world.overseers_world_ai().and_then(|w| w.player_guide),
        Some(new)
    );
    // The overseer counts as a kill: the peace is broken, and it has the `All`
    // community (influence 0.5 times -0.05).
    assert!(!g.world.session.peaceful);
    assert_eq!(g.world.session.kills[0].kind, CreatureType::Overseer);
    let i = g.world.session.influences[0];
    assert_eq!(i.community, Community::All);
    assert!((i.influence - -0.025).abs() < 1e-7);
    let _ = ids;
}

#[test]
fn the_guide_killed_by_the_player_in_view_is_gone_till_next_cycle() {
    let (mut g, _) = game(vec![]);
    let o = g.hit_test_overseer(Vec2::new(200.0, 90.0), true);
    let before = g.world.creatures().count();
    g.world.set_kill_tag(o, g.player_id);
    g.overseer_die(o);
    assert!(g.world.realized_dead(o));
    assert_eq!(g.world.creatures().count(), before);
}

#[test]
fn a_dead_overseer_dies_away_in_six_ticks() {
    // `Update`'s dead branch: four draws a tick for the shake, `Overseer_Death` on the
    // first, `dying` up by a sixth a tick, destroyed at 1.
    let (mut g, _) = game(vec![]);
    let o = g.hit_test_overseer(Vec2::new(200.0, 90.0), false);
    g.overseer_die(o);
    for tick in 1..=6 {
        g.run_overseer(o, OverseerCall::Update, &mut ());
        let b = g.world.creature(o).overseer_body().expect("body");
        assert!(
            (b.dying - (tick as f32 / 6.0).min(1.0)).abs() < 1e-5,
            "{tick}"
        );
        assert_eq!(b.slated, tick == 6, "{tick}");
    }
    assert!(g.world.room_overseers(0).is_empty());
}

#[test]
fn a_lineage_killed_by_the_player_may_progress_and_waits_to_respawn() {
    // `AddCreatureToRespawn`: killed by the slugcat, the spawner goes on the wait list;
    // otherwise straight back on the respawn list. A fly from no spawner is not added.
    let (mut g, _) = game(vec![]);
    let by_player = g.hit_test_fly(Vec2::new(200.0, 90.0), 7);
    let other = g.hit_test_fly(Vec2::new(250.0, 90.0), 8);
    let none = g.hit_test_fly(Vec2::new(300.0, 90.0), -1);
    g.world.set_kill_tag(by_player, g.player_id);
    for f in [by_player, other, none] {
        g.world.fly_die(f, &mut g.rng);
    }
    assert_eq!(g.world.session.wait_respawn_creatures, [7]);
    assert_eq!(g.world.session.respawn_creatures, [8]);
}

#[test]
fn violence_divides_by_the_templates_resistances() {
    // `Creature.Violence`'s damage and stun before they are applied: a fly's limit is 1,
    // so a spear's 1 damage over 0.15 kills it at once.
    let fly = ViolenceTemplate::of(CreatureType::Fly);
    let (dmg, stun) = fly.violence(DamageType::Stab, 1.0, 20.0, 1.0);
    assert!((dmg - 1.0 / 0.15).abs() < 1e-5);
    assert!((stun - 50.0 / 0.2).abs() < 1e-3);
    assert!(dmg >= fly.instant_death_damage_limit);
    // A red lizard resists explosions by 1.9 on top of its toughness.
    let red = ViolenceTemplate::of(CreatureType::RedLizard);
    let (dmg, _) = red.violence(DamageType::Explosion, 1.0, 0.0, 1.0);
    assert!((dmg - 1.0 / 6.0 / 1.9).abs() < 1e-6);
    assert_eq!(
        ViolenceTemplate::of(CreatureType::Overseer).counts_as_a_kill,
        1
    );
}

#[test]
fn line_intersection_matches_custom() {
    let p = Vec2::new(0.0, 0.0).line_intersection(
        Vec2::new(10.0, 10.0),
        Vec2::new(0.0, 10.0),
        Vec2::new(10.0, 0.0),
    );
    assert!((p.x - 5.0).abs() < 1e-5 && (p.y - 5.0).abs() < 1e-5);
    // A vertical first line crosses at its x.
    let p = Vec2::new(3.0, 0.0).line_intersection(
        Vec2::new(3.0, 10.0),
        Vec2::new(0.0, 2.0),
        Vec2::new(10.0, 4.0),
    );
    assert!((p.x - 3.0).abs() < 1e-6 && (p.y - 2.6).abs() < 1e-5);
}

#[test]
fn a_spear_flying_at_the_still_guide_makes_it_withdraw_and_like_the_player_less() {
    // `Weapon.Update` tells the overseer (closest ahead of the throw); `FlyingWeapon`:
    // still for 40 ticks and watching, `Random.value < 0.75` with the spear heading
    // within 100 px of it: it withdraws into the wall, and the guide likes the player
    // 0.1 less for a spear, and looks at the spear.
    let (mut g, ids) = game(vec![spear(1)]);
    let o = g.hit_test_overseer(Vec2::new(260.0, 90.0), true);
    if let Some(RealizedCreature::Overseer(b)) = &mut g.world.creature_mut(o).realized {
        b.stationary_counter = 40;
    }
    g.fly_through(ids[0], Vec2::new(170.0, 90.0), Vec2::new(200.0, 90.0));
    g.world.weapon_mut(ids[0]).expect("spear").thrown_pos = Vec2::new(170.0, 90.0);
    g.rng = UnityRandom::new(seed_with_first_value(0.75, true));
    let like = g.guide.likes_player;
    g.notify_of_flying_weapon(0, ids[0], &mut ());
    let w = g.world.weapon_mut(ids[0]).expect("spear");
    assert_eq!(w.thrown_closest_to_creature, Some(o));
    assert!((w.closest_crit_dist - 60.0).abs() < 1e-4);
    let b = g.world.creature(o).overseer_body().expect("body");
    assert_eq!(b.mode, crate::creature::overseer::OverseerMode::Withdrawing);
    assert_eq!(
        b.after_withdraw_mode,
        crate::creature::overseer::OverseerMode::SittingInWall
    );
    assert!((g.guide.likes_player - (like - 0.1)).abs() < 1e-6);
    let ai = g.world.creature(o).overseer_ai().expect("ai");
    let real = ai.real_ai.as_ref().expect("realized AI");
    assert_eq!(real.look_at_flying_weapon, Some(ids[0]));
}

#[test]
fn a_rock_flying_by_the_moving_guide_is_only_looked_at() {
    // Not still (`stationaryCounter` under 10): the roll is made but fails; no change of
    // like.
    let (mut g, ids) = game(vec![rock(1)]);
    let o = g.hit_test_overseer(Vec2::new(260.0, 90.0), true);
    if let Some(RealizedCreature::Overseer(b)) = &mut g.world.creature_mut(o).realized {
        b.stationary_counter = 0;
    }
    g.fly_through(ids[0], Vec2::new(170.0, 90.0), Vec2::new(200.0, 90.0));
    let like = g.guide.likes_player;
    let mut expected = g.rng.clone();
    expected.value();
    g.notify_of_flying_weapon(0, ids[0], &mut ());
    assert_eq!(g.rng, expected, "one draw");
    assert_eq!(g.guide.likes_player, like);
    let ai = g.world.creature(o).overseer_ai().expect("ai");
    assert_eq!(
        ai.real_ai
            .as_ref()
            .expect("realized AI")
            .look_at_flying_weapon,
        Some(ids[0])
    );
}

/// A coral stem's segment is a layer-0 object that can be hit: a weapon whose last move
/// crossed one of its chunks pushes it along by its velocity; the hit chunks' masses sum
/// toward 0.6, a light one lets the weapon fly on, a heavy one stops it there (`Free`).
/// No draws but the stop's sound.
#[test]
fn a_thrown_rock_hits_a_stem_segment() {
    let (mut g, ids) = game_with_placed(vec![rock(1)], "CoralStem><200><60><0~40");
    let at = g.world.realized(0).unwrap().coral.as_ref().unwrap().stems[0].segments[0]
        .body
        .chunks[1]
        .pos;
    let (from, to) = (at - Vec2::new(30.0, 0.0), at + Vec2::new(30.0, 0.0));
    g.set_stem_chunk_mass(0.1);
    let vel_before = g.stem_chunk().vel;
    g.fly_through(ids[0], from, to);
    let state = g.rng.clone();
    g.thrown_through_layer_zero(0, ids[0], &mut ());
    assert_eq!(g.stem_chunk().vel, vel_before + Vec2::new(60.0, 0.0));
    assert_eq!(g.weapon_mode(ids[0]), WeaponMode::Thrown);
    assert_eq!(g.rng, state, "no draws");
    g.set_stem_chunk_mass(0.7);
    g.fly_through(ids[0], from, to);
    g.thrown_through_layer_zero(0, ids[0], &mut ());
    assert_eq!(g.weapon_mode(ids[0]), WeaponMode::Free);
    let w = g.world.weapon_mut(ids[0]).unwrap();
    assert_eq!(w.chunk_mut().pos, at);
}

impl RainWorldGame {
    fn stem_chunk(&self) -> crate::BodyChunk {
        self.world
            .realized(0)
            .unwrap()
            .coral
            .as_ref()
            .unwrap()
            .stems[0]
            .segments[0]
            .body
            .chunks[1]
            .clone()
    }

    fn set_stem_chunk_mass(&mut self, mass: f32) {
        self.world
            .realized_mut(0)
            .unwrap()
            .coral
            .as_mut()
            .unwrap()
            .stems[0]
            .segments[0]
            .body
            .chunks[1]
            .mass = mass;
    }
}
