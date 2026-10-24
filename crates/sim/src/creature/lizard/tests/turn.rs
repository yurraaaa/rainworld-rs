//! One green lizard in one test room. Expected values from the `Lizard` constructor
//! (`Lizard.ctor`), `Lizard.Update`'s health rolls, `Violence`,
//! `HitHeadShield`, `HitInMouth`, `Stun`, `SpearStick`, `Rock.HitSomething`,
//! `Spear.HitSomething`/`LodgeInCreature`, `Creature.Update`'s health death roll and
//! `LizardBreeds.BreedTemplate` (green: toughness 2.5, so damage resistance 5 and stun
//! resistance 2.5; body size 1.2, mass 7.5).

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, CreatureType, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::creature::lizard::MovementAnimation;
use crate::creature::lizard::hits_turn::{Hit, ViolenceSource};
use crate::creature::{
    CreatureId, CreatureTemplate, DamageType, EntityId, Lizard, WorldCoordinate,
};
use crate::effects::cosmetic::CosmeticKind;
use crate::item::{AbstractObject, Entity, ObjectId, SpearStickKind, WeaponMode};
use crate::tests::room;
use crate::world::{MovementConnection, MovementType, World};
use crate::{Input, RainWorldGame, Sounds, Start, Vec2};

/// A Survivor story in a 20 x 8 walled room with `objects`, the player at (2, 1).
fn game(objects: Vec<AbstractObject>) -> (RainWorldGame, Vec<ObjectId>) {
    game_with_sounds(objects, Sounds::default())
}

/// [`game`] playing sounds into `sounds`.
fn game_with_sounds(
    objects: Vec<AbstractObject>,
    sounds: Sounds,
) -> (RainWorldGame, Vec<ObjectId>) {
    let mut rows = vec!["####################"];
    rows.extend(["#..................#"; 6]);
    rows.push("####################");
    let room = room(&rows);
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        // No rain danger: `Room.Update`'s ceiling-drip roll makes no draws here.
        danger_type: Some(rw_assets::DangerType::None),
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
        sounds,
    );
    g.tick_until_ready(&mut ());
    for _ in 0..20 {
        g.tick(Input::NONE, &mut ());
    }
    (g, ids)
}

/// Head, middle and hips along the floor, facing right, head at x 200.
const BODY: [Vec2; 3] = [
    Vec2::new(200.0, 29.6),
    Vec2::new(181.3, 29.6),
    Vec2::new(162.6, 29.6),
];

impl RainWorldGame {
    /// An abstract green lizard at tile (10, 3) of room 0.
    fn abstract_lizard(&mut self) -> CreatureId {
        let session = self.story_session();
        let template = CreatureTemplate::of(CreatureType::GreenLizard, ActiveMods::NONE);
        let id = self.ids.new_id(-1);
        let at = WorldCoordinate::new(0, 10, 3, -1);
        let c = self
            .world
            .new_creature(template, at, id, &session, &mut self.ids, &mut self.rng);
        let l = self.world.insert_creature(c);
        self.world.add_entity(l);
        l
    }

    /// A realized green lizard in room 0, placed at its tile (`PlaceInRoom`).
    fn placed_lizard(&mut self) -> CreatureId {
        let l = self.abstract_lizard();
        assert!(self.world.realize_lizard(l, &mut self.rng));
        let mut cx = crate::world::LizardRoomCx {
            rng: &mut self.rng,
            observer: &mut (),
            camera_room: -1,
        };
        self.world.place_lizard(l, 0, &mut cx);
        l
    }

    /// A realized green lizard lying still at [`BODY`].
    fn lying_lizard(&mut self) -> CreatureId {
        let l = self.placed_lizard();
        for (c, pos) in self.world.lizard_at(l).body.chunks.iter_mut().zip(BODY) {
            c.hard_set_position(pos);
            c.vel = Vec2::ZERO;
        }
        l
    }

    /// Spark `id`'s `whiteFlicker` as last read, while it lives in room 0.
    fn spark_flicker(&self, id: u64) -> Option<i32> {
        self.world
            .room_cosmetics(0)
            .iter()
            .find(|c| c.id == id)
            .and_then(|c| match &c.kind {
                CosmeticKind::Spark(s) => Some(s.white_flicker),
                _ => None,
            })
    }

    fn rock_hit(&self, chunk: usize, dir: Vec2) -> Hit {
        Hit {
            source: None,
            direction_and_momentum: Some(dir),
            chunk: Some(chunk),
            kind: DamageType::Blunt,
            damage: 0.01,
            stun_bonus: 45.0,
        }
    }

    /// Weapon `id` thrown by the player from `pos` at `vel`.
    fn throw_at(&mut self, id: ObjectId, pos: Vec2, vel: Vec2) {
        let player = self.player_id;
        let w = self.world.weapon_mut(id).expect("realized");
        w.mode = WeaponMode::Thrown;
        w.thrown_by = Some(player);
        w.throw_dir = (vel.x.signum() as i32, 0);
        w.rotation = vel.normalized();
        w.first_frame_trace_from_pos = None;
        let c = w.chunk_mut();
        c.hard_set_position(pos);
        c.vel = vel;
    }

    fn lizard_test_weapon_mode(&self, id: ObjectId) -> WeaponMode {
        self.world
            .object(id)
            .realized
            .as_deref()
            .expect("realized")
            .mode
    }

    fn lizard_tick(&mut self, l: CreatureId) {
        let room = Arc::clone(&self.world.realized(0).expect("room 0").room);
        self.run_lizard(0, l, &room, &mut ());
    }
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

#[test]
fn the_constructor_builds_the_green_body() {
    let (mut g, _) = game(vec![]);
    let l = g.abstract_lizard();
    let before = g.rng.clone();
    let lizard = Lizard::new(g.world.creature(l), &mut g.rng).expect("green data");
    assert_eq!(g.rng, before, "effectColor is drawn under the ID's seed");
    for c in lizard.chunks() {
        // 8 * bodySizeFac 1.2; bodyMass 7.5 / 3.
        assert!((c.rad - 9.6).abs() < 1e-5);
        assert!((c.mass - 2.5).abs() < 1e-5);
        assert!(c.go_through_floors());
    }
    let rods = &lizard.body.connections;
    // 17 * 1 * (1.2 + 1) / 2, and the push at 1 + bodyStiffnes 0.5 times it with
    // elasticity 1 - Lerp(0.9, 0.5, 0.5).
    assert!((rods[0].distance - 18.7).abs() < 1e-4);
    assert!((rods[1].distance - 18.7).abs() < 1e-4);
    assert!((rods[2].distance - 28.05).abs() < 1e-4);
    assert!((rods[2].elasticity - 0.3).abs() < 1e-5);
    // Hue 0.32 +- 0.1, full saturation: green is the largest channel.
    let [r, gr, b] = lizard.effect_color;
    assert!(gr >= r && gr >= b, "{:?}", lizard.effect_color);
    let again = Lizard::new(g.world.creature(l), &mut g.rng).expect("green data");
    assert_eq!(again.effect_color, lizard.effect_color, "seeded by the ID");
}

#[test]
fn the_mean_spawn_flag_sets_spawn_data_evil() {
    let (mut g, _) = game(vec![]);
    let l = g.abstract_lizard();
    g.world.creature_mut(l).spawn_data = Some("{Mean:0.7}".into());
    let lizard = Lizard::new(g.world.creature(l), &mut g.rng).expect("green data");
    assert!((lizard.spawn_data_evil - 0.7).abs() < 1e-6);
}

#[test]
fn a_placed_lizard_falls_and_rests_on_the_floor() {
    let (mut g, _) = game(vec![]);
    // The idle lizard lifts its head or hops at some seeds (3, 5 and 7 of the first 8, the
    // same on main: the lizard's construction draws decide them); seed 1 rests.
    g.rng = UnityRandom::new(1);
    let l = g.placed_lizard();
    assert_eq!(g.world.lizard_ref(l).new_to_room_invinsibility, 40);
    // Held still (no idle walk) to see the body settle.
    g.world.lizard_at(l).ai.dev_run_speed = Some(0.0);
    for _ in 0..60 {
        g.tick(Input::NONE, &mut ());
    }
    let lizard = g.world.lizard_ref(l);
    for c in lizard.chunks() {
        assert!((c.pos.y - 29.6).abs() < 1.0, "on the floor: {:?}", c.pos);
    }
    assert!(!lizard.dead);
    assert_eq!(lizard.new_to_room_invinsibility, 0);
    assert_eq!(g.world.creature(l).pos.room, 0);
    assert_eq!(g.world.creature(l).pos.tile.1, 1);
}

#[test]
fn a_healthy_stunned_lizard_update_makes_four_draws() {
    // Three health rolls (each stops after `value * k > health` fails) and the lungs';
    // stunned, it does not `Act`.
    let (mut g, _) = game(vec![]);
    let l = g.lying_lizard();
    g.world.lizard_at(l).stun = 1000;
    for _ in 0..40 {
        g.lizard_tick(l);
    }
    for _ in 0..5 {
        let mut expected = g.rng.clone();
        for _ in 0..4 {
            expected.value();
        }
        g.lizard_tick(l);
        assert_eq!(g.rng, expected);
    }
}

#[test]
fn a_rock_to_the_body_hurts_and_stuns_with_one_draw() {
    let (mut g, _) = game(vec![]);
    let l = g.lying_lizard();
    let mut expected = g.rng.clone();
    expected.value();
    g.creature_violence(l, g.rock_hit(1, Vec2::new(0.0, -5.0)));
    assert_eq!(g.rng, expected, "the stun's lengthening draw");
    // 0.01 / 5, and (0.01 * 30 + 45) / 2.5 at full health.
    assert!((g.world.creature(l).state.health - 0.998).abs() < 1e-6);
    let lizard = g.world.lizard_ref(l);
    assert_eq!(lizard.stun, 18);
    // The push over the chunk's mass.
    assert!((lizard.chunks()[1].vel.y + 2.0).abs() < 1e-5);
}

#[test]
fn the_head_shield_turns_a_hit_from_the_front() {
    let (mut g, _) = game(vec![]);
    let l = g.lying_lizard();
    let mut expected = g.rng.clone();
    expected.value();
    g.creature_violence(l, g.rock_hit(0, Vec2::new(-3.0, 0.0)));
    assert_eq!(g.rng, expected, "a silent room: the stun's draw only");
    // A tenth of the damage; (0.01 * 0.5 * 30 + 45 * 2/3) / 2.5 stun; a third of the push.
    assert!((g.world.creature(l).state.health - 0.9998).abs() < 1e-6);
    let lizard = g.world.lizard_ref(l);
    assert_eq!(lizard.stun, 12);
    assert!((lizard.chunks()[0].vel.x + 0.4).abs() < 1e-5);
}

#[test]
fn a_hit_in_the_open_mouth_hurts_more_and_the_throat() {
    let (mut g, _) = game(vec![]);
    let l = g.lying_lizard();
    *g.world.lizard_at(l).jaw_open_field_mut() = 1.0;
    g.creature_violence(l, g.rock_hit(0, Vec2::new(-3.0, 0.0)));
    // Half again the damage, double the stun.
    let state = &g.world.creature(l).state;
    assert!((state.health - 0.997).abs() < 1e-6);
    let (throat, _) = state.lizard_health_parts().expect("a lizard state");
    assert!((throat - 0.997).abs() < 1e-6);
    assert_eq!(g.world.lizard_ref(l).stun, 36);
    assert_eq!(
        g.world.creature(l).state_save_string(),
        "<cB>Health<cC>0.997<cB>ThroatHealth<cC>0.997"
    );
}

#[test]
fn a_hurt_lizard_is_stunned_longer() {
    // `Stun` below half health: times 1 + InverseLerp(0.75, 0, health) * 1.5.
    let (mut g, _) = game(vec![]);
    let l = g.lying_lizard();
    g.world.creature_mut(l).state.health = 0.25;
    g.lizard_stun(l, 30);
    assert_eq!(g.world.lizard_ref(l).stun, 60);
}

/// `Lizard.Stun` with a grip point and a movement animation set: stun, grip point,
/// whether the animation is left.
fn stun_gripping(st: i32) -> (i32, Option<Vec2>, bool) {
    let (mut g, _) = game(vec![]);
    let l = g.lying_lizard();
    let lizard = g.world.lizard_at(l);
    let chunks = [
        lizard.body.chunks[0].pos,
        lizard.body.chunks[1].pos,
        lizard.body.chunks[2].pos,
    ];
    let conn = MovementConnection::new(
        MovementType::ReachUp,
        WorldCoordinate::new(0, 8, 1, -1),
        WorldCoordinate::new(0, 8, 3, -1),
        2,
    );
    lizard.movement_animation = Some(MovementAnimation::new(chunks, conn, 1, 0.99, 0.75));
    lizard.grip_point = Some(Vec2::middle_of_tile((8, 3)));
    let rng = g.rng.clone();
    g.lizard_stun(l, st);
    assert_eq!(g.rng, rng, "Stun draws nothing");
    let lizard = g.world.lizard_ref(l);
    (
        lizard.stun,
        lizard.grip_point,
        lizard.movement_animation.is_some(),
    )
}

#[test]
fn a_stunned_lizard_lets_go_of_its_grip_point() {
    // `if (Stunned) { gripPoint = null; movementAnimation = null; ... }`.
    assert_eq!(stun_gripping(10), (10, None, false));
}

#[test]
fn a_stun_below_ten_keeps_the_grip_point() {
    // `Stunned` is `stun >= 10`.
    assert_eq!(
        stun_gripping(9),
        (9, Some(Vec2::middle_of_tile((8, 3))), true)
    );
}

#[test]
fn a_thrown_rock_hits_the_lizard_and_bounces_off() {
    let (mut g, ids) = game(vec![rock(1)]);
    let l = g.lying_lizard();
    g.throw_at(ids[0], Vec2::new(181.3, 90.0), Vec2::new(0.0, -50.0));
    assert!(!g.trace_layer_one(0, ids[0]));
    assert_eq!(g.lizard_test_weapon_mode(ids[0]), WeaponMode::Free);
    assert!((g.world.creature(l).state.health - 0.998).abs() < 1e-6);
    assert_eq!(g.world.lizard_ref(l).stun, 18);
    assert_eq!(g.world.creature(l).kill_tag, Some(g.player_id));
    let w = g.world.weapon_mut(ids[0]).expect("realized");
    assert_eq!(w.forbidden_to_player, 40);
    assert!(w.thrown_by.is_none());
    assert!(w.chunk().vel.y > 0.0, "bounced back up");
    assert!(!g.world.session.peaceful, "a slugcat's hit");
}

#[test]
fn a_spear_lodges_in_the_hips_and_the_lizard_dies_of_its_wounds() {
    let (mut g, ids) = game(vec![spear(1)]);
    let l = g.lying_lizard();
    g.throw_at(ids[0], Vec2::new(120.0, 30.0), Vec2::new(50.0, 0.0));
    assert!(!g.trace_layer_one(0, ids[0]));
    assert_eq!(
        g.lizard_test_weapon_mode(ids[0]),
        WeaponMode::StuckInCreature
    );
    // Stab 1 / 5 and (30 + 20) / 2.5.
    assert!((g.world.creature(l).state.health - 0.8).abs() < 1e-6);
    assert_eq!(g.world.lizard_ref(l).stun, 20);
    let sticks = g.world.spear_sticks_of(Entity::Creature(l));
    assert_eq!(sticks.len(), 1);
    assert!(matches!(
        sticks[0].kind,
        SpearStickKind::Lodged { chunk: 2, .. }
    ));
    // Below zero the update rolls `value < -health && value < 0.025` every tick.
    g.world.creature_mut(l).state.health = -1.0;
    for _ in 0..2000 {
        g.tick(Input::NONE, &mut ());
        if g.world.lizard_ref(l).dead {
            break;
        }
    }
    assert!(g.world.lizard_ref(l).dead);
    assert!(!g.world.creature(l).state.alive);
    let kills = &g.world.session.kills;
    assert_eq!(kills.len(), 1);
    assert_eq!(kills[0].kind, CreatureType::GreenLizard);
    assert_eq!(
        g.world.creature(l).state_save_string(),
        "Dead<cB><cB>Health<cC>-1"
    );
}

#[test]
fn a_spear_on_the_head_shield_bounces_off() {
    let (mut g, ids) = game(vec![spear(1)]);
    let l = g.lying_lizard();
    g.throw_at(ids[0], Vec2::new(260.0, 30.0), Vec2::new(-50.0, 0.0));
    assert!(!g.trace_layer_one(0, ids[0]));
    assert_eq!(g.lizard_test_weapon_mode(ids[0]), WeaponMode::Free);
    assert!(g.world.spear_sticks_of(Entity::Creature(l)).is_empty());
    // A tenth of 1 / 5; (0.5 * 30 + 20 * 2/3) / 2.5.
    assert!((g.world.creature(l).state.health - 0.98).abs() < 1e-6);
    assert_eq!(g.world.lizard_ref(l).stun, 11);
}

#[test]
fn an_abstracted_lizard_realizes_again() {
    let (mut g, _) = game(vec![]);
    let l = g.lying_lizard();
    let pos = g.world.creature(l).pos;
    g.world.abstractize_lizard(l, pos, &mut g.rng);
    assert!(g.world.creature(l).lizard().is_none());
    assert!(g.world.room_lizards(0).is_empty());
    assert!(g.world.realize_lizard(l, &mut g.rng));
    let mut cx = crate::world::LizardRoomCx {
        rng: &mut g.rng,
        observer: &mut (),
        camera_room: -1,
    };
    g.world.place_lizard(l, 0, &mut cx);
    assert_eq!(g.world.room_lizards(0).len(), 1);
}

#[test]
fn a_realized_lizard_is_a_danger_to_the_player() {
    // `CreatureDangerScore`'s inputs: green `dangerousToPlayer`, `LizardAI.
    // CurrentPlayerAggression` without a player relationship (1) and the head.
    let (mut g, _) = game(vec![]);
    let l = g.lying_lizard();
    let d = g.world.creature_danger(l, g.player_id).expect("realized");
    assert_eq!(d.player_aggression, Some(1.0));
    assert_eq!(d.danger_pos, BODY[0]);
    assert!(d.dangerous_to_player > 0.0);
}

/// `LizardGraphics.TerrainImpact`: with graphics a first contact over 10 px a tick rolls
/// once; the head (or 60%) blows `1 + InverseLerp(20, 90, speed) * 3` bubbles, each a
/// stickiness draw before its constructor. Without graphics nothing is drawn.
#[test]
fn a_hard_landing_with_graphics_blows_bubbles() {
    let (mut g, _) = game(vec![]);
    let l = g.lying_lizard();
    let impact = crate::TerrainImpact {
        chunk: 0,
        direction: crate::Contact { x: 0, y: -1 },
        speed: 60.0,
        first_contact: true,
    };
    let before = g.rng.clone();
    g.lizard_impact_bubbles(0, l, &impact);
    assert_eq!(g.rng, before, "no graphics, no draws");
    g.world.lizard_at(l).graphics = Some(crate::creature::lizard::LizardGraphicsLink::default());
    let count = g.world.room_cosmetics(0).len();
    g.lizard_impact_bubbles(0, l, &impact);
    // 1 + (int)(InverseLerp(20, 90, 60) * 3) = 2.
    assert_eq!(g.world.room_cosmetics(0).len(), count + 2);
    for c in &g.world.room_cosmetics(0)[count..] {
        let crate::effects::cosmetic::CosmeticKind::LizardBubble(b) = &c.kind else {
            panic!("a bubble");
        };
        assert!(b.vel.y >= 0.0, "thrown up off the floor");
    }
}

/// `Lizard.Violence`'s head-shield sparks and flash are given the lizard's graphics
/// module; a spark reads `whiteFlicker` of that module only, not of a later one.
#[test]
fn head_shield_sparks_and_flash_keep_their_module() {
    let (mut g, _) = game(vec![]);
    let l = g.lying_lizard();
    let link = crate::creature::lizard::LizardGraphicsLink {
        module: 3,
        ..Default::default()
    };
    g.world.lizard_at(l).graphics = Some(link);
    let source = ViolenceSource {
        owner: Entity::Creature(g.player_id),
        pos: BODY[0] + Vec2::new(10.0, 0.0),
        last_pos: BODY[0] + Vec2::new(13.0, 0.0),
        vel: Vec2::new(-3.0, 0.0),
    };
    let hit = Hit {
        source: Some(source),
        ..g.rock_hit(0, Vec2::new(-3.0, 0.0))
    };
    let count = g.world.room_cosmetics(0).len();
    g.creature_violence(l, hit);
    let added: Vec<_> = g.world.room_cosmetics(0)[count..].to_vec();
    assert!(added.len() > 1, "sparks and the flash");
    for c in &added {
        let lizard = match &c.kind {
            CosmeticKind::Spark(s) => s.lizard,
            CosmeticKind::Flash(f) => f.lizard,
            _ => panic!("a spark or the flash"),
        };
        assert_eq!(lizard, Some((l, 3)));
    }
    let n = i32::try_from(added.len() - 1).expect("count");
    let spark = added[0].id;
    // Long-lived and still, so it outlives the two updates.
    for c in &mut g.world.realized_mut(0).expect("room").cosmetics {
        if let CosmeticKind::Spark(s) = &mut c.kind {
            s.life_time = 1000;
            s.pos = BODY[0];
            s.vel = Vec2::ZERO;
            s.gravity = 0.0;
        }
    }
    g.run_cosmetic(0, spark);
    let flicker = g.spark_flicker(spark);
    assert_eq!(flicker, Some(n), "the module's WhiteFlicker(n)");
    let link = g.world.lizard_at(l).graphics.as_mut().expect("link");
    link.module = 4;
    link.white_flicker = 0;
    g.run_cosmetic(0, spark);
    assert_eq!(
        g.spark_flicker(spark),
        flicker,
        "a later module is not read"
    );
}

/// `Lizard.Update`'s white-flicker roll reaches the graphics: a sick lizard with
/// graphics flickers within a few hundred ticks.
#[test]
fn a_sick_lizard_with_graphics_flickers_white() {
    let (mut g, _) = game(vec![]);
    let l = g.lying_lizard();
    g.world.lizard_at(l).graphics = Some(crate::creature::lizard::LizardGraphicsLink::default());
    g.world.creature_mut(l).state.health = 0.01;
    g.world.lizard_at(l).stun = 1000;
    let mut flickered = false;
    for _ in 0..400 {
        g.lizard_tick(l);
        let link = g.world.lizard_ref(l).graphics.as_ref().expect("link");
        flickered |= link.white_flicker_calls > 0 && (1..5).contains(&link.white_flicker);
    }
    assert!(flickered);
}

/// `Violence` voices the pain before the head shield and the stun:
/// `MakeSound(PainImpact, Max(damage * 2, stun / 40))` (no draw: its intensity is given),
/// here `Max(0.002 * 2, 18.12 / 40)`; the lizard's next update plays the green voice
/// on its head (the microphone's 7 draws) after its other rolls.
#[test]
fn violence_voices_pain_and_the_next_update_plays_it() {
    let text =
        "Volume: 6\nVolume exponent: 1.8\nLizard_Voice_Green_A : LizardVoiceGreenA/vol=0.3\n";
    let names = rw_assets::sound_table::ClipNames::new(["lizardvoicegreena".to_owned()], None);
    let table = Arc::new(rw_assets::SoundTable::parse(text, &names).unwrap());
    let (mut g, _) = game_with_sounds(vec![], Sounds::new(table));
    let l = g.lying_lizard();
    let mut expected = g.rng.clone();
    expected.value();
    g.creature_violence(l, g.rock_hit(1, Vec2::new(0.0, -5.0)));
    assert_eq!(g.rng, expected, "the stun's draw only");
    let v = g.world.lizard_ref(l).voice.clone().expect("built");
    assert_eq!(
        v.articulation,
        Some(crate::creature::lizard::LizardEmotion::PainImpact)
    );
    assert!((v.current_emotion_intensity - (0.01 * 30.0 + 45.0) / 2.5 / 40.0).abs() < 1e-6);
    assert_eq!(v.sound_id, Some(crate::SoundId::LizardVoiceGreenA));
    g.sounds.take_events();
    g.lizard_tick(l);
    let played = g
        .sounds
        .take_events()
        .into_iter()
        .filter(|e| matches!(e, crate::SoundEvent::Play(p) if p.id == crate::SoundId::LizardVoiceGreenA))
        .count();
    assert_eq!(played, 1);
    assert!(
        g.world
            .lizard_ref(l)
            .voice
            .as_ref()
            .unwrap()
            .making_a_sound()
    );
}

/// `Room.NoLongerViewed` drops the lizard's graphics module (`RemoveGraphicsModule`)
/// without touching its `soundLoop`'s emitter, which the module's last update kept
/// alive. The emitter stays in the room: its next update spends that `alive`, the one
/// after finds no upkeep and `Destroy`s it.
#[test]
fn a_dropped_graphics_loop_emitter_dies_at_its_second_update() {
    let (mut g, _) = game(vec![]);
    let l = g.lying_lizard();
    let mut link = crate::creature::lizard::LizardGraphicsLink {
        drawn_in: vec![0],
        ..Default::default()
    };
    let sound = crate::EmitterId(900);
    link.sound_loop.emitter = Some(crate::LoopEmitter {
        sound,
        room: 0,
        update_order: g.world.next_update_seq(),
        alive: true,
        slated: false,
        still_playing: true,
        last_playing_tick: g.tick,
        playing: true,
    });
    g.world.lizard_at(l).graphics = Some(link);
    g.sounds.take_events();
    g.modules_no_longer_viewed(0, &mut ());
    g.modules_now_viewed(0, &mut ());
    let stops = g.lizard_loop_stops();
    assert_eq!(stops, vec![], "the drop");
    assert_eq!(g.world.lizard_ref(l).dropped_loop_emitters.len(), 1);
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.lizard_loop_stops(), vec![], "its next update");
    let e = g.world.lizard_ref(l).dropped_loop_emitters[0];
    assert!(!e.alive && !e.slated);
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.lizard_loop_stops(), vec![sound], "the one after");
    assert!(g.world.lizard_ref(l).dropped_loop_emitters.is_empty());
}

impl RainWorldGame {
    /// The `Stop`s since the last call.
    fn lizard_loop_stops(&mut self) -> Vec<crate::EmitterId> {
        self.sounds
            .take_events()
            .into_iter()
            .filter_map(|e| match e {
                crate::SoundEvent::Stop(id) => Some(id),
                _ => None,
            })
            .collect()
    }

    /// The test room as region 5's world with one lineage at its node 0,
    /// `Green-chance, NONE-0.5, Yellow-0.2, Red-0` (SU_B08's White chain with the
    /// first chance replaced), its counter at `stage`; returns the spawner's ID.
    fn with_lineage(&mut self, chance: f32, stage: i32) -> i32 {
        use rw_assets::creatures::{CreatureSpawner, Lineage, SpawnerDen};
        let data = &mut self.world.data;
        data.single_room = false;
        data.region = Some(rw_assets::world::RegionInfo {
            name: "SU".to_owned(),
            first_room_index: 0,
            number_of_rooms: 2,
            region_number: 5,
        });
        let lineage = Lineage {
            den: SpawnerDen { room: 0, node: 0 },
            creatures: vec![
                Some(CreatureType::GreenLizard),
                None,
                Some(CreatureType::YellowLizard),
                Some(CreatureType::RedLizard),
            ],
            progression_chances: vec![chance, 0.5, 0.2, 0.0],
            spawn_data: vec![None; 4],
            den_lineage: 0,
            night_creature: false,
        };
        let den = self.world.lineage_den_string(&lineage);
        self.world.data.spawners = vec![CreatureSpawner::Lineage(lineage)];
        let mut state = crate::world::RegionState::new_game(&self.world.data);
        state.set_lineage_counter(&den, stage);
        self.world.region_state = Some(state);
        5000
    }

    fn lineage_stage(&self) -> Option<i32> {
        let den = format!("{}.-1.-1.0", self.world.data.rooms[0].name);
        self.world.region_state.as_ref()?.lineage_counter(&den)
    }

    /// A lying green lizard of spawner `spawner`.
    fn lineage_lizard(&mut self, spawner: i32) -> CreatureId {
        let l = self.lying_lizard();
        self.world.creature_mut(l).id = EntityId::new(spawner, 77);
        l
    }
}

/// `AbstractCreature.Die` -> `AddCreatureToRespawn`: a slugcat's kill of a lineage
/// creature rolls `ChanceToProgress` once (`Random.value < progressionChances[n]`) and
/// puts the spawner on the wait list; at the last stage there is no roll.
#[test]
fn a_slugcat_kill_of_a_lineage_lizard_rolls_once_and_waits() {
    let (mut g, _) = game(vec![]);
    let spawner = g.with_lineage(1.0, 0);
    let l = g.lineage_lizard(spawner);
    g.world.set_kill_tag(l, g.player_id);
    let mut expected = g.rng.clone();
    expected.value();
    g.lizard_die(l);
    assert_eq!(g.rng, expected, "one draw");
    assert_eq!(g.lineage_stage(), Some(1), "Green -> NONE");
    assert_eq!(g.world.session.wait_respawn_creatures, [spawner]);
    assert!(g.world.session.respawn_creatures.is_empty());
    assert!(!g.world.creature(l).state.alive);
    // `Killing`: the session kill and the lizards' like of the player.
    assert_eq!(g.world.session.kills.len(), 1);
    assert!(g.world.session.kills[0].lizard);
    let i = g.world.session.influences.last().expect("influence");
    assert_eq!(i.community, crate::creature::Community::Lizards);

    // A chance of 0 still draws; the last stage (Red, `n == length - 1`) does not.
    let (mut g, _) = game(vec![]);
    let spawner = g.with_lineage(0.0, 0);
    let l = g.lineage_lizard(spawner);
    g.world.set_kill_tag(l, g.player_id);
    let mut expected = g.rng.clone();
    expected.value();
    g.lizard_die(l);
    assert_eq!(g.rng, expected);
    assert_eq!(g.lineage_stage(), Some(0));
    let (mut g, _) = game(vec![]);
    let spawner = g.with_lineage(1.0, 3);
    let l = g.lineage_lizard(spawner);
    g.world.set_kill_tag(l, g.player_id);
    let before = g.rng.clone();
    g.lizard_die(l);
    assert_eq!(g.rng, before, "no roll at the last stage");
    assert_eq!(g.lineage_stage(), Some(3));
    assert_eq!(g.world.session.wait_respawn_creatures, [spawner]);
}

/// Killed by another lizard (7c's fights), or with no kill tag (the health roll,
/// drowning), the spawner goes straight back on the respawn list, the lineage as it
/// was.
#[test]
fn a_lizard_not_killed_by_the_slugcat_respawns_next_cycle() {
    let (mut g, _) = game(vec![]);
    let spawner = g.with_lineage(1.0, 0);
    let l = g.lineage_lizard(spawner);
    let rival = g.placed_lizard();
    g.world.set_kill_tag(l, rival);
    let before = g.rng.clone();
    g.lizard_die(l);
    assert_eq!(g.rng, before);
    assert_eq!(g.lineage_stage(), Some(0));
    assert_eq!(g.world.session.respawn_creatures, [spawner]);
    assert!(g.world.session.wait_respawn_creatures.is_empty());
    assert!(g.world.session.kills.is_empty());
    // A second death adds nothing (`state.alive` is already false).
    g.lizard_die(l);
    assert_eq!(g.world.session.respawn_creatures, [spawner]);
}

/// `Creature.Update`'s fall out of the room: `Die` (no kill tag: respawn), then the
/// creature is destroyed.
#[test]
fn a_lizard_falling_out_of_the_room_is_put_up_for_respawn() {
    let (mut g, _) = game(vec![]);
    let spawner = g.with_lineage(1.0, 0);
    let l = g.lineage_lizard(spawner);
    for c in &mut g.world.lizard_at(l).body.chunks {
        c.hard_set_position(Vec2::new(200.0, -2000.0));
    }
    g.tick(Input::NONE, &mut ());
    assert!(!g.world.creature(l).state.alive);
    assert!(g.world.creature(l).slated_for_deletion);
    assert_eq!(g.world.session.respawn_creatures, [spawner]);
}
