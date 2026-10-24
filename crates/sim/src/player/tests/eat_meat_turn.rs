//! A dead green lizard picked up, dragged and eaten, in one test room. Expected values
//! from `Player.Grabability`, `SlugcatGrab`, `HeavyCarry`, `Jump`, `TossObject`,
//! `CanEatMeat`, the meat block of `GrabUpdate` and `EatMeatUpdate`, and
//! `LizardBreeds.BreedTemplate` (green: `meatPoints` 8, three chunks of mass 2.5 and
//! radius 9.6).

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, CreatureType, Room, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::creature::{CreatureId, CreatureTemplate, WorldCoordinate};
use crate::game::{PlayerHands, PlayerSurroundings};
use crate::item::Entity;
use crate::item::turn::Grabability;
use crate::tests::room;
use crate::world::World;
use crate::{Input, Player, RainWorldGame, SoundEvent, SoundId, Sounds, Start, Vec2, Water};

const PICKUP: Input = Input {
    x: 0,
    y: 0,
    jump: false,
    map: false,
    pickup: true,
    throw: false,
};

/// A player graphics module that only answers what `EatMeatUpdate` asks: it exists,
/// its head is at `head`, and the nudges it got.
#[derive(Default)]
struct Graphics {
    head: Vec2,
    nudges: Vec<Vec2>,
}

impl crate::game::PlayerObserver for Graphics {
    fn created(&mut self, _: &Player, _: &mut UnityRandom) {}
    fn updated(
        &mut self,
        player: &Player,
        _: &Room,
        _: Option<&Water>,
        _: &PlayerSurroundings<'_>,
        _: &mut UnityRandom,
    ) {
        self.head = player.head().pos;
    }
    fn spat_out(&mut self, _: &Player, _: &mut UnityRandom) {}
    fn has_player_graphics(&self) -> bool {
        true
    }
    fn player_hands(&self) -> Option<PlayerHands> {
        Some(PlayerHands {
            hands: [(self.head, Vec2::ZERO); 2],
            spear_dir: 0.0,
            overlap: [None; 2],
            body_push: Vec2::ZERO,
            draw_positions: [self.head; 2],
            head: self.head,
        })
    }
    fn nudge_player_head(&mut self, dv: Vec2) {
        self.nudges.push(dv);
    }
}

/// A story of `campaign` in a 20 x 8 walled room, the player at (8, 1), stood still.
fn game(campaign: &'static SlugcatCampaign) -> RainWorldGame {
    let mut rows = vec!["####################"];
    rows.extend(["#..................#"; 6]);
    rows.push("####################");
    game_in(room(&rows), campaign)
}

/// [`game`] in `room`.
fn game_in(room: Room, campaign: &'static SlugcatCampaign) -> RainWorldGame {
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let mut g = RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some((8, 1)),
        },
        UnityRandom::new(5),
        Sounds::default(),
    );
    g.tick_until_ready(&mut ());
    for _ in 0..20 {
        g.tick(Input::NONE, &mut ());
    }
    g
}

/// Head, middle and hips along the floor, facing right, head at x 200.
const BODY: [Vec2; 3] = [
    Vec2::new(200.0, 29.6),
    Vec2::new(181.3, 29.6),
    Vec2::new(162.6, 29.6),
];

impl RainWorldGame {
    /// A realized green lizard lying at [`BODY`], dead unless `alive`.
    fn meat_lizard(&mut self, alive: bool) -> CreatureId {
        let session = self.story_session();
        let template = CreatureTemplate::of(CreatureType::GreenLizard, ActiveMods::NONE);
        let id = self.ids.new_id(-1);
        let at = WorldCoordinate::new(0, 9, 1, -1);
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
        for (c, pos) in self.world.lizard_at(l).body.chunks.iter_mut().zip(BODY) {
            c.hard_set_position(pos);
            c.vel = Vec2::ZERO;
        }
        if !alive {
            self.world.creature_die(l, &mut self.rng);
        }
        l
    }

    /// The player standing with its head at `x` on the floor.
    fn stand_at(&mut self, x: f32) {
        let p = self.player_mut();
        p.body.chunks[0].hard_set_position(Vec2::new(x, 37.0));
        p.body.chunks[1].hard_set_position(Vec2::new(x, 23.0));
        for c in &mut p.body.chunks {
            c.vel = Vec2::ZERO;
        }
    }

    /// The lizard picked up from beside its middle chunk.
    fn pick_up_meat(&mut self, l: CreatureId, observer: &mut dyn crate::game::PlayerObserver) {
        self.stand_at(176.0);
        self.tick(Input::NONE, observer);
        assert_eq!(
            self.player_ref().grab_state().pick_up_candidate,
            Some(Entity::Creature(l))
        );
        self.tick(PICKUP, observer);
        self.tick(Input::NONE, observer);
        assert_eq!(self.player_grasps()[0], Some(Entity::Creature(l)));
    }
}

#[test]
fn a_dead_green_is_dragged_by_the_chunk_nearest_the_main_chunk() {
    let mut g = game(&SlugcatCampaign::SURVIVOR);
    let l = g.meat_lizard(false);
    assert_eq!(g.grabability(Entity::Creature(l)), Grabability::Drag);
    g.pick_up_meat(l, &mut ());
    let grasp = g.player_grasp(0).expect("held");
    assert!(grasp.pacifying);
    assert!(g.heavy_carry(Entity::Creature(l)));
    assert_eq!(
        g.free_hand(),
        None,
        "a heavy carry in hand 0 leaves no free hand"
    );
}

#[test]
fn the_nearest_chunk_is_the_first_of_the_closest() {
    let mut g = game(&SlugcatCampaign::SURVIVOR);
    let l = g.meat_lizard(false);
    g.stand_at(176.0);
    // Head at x 176: the middle chunk (181.3) is nearest.
    assert_eq!(g.nearest_chunk_to_main(Entity::Creature(l)), 1);
    g.stand_at(150.0);
    assert_eq!(g.nearest_chunk_to_main(Entity::Creature(l)), 2);
    g.stand_at(230.0);
    assert_eq!(g.nearest_chunk_to_main(Entity::Creature(l)), 0);
}

#[test]
fn a_living_green_cannot_be_picked_up() {
    let mut g = game(&SlugcatCampaign::HUNTER);
    let l = g.meat_lizard(true);
    g.world.lizard_at(l).stun = 200;
    assert_eq!(g.grabability(Entity::Creature(l)), Grabability::CantGrab);
    g.stand_at(176.0);
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.player_ref().grab_state().pick_up_candidate, None);
}

#[test]
fn only_the_hunter_eats_a_dead_lizard() {
    let mut g = game(&SlugcatCampaign::SURVIVOR);
    let l = g.meat_lizard(false);
    assert!(!g.can_eat_meat(l));
    let mut g = game(&SlugcatCampaign::MONK);
    let l = g.meat_lizard(false);
    assert!(!g.can_eat_meat(l));
    let mut g = game(&SlugcatCampaign::HUNTER);
    let l = g.meat_lizard(false);
    assert!(g.can_eat_meat(l));
    let alive = g.meat_lizard(true);
    assert!(!g.can_eat_meat(alive));
    assert_eq!(g.world.creature(l).state.meat_left, 8);
}

#[test]
fn the_survivor_holding_pickup_on_a_lizard_does_not_chew() {
    let mut g = game(&SlugcatCampaign::SURVIVOR);
    let l = g.meat_lizard(false);
    g.pick_up_meat(l, &mut ());
    for _ in 0..100 {
        g.tick(PICKUP, &mut ());
    }
    assert_eq!(g.player_ref().grab_state().eat_meat, 0);
    assert_eq!(g.world.creature(l).state.meat_left, 8);
}

#[test]
fn the_hunter_eats_a_green_in_eight_bites_then_tosses_it() {
    let mut g = game(&SlugcatCampaign::HUNTER);
    let l = g.meat_lizard(false);
    let mut gfx = Graphics::default();
    g.pick_up_meat(l, &mut gfx);
    let food = g.food_in_stomach();
    let mut bites = Vec::new();
    let mut tossed_at = None;
    for _ in 0..200 {
        let before = g.world.creature(l).state.meat_left;
        g.tick(PICKUP, &mut gfx);
        let n = g.player_ref().grab_state().eat_meat;
        if g.world.creature(l).state.meat_left < before {
            bites.push(n);
        }
        if g.player_grasps()[0].is_none() {
            tossed_at = Some(bites.len());
            break;
        }
    }
    // A bite every 15 ticks from 48 (past 40, at 3 mod 15).
    assert_eq!(bites, [48, 63, 78, 93, 108, 123, 138, 153]);
    assert_eq!(g.world.creature(l).state.meat_left, 0);
    assert_eq!(g.food_in_stomach(), food + 8);
    assert_eq!(g.eaten(), [g.world.creature(l).id], "SessionRecord.AddEat");
    // Tossed at the next multiple of 80 (160) with no meat left.
    assert_eq!(tossed_at, Some(8));
    assert_eq!(g.player_ref().grab_state().eat_meat, 0);
    assert!(!gfx.nudges.is_empty(), "the head is kept at the held chunk");
}

#[test]
fn without_graphics_the_hunter_chews_but_takes_no_meat() {
    let mut g = game(&SlugcatCampaign::HUNTER);
    let l = g.meat_lizard(false);
    g.pick_up_meat(l, &mut ());
    for _ in 0..170 {
        g.tick(PICKUP, &mut ());
    }
    assert_eq!(g.world.creature(l).state.meat_left, 8);
    // Still meat left at 80 and 160: kept, chewing on.
    assert_eq!(g.player_grasps()[0], Some(Entity::Creature(l)));
    assert_eq!(g.player_ref().grab_state().eat_meat, 170);
}

#[test]
fn letting_go_of_pickup_after_sixty_ticks_tosses_the_body() {
    let mut g = game(&SlugcatCampaign::HUNTER);
    let l = g.meat_lizard(false);
    g.pick_up_meat(l, &mut ());
    for _ in 0..61 {
        g.tick(PICKUP, &mut ());
    }
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.player_grasps()[0], None);
    assert_eq!(g.player_ref().grab_state().eat_meat, 0);
}

#[test]
fn a_bite_plays_eat_meat_b_and_the_others_eat_meat_a() {
    let mut g = game(&SlugcatCampaign::HUNTER);
    let l = g.meat_lizard(false);
    let mut gfx = Graphics::default();
    g.pick_up_meat(l, &mut gfx);
    let text = "Volume: 6\nVolume exponent: 1.8\nSlugcat_Eat_Meat_A : a\nSlugcat_Eat_Meat_B : b\n";
    let names = rw_assets::ClipNames::new(["a".to_owned(), "b".to_owned()], None);
    let table = rw_assets::SoundTable::parse(text, &names).unwrap();
    g.sounds = Sounds::new(Arc::new(table));
    let mut played = Vec::new();
    for _ in 0..64 {
        g.tick(PICKUP, &mut gfx);
        let n = g.player_ref().grab_state().eat_meat;
        for e in g.sounds.take_events() {
            if let SoundEvent::Play(s) = e {
                played.push((n, s.id));
            }
        }
    }
    assert_eq!(
        played,
        [
            (33, SoundId::SlugcatEatMeatA),
            (48, SoundId::SlugcatEatMeatB),
            (63, SoundId::SlugcatEatMeatB),
        ]
    );
}

#[test]
fn a_bites_drips_draw_a_count_then_eight_and_the_drips_own() {
    let mut g = game(&SlugcatCampaign::HUNTER);
    let l = g.meat_lizard(false);
    let chunk = g.world.lizard_ref(l).body.chunks[1].clone();
    let mut expect = g.rng.clone();
    g.eat_meat_drips(0, &chunk, BODY[1], Vec2::new(176.0, 40.0));
    let n = expect.range_i32(0, 3);
    for _ in 0..=n {
        for _ in 0..8 {
            expect.value();
        }
        crate::WaterDrip::new(Vec2::ZERO, Vec2::ZERO, false, &mut expect);
    }
    assert_eq!(g.rng, expect);
}

#[test]
fn a_heavy_carry_jumps_with_the_games_num() {
    let mut g = game(&SlugcatCampaign::SURVIVOR);
    let l = g.meat_lizard(false);
    g.pick_up_meat(l, &mut ());
    let carry = g.heavy_carry_loan(0).expect("heavy");
    assert!((carry.total_mass - 7.5).abs() < 1e-4);
    assert!(carry.drag);
    let p = g.player_mut();
    p.lend_heavy_carry(Some(carry));
    // 1 + Min(Max(0, 7.5 - 0.2) * 1.5, 1.3).
    assert!((p.jump_power() - 2.3).abs() < 1e-5);
    p.lend_heavy_carry(None);
    assert!((p.jump_power() - 1.0).abs() < 1e-6);
}

#[test]
fn a_tossed_body_is_pushed_by_its_held_chunk_only() {
    let mut g = game(&SlugcatCampaign::SURVIVOR);
    let l = g.meat_lizard(false);
    g.pick_up_meat(l, &mut ());
    let before: Vec<Vec2> = g
        .world
        .lizard_ref(l)
        .body
        .chunks
        .iter()
        .map(|c| c.vel)
        .collect();
    g.toss_object(0, 0);
    let after: Vec<Vec2> = g
        .world
        .lizard_ref(l)
        .body
        .chunks
        .iter()
        .map(|c| c.vel)
        .collect();
    // No input: 45 degrees, power 4, over Max(0.5, 2.5).
    let dir = g.player_ref().throw_direction() as f32;
    let push = Vec2::from_degrees(45.0 * dir) * 4.0 / 2.5;
    let held = g.player_grasp(0).expect("held").chunk_grabbed;
    for i in 0..3 {
        let dv = after[i] - before[i];
        if i == held {
            assert!((dv - push).length() < 1e-4, "{dv:?} {push:?}");
        } else {
            assert_eq!(dv, Vec2::ZERO);
        }
    }
    // `TossObject` ends with `CreaturePutItemOnGround` (the drag branch too).
    let owned = &g.world.realized(0).expect("room").social_events;
    let o = owned.item_ownership(Entity::Creature(l)).expect("owned");
    assert_eq!(o.owner, g.player_id);
}

#[test]
fn a_dragged_body_goes_through_a_shortcut_still_held() {
    // A wall at x = 12 with a pipe through it: entrances (11, 1) and (13, 1).
    let mut rows = vec!["####################"];
    rows.extend(["#...........#......#"; 5]);
    rows.push("#..........ExE.....#");
    rows.push("####################");
    let mut g = game_in(room(&rows), &SlugcatCampaign::SURVIVOR);
    let l = g.meat_lizard(false);
    for (c, x) in g
        .world
        .lizard_at(l)
        .body
        .chunks
        .iter_mut()
        .zip([190.0, 171.3, 152.6])
    {
        c.hard_set_position(Vec2::new(x, 29.6));
    }
    g.stand_at(205.0);
    g.slugcat_grab(Entity::Creature(l), 0, &mut ());
    let right = Input::new(1, 0, false);
    let mut in_tube = false;
    for _ in 0..300 {
        g.tick(right, &mut ());
        in_tube |= g.world.lizard_ref(l).in_shortcut;
        if in_tube && g.world.lizard_ref(l).room == Some(0) {
            break;
        }
    }
    assert!(in_tube, "the body left the room with the slugcat");
    let lz = g.world.lizard_ref(l);
    assert_eq!(lz.room, Some(0));
    assert!(
        lz.body.chunks.iter().all(|c| c.pos.x > 250.0),
        "out of the far end"
    );
    assert_eq!(g.player_grasps()[0], Some(Entity::Creature(l)));
}

#[test]
fn eaten_meat_is_saved_in_the_state_string() {
    let mut g = game(&SlugcatCampaign::HUNTER);
    let l = g.meat_lizard(false);
    let c = g.world.creature(l);
    assert!(!g.world.creature_save_string(c, c.pos).contains("MeatLeft"));
    g.world.creature_mut(l).state.meat_left = 5;
    let c = g.world.creature(l);
    assert!(
        g.world
            .creature_save_string(c, c.pos)
            .contains("MeatLeft<cC>5<cB>")
    );
}
