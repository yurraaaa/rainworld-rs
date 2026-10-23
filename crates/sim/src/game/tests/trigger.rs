use super::*;

const A07: &str = "Spot<tA>event<tB>MusicEvent<eA>songName<eB>NA_01 - Proxima<eA>vol<eB>0.3108693<tA>multiUse<tB>0<tA>toCycle<tB>-1<tA>fromCycle<tB>0<tA>delay<tB>0<tA>fireChance<tB>1<tA>entrance<tB>-1<tA>karma<tB>0<tA>slugcats<tB><tA>pos<tB>710.0718<tB>289.2014<tA>handlePos<tB>-103.3553<tB>-215.3553";

impl TriggerPlayer {
    fn at(pos: Vec2) -> Self {
        TriggerPlayer {
            in_room: true,
            realized: true,
            node: 1,
            still_in_start_shelter: false,
            in_shortcut: false,
            karma: 0,
            pos,
        }
    }
}

/// `wait` 10: the first ten ticks with the player in the room count down; the
/// eleventh inside the circle (|handlePos| = 238.9 px) fires.
#[test]
fn spot_waits_ten_ticks_then_fires_inside_the_circle() {
    let t = EventTrigger::parse(A07).unwrap();
    let mut c = ActiveTriggerChecker::new(0);
    let inside = TriggerPlayer::at(Vec2::new(710.0, 100.0));
    for _ in 0..10 {
        assert!(!c.update(&t, &inside, TriggerHud::default()));
    }
    assert_eq!(c.wait, 0);
    let outside = TriggerPlayer::at(Vec2::new(710.0, 30.0));
    assert!(!c.update(&t, &outside, TriggerHud::default()));
    assert!(c.update(&t, &inside, TriggerHud::default()));
}

#[test]
fn conditions_block_and_absence_keeps_the_wait() {
    let t = EventTrigger::parse(&A07.replace("entrance<tB>-1", "entrance<tB>3")).unwrap();
    let mut c = ActiveTriggerChecker::new(0);
    c.wait = 0;
    let mut p = TriggerPlayer::at(Vec2::new(710.0, 289.0));
    assert!(!c.update(&t, &p, TriggerHud::default()));
    p.node = 3;
    let over = TriggerHud {
        game_over_mode: true,
        region_bumps: None,
    };
    assert!(!c.update(&t, &p, over));
    p.in_shortcut = true;
    assert!(!c.update(&t, &p, TriggerHud::default()));
    p.in_shortcut = false;
    assert!(c.update(&t, &p, TriggerHud::default()));
    let mut d = ActiveTriggerChecker::new(0);
    p.in_room = false;
    assert!(!d.update(&t, &p, TriggerHud::default()));
    assert_eq!(d.wait, 10);
}

/// A delay starts the counter on the first positive tick; the event fires once the
/// counter reaches it, wherever the player is by then.
#[test]
fn delay_counts_from_the_first_positive_tick() {
    let t = EventTrigger::parse(&A07.replace("delay<tB>0", "delay<tB>3")).unwrap();
    let mut c = ActiveTriggerChecker::new(0);
    c.wait = 0;
    let inside = TriggerPlayer::at(Vec2::new(710.0, 289.0));
    assert!(!c.update(&t, &inside, TriggerHud::default()));
    assert_eq!(c.counter, 0);
    let mut away = inside;
    away.in_room = false;
    assert!(!c.update(&t, &away, TriggerHud::default()));
    assert!(!c.update(&t, &away, TriggerHud::default()));
    assert!(c.update(&t, &away, TriggerHud::default()));
    assert_eq!(c.counter, -1);
}

#[test]
fn region_bump_triggers_read_the_tracker() {
    let t = EventTrigger::parse("RegionBump<tA>event<tB>StopMusicEvent").unwrap();
    let mut c = ActiveTriggerChecker::new(0);
    let p = TriggerPlayer::at(Vec2::ZERO);
    assert!(!c.update(&t, &p, TriggerHud::default()));
    let pre = TriggerHud {
        game_over_mode: false,
        region_bumps: Some((true, false)),
    };
    assert!(!c.update(&t, &p, pre));
    let bump = TriggerHud {
        game_over_mode: false,
        region_bumps: Some((false, true)),
    };
    assert!(c.update(&t, &p, bump));
    let see = EventTrigger::parse("SeeCreature<tA>event<tB>StopMusicEvent").unwrap();
    assert!(!c.update(&see, &p, bump));
}

/// SU_B04's 0.4565217 chance draws one `Random.value` per fire; a chance of 1 and a
/// trigger without an event draw nothing.
/// SU_A13's object: the line from handle 2 (468.6972, 297.9665) to handle 1
/// (210.6972, 203.9665). A main chunk falling across it fires; rising across it, or
/// falling straight down (outside `IsPointBetweenPoints`' strict box), does not.
#[test]
fn super_jump_trigger_line() {
    let quad = QuadData::parse("-271~-102~-265~-126~-6~-32").unwrap();
    let s = SuperJumpInstruction::new(Vec2::new(475.6972, 329.9665), quad);
    assert_eq!(s.trigger.instruction_pos, Vec2::new(475.6972, 329.9665));
    let mid = (s.trg_a() + s.trg_b()) / 2.0;
    let above = mid + Vec2::new(-1.0, 8.0);
    let below = mid + Vec2::new(1.0, -8.0);
    assert!(s.crossed(above, below));
    assert!(!s.crossed(below, above));
    assert!(!s.crossed(mid + Vec2::new(0.0, 8.0), mid - Vec2::new(0.0, 8.0)));
    assert!(!s.crossed(above + Vec2::new(0.0, 30.0), above + Vec2::new(2.0, 20.0)));
}

#[test]
fn fire_chance_draws_only_below_one() {
    let b04 = EventTrigger::parse(
        "Spot<tA>event<tB>MusicEvent<eA>songName<eB>RW_1 - Urban Jungle<tA>fireChance<tB>0.4565217",
    )
    .unwrap();
    let mut rng = UnityRandom::new(5);
    let mut copy = rng.clone();
    let first = copy.value();
    let passed = ActiveTriggerChecker::roll(&b04, &mut rng);
    assert_eq!(passed, Some(first < 0.456_521_7));
    assert_eq!(rng.state(), copy.state());
    let a07 = EventTrigger::parse(A07).unwrap();
    assert_eq!(ActiveTriggerChecker::roll(&a07, &mut rng), Some(true));
    let none = EventTrigger::parse("Spot<tA>event<tB>NONE<tA>fireChance<tB>0.5").unwrap();
    assert_eq!(ActiveTriggerChecker::roll(&none, &mut rng), None);
    assert_eq!(rng.state(), copy.state());
}
