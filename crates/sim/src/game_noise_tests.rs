//! One green lizard and the player in one room, hearing noises. Expected values from
//! `NoiseTracker.cs` (`HeardNoise`, `TheorizedSource.Refresh`, `UpdateExamineSound`,
//! `Update`), `Tracker.cs` (`HeardThisCreature`), `LizardAI.cs` (`SocialEvent`,
//! `LizardPlayerRelationChange`) and `DiscomfortTracker.DiscomfortOfTile`.

use crate::creature::lizard::LizardBehavior;
use crate::creature::{CreatureId, LizardAnimation, RepKind, WorldCoordinate};
use crate::game::lizard_perception::tests::{HALL, SPLIT, game};
use crate::item::Entity;
use crate::world::SocialEventId;
use crate::{FloatExt, InGameNoise, Input, RainWorldGame, Vec2};

impl RainWorldGame {
    /// `room.InGameNoise` in room 0 of a noise the player made at `pos`.
    fn player_noise(&mut self, pos: Vec2, strength: f32) {
        let noise = InGameNoise {
            pos,
            strength,
            source: Some(Entity::Creature(self.player_id)),
            interesting: 1.0,
        };
        let player = self.player_body();
        self.world
            .in_game_noise(0, noise, None, player.as_ref(), &mut self.rng);
    }

    fn head(&self, l: CreatureId) -> Vec2 {
        self.world.lizard_ref(l).body.chunks[0].pos
    }
}

/// A noise behind a lizard that tracks nothing: no creature to guess, so a new unknown
/// source, mysterious (`interesting` 1 from a player: not kin, not a rock), the
/// counter at 200 and the source the one to examine; the idle lizard listens
/// (`ReactToNoise`: `HearSound`, whose curious voice is the one draw).
#[test]
fn an_unseen_noise_is_mysterious() {
    let mut g = game(&HALL, (3, 1));
    let l = g.seeing_lizard((20, 1), 1.0, 5);
    assert!(g.world.lizard_ref(l).ai.tracker.reps.is_empty());
    assert_eq!(g.world.lizard_ref(l).ai.behavior, LizardBehavior::Idle);
    let pos = g.head(l) - Vec2::new(60.0, 0.0);
    let mut probe = g.rng.clone();
    let _ = probe.value();
    g.player_noise(pos, 150.0);
    assert_eq!(g.rng, probe, "one draw");
    let lz = g.world.lizard_ref(l);
    assert_eq!(
        (lz.animation, lz.time_to_remain_in_animation, lz.bubble),
        (LizardAnimation::HearSound, 40, 40)
    );
    let n = &g.world.lizard_ref(l).ai.noise;
    assert_eq!(n.sources.len(), 1);
    let s = &n.sources[0];
    assert_eq!(
        (s.pos, s.age, s.creature_rep, s.last_sound_strength),
        (pos, 0, None, 150.0)
    );
    assert_eq!(
        (n.mysterious_noises, n.mysterious_noise_counter),
        (1.0, 200)
    );
    assert_eq!(n.sound_to_examine, Some(s.serial));
    assert!(n.heard[0].mysterious);
}

/// Heard only within `strength * hearingSkill` of the head (green:
/// `LerpMap(runSpeed, 0, 0.6, 1.3, 0.6)`), and not while the AI sees the spot.
#[test]
fn earshot_and_seen_noises() {
    let mut g = game(&HALL, (3, 1));
    let l = g.seeing_lizard((20, 1), 1.0, 5);
    let skill = g.world.lizard_ref(l).ai.noise.hearing_skill;
    let run = g.world.lizard_ref(l).ai.run_speed;
    assert_eq!(skill, run.lerp_map(0.0, 0.6, 1.3, 0.6));
    let head = g.head(l);
    g.player_noise(head - Vec2::new(100.0 * skill + 0.5, 0.0), 100.0);
    assert!(
        g.world.lizard_ref(l).ai.noise.sources.is_empty(),
        "out of earshot"
    );
    g.player_noise(head + Vec2::new(60.0, 0.0), 150.0);
    assert!(g.world.lizard_ref(l).ai.noise.sources.is_empty(), "seen");
    g.player_noise(head - Vec2::new(100.0 * skill - 0.5, 0.0), 100.0);
    assert_eq!(g.world.lizard_ref(l).ai.noise.sources.len(), 1);
}

/// The same unknown source heard again: refreshed (the match is distance + 200 + age / 5,
/// under 300), mysterious only once older than 10 ticks.
#[test]
fn a_repeated_noise_refreshes_its_source() {
    let mut g = game(&HALL, (3, 1));
    let l = g.seeing_lizard((20, 1), 1.0, 5);
    let pos = g.head(l) - Vec2::new(60.0, 0.0);
    g.player_noise(pos, 150.0);
    for _ in 0..5 {
        g.tick(Input::NONE, &mut ());
    }
    g.player_noise(pos - Vec2::new(10.0, 0.0), 150.0);
    let n = &g.world.lizard_ref(l).ai.noise;
    assert_eq!((n.sources.len(), n.sources[0].age), (1, 0));
    assert_eq!(n.mysterious_noises, 1.0, "young: not mysterious");
    for _ in 0..11 {
        g.tick(Input::NONE, &mut ());
    }
    g.player_noise(pos, 150.0);
    let n = &g.world.lizard_ref(l).ai.noise;
    assert_eq!(n.sources.len(), 1);
    assert_eq!(
        (n.mysterious_noises, n.mysterious_noise_counter),
        (2.0, 200)
    );
}

/// The sound to examine is done with once the lizard is within 3 tiles of it
/// (`ManhattanDistance`): slated, and gone the tick after.
#[test]
fn a_close_sound_is_examined() {
    let mut g = game(&HALL, (3, 1));
    let l = g.seeing_lizard((20, 1), 1.0, 5);
    let pos = g.head(l) - Vec2::new(30.0, 0.0);
    g.player_noise(pos, 150.0);
    g.tick(Input::NONE, &mut ());
    let n = &g.world.lizard_ref(l).ai.noise;
    assert!(n.sources[0].slated_for_deletion);
    assert_eq!(n.sound_to_examine, None);
    g.tick(Input::NONE, &mut ());
    assert!(g.world.lizard_ref(l).ai.noise.sources.is_empty());
}

/// A tracked player out of sight behind a wall makes a noise: it is guessed as the
/// source (`NoiseMatch` within `LerpMap(ticksSinceSeen, 20, 600, 200, 1000)`), not
/// mysterious; `HeardThisCreature` halves the unseen ticks past `seeAroundCorners + 5`;
/// its nearest ghost within 100 px moves to the noise, a still one given a random
/// heading (one draw), the newest guess (generation 0, the others one older). A
/// creature's noise is not the sound to examine: the lizard answers with a small noise
/// of its own and a bubble (`ReactToNoise`; the voice's intensity the second draw).
#[test]
fn a_tracked_creature_is_heard() {
    let mut g = game(&SPLIT, (18, 1));
    let l = g.seeing_lizard((8, 1), 1.0, 40);
    let player = g.player_id;
    let (mut t, mut rel) = g.take_lizard_perception(l);
    g.tracker_see_creature(l, &mut t, &mut rel, player);
    let see = t.see_around_corners;
    let r = &mut t.reps[0];
    r.visual_contact = false;
    r.ticks_since_seen = 100;
    let RepKind::Elaborate(e) = &mut r.kind else {
        panic!("elaborate");
    };
    assert_eq!(e.ghosts.len(), 1);
    e.ghosts[0].coord = WorldCoordinate::new(0, 15, 1, -1);
    e.ghosts[0].vel = Vec2::ZERO;
    e.ghosts[0].generation = 3;
    g.put_lizard_perception(l, t, rel);
    let pos = Vec2::middle_of_tile((17, 1));
    let mut probe = g.rng.clone();
    let _ = probe.value();
    let _ = probe.value();
    g.player_noise(pos, 400.0);
    assert_eq!(g.rng, probe, "two draws");
    let ai = &g.world.lizard_ref(l).ai;
    let key = ai.tracker.reps[0].key;
    // Just noticed, the player is still the static `SocialDependent`: not prey yet.
    assert_eq!(ai.prey.current, None);
    assert!(g.world.lizard_ref(l).bubble >= 5);
    assert_eq!(ai.noise.sources[0].creature_rep, Some(key));
    assert_eq!(ai.noise.mysterious_noises, 0.0);
    let r = &ai.tracker.reps[0];
    assert_eq!(r.ticks_since_seen, (100 - see) / 2);
    let RepKind::Elaborate(e) = &r.kind else {
        panic!("elaborate");
    };
    let gh = &e.ghosts[0];
    assert_eq!((gh.coord.tile, gh.pos, gh.generation), ((17, 1), pos, 0));
    assert!((gh.vel.length() - 1.0).abs() < 1e-5);
    assert_eq!(gh.last_coord, WorldCoordinate::new(-1, -1, -1, -1));
    assert!(e.best_ghost_dirty);
}

/// Further than 100 px from its ghosts the noise clones the nearest one there (the
/// clone's move buffer: one draw; the lizard's small noise in answer: another).
#[test]
fn a_far_noise_clones_a_ghost() {
    let mut g = game(&SPLIT, (18, 1));
    let l = g.seeing_lizard((8, 1), 1.0, 40);
    let player = g.player_id;
    let (mut t, mut rel) = g.take_lizard_perception(l);
    g.tracker_see_creature(l, &mut t, &mut rel, player);
    let r = &mut t.reps[0];
    r.visual_contact = false;
    r.ticks_since_seen = 100;
    let RepKind::Elaborate(e) = &mut r.kind else {
        panic!("elaborate");
    };
    e.ghosts[0].coord = WorldCoordinate::new(0, 14, 1, -1);
    g.put_lizard_perception(l, t, rel);
    let pos = Vec2::middle_of_tile((21, 1));
    let mut probe = g.rng.clone();
    let _ = probe.value();
    let _ = probe.value();
    g.player_noise(pos, 400.0);
    assert_eq!(g.rng, probe, "two draws");
    let RepKind::Elaborate(e) = &g.world.lizard_ref(l).ai.tracker.reps[0].kind else {
        panic!("elaborate");
    };
    assert_eq!(e.ghosts.len(), 2);
    let new = e
        .ghosts
        .iter()
        .find(|g| g.coord.tile == (21, 1))
        .expect("clone");
    assert_eq!(new.generation, 0);
}

/// A rock the player throws at a lizard that tracks it: `NonLethalAttack`, offence
/// 0.15, all of it as the victim: temporary like by 1.5 times the change, like by 0.75
/// times, knowing up by a quarter of its size; the community's like by a twentieth.
#[test]
fn an_attack_changes_the_lizards_like() {
    let mut g = game(&HALL, (3, 1));
    let l = g.seeing_lizard((10, 1), -1.0, 40);
    let player = g.player_id;
    let subject = g.world.creature(player).id;
    let before = g
        .world
        .creature(l)
        .state
        .social_memory
        .as_ref()
        .and_then(|m| m.get(subject))
        .is_some();
    assert!(!before);
    g.world
        .social_event(0, SocialEventId::NonLethalAttack, player, l, None);
    let m = g
        .world
        .creature(l)
        .state
        .social_memory
        .as_ref()
        .expect("memory");
    let r = m.get(subject).expect("relationship");
    assert!((r.temp_like + 0.225).abs() < 1e-6);
    assert!((r.like + 0.1125).abs() < 1e-6);
    assert!((r.know - 0.0375).abs() < 1e-6);
    let i = g.world.session.influences.last().expect("influence");
    assert!((i.influence + 0.0075).abs() < 1e-7);
    assert_eq!((i.inter_region_bleed, i.inter_community_bleed), (0.25, 0.3));
}

/// A lizard that does not track the attacker ignores the attack.
#[test]
fn an_unseen_attacker_changes_nothing() {
    let mut g = game(&SPLIT, (18, 1));
    let l = g.seeing_lizard((8, 1), 1.0, 40);
    let player = g.player_id;
    let n = g.world.session.influences.len();
    g.world
        .social_event(0, SocialEventId::LethalAttack, player, l, None);
    assert_eq!(g.world.session.influences.len(), n);
    let subject = g.world.creature(player).id;
    let m = g.world.creature(l).state.social_memory.as_ref();
    assert!(m.and_then(|m| m.get(subject)).is_none());
}

/// `DiscomfortOfTile`'s unknown-noise term (lizards have it at 0; set here): for each
/// unknown source, `Pow(InverseLerp(600, 0, dist) * InverseLerp(1000, 50, age), 0.5)`
/// pulls toward its maximum by `(0.5 + 0.5 * Utility) * uncomfortableWithUnknownNoises`.
#[test]
fn unknown_noises_are_uncomfortable() {
    let mut g = game(&HALL, (3, 1));
    let l = g.seeing_lizard((20, 1), 1.0, 5);
    let at = WorldCoordinate::new(0, 10, 1, -1);
    let pf = g.world.path_finder(l).expect("path finder").clone();
    assert_eq!(g.world.discomfort_of_tile(l, at, &pf), 0.0);
    let lz = g.world.lizard_at(l);
    lz.ai.discomfort.uncomfortable_with_unknown_noises = 1.0;
    let n = &mut lz.ai.noise;
    let i = n.add_source(Vec2::middle_of_tile((10, 1)) + Vec2::new(300.0, 0.0), None);
    n.sources[i].age = 50;
    n.sound_to_examine = Some(n.sources[i].serial);
    n.mysterious_noises = 20.0;
    let d = g.world.discomfort_of_tile(l, at, &pf);
    assert!((d - 0.5f32.sqrt()).abs() < 1e-6);
}
