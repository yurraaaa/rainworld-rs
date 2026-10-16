//! The guide's room objects and its communication in one test room: the concern's
//! entity draw (`ReevaluateConcern`, 231543-231576), `SuperJumpInstruction` (228116)
//! and its in-world hologram (`SuperJump.Update`, 233950-233967),
//! `ReliableIggyDirection` (236584-236700) with `ForcedDirectionPointer` (323713), the
//! `ScavengerTradeInstructionTrigger` (322560) and `OverseerImage`'s
//! `ProjectedImagePosition` (325335).

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, CreatureType, PlacedObject, RoomSettings, ShowProjectedImageEvent};
use rw_random::UnityRandom;

use super::super::OverseerCall;
use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::creature::overseer::hologram::{
    Controller, HologramKind, HologramMessage, ImageId, PartKind, PointerKind,
};
use crate::creature::overseer::{CommunicationModule, OverseerBody, PlayerConcern};
use crate::creature::{CreatureId, CreatureTemplate, EntityId, RealizedCreature, WorldCoordinate};
use crate::item::{AbstractObject, Entity, ObjectId};
use crate::tests::room;
use crate::trigger::TriggerObjectKind;
use crate::world::World;
use crate::{Input, RainWorldGame, Sounds, Start, Vec2};

/// SU_A13's `SuperJumpInstruction` handles moved to (200, 100): the trigger line runs
/// from (240, 70) (handle 2) to (160, 50) (handle 1); handle 0 at (260, 100) is the
/// landing.
const SUPER_JUMP: &str = "SuperJumpInstruction><200><100><60~0~-40~-50~40~-30";

/// A Survivor story (cycle `cycle`) in a 20 x 8 room with walls round it, `placed`
/// objects and `objects`, the player stood at (2, 1), after 20 ticks.
fn game(
    placed: &str,
    objects: Vec<AbstractObject>,
    story: StoryStart,
) -> (RainWorldGame, Vec<ObjectId>) {
    let mut rows = vec!["####################"];
    rows.extend(["#..................#"; 6]);
    rows.push("####################");
    let room = room(&rows);
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        placed_objects: PlacedObject::parse_list(placed),
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let campaign = &SlugcatCampaign::SURVIVOR;
    let mut world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let ids: Vec<ObjectId> = objects.into_iter().map(|o| world.add_object(o)).collect();
    let mut g = RainWorldGame::new_story(
        world,
        campaign,
        story,
        Start {
            room: 0,
            tile: Some((2, 1)),
        },
        UnityRandom::new(5),
        Sounds::default(),
        &mut (),
    );
    for _ in 0..20 {
        g.tick(Input::NONE, &mut ());
    }
    (g, ids)
}

/// The first seed whose first `Range(0, 3)` is `n`.
fn seed_for_entity(n: i32) -> i32 {
    (0..)
        .find(|&s| UnityRandom::new(s).range_i32(0, 3) == n)
        .expect("a seed")
}

/// Whether a `ReliableIggyDirection` with `data` survives the 20 ticks (its first
/// update), the room given `given` times before.
fn iggy_alive(data: &str, met_moon: bool, given: i32) -> bool {
    let mut story = StoryStart {
        ever_met_moon: met_moon,
        ..new_game()
    };
    story.guide.forced_directions_given = vec![(0, given)];
    let placed = format!("ReliableIggyDirection><60><40><{data}");
    let (g, _) = game(&placed, Vec::new(), story);
    g.trigger_objects
        .iter()
        .any(|o| matches!(o.kind, TriggerObjectKind::ReliableIggyDirection(_)))
}

fn new_game() -> StoryStart {
    StoryStart::new_game(&SlugcatCampaign::SURVIVOR)
}

impl RainWorldGame {
    /// The realized player guide in room 0, its body still at `pos`.
    fn placed_test_guide(&mut self, pos: Vec2) -> CreatureId {
        let session = self.story_session();
        let template = CreatureTemplate::of(CreatureType::Overseer, ActiveMods::NONE);
        let at = WorldCoordinate::new(0, 10, 4, -1);
        let c = self.world.new_creature(
            template,
            at,
            EntityId::new(-1, 5),
            &session,
            &mut self.ids,
            &mut self.rng,
        );
        let o = self.world.insert_creature(c);
        self.world.add_entity(o);
        if let Some(a) = self.world.creature_mut(o).overseer_ai_mut() {
            a.set_as_player_guide(1);
        }
        assert!(self.world.realize_overseer(o, 0, &mut self.rng));
        self.run_overseer(o, OverseerCall::Place, &mut ());
        let b = self.placed_test_body(o);
        b.chunk_mut().hard_set_position(pos);
        b.chunk_mut().vel = Vec2::ZERO;
        o
    }

    fn placed_test_body(&mut self, o: CreatureId) -> &mut OverseerBody {
        let Some(RealizedCreature::Overseer(b)) = &mut self.world.creature_mut(o).realized else {
            panic!("realized");
        };
        b
    }

    fn placed_test_comm(&mut self, o: CreatureId) -> &mut CommunicationModule {
        self.world
            .creature_mut(o)
            .overseer_ai_mut()
            .and_then(|a| a.real_ai.as_mut())
            .and_then(|ai| ai.communication.as_mut())
            .expect("the guide's communication")
    }

    fn trigger_id(&self, kind: fn(&TriggerObjectKind) -> bool) -> u64 {
        self.trigger_objects
            .iter()
            .find(|o| kind(&o.kind))
            .expect("the room object")
            .id
    }

    /// The player's main chunk moved from `from` to `to` this tick.
    fn move_main_chunk(&mut self, from: Vec2, to: Vec2) {
        let c = &mut self.player.as_mut().expect("player").body.chunks[0];
        c.last_pos = from;
        c.pos = to;
    }
}

/// `entities[Range(0, entities.Count)]` draws over the room's objects and creatures in
/// the order they joined: the rock, the player, the guide. A draw of 0 is the rock and
/// 2 the guide (the creature list, player and guide, has no index 2).
#[test]
fn the_food_concern_draws_over_creatures_and_objects() {
    let rock = AbstractObject::new_rock(EntityId::new(0, 1), WorldCoordinate::new(0, 10, 4, -1));
    let (mut g, ids) = game("", vec![rock], new_game());
    let guide = g.placed_test_guide(Vec2::new(200.0, 100.0));
    let entities = g.world.room_entities(0).to_vec();
    assert_eq!(
        entities,
        [
            Entity::Object(ids[0]),
            Entity::Creature(g.player_id),
            Entity::Creature(guide)
        ]
    );
    assert_eq!(g.world.room_creatures(0), [g.player_id, guide]);
    let seed = seed_for_entity(0);
    assert_eq!(
        g.world.random_room_entity(0, &mut UnityRandom::new(seed)),
        Some(Entity::Object(ids[0]))
    );
    assert_eq!(
        g.world
            .random_room_entity(0, &mut UnityRandom::new(seed_for_entity(2))),
        Some(Entity::Creature(guide))
    );

    // Hungry: the creature `Range`, a `value` for the realized pick, the entity
    // `Range`. Nothing else draws: no concern, so no hologram.
    g.rng = UnityRandom::new(seed);
    let mut expected = g.rng.clone();
    expected.range_i32(0, 2);
    expected.value();
    expected.range_i32(0, 3);
    g.run_overseer(guide, OverseerCall::Communication, &mut ());
    assert_eq!(g.rng, expected, "three draws");
    let comm = g.placed_test_comm(guide);
    assert_eq!(comm.current_concern, PlayerConcern::None);
    // A rock is no food: `FoodDelicousScore` 0.
    assert_eq!(comm.most_delicious_food_in_room, None);
    assert_eq!(comm.most_delicous_food_delicious, 0.0);

    // Fed: no entity draw.
    g.world.creature_mut(g.player_id).state.food_in_stomach = 4;
    let mut expected = g.rng.clone();
    expected.range_i32(0, 2);
    expected.value();
    g.run_overseer(guide, OverseerCall::Communication, &mut ());
    assert_eq!(g.rng, expected, "two draws");
}

/// The placed `SuperJumpInstruction` is made on load; a falling main chunk crossing
/// its line activates it, and on its next update it asks the guide (while it has
/// shown the super jump at most 3 times).
#[test]
fn super_jump_instruction_activates_and_asks_the_guide() {
    let (mut g, _) = game(SUPER_JUMP, Vec::new(), new_game());
    let id = g.trigger_id(|k| matches!(k, TriggerObjectKind::SuperJumpInstruction(_)));
    let guide = g.placed_test_guide(Vec2::new(200.0, 120.0));
    // Rising through it does nothing; nor does a straight drop (the strict box test).
    g.move_main_chunk(Vec2::new(201.0, 50.0), Vec2::new(199.0, 70.0));
    g.run_super_jump_instruction(0, id);
    g.move_main_chunk(Vec2::new(200.0, 70.0), Vec2::new(200.0, 50.0));
    g.run_super_jump_instruction(0, id);
    assert_eq!(g.placed_test_comm(guide).input_instruction, None);
    g.move_main_chunk(Vec2::new(199.0, 70.0), Vec2::new(201.0, 50.0));
    g.run_super_jump_instruction(0, id);
    assert_eq!(g.placed_test_comm(guide).input_instruction, None);
    g.run_super_jump_instruction(0, id);
    assert_eq!(g.placed_test_comm(guide).input_instruction, Some(id));

    g.placed_test_comm(guide).input_instruction = None;
    g.guide.set_super_jumps_shown(4);
    g.run_super_jump_instruction(0, id);
    assert_eq!(g.placed_test_comm(guide).input_instruction, None);
}

/// The guide's concern 0.7 shows `InWorldSuperJump`: its ghost on the placed object,
/// turned by the sign of handle 0's x; the main chunk within 50 px of handle 0
/// completes it and counts a super jump shown.
#[test]
fn in_world_super_jump_completes_at_the_first_handle() {
    let (mut g, _) = game(SUPER_JUMP, Vec::new(), new_game());
    let id = g.trigger_id(|k| matches!(k, TriggerObjectKind::SuperJumpInstruction(_)));
    let guide = g.placed_test_guide(Vec2::new(200.0, 120.0));
    g.world.creature_mut(g.player_id).state.food_in_stomach = 4;
    g.move_main_chunk(Vec2::new(199.0, 70.0), Vec2::new(201.0, 50.0));
    g.run_super_jump_instruction(0, id);
    g.run_super_jump_instruction(0, id);
    g.run_overseer(guide, OverseerCall::Communication, &mut ());
    assert_eq!(
        g.placed_test_comm(guide).current_concern,
        PlayerConcern::InputInstruction
    );
    let holo = g
        .placed_test_body(guide)
        .current_hologram()
        .cloned()
        .expect("hologram");
    assert_eq!(holo.message, HologramMessage::InWorldSuperJump);
    g.run_overseer(guide, OverseerCall::Hologram(holo.id), &mut ());
    let holo = g
        .placed_test_body(guide)
        .current_hologram()
        .cloned()
        .expect("hologram");
    let instruction = holo.instruction().expect("instruction");
    assert!(matches!(
        instruction.controller,
        Some(Controller::SuperJump {
            in_tutorial_section: false,
            jump_direction: 1,
            ..
        })
    ));
    assert_eq!(instruction.close_to_pos, Vec2::new(200.0, 120.0));
    let ghost = &holo.parts[instruction.player_ghost.expect("ghost")].kind;
    assert!(matches!(
        ghost,
        PartKind::PlayerGhost { symbol, room_position: Some(p) } if symbol.scale_x == 1.0 && *p == Vec2::new(200.0, 100.0)
    ));
    assert_eq!(g.guide.super_jumps_shown(), 0);
    g.move_main_chunk(Vec2::new(255.0, 95.0), Vec2::new(255.0, 95.0));
    g.run_overseer(guide, OverseerCall::Hologram(holo.id), &mut ());
    assert_eq!(g.guide.super_jumps_shown(), 1);
    let completed = g.trigger_objects.iter().any(
        |o| matches!(&o.kind, TriggerObjectKind::SuperJumpInstruction(s) if s.trigger.completed),
    );
    assert!(completed);
    // Completed: destroyed on its next update.
    g.run_super_jump_instruction(0, id);
    g.remove_slated_trigger_objects();
    assert!(g.trigger_objects.iter().all(|o| o.id != id));
}

/// `ReliableIggyDirection`: within its radius it activates the guide once (counting
/// the room), then gives the guide its direction, which the guide shows as a
/// `ForcedDirectionPointer` (concern 0.98) with the symbol's element.
#[test]
fn reliable_iggy_direction_gives_the_guide_its_exit() {
    let placed = "ReliableIggyDirection><60><40><0~100~50~50~0~Shelter~AnyTime~2~1~White";
    let (mut g, _) = game(placed, Vec::new(), new_game());
    let id = g.trigger_id(|k| matches!(k, TriggerObjectKind::ReliableIggyDirection(_)));
    let guide = g.placed_test_guide(Vec2::new(200.0, 120.0));
    g.world.creature_mut(g.player_id).state.food_in_stomach = 4;
    g.move_main_chunk(Vec2::new(60.0, 50.0), Vec2::new(60.0, 50.0));
    g.run_iggy_direction(0, id);
    assert_eq!(g.guide.forced_directions_given, [(0, 1)]);
    assert_eq!(g.placed_test_comm(guide).forced_direction_to_give, None);
    g.run_iggy_direction(0, id);
    assert_eq!(g.guide.forced_directions_given, [(0, 1)]);
    assert_eq!(g.placed_test_comm(guide).forced_direction_to_give, Some(id));
    g.run_overseer(guide, OverseerCall::Communication, &mut ());
    let comm = g.placed_test_comm(guide);
    assert_eq!(comm.current_concern, PlayerConcern::ForcedDirection);
    assert_eq!(comm.current_concern_weight, 0.98);
    let holo = g
        .placed_test_body(guide)
        .current_hologram()
        .cloned()
        .expect("hologram");
    assert_eq!(holo.message, HologramMessage::ForcedDirection);
    assert!(matches!(
        holo.kind,
        HologramKind::Door {
            pointer: PointerKind::Forced { direction, .. },
            ..
        } if direction == id
    ));
    assert!(matches!(&holo.parts[2].kind, PartKind::Symbol(s) if s.element == "shelterSymbol"));
}

/// Its first update destroys it for another campaign, a failed Moon condition, or a
/// room already given `cyclesToShow` (2) times; the 20 ticks run it.
#[test]
fn reliable_iggy_direction_first_update_checks() {
    assert!(iggy_alive(
        "0~100~0~0~0~Shelter~AnyTime~2~1~White",
        false,
        1
    ));
    assert!(!iggy_alive(
        "0~100~0~0~0~Shelter~AnyTime~2~1~Yellow",
        false,
        1
    ));
    assert!(!iggy_alive(
        "0~100~0~0~0~Shelter~BeforeMoon~2~1~White",
        true,
        1
    ));
    assert!(iggy_alive(
        "0~100~0~0~0~Shelter~BeforeMoon~2~1~White",
        false,
        1
    ));
    assert!(!iggy_alive(
        "0~100~0~0~0~Shelter~AfterMoon~2~1~White",
        false,
        1
    ));
    assert!(!iggy_alive(
        "0~100~0~0~0~Shelter~AfterPebblesAndMoon~2~1~White",
        true,
        1
    ));
    assert!(!iggy_alive(
        "0~100~0~0~0~Shelter~AnyTime~2~1~White",
        false,
        2
    ));
    assert!(iggy_alive(
        "0~100~0~0~0~Shelter~AnyTime~0~1~White",
        false,
        9
    ));
}

/// The scavenger trade trigger waits on a realized data pearl from its room; a rock
/// never activates it.
#[test]
fn scav_trade_trigger_waits_for_a_pearl() {
    let rock = AbstractObject::new_rock(EntityId::new(0, 1), WorldCoordinate::new(0, 10, 4, -1));
    let (g, _) = game(
        "ScavTradeInstruction><200><80><-16~-208",
        vec![rock],
        new_game(),
    );
    let t = g
        .trigger_objects
        .iter()
        .find_map(|o| match &o.kind {
            TriggerObjectKind::ScavTradeInstruction(s) => Some(s.clone()),
            _ => None,
        })
        .expect("made on load");
    assert!(!t.trigger.activated);
    assert_eq!(t.pearl, None);
    assert_eq!(t.trigger.instruction_pos, Vec2::new(200.0, 80.0));
}

/// A placed scene (`ShowProjectedImageEvent`) is shown at the room's
/// `ProjectedImagePosition`; the `OverseerImage`'s `HoloImage` draws its random image.
#[test]
fn placed_scene_shows_at_the_projected_image_position() {
    let (mut g, _) = game(
        "ProjectedImagePosition><577.9059><432.9265><",
        Vec::new(),
        new_game(),
    );
    let guide = g.placed_test_guide(Vec2::new(200.0, 120.0));
    g.world.creature_mut(g.player_id).state.food_in_stomach = 4;
    let comm = g.placed_test_comm(guide);
    comm.image_to_show = Some(ShowProjectedImageEvent::default());
    comm.show_image_room = 0;
    let mut expected = g.rng.clone();
    expected.range_i32(0, 2);
    expected.value();
    expected.range_i32(0, 25);
    g.run_overseer(guide, OverseerCall::Communication, &mut ());
    assert_eq!(g.rng, expected);
    let holo = g
        .placed_test_body(guide)
        .current_hologram()
        .cloned()
        .expect("hologram");
    let HologramKind::Image(image) = &holo.kind else {
        panic!("an image");
    };
    assert_eq!(image.show_pos, Some(Vec2::new(577.9059, 432.9265)));
    assert_eq!(image.images, [ImageId::MoonDoubleSize]);
}
