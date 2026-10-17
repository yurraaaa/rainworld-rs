//! One green lizard and the player in one room. Expected values from `Tracker.cs`
//! (`SeeCreature`, `CreatureNoticed`, `Ghost`'s constructor), `LizardAI.cs`
//! (`CreateTrackerRepresentationForCreature`, `UpdateDynamicRelationship`, the casual
//! aggression in `Update`, `DoIWantToBiteThisCreature`, `CurrentPlayerAggression`) and
//! `LizardBreeds.BreedTemplate` (green: `aggressionCurveExponent` 0.95).

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, CreatureType, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::creature::{CreatureId, CreatureTemplate, RelationshipType, RepKind, WorldCoordinate};
use crate::tests::room;
use crate::world::World;
use crate::{FloatExt, Input, RainWorldGame, Sounds, Start, Vec2};

/// A 30 x 6 room with a floor at y 1.
const HALL: [&str; 6] = [
    "##############################",
    "#............................#",
    "#............................#",
    "#............................#",
    "#............................#",
    "##############################",
];

/// Two halves split by a wall.
const SPLIT: [&str; 7] = [
    "########################",
    "#...........#..........#",
    "#...........#..........#",
    "#...........#..........#",
    "#...........#..........#",
    "#...........#..........#",
    "########################",
];

/// A Survivor story in `rows` with the player at `player`, after 20 ticks.
fn game(rows: &[&str], player: (i32, i32)) -> RainWorldGame {
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
        Sounds::default(),
        &mut (),
    );
    for _ in 0..20 {
        g.tick(Input::NONE, &mut ());
    }
    g
}

impl RainWorldGame {
    /// A realized green lizard at `tile` of room 0, its head toward `facing`, after
    /// `ticks` ticks.
    fn seeing_lizard(&mut self, tile: (i32, i32), facing: f32, ticks: usize) -> CreatureId {
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
        for _ in 0..ticks {
            self.tick(Input::NONE, &mut ());
        }
        l
    }
}

/// A lizard facing the player across a hall sees it: an elaborate representation (a
/// player is not small) in visual contact, related to by `UpdateDynamicRelationship`:
/// with no community opinion, no spawn evil and no temporary like, `LikeOfPlayer` is 0,
/// so it eats the player at `Pow(InverseLerp(0.5, -1, 0), 0.95)`, the representation's
/// priority, and its `CurrentPlayerAggression` is `InverseLerp(0.5, 0, 0)` = 1.
#[test]
fn a_lizard_tracks_the_player_it_sees() {
    let mut g = game(&HALL, (3, 1));
    let l = g.seeing_lizard((10, 1), -1.0, 40);
    assert_eq!(g.world.lizard_ref(l).spawn_data_evil, 0.0);
    let player = g.player_id;
    let t = &g.world.lizard_ref(l).ai.tracker;
    assert_eq!(t.reps.len(), 1);
    let r = &t.reps[0];
    assert_eq!(r.creature, player);
    assert!(matches!(r.kind, RepKind::Elaborate(_)));
    assert!(r.visual_contact);
    let d = r.dynamic_relationship.as_ref().expect("related");
    assert_eq!(d.current.kind, RelationshipType::Eats);
    let expected = 0.0f32.inverse_lerp(0.5, -1.0).mathf_pow(0.95);
    assert!((d.current.intensity - expected).abs() < 1e-6);
    assert!((r.priority - expected).abs() < 1e-6);
    assert_eq!(g.world.current_player_aggression(l, player), Some(1.0));
    let readings = g.tracker_readings(l);
    assert_eq!(readings.len(), 1);
    assert!(readings[0].visual_contact);
}

/// The player crawls after its start, so `privSneak` reaches 1 and `Player.VisibilityBonus`
/// is `0 - 0.5 * Sneak` = -0.5; its still chunks add `-0.7 * 0.7`, and the owner's bonus
/// counts at `1 - 0.7 / 2`: -0.815, a radius of `850 * 0.185` = 157 px. From 212 px away
/// the lizard never sees it, and its `CurrentPlayerAggression` stays the base 1.
#[test]
fn a_sneaking_player_is_unseen_from_afar() {
    let mut g = game(&HALL, (3, 1));
    let l = g.seeing_lizard((15, 1), -1.0, 40);
    let player = g.player_id;
    assert_eq!(g.player().expect("player").priv_sneak(), 1.0);
    let body = g.tracked_body(player).expect("realized");
    assert!((body.visibility + 0.5).abs() < 1e-6);
    assert!(g.world.lizard_ref(l).ai.tracker.reps.is_empty());
    assert_eq!(g.world.current_player_aggression(l, player), Some(1.0));
}

/// Behind a wall the player is never seen.
#[test]
fn a_wall_hides_the_player() {
    let mut g = game(&SPLIT, (18, 1));
    let l = g.seeing_lizard((8, 1), 1.0, 40);
    assert!(g.world.lizard_ref(l).ai.tracker.reps.is_empty());
}

/// `SeeCreature` of an untracked creature: `CreatureNoticed` makes an elaborate
/// representation whose first `Ghost` draws `moveBuffer` (one draw); its relationship
/// starts as the static `SocialDependent` (the dynamic one comes at the relationship
/// tracker's next update), so `CreatureSpotted` (no graphics: no draws) does not enter
/// `PreySpotted`.
#[test]
fn noticing_the_player_draws_its_first_ghost() {
    let mut g = game(&SPLIT, (18, 1));
    let l = g.seeing_lizard((8, 1), 1.0, 40);
    let player = g.player_id;
    let mut probe = g.rng.clone();
    let _ = probe.value();
    let (mut t, mut rel) = g.take_lizard_perception(l);
    g.tracker_see_creature(l, &mut t, &mut rel, player);
    g.put_lizard_perception(l, t, rel);
    assert_eq!(g.rng, probe, "one draw");
    let t = &g.world.lizard_ref(l).ai.tracker;
    assert_eq!(t.reps.len(), 1);
    let d = t.reps[0].dynamic_relationship.as_ref().expect("related");
    assert_eq!(d.current.kind, RelationshipType::SocialDependent);
    assert_ne!(
        g.world.lizard_ref(l).animation,
        crate::creature::LizardAnimation::PreySpotted
    );
}

/// The casual aggression with one tracked creature: `Range(0, 1)`, then
/// `DoIWantToBiteThisCreature`'s first roll (its second only under 0.1; a full-health
/// lizard's `value > health` never holds; the lizard goes for the player), then the bite's
/// coin. `VisualContact` only asks for the same room, and `SeeCreature` made the player
/// visible, so even behind the wall it becomes the target, too far to bite.
#[test]
fn casual_aggression_draws() {
    let mut g = game(&SPLIT, (18, 1));
    let l = g.seeing_lizard((8, 1), 1.0, 40);
    let player = g.player_id;
    let (mut t, mut rel) = g.take_lizard_perception(l);
    g.tracker_see_creature(l, &mut t, &mut rel, player);
    g.relationships_update(l, &mut t, &mut rel);
    g.put_lizard_perception(l, t, rel);
    let mut probe = g.rng.clone();
    let _ = probe.range_i32(0, 1);
    if probe.value() < 0.1 {
        let _ = probe.value();
    }
    let _ = probe.value();
    g.lizard_casual_aggression(l);
    let key = g.world.lizard_ref(l).ai.tracker.reps[0].key;
    assert_eq!(g.world.lizard_ref(l).ai.casual_aggression_target, Some(key));
    assert_eq!(g.rng, probe);
    assert!(g.world.creature(l).grasps[0].is_none());
}
