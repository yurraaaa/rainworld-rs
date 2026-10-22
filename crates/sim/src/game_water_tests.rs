//! `Water.Update` reading every physical object of the room (`physicalObjects`): a green
//! lizard crossing the surface splashes and leaves bubbles as the player does; the
//! bubbles `Water.Update` adds update in the same tick (`Room.Update` walks the update
//! list after the water).

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, CreatureType, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::cosmetic::CosmeticKind;
use crate::creature::{CreatureId, CreatureTemplate, WorldCoordinate};
use crate::item::Entity;
use crate::tests::room_with_water;
use crate::world::World;
use crate::{ChunkOwner, FloatExt, Input, RainWorldGame, Sounds, Start, Vec2};

/// A Survivor story in a 20 x 8 walled room with water up to row 3 (level 70).
fn game() -> RainWorldGame {
    let mut rows = vec!["####################"];
    rows.extend(["#..................#"; 6]);
    rows.push("####################");
    let room = room_with_water(&rows, 3);
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        danger_type: Some(rw_assets::DangerType::None),
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
            tile: Some((2, 1)),
        },
        UnityRandom::new(5),
        Sounds::default(),
        &mut (),
    );
    for _ in 0..20 {
        g.tick(Input::NONE, &mut ());
    }
    g
}

impl RainWorldGame {
    /// A realized green lizard placed at tile (10, 5) of room 0.
    fn water_lizard(&mut self) -> CreatureId {
        let session = self.story_session();
        let template = CreatureTemplate::of(CreatureType::GreenLizard, ActiveMods::NONE);
        let id = self.ids.new_id(-1);
        let at = WorldCoordinate::new(0, 10, 5, -1);
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
        l
    }

    /// Lizard `l`'s head crossing the surface going down at 20 px/tick at x 200.
    fn dive(&mut self, l: CreatureId) {
        let head = &mut self.world.lizard_at(l).body.chunks[0];
        head.pos = Vec2::new(200.0, 65.0);
        head.last_pos = Vec2::new(200.0, 85.0);
        head.vel = Vec2::new(0.0, -20.0);
        head.splash_stop = 0;
    }

    fn room_bubbles(&self) -> Vec<&crate::Bubble> {
        self.world
            .room_cosmetics(0)
            .iter()
            .filter_map(|c| match &c.kind {
                CosmeticKind::Bubble(b) => Some(b),
                _ => None,
            })
            .collect()
    }
}

/// The lizard (layer 1) joined after the player: `physicalObjects[1]` is the player,
/// then the lizard.
#[test]
fn lizard_and_player_share_layer_one_in_joining_order() {
    let mut g = game();
    let l = g.water_lizard();
    let seq = g.world.lizard_ref(l).update_order;
    assert!(seq > g.player_seq);
    let order = g.world.physical_objects(0, Some(g.player_seq));
    assert_eq!(
        order,
        [
            (1, g.player_seq, ChunkOwner::Player),
            (1, seq, ChunkOwner::Entity(Entity::Creature(l))),
        ]
    );
}

/// A lizard's head crossing down: the splash marks its chunk, a bubble emitter follows
/// it, and the entry bubbles join the room.
#[test]
fn a_diving_lizard_splashes_and_leaves_bubbles() {
    let mut g = game();
    let l = g.water_lizard();
    g.dive(l);
    let before = g.room_bubbles().len();
    let mut sounds = Sounds::default();
    g.world.update_water(0, None, &mut g.rng, &mut sounds);
    assert_eq!(g.world.lizard_ref(l).body.chunks[0].splash_stop, 10);
    let water = g
        .world
        .realized(0)
        .and_then(|r| r.water.as_ref())
        .expect("water");
    let owners: Vec<_> = water.bubble_emitters().iter().map(|e| e.owner).collect();
    assert_eq!(owners, [ChunkOwner::Entity(Entity::Creature(l))]);
    assert!(g.room_bubbles().len() > before);
}

/// A tick with the lizard diving: its entry bubbles (free, around x 200) were updated
/// once already.
#[test]
fn water_bubbles_update_in_the_tick_they_are_made() {
    let mut g = game();
    let l = g.water_lizard();
    g.dive(l);
    g.tick(Input::NONE, &mut ());
    let entry: Vec<_> = g
        .room_bubbles()
        .into_iter()
        .filter(|b| b.mode == crate::BubbleMode::Free && (b.last_pos.x - 200.0).abs() < 40.0)
        .collect();
    assert!(!entry.is_empty());
    assert!(entry.iter().all(|b| b.age == 1));
}

/// `AirBreatherCreature.Update` under water short of air: `value < Sin(InverseLerp(0.3,
/// -0.3, lungs) * pi) * 0.5` lets out `Bubble(mainBodyChunk.pos, RNV() * (value * 6))`
/// into the room.
#[test]
fn a_drowning_lizard_lets_out_bubbles() {
    let mut made = 0;
    for seed in 0..12 {
        let mut g = game();
        let l = g.water_lizard();
        for c in &mut g.world.lizard_at(l).body.chunks {
            c.pos.y = 30.0;
        }
        g.world.lizard_at(l).lungs = 0.0;
        g.rng = UnityRandom::new(seed);
        let mut copy = g.rng.clone();
        let before = g.room_bubbles().len();
        g.lizard_lungs(0, l);
        let lungs = g.world.lizard_ref(l).lungs;
        let drown = (lungs.inverse_lerp(0.3, -0.3) * std::f32::consts::PI).sin();
        let bubbles = g.room_bubbles();
        if copy.value() < drown * 0.5 {
            made += 1;
            let vel = Vec2::from_degrees(copy.value() * 360.0) * (copy.value() * 6.0);
            let b = bubbles.last().expect("a bubble");
            assert_eq!(bubbles.len(), before + 1);
            assert_eq!(b.pos, g.world.lizard_ref(l).body.chunks[0].pos);
            assert_eq!(b.vel, vel);
        } else {
            assert_eq!(bubbles.len(), before);
        }
    }
    assert!(made > 0 && made < 12);
}
