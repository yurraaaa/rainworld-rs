//! Two green lizards (and the player) in one hall. Expected values from `LizardAI.cs`
//! (`Update`'s Fighting and ActingOutMission bodies, `SendCommunication`,
//! `RecieveCommunication`), `AgressionTracker.cs` (`IncrementAnger`, `SetAnger`),
//! `Lizard.cs` (`Collide`, `EnterAnimation` and `ActAnimation` of FightingStance),
//! `LizardVoice.MakeSound` (one draw), `MissionTracker.cs` and `StaticWorld` (green to
//! green: AgressiveRival 0.8).

use rw_random::UnityRandom;

use crate::creature::lizard::{LizardBehavior, LizardCommunication, LizardGraphicsLink};
use crate::creature::{CreatureId, LizardAnimation, MissionKind, RelationshipType, RepKey};
use crate::game::lizard_hits::ChunkRef;
use crate::game::lizard_perception::tests::{HALL, game};
use crate::item::Entity;
use crate::{FloatExt, RainWorldGame};

/// The draws between `before` and `after`.
fn draws(before: &UnityRandom, after: &UnityRandom) -> usize {
    let mut p = before.clone();
    for n in 0..100_000 {
        if &p == after {
            return n;
        }
        p.value();
    }
    panic!("streams never meet");
}

/// Two rivals: `a` at (12, 1) facing right, `b` at (17, 1) facing left, held still for
/// 40 ticks so each tracks the other; `ka` is `a`'s representation of `b`, `kb`
/// `b`'s of `a`.
struct Rivals {
    a: CreatureId,
    b: CreatureId,
    ka: RepKey,
    kb: RepKey,
}

impl RainWorldGame {
    fn rivals(&mut self) -> Rivals {
        let a = self.seeing_lizard((12, 1), 1.0, 0);
        let b = self.seeing_lizard((17, 1), -1.0, 0);
        for (l, tile) in [(a, (12, 1)), (b, (17, 1))] {
            let at = crate::creature::WorldCoordinate::new(0, tile.0, tile.1, -1);
            self.dev_set_lizard_destination(l, at, Some(0.0));
        }
        for _ in 0..40 {
            self.tick(crate::Input::NONE, &mut ());
        }
        self.dev_release_lizard(a);
        self.dev_release_lizard(b);
        let ka = self.rep_key_of(a, b);
        let kb = self.rep_key_of(b, a);
        Rivals { a, b, ka, kb }
    }

    fn rep_key_of(&self, l: CreatureId, other: CreatureId) -> RepKey {
        self.world
            .lizard_ref(l)
            .ai
            .tracker
            .representation_for(other)
            .expect("tracks the other lizard")
            .key
    }

    /// `l`'s anger at representation `k`.
    fn anger(&self, l: CreatureId, k: RepKey) -> (f32, f32) {
        let a = &self.world.lizard_ref(l).ai.agression;
        let c = a.creatures.iter().find(|c| c.rep == k).expect("an anger");
        (c.anger, c.base_anger)
    }

    /// Makes `k` lizard `l`'s aggression target (anger 0.5).
    fn angry_at(&mut self, l: CreatureId, k: RepKey) {
        let a = &mut self.world.lizard_at(l).ai.agression;
        a.add(k, 0.5, 0.8);
        a.set_anger(Some(k), 0.5, -1.0);
        a.highest_anger = 0.5;
        a.highest_target = Some(k);
    }

    fn set_visual(&mut self, l: CreatureId, k: RepKey, visual: bool) {
        let t = &mut self.world.lizard_at(l).ai.tracker;
        t.rep_mut(k).expect("rep").visual_contact = visual;
    }

    /// Advances the stream until the next draws satisfy `want` (a copy is tried).
    fn rng_until_draws(&mut self, want: fn(&mut UnityRandom) -> bool) {
        loop {
            let mut p = self.rng.clone();
            if want(&mut p) {
                return;
            }
            self.rng.value();
        }
    }
}

/// The rivals relate as the static table says (AgressiveRival 0.8), sorted into the
/// aggression tracker (`AddCreature(rep, 0.1, intensity)`).
#[test]
fn green_lizards_are_aggressive_rivals() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    let rel = g.world.dynamic_relationship(r.a, r.b);
    assert_eq!(rel.kind, RelationshipType::AgressiveRival);
    assert!((rel.intensity - 0.8).abs() < 1e-6);
    let base = g.anger(r.a, r.ka).1;
    assert!((base - 0.8).abs() < 1e-6);
}

/// No aggression target: the focus is cleared, nothing drawn, `runSpeed` untouched.
#[test]
fn fighting_without_a_target_only_clears_the_focus() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    let ai = &mut g.world.lizard_at(r.a).ai;
    ai.agression.highest_target = None;
    ai.focus_creature = Some(r.ka);
    ai.run_speed = 0.3;
    let before = g.rng.clone();
    g.lizard_fighting(r.a);
    assert_eq!(g.rng, before);
    let ai = &g.world.lizard_ref(r.a).ai;
    assert_eq!(ai.focus_creature, None);
    assert_eq!(ai.run_speed, 0.3);
}

/// An unseen rival: not close, so `runSpeed` eases to 0.8 by 0.15 and a 1/180 draw
/// (missed) for the stance; a 1/20 draw (missed) for the signal; `AggressiveBehavior`
/// at an unseen target draws nothing. Two draws; the focus is the target.
#[test]
fn fighting_an_unseen_rival_draws_twice() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    g.angry_at(r.a, r.ka);
    g.set_visual(r.a, r.ka, false);
    g.world.lizard_at(r.a).ai.run_speed = 0.0;
    g.rng_until_draws(|p| p.value() >= 1.0 / 180.0 && p.value() >= 0.05);
    let before = g.rng.clone();
    g.lizard_fighting(r.a);
    assert_eq!(draws(&before, &g.rng), 2);
    let ai = &g.world.lizard_ref(r.a).ai;
    assert!((ai.run_speed - 0.12).abs() < 1e-6);
    assert_eq!(ai.focus_creature, Some(r.ka));
    assert_ne!(
        g.world.lizard_ref(r.a).animation,
        LizardAnimation::FightingStance
    );
}

/// The 1/180 draw hit: `EnterAnimation(FightingStance)` sets bubble 10, intensity 1
/// and `Random.Range(20, 50)` ticks (a third draw before the signal's).
#[test]
fn the_stance_from_afar_draws_its_time() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    g.angry_at(r.a, r.ka);
    g.set_visual(r.a, r.ka, false);
    g.world.lizard_at(r.a).animation = LizardAnimation::Standard;
    g.rng_until_draws(|p| {
        let hit = p.value() < 1.0 / 180.0;
        p.range_i32(20, 50);
        hit && p.value() >= 0.05
    });
    let before = g.rng.clone();
    let mut probe = g.rng.clone();
    probe.value();
    let time = probe.range_i32(20, 50);
    g.lizard_fighting(r.a);
    assert_eq!(draws(&before, &g.rng), 3);
    let l = g.world.lizard_ref(r.a);
    assert_eq!(l.animation, LizardAnimation::FightingStance);
    assert_eq!(l.time_to_remain_in_animation, time);
    assert_eq!(l.bubble, 10);
    assert_eq!(l.bubble_intensity, 1.0);
}

/// Close and seen (within 160 px, nearer `b`'s head than its tail): the 1/20 stance
/// draw (missed) and `runSpeed` toward 0.2 by 0.15.
#[test]
fn a_close_rival_slows_it_down() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    g.angry_at(r.a, r.ka);
    g.set_visual(r.a, r.ka, true);
    let head = g.world.lizard_ref(r.a).head().pos;
    let theirs = &g.world.lizard_ref(r.b).body.chunks;
    assert!(head.dist_less(theirs[0].pos, 160.0));
    assert!(head.distance(theirs[0].pos) < head.distance(theirs[2].pos));
    g.world.lizard_at(r.a).ai.run_speed = 1.0;
    g.world.lizard_at(r.a).lounge_delay = 100;
    g.rng_until_draws(|p| p.value() >= 0.05 && p.value() >= 0.05);
    g.lizard_fighting(r.a);
    assert!((g.world.lizard_ref(r.a).ai.run_speed - 1.0f32.lerp(0.2, 0.15)).abs() < 1e-6);
}

/// The signal: the 1/20 draw and one under the intensity (0.8) send aggression. With
/// no graphics module no dominance draw; the dominance voice (one draw); `b`, seeing
/// `a`'s head, looks at it, is angered by 0.1 and misses its 1/24 submission draw.
/// Five draws.
#[test]
fn the_aggression_signal_angers_the_rival() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    g.angry_at(r.a, r.ka);
    g.set_visual(r.a, r.ka, false);
    g.world.lizard_at(r.b).ai.focus_creature = None;
    let anger = g.anger(r.b, r.kb).0;
    g.rng_until_draws(|p| {
        p.value() >= 1.0 / 180.0 && p.value() < 0.05 && p.value() < 0.8 && {
            p.value();
            p.value() >= 1.0 / 24.0
        }
    });
    let before = g.rng.clone();
    g.lizard_fighting(r.a);
    assert_eq!(draws(&before, &g.rng), 5);
    let sent = g.world.lizard_ref(r.a).ai.last_signal.map(|s| (s.0, s.1));
    assert_eq!(sent, Some((LizardCommunication::Agression, r.b)));
    assert_eq!(g.world.lizard_ref(r.b).ai.focus_creature, Some(r.kb));
    assert!((g.anger(r.b, r.kb).0 - (anger + 0.1).clamp(0.0, 1.0)).abs() < 1e-6);
    assert_eq!(g.world.lizard_ref(r.b).ai.submitted_to, None);
}

/// With a graphics module the sender's `showDominance` grows by a draw made before the
/// voice's.
#[test]
fn the_dominance_display_takes_a_draw() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    g.world.lizard_at(r.a).graphics = Some(LizardGraphicsLink::default());
    g.rng_until_draws(|p| {
        p.value();
        p.value();
        p.value() >= 1.0 / 24.0
    });
    let mut probe = g.rng.clone();
    let shown = probe.value();
    let before = g.rng.clone();
    g.lizard_send_communication(r.a, r.b, LizardCommunication::Agression);
    assert_eq!(draws(&before, &g.rng), 3);
    let add = g
        .world
        .lizard_ref(r.a)
        .graphics
        .as_ref()
        .unwrap()
        .show_dominance_add;
    assert_eq!(add, shown);
}

/// The 1/24 draw hit: `b` submits. It sends submission back (voice: one draw; `a`
/// seeing `b` settles its anger at `b` to 0, its base kept), its own anger and base
/// at `a` go to 0, `a` is the one it submitted to, and it takes a `LeaveRoom` mission
/// (0.4 for 1000 ticks), abandoned at once in this single-room world. Three draws:
/// the two voices and the roll.
#[test]
fn a_submission_settles_both_and_gives_a_mission() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    g.angry_at(r.a, r.ka);
    let base_a = g.anger(r.a, r.ka).1;
    g.rng_until_draws(|p| {
        p.value();
        p.value() < 1.0 / 24.0
    });
    let before = g.rng.clone();
    g.lizard_send_communication(r.a, r.b, LizardCommunication::Agression);
    assert_eq!(draws(&before, &g.rng), 3);
    let ai_b = &g.world.lizard_ref(r.b).ai;
    assert_eq!(ai_b.submitted_to, Some(r.a));
    let sent = ai_b.last_signal.map(|s| (s.0, s.1));
    assert_eq!(sent, Some((LizardCommunication::Submission, r.a)));
    assert_eq!(g.anger(r.b, r.kb), (0.0, 0.0));
    assert_eq!(g.anger(r.a, r.ka), (0.0, base_a));
    let m = &g.world.lizard_ref(r.b).ai.missions.missions;
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].kind(), MissionKind::LeaveRoom);
    assert_eq!((m[0].current_utility, m[0].forget_counter), (0.4, 1000));
    assert!(g.world.single_room());
    assert!(m[0].delete_me_next_frame);
}

/// Aggression from a lizard that submitted to the receiver does nothing past the look
/// (no anger, no draw); one the receiver submitted to raises no anger but still rolls.
#[test]
fn submitted_lizards_are_not_angered() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    let anger = g.anger(r.b, r.kb).0;
    g.world.lizard_at(r.a).ai.submitted_to = Some(r.b);
    let before = g.rng.clone();
    g.lizard_recieve_communication(r.b, r.a, LizardCommunication::Agression);
    assert_eq!(g.rng, before);
    assert_eq!(g.anger(r.b, r.kb).0, anger);
    g.world.lizard_at(r.a).ai.submitted_to = None;
    g.world.lizard_at(r.b).ai.submitted_to = Some(r.a);
    g.rng_until_draws(|p| p.value() >= 1.0 / 24.0);
    let before = g.rng.clone();
    g.lizard_recieve_communication(r.b, r.a, LizardCommunication::Agression);
    assert_eq!(draws(&before, &g.rng), 1);
    assert_eq!(g.anger(r.b, r.kb).0, anger);
}

/// No signal to a dead lizard or one in another room: no look, draw or voice.
#[test]
fn no_signal_to_the_dead() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    g.world.lizard_at(r.b).dead = true;
    g.world.lizard_at(r.a).ai.focus_creature = None;
    let before = g.rng.clone();
    g.lizard_send_communication(r.a, r.b, LizardCommunication::Agression);
    assert_eq!(g.rng, before);
    assert_eq!(g.world.lizard_ref(r.a).ai.focus_creature, None);
    assert!(g.world.lizard_ref(r.a).ai.last_signal.is_none());
}

/// Touching a rival angers by `0.05 * 0.8`; not one it submitted to.
#[test]
fn touching_a_rival_angers() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    g.world
        .lizard_at(r.a)
        .ai
        .agression
        .set_anger(Some(r.ka), 0.2, -1.0);
    g.lizard_rival_touch(r.a, r.b);
    assert!((g.anger(r.a, r.ka).0 - 0.24).abs() < 1e-6);
    g.world.lizard_at(r.a).ai.submitted_to = Some(r.b);
    g.lizard_rival_touch(r.a, r.b);
    assert!((g.anger(r.a, r.ka).0 - 0.24).abs() < 1e-6);
}

/// `ActAnimation(FightingStance)`: jaw open, run speed 0, and with a seen focus off a
/// safe tile the head pulled toward it by `4 * LegsGripping` and the middle and tail
/// pushed back by 2.2 and 2.6 times it (`LegsGripping` 2 without graphics).
#[test]
fn the_stance_leans_toward_a_seen_focus() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    g.set_visual(r.a, r.ka, true);
    let l = g.world.lizard_at(r.a);
    l.animation = LizardAnimation::FightingStance;
    l.ai.focus_creature = Some(r.ka);
    for c in &mut l.body.chunks {
        c.vel = crate::Vec2::ZERO;
    }
    let head = l.head().pos;
    let danger = g.world.lizard_ref(r.b).head().pos;
    let dir = head.direction_to(danger);
    let run = g.lizard_act_animation(r.a);
    assert_eq!(run, 0.0);
    let l = g.world.lizard_ref(r.a);
    assert_eq!(l.jaw_open(false), 1.0);
    let v = [
        l.body.chunks[0].vel,
        l.body.chunks[1].vel,
        l.body.chunks[2].vel,
    ];
    assert!((v[0] - dir * 8.0).length() < 1e-4);
    assert!((v[1] + dir * 4.4).length() < 1e-4);
    assert!((v[2] + dir * 5.2).length() < 1e-4);
}

/// ActingOutMission without a mission: only `runSpeed`, toward 0.6 by 0.05.
#[test]
fn acting_out_no_mission_only_eases_the_run() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    g.world.lizard_at(r.a).ai.run_speed = 0.0;
    let before = g.rng.clone();
    g.lizard_acting_out_mission(r.a);
    assert_eq!(g.rng, before);
    assert!((g.world.lizard_ref(r.a).ai.run_speed - 0.03).abs() < 1e-7);
}

/// The mission tracker's utility reaches the comparer (`MissionTracker` 0.8 weight): a
/// `LeaveRoom` that is not abandoned gives 0.32 once the tracker has updated. The
/// comparer updates before the mission tracker and `DetermineBehavior` reads the
/// comparer's last update, so the behaviour follows two ticks after the mission.
#[test]
fn a_mission_reaches_the_utility_comparer() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    let ai = &mut g.world.lizard_at(r.a).ai;
    ai.missions.add(
        crate::creature::Mission::leave_room(0.4, 1000, false),
        false,
    );
    g.tick(crate::Input::NONE, &mut ());
    assert_eq!(g.world.lizard_ref(r.a).ai.missions.utility(), 0.4);
    g.tick(crate::Input::NONE, &mut ());
    g.tick(crate::Input::NONE, &mut ());
    let readings = g.lizard_utility_readings(r.a).unwrap();
    let m = readings
        .modules
        .iter()
        .find(|m| m.0 == crate::creature::UtilityModule::MissionTracker)
        .unwrap();
    assert!((m.1 - 0.32).abs() < 1e-6);
    assert_eq!(
        readings.highest,
        Some(crate::creature::UtilityModule::MissionTracker)
    );
    assert_eq!(
        g.world.lizard_ref(r.a).ai.behavior,
        LizardBehavior::ActingOutMission
    );
}

/// `Bite` on a rival: not prey, so the grab needs a draw under 0.5 (the rival is not
/// 1.2 times heavier); grabbed, `b` is held in `a`'s jaws.
#[test]
fn a_bite_grabs_a_rival_half_the_time() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    g.rng_until_draws(|p| p.value() < 0.5);
    g.lizard_bite(r.a, Some(chunk_of(r.b, 1)));
    let held = g.world.creature(r.a).grasps[0].map(|gr| gr.grabbed);
    assert_eq!(held, Some(Entity::Creature(r.b)));
    assert!(
        g.world
            .creature(r.b)
            .grabbed_by
            .iter()
            .any(|gr| gr.grabber == r.a)
    );
}

/// `Bite` on a rival with the grab draw missed: the next draw (an attacked creature's
/// `DamageAttack`) does not matter, for a rival is not attacked; nothing is held.
#[test]
fn a_missed_bite_on_a_rival_holds_nothing() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    g.rng_until_draws(|p| p.value() >= 0.5);
    let health = g.world.creature(r.b).state.health;
    g.lizard_bite(r.a, Some(chunk_of(r.b, 1)));
    assert!(g.world.creature(r.a).grasps[0].is_none());
    assert_eq!(g.world.creature(r.b).state.health, health);
}

/// `DamageAttack` on a rival: `dmgFac` 0, so no damage, the attacker not stunned
/// (`16 * 0`); the 30 stun bonus stuns the rival `30 / 2.5 / 3` (base stun resistance,
/// the green's bite stun resistance) = 4 ticks at full health.
#[test]
fn a_damage_attack_spares_a_rival() {
    let mut g = game(&HALL, (3, 1));
    let r = g.rivals();
    g.world.lizard_at(r.a).stun = 0;
    g.world.lizard_at(r.b).stun = 0;
    let health = g.world.creature(r.b).state.health;
    g.lizard_damage_attack(r.a, chunk_of(r.b, 1), 1.0);
    assert_eq!(g.world.creature(r.b).state.health, health);
    assert_eq!(g.world.lizard_ref(r.a).stun, 0);
    assert_eq!(g.world.lizard_ref(r.b).stun, 4);
}

fn chunk_of(c: CreatureId, index: usize) -> ChunkRef {
    ChunkRef {
        owner: Entity::Creature(c),
        index,
    }
}
