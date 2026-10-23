//! Items the slugcat puts on the ground and a green lizard's gift, in one hall.
//! Expected values from `SocialEventRecognizer.cs` (`CreaturePutItemOnGround`,
//! `OwnedItemOnGround.Update`, `ItemOffered`), `FriendTracker.cs` (`ItemOffered`,
//! `Update`, `GiftRecieved`), `LizardAI.cs` (`UpdateDynamicRelationship`,
//! `LikeOfPlayer`, `GiftRecieved`, the constructor's `tamingDifficlty`) and
//! `LizardBreeds.BreedTemplate` (green: `tamingDifficulty` 0.8). No lizard eats a
//! ported creature, so the gift tests set `giftOfferedToMe` themselves.

use rw_assets::CreatureType;

use crate::creature::{
    CreatureId, GiftOffer, GrabRequest, RelationshipType, Shareability, WorldCoordinate,
};
use crate::game::lizard_perception::tests::{HALL, game};
use crate::item::Entity;
use crate::{FloatExt, Input, RainWorldGame, Vec2};

impl RainWorldGame {
    fn owned(&self) -> Vec<crate::world::OwnedItemOnGround> {
        self.world
            .realized(0)
            .expect("room")
            .social_events
            .owned_items_on_ground
            .clone()
    }

    /// The player's chunks moved to `x` on the floor, at rest.
    fn player_to(&mut self, x: f32) {
        let p = self.player_mut();
        p.body.chunks[0].hard_set_position(Vec2::new(x, 37.0));
        p.body.chunks[1].hard_set_position(Vec2::new(x, 23.0));
        for c in &mut p.body.chunks {
            c.vel = Vec2::ZERO;
        }
    }

    /// A green lizard at `tile` facing left, held on its tile (dev run speed 0) and
    /// watching for 40 ticks.
    fn held_lizard(&mut self, tile: (i32, i32)) -> CreatureId {
        let l = self.seeing_lizard(tile, -1.0, 0);
        let at = WorldCoordinate::new(0, tile.0, tile.1, -1);
        self.dev_set_lizard_destination(l, at, Some(0.0));
        for _ in 0..40 {
            self.tick(Input::NONE, &mut ());
        }
        l
    }

    /// A dead bat fly at `tile`.
    fn dead_fly(&mut self, tile: (i32, i32)) -> CreatureId {
        let f = self
            .dev_spawn_creature(CreatureType::Fly, 0, tile, None, &mut ())
            .expect("fly");
        assert!(self.world.creature(f).fly().is_some(), "realized");
        self.world.creature_die(f, &mut self.rng);
        f
    }

    /// Lizard `l` waiting for the fly `f` the player put down as its gift (what
    /// `FriendTracker.ItemOffered` sets for a creature it eats).
    fn give(&mut self, l: CreatureId, f: CreatureId) -> GiftOffer {
        self.player_put_item_on_ground(Entity::Creature(f));
        let o = self.owned()[0];
        let gift = GiftOffer {
            room: 0,
            serial: o.serial,
            item: o.item,
            owner: o.owner,
        };
        self.world.lizard_at(l).ai.friend.gift_offered_to_me = Some(gift);
        gift
    }
}

#[test]
fn owned_items_lapse_after_their_lifetime() {
    // `age > (offeredTo != null ? 30 : 10) * (living creature ? 20 : 40)`.
    let mut o = crate::world::OwnedItemOnGround {
        serial: 1,
        item: Entity::Creature(CreatureId(0)),
        owner: CreatureId(1),
        offered_to: None,
        active: true,
        age: 0,
        offered: false,
    };
    assert_eq!(o.lifetime(true), 200);
    assert_eq!(o.lifetime(false), 400);
    o.offered_to = Some(CreatureId(2));
    assert_eq!(o.lifetime(true), 600);
    assert_eq!(o.lifetime(false), 1200);
}

#[test]
fn putting_an_item_down_twice_keeps_one_entry() {
    let mut g = game(&HALL, (3, 1));
    let f = g.dead_fly((5, 1));
    g.player_put_item_on_ground(Entity::Creature(f));
    g.player_put_item_on_ground(Entity::Creature(f));
    assert_eq!(g.owned().len(), 1);
}

#[test]
fn an_owned_item_ages_lapses_and_is_dropped() {
    // A dead fly, nobody to offer it to: one a tick, inactive past 10 * 40, removed
    // by the next update.
    let mut g = game(&HALL, (3, 1));
    let f = g.dead_fly((6, 1));
    g.player_put_item_on_ground(Entity::Creature(f));
    for _ in 0..400 {
        g.tick(Input::NONE, &mut ());
    }
    assert_eq!(g.owned()[0].age, 400);
    assert!(g.owned()[0].active);
    g.tick(Input::NONE, &mut ());
    assert!(!g.owned()[0].active);
    g.tick(Input::NONE, &mut ());
    assert!(g.owned().is_empty());
}

#[test]
fn a_dead_fly_is_offered_to_the_lizard_watching_but_is_no_gift() {
    // `OwnedItemOnGround.Update`: once the slugcat is 50 px from its item, `ItemOffered`
    // picks the closest living lizard within 500 px that saw the slugcat in the last
    // 120 ticks. `FriendTracker.ItemOffered` takes only a creature it eats: a lizard
    // ignores a bat fly, so no gift. The offer stretches the item's life to 30 * 40.
    let mut g = game(&HALL, (3, 1));
    let l = g.held_lizard((10, 1));
    let player = g.player_id;
    assert!(
        g.world
            .lizard_ref(l)
            .ai
            .tracker
            .representation_for(player)
            .is_some()
    );
    let f = g.dead_fly((6, 1));
    assert_eq!(
        g.world.dynamic_relationship(l, f).kind,
        RelationshipType::Ignores
    );
    g.player_to(130.0);
    g.player_put_item_on_ground(Entity::Creature(f));
    g.tick(Input::NONE, &mut ());
    assert!(!g.owned()[0].offered, "within 50 px");
    g.player_to(60.0);
    g.tick(Input::NONE, &mut ());
    let o = g.owned()[0];
    assert!(o.offered);
    assert_eq!(o.offered_to, Some(l));
    assert_eq!(o.lifetime(false), 1200);
    assert_eq!(g.world.lizard_ref(l).ai.friend.gift_offered_to_me, None);
}

#[test]
fn nobody_watching_means_no_offer() {
    let mut g = game(&HALL, (3, 1));
    let f = g.dead_fly((6, 1));
    g.player_to(60.0);
    g.player_put_item_on_ground(Entity::Creature(f));
    g.tick(Input::NONE, &mut ());
    let o = g.owned()[0];
    assert!(o.offered);
    assert_eq!(o.offered_to, None);
}

#[test]
fn a_gift_raises_the_like_of_its_giver() {
    // `LikeOfPlayer` with a gift from the player: `LerpMap(like, -0.5, 1, 0, 1, 0.8)`.
    let mut g = game(&HALL, (3, 1));
    let l = g.held_lizard((10, 1));
    let f = g.dead_fly((6, 1));
    let player = g.player_id;
    let rep = g
        .world
        .lizard_ref(l)
        .ai
        .tracker
        .representation_for(player)
        .cloned();
    let before = g.world.lizard_like_of_player(l, rep.as_ref());
    g.give(l, f);
    let like = g.world.lizard_like_of_player(l, rep.as_ref());
    let expected = before.lerp_map_pow(-0.5, 1.0, 0.0, 1.0, 0.8);
    assert!((like - expected).abs() < 1e-6, "{like} vs {expected}");
}

#[test]
fn an_active_gift_is_eaten_in_the_dynamic_relationship() {
    // `UpdateDynamicRelationship`: the gift creature is eaten, at 1 when dead. (A
    // representation of the fly made from the player's.)
    let mut g = game(&HALL, (3, 1));
    let l = g.held_lizard((10, 1));
    let f = g.dead_fly((6, 1));
    g.give(l, f);
    let mut t = g.world.lizard_ref(l).ai.tracker.clone();
    let mut rep = t.reps[0].clone();
    rep.creature = f;
    t.reps.push(rep);
    let i = t.reps.len() - 1;
    let rel = g.ai_update_dynamic_relationship(l, &mut t, i);
    assert_eq!(rel.kind, RelationshipType::Eats);
    assert_eq!(rel.intensity, 1.0);
}

#[test]
fn a_living_gift_is_eaten_at_0_65() {
    // `UpdateDynamicRelationship`: a living gift creature is eaten at 0.65.
    let mut g = game(&HALL, (3, 1));
    let l = g.held_lizard((10, 1));
    let f = g
        .dev_spawn_creature(CreatureType::Fly, 0, (6, 1), None, &mut ())
        .expect("fly");
    assert!(g.world.creature(f).state.alive);
    g.give(l, f);
    let mut t = g.world.lizard_ref(l).ai.tracker.clone();
    let mut rep = t.reps[0].clone();
    rep.creature = f;
    t.reps.push(rep);
    let i = t.reps.len() - 1;
    let rel = g.ai_update_dynamic_relationship(l, &mut t, i);
    assert_eq!(rel.kind, RelationshipType::Eats);
    assert_eq!(rel.intensity, 0.65);
}

#[test]
fn holding_the_gift_receives_it() {
    // `FriendTracker.Update`: the lizard holding its gift receives it: like
    // `+1.2 / tamingDifficlty` (a dead creature) damped by `know`, temp like
    // `+1.7 / tamingDifficlty`; the gift forgotten. `tamingDifficlty` is green's 0.8
    // times `Lerp(0.9, 1.1, dominance)`.
    let mut g = game(&HALL, (3, 1));
    let l = g.held_lizard((10, 1));
    let f = g.dead_fly((6, 1));
    let player = g.world.creature(g.player_id).id;
    let dominance = g.world.creature(l).personality.dominance;
    let taming = 0.8 * 0.9_f32.lerp(1.1, dominance);
    assert_eq!(g.world.lizard_ref(l).ai.friend.taming_difficulty, taming);
    g.give(l, f);
    let (like0, temp0, know) = g
        .world
        .creature(l)
        .state
        .social_memory
        .as_ref()
        .and_then(|m| m.get(player))
        .map_or((0.0, 0.0, 0.0), |r| (r.like, r.temp_like, r.know));
    let grabbed = g.world.grab(
        GrabRequest {
            grabber: l,
            grabbed: Entity::Creature(f),
            grasp_used: 0,
            chunk_grabbed: 0,
            shareability: Shareability::CanOnlyShareWithNonExclusive,
            dominance: 1.0,
            override_equally_dominant: true,
            pacifying: true,
        },
        &mut g.rng,
    );
    assert!(grabbed);
    g.world.friend_tracker_update(l);
    assert_eq!(g.world.lizard_ref(l).ai.friend.gift_offered_to_me, None);
    let r = g
        .world
        .creature(l)
        .state
        .social_memory
        .as_ref()
        .and_then(|m| m.get(player))
        .cloned()
        .expect("relationship");
    let like = (like0 + 1.2 / taming * 1.0_f32.lerp(0.1, know)).clamp(-1.0, 1.0);
    assert!((r.like - like).abs() < 1e-6, "{} vs {like}", r.like);
    let temp = (temp0 + 1.7 / taming).clamp(-1.0, 1.0);
    assert!((r.temp_like - temp).abs() < 1e-6);
    // The recognizer: picked up, no longer active.
    g.tick(Input::NONE, &mut ());
    assert!(!g.owned()[0].active);
}

#[test]
fn the_offered_lizard_picking_it_up_receives_the_gift_in_the_recognizer() {
    // `OwnedItemOnGround.Update`: grabbed by the creature it was offered to, an
    // `ItemTransaction` and that creature's `friendTracker.GiftRecieved()` (the gift
    // being this item), before the lizard's own update.
    let mut g = game(&HALL, (3, 1));
    let l = g.held_lizard((10, 1));
    let f = g.dead_fly((6, 1));
    g.give(l, f);
    g.world
        .realized_mut(0)
        .expect("room")
        .social_events
        .owned_items_on_ground[0]
        .offered_to = Some(l);
    g.social_event_recognizer_update(0);
    assert!(g.owned()[0].active);
    let request = GrabRequest {
        grabber: l,
        grabbed: Entity::Creature(f),
        grasp_used: 0,
        chunk_grabbed: 0,
        shareability: Shareability::CanOnlyShareWithNonExclusive,
        dominance: 1.0,
        override_equally_dominant: true,
        pacifying: true,
    };
    assert!(g.world.grab(request, &mut g.rng));
    g.social_event_recognizer_update(0);
    assert_eq!(g.world.lizard_ref(l).ai.friend.gift_offered_to_me, None);
    assert!(!g.owned()[0].active);
    let player = g.world.creature(g.player_id).id;
    let like = g
        .world
        .creature(l)
        .state
        .social_memory
        .as_ref()
        .and_then(|m| m.get(player))
        .map_or(0.0, |r| r.like);
    assert!(like > 0.0);
}

#[test]
fn a_lapsed_gift_is_forgotten() {
    // `FriendTracker.Update`: an inactive `giftOfferedToMe` is cleared.
    let mut g = game(&HALL, (3, 1));
    let l = g.held_lizard((10, 1));
    let f = g.dead_fly((6, 1));
    g.give(l, f);
    g.world
        .realized_mut(0)
        .expect("room")
        .social_events
        .owned_items_on_ground[0]
        .active = false;
    g.world.friend_tracker_update(l);
    assert_eq!(g.world.lizard_ref(l).ai.friend.gift_offered_to_me, None);
}

#[test]
fn the_recognizer_makes_no_draws() {
    // `OwnedItemOnGround.Update`, `ItemOffered`, `ItemTransaction` and `GiftRecieved`
    // call no `Random`.
    let mut g = game(&HALL, (3, 1));
    let l = g.held_lizard((10, 1));
    let f = g.dead_fly((6, 1));
    g.player_to(60.0);
    g.give(l, f);
    let before = g.rng.state();
    g.social_event_recognizer_update(0);
    assert_eq!(g.owned()[0].offered_to, Some(l));
    let request = GrabRequest {
        grabber: l,
        grabbed: Entity::Creature(f),
        grasp_used: 0,
        chunk_grabbed: 0,
        shareability: Shareability::CanOnlyShareWithNonExclusive,
        dominance: 1.0,
        override_equally_dominant: true,
        pacifying: true,
    };
    let mut rng = g.rng.clone();
    assert!(g.world.grab(request, &mut rng));
    g.social_event_recognizer_update(0);
    assert_eq!(g.world.lizard_ref(l).ai.friend.gift_offered_to_me, None);
    assert_eq!(g.rng.state(), before);
}
