//! One rock or spear in a small room for a few dozen ticks. Expected values come from
//! the constructors and `Update`s quoted in each test (decompile lines in `weapon.rs`).

use std::sync::Arc;

use rw_assets::{ClipNames, Room, SoundTable};
use rw_random::UnityRandom;

use super::{ModeChange, Throw, Weapon, WeaponKind, WeaponMode, WeaponTick};
use crate::creature::{CreatureId, EntityId, WorldCoordinate};
use crate::item::AbstractObject;
use crate::tests::room;
use crate::{CosmeticKind, FloatExt, SoundEvent, SoundId, Sounds, Spark, Vec2};

/// One item's room, stream and sound sink.
struct Bench {
    room: Arc<Room>,
    rng: UnityRandom,
    sounds: Sounds,
    table: Arc<SoundTable>,
    seq: u64,
    tick: u64,
    /// The sparks the updates added.
    cosmetics: Vec<(u64, CosmeticKind)>,
    /// The room calls (screen movements) the updates made.
    calls: Vec<crate::RoomCall>,
    stuck: Vec<(i32, i32)>,
    zones: Vec<(Vec2, f32)>,
    door_moving: bool,
}

/// The thrower in these tests.
const THROWER: CreatureId = CreatureId(0);

impl Bench {
    fn new(rows: &[&str], seed: i32) -> Self {
        Self {
            room: Arc::new(room(rows)),
            rng: UnityRandom::new(seed),
            sounds: Sounds::default(),
            table: Arc::new(SoundTable::default()),
            seq: 0,
            tick: 0,
            cosmetics: Vec::new(),
            calls: Vec::new(),
            stuck: Vec::new(),
            zones: Vec::new(),
            door_moving: false,
        }
    }

    /// `Thrown` by [`THROWER`] (standing still) from `pos` along `dir`, `frc` 1.
    fn throw(
        &mut self,
        w: &mut Weapon,
        o: &mut AbstractObject,
        pos: Vec2,
        dir: (i32, i32),
    ) -> SoundId {
        let room = Arc::clone(&self.room);
        let mut cx = ModeChange {
            room: &room,
            rng: &mut self.rng,
            update_seq: &mut self.seq,
        };
        let throw = Throw {
            by: THROWER,
            by_vel: Vec2::ZERO,
            pos,
            first_frame_trace_from_pos: None,
            dir,
            frc: 1.0,
            eu: w.even_update,
        };
        w.thrown(throw, o, &mut cx)
    }

    /// A sink and table where these sounds play (one clip each).
    fn with_sounds(mut self, ids: &[SoundId]) -> Self {
        let mut text = String::from("Volume: 6\nVolume exponent: 1.8\n");
        for id in ids {
            text.push_str(&format!("{} : clip\n", id.name()));
        }
        let names = ClipNames::new(["clip".to_owned()], None);
        let table = Arc::new(SoundTable::parse(&text, &names).expect("table"));
        self.sounds = Sounds::new(Arc::clone(&table));
        self.table = table;
        self
    }

    /// `Realize` then `PlaceInRoom` of `object` in room 0.
    fn place(&mut self, object: &mut AbstractObject) -> Weapon {
        let mut w = Weapon::realize(object, &mut self.rng).expect("a weapon");
        let room = Arc::clone(&self.room);
        let mut cx = ModeChange {
            room: &room,
            rng: &mut self.rng,
            update_seq: &mut self.seq,
        };
        w.place_in_room(object, 0, &mut cx);
        w
    }

    fn step(&mut self, w: &mut Weapon, object: &mut AbstractObject) {
        let mut cx = WeaponTick {
            room_index: 0,
            room: &mut self.room,
            water: None,
            ready_for_ai: true,
            rng: &mut self.rng,
            sounds: &mut self.sounds,
            triggers: &self.table,
            camera_room: 0,
            tick: self.tick,
            eu: self.tick.is_multiple_of(2),
            update_seq: &mut self.seq,
            player: None,
            stuck_spear_tiles: &self.stuck,
            no_stick_zones: &self.zones,
            shelter_door_moving: self.door_moving,
            cosmetics: &mut self.cosmetics,
            calls: &mut self.calls,
        };
        w.update(object, &mut cx);
        self.tick += 1;
    }

    /// Steps until the weapon leaves `Thrown` (at most 100 ticks); returns the ticks.
    fn fly(&mut self, w: &mut Weapon, o: &mut AbstractObject) -> usize {
        let mut ticks = 0;
        while w.mode == WeaponMode::Thrown {
            self.step(w, o);
            ticks += 1;
            assert!(ticks < 100, "still thrown");
        }
        ticks
    }

    fn played(&mut self) -> Vec<SoundId> {
        self.sounds
            .take_events()
            .into_iter()
            .filter_map(|e| match e {
                SoundEvent::Play(p) => Some(p.id),
                _ => None,
            })
            .collect()
    }
}

fn rock(tile: (i32, i32)) -> AbstractObject {
    AbstractObject::new_rock(
        EntityId::new(0, 1001),
        WorldCoordinate::new(0, tile.0, tile.1, -1),
    )
}

fn spear(tile: (i32, i32), cycles: i32) -> AbstractObject {
    let mut o = AbstractObject::new_spear(
        EntityId::new(0, 1002),
        WorldCoordinate::new(0, tile.0, tile.1, -1),
        false,
    );
    if let Some(s) = o.spear_mut() {
        s.stuck_in_wall_cycles = cycles;
    }
    o
}

/// 5 x 6: a floor at row 0, a wall on the right from row 1 up.
const BOX: [&str; 6] = [".....", "....#", "....#", "....#", "....#", "#####"];

#[test]
fn a_rock_costs_two_draws_and_a_spear_four() {
    // `Weapon()`: rotation = DegToVec(value * 360); `PlaceInRoom` -> `SetRandomSpin`:
    // a rock's Lerp(-100, 100, value) * Lerp(0.05, 1, gravity 1).
    let mut b = Bench::new(&BOX, 7);
    let mut expect = UnityRandom::new(7);
    let mut o = rock((1, 3));
    let w = b.place(&mut o);
    let rotation = Vec2::from_degrees(expect.value() * 360.0);
    let spin = (-100.0f32).lerp(100.0, expect.value());
    assert_eq!(b.rng.state(), expect.state());
    assert_eq!(w.rotation, rotation);
    assert_eq!(w.rotation_speed, spin);
    assert_eq!(w.chunk().pos, Vec2::new(30.0, 70.0), "MiddleOfTile");
    assert_eq!((w.mode, w.collision_layer), (WeaponMode::Free, 2));
    assert!(!w.chunk().go_through_floors, "no ChangeMode on placement");

    // `Spear()` adds `spearmasterNeedleType = Range(0, 3)`; its `SetRandomSpin` draws
    // the direction (value < 0.5) and Lerp(50, 150, value).
    let mut b = Bench::new(&BOX, 7);
    let mut expect = UnityRandom::new(7);
    let mut o = spear((1, 3), 0);
    let w = b.place(&mut o);
    let rotation = Vec2::from_degrees(expect.value() * 360.0);
    let needle = expect.range_i32(0, 3);
    let sign = if expect.value() < 0.5 { -1.0 } else { 1.0 };
    let spin = sign * 50.0f32.lerp(150.0, expect.value());
    assert_eq!(b.rng.state(), expect.state());
    assert_eq!(w.rotation, rotation);
    assert_eq!(
        w.spear().map(|s| (s.needle_type, s.spinning)),
        Some((needle, true))
    );
    assert_eq!(w.rotation_speed, spin);
}

#[test]
fn a_dropped_rock_lands_and_rests_without_drawing() {
    // From the middle of tile (1, 3), y 70: gravity 0.9 and air friction 0.999 bring it
    // to the floor (top at y 20) with its 5 px radius at y 25; bounce 0.4 is below the
    // impact threshold soon, so it rests there. A rock's update draws nothing (no blizzard,
    // no sound with an empty table).
    let mut b = Bench::new(&BOX, 3);
    let mut o = rock((1, 3));
    let mut w = b.place(&mut o);
    let after_place = b.rng.state();
    for _ in 0..60 {
        b.step(&mut w, &mut o);
    }
    assert_eq!(b.rng.state(), after_place);
    let c = w.chunk();
    assert!((c.pos.y - 25.0).abs() < 0.01, "{:?}", c.pos);
    assert_eq!(c.contact().y, -1);
    assert_eq!(o.pos.tile, (1, 1), "the abstract tile follows the chunk");
    // On the floor `rotationSpeed = (rotationSpeed * 2 + vel.x * 5) / 3`: with vel.x 0
    // it falls by a third a tick.
    assert!(w.rotation_speed.abs() < 1e-6);
}

#[test]
fn a_landing_spear_stops_spinning_with_one_draw_and_a_sound() {
    // Spinning, it lands (`ContactPoint.y < 0`): rotationSpeed 0, rotation
    // DegToVec(Lerp(-50, 50, value) + 180), vel 0, `Spear_Stick_In_Ground` on its chunk.
    let mut b = Bench::new(&BOX, 11).with_sounds(&[SoundId::SpearStickInGround]);
    let mut o = spear((1, 3), 0);
    let mut w = b.place(&mut o);
    let mut ticks = 0;
    while w.spear().is_some_and(|s| s.spinning) {
        let before = b.rng.clone();
        b.step(&mut w, &mut o);
        ticks += 1;
        assert!(ticks < 40, "never landed");
        if w.spear().is_some_and(|s| s.spinning) {
            assert_eq!(b.rng.state(), before.state(), "falling draws nothing");
            continue;
        }
        let mut expect = before;
        let angle = (-50.0f32).lerp(50.0, expect.value()) + 180.0;
        let mut replay = Sounds::new(Arc::clone(&b.table));
        replay.play_on_object(
            SoundId::SpearStickInGround,
            w.chunk().pos,
            false,
            1.0,
            1.0,
            &mut expect,
        );
        assert_eq!(b.rng.state(), expect.state());
        assert_eq!(w.rotation, Vec2::from_degrees(angle));
        assert_eq!(w.chunk().vel, Vec2::ZERO);
        assert_eq!(w.rotation_speed, 0.0);
    }
    let played: Vec<SoundId> = b
        .sounds
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            SoundEvent::Play(p) => Some(p.id),
            _ => None,
        })
        .collect();
    assert_eq!(played, [SoundId::SpearStickInGround]);
    // Lying still it draws nothing more.
    let state = b.rng.state();
    for _ in 0..20 {
        b.step(&mut w, &mut o);
    }
    assert_eq!(b.rng.state(), state);
}

#[test]
fn a_saved_stuck_spear_is_stuck_again_and_makes_poles() {
    // `PlaceInRoom`: stuckInWall = MiddleOfTile(3, 3) = (70, 70); `ChangeMode
    // (StuckInWall)` with 4 cycles (horizontal): the left side (50, 70) is open, so
    // setRotation = (1, 0) (into the wall) and stuckInWall moves 5 px out, to (65, 70).
    // No draw: the cycles are set. Layer 0, no terrain collision.
    let mut b = Bench::new(&BOX, 5);
    let mut o = spear((3, 3), 4);
    let mut w = b.place(&mut o);
    let mut expect = UnityRandom::new(5);
    for _ in 0..4 {
        expect.next_u32();
    }
    assert_eq!(b.rng.state(), expect.state());
    assert_eq!(w.mode, WeaponMode::StuckInWall);
    assert_eq!(w.collision_layer, 0);
    assert!(!w.chunk().collide_with_terrain && w.chunk().go_through_floors);
    assert_eq!(w.set_rotation, Some(Vec2::new(1.0, 0.0)));
    assert_eq!(
        w.spear().and_then(|s| s.stuck_in_wall),
        Some(Vec2::new(65.0, 70.0))
    );
    assert_eq!(w.rotation_speed, 0.0);
    assert!(!b.room.tile(3, 3).horizontal_pole());

    // The first update (`readyForAI`) writes the poles: the stuck tile and its open
    // left neighbour; the wall tile on the right stays as it was.
    b.step(&mut w, &mut o);
    assert!(b.room.tile(3, 3).horizontal_pole());
    assert!(b.room.tile(2, 3).horizontal_pole());
    assert!(!b.room.tile(4, 3).horizontal_pole());
    assert!(!b.room.tile(1, 3).horizontal_pole());
    let s = w.spear().expect("spear");
    assert!(!s.add_poles && s.has_horizontal_beam_state);
    assert_eq!(s.was_horizontal_beam, [false; 3]);
    assert_eq!(w.rotation, Vec2::new(1.0, 0.0));
    // It stays put, and draws nothing.
    let state = b.rng.state();
    for _ in 0..30 {
        b.step(&mut w, &mut o);
    }
    assert_eq!(w.chunk().pos, Vec2::new(65.0, 70.0));
    assert_eq!(b.rng.state(), state);
    assert_eq!(o.spear().map(|s| s.stuck_in_wall_cycles), Some(4));
}

#[test]
fn a_spear_stuck_in_the_floor_makes_a_vertical_pole() {
    // -3 cycles: vertical. At tile (1, 1) over the floor: below (1, 0) is Solid, above
    // (1, 2) is open, so setRotation = (0, -1) (pointing down) and stuckInWall = (30, 35).
    let mut b = Bench::new(&BOX, 5);
    let mut o = spear((1, 1), -3);
    let mut w = b.place(&mut o);
    assert_eq!(w.set_rotation, Some(Vec2::new(0.0, -1.0)));
    assert_eq!(
        w.spear().and_then(|s| s.stuck_in_wall),
        Some(Vec2::new(30.0, 35.0))
    );
    b.step(&mut w, &mut o);
    assert!(b.room.tile(1, 1).vertical_pole());
    assert!(b.room.tile(1, 2).vertical_pole());
    assert!(!b.room.tile(1, 0).vertical_pole());
    assert!(!b.room.tile(1, 1).horizontal_pole());
}

#[test]
fn poles_wait_for_ready_for_ai_and_reset_restores_the_tiles() {
    let mut b = Bench::new(&BOX, 5);
    let mut o = spear((3, 3), 4);
    let mut w = b.place(&mut o);
    let mut cx = WeaponTick {
        room_index: 0,
        room: &mut b.room,
        water: None,
        ready_for_ai: false,
        rng: &mut b.rng,
        sounds: &mut b.sounds,
        triggers: &b.table,
        camera_room: 0,
        tick: 0,
        eu: true,
        update_seq: &mut b.seq,
        player: None,
        stuck_spear_tiles: &[],
        no_stick_zones: &[],
        shelter_door_moving: false,
        cosmetics: &mut b.cosmetics,
        calls: &mut b.calls,
    };
    w.update(&mut o, &mut cx);
    assert!(!b.room.tile(3, 3).horizontal_pole());
    b.step(&mut w, &mut o);
    assert!(b.room.tile(2, 3).horizontal_pole());
    let WeaponKind::Spear(s) = &mut w.kind else {
        panic!("a spear")
    };
    let a = o.spear_mut().expect("abstract spear");
    s.reset_horizontal_beam_state(Arc::make_mut(&mut b.room), a);
    assert!(!b.room.tile(3, 3).horizontal_pole());
    assert!(!b.room.tile(2, 3).horizontal_pole());
    assert_eq!(a.stuck_in_wall_cycles, 0);
}

#[test]
fn a_fast_rock_on_the_ground_starts_the_skid_loop() {
    // Speed above 5 on the floor: `Rock_Skidding_On_Ground_LOOP` at volume
    // InverseLerp(5, 15, speed), started once (`Room.PlaySound(.., loop: true, v, p)`).
    // In the air the sound is `Rock_Through_Air_LOOP`, which has no trigger: nothing.
    let mut b = Bench::new(&BOX, 9).with_sounds(&[SoundId::RockSkiddingOnGroundLoop]);
    let mut o = rock((1, 1));
    let mut w = b.place(&mut o);
    for _ in 0..30 {
        b.step(&mut w, &mut o);
    }
    w.chunk_mut().vel = Vec2::new(12.0, 0.0);
    b.step(&mut w, &mut o);
    let speed = w.chunk().vel.length();
    assert_eq!(w.chunk().contact().y, -1);
    assert!(speed > 5.0);
    assert_eq!(w.sound_loop.sound, Some(SoundId::RockSkiddingOnGroundLoop));
    assert_eq!(w.sound_loop.volume(), speed.inverse_lerp(5.0, 15.0));
    let e = w.sound_loop.emitter.expect("an emitter");
    assert!(e.alive && e.playing && e.update_order > w.update_order);
    let plays = b
        .sounds
        .take_events()
        .iter()
        .filter(|e| matches!(e, SoundEvent::Play(p) if p.looped))
        .count();
    assert_eq!(plays, 1);
    b.step(&mut w, &mut o);
    assert_eq!(
        w.sound_loop.emitter.map(|e| e.sound),
        Some(e.sound),
        "kept, not restarted"
    );
}

/// 40 x 6: a floor, open above.
const FLAT: [&str; 6] = [
    "........................................",
    "........................................",
    "........................................",
    "........................................",
    "........................................",
    "########################################",
];

/// 10 x 6: a floor and a wall from x 7 on (its face at x 140).
const WALL: [&str; 6] = [
    "..........",
    ".......###",
    ".......###",
    ".......###",
    ".......###",
    "##########",
];

/// The wall at x 1: a spear in front of it is in the room's outer ring.
const EDGE: [&str; 6] = ["......", ".#....", ".#....", ".#....", ".#....", "######"];

/// The same wall one tile in.
const INNER: [&str; 6] = ["......", "..#...", "..#...", "..#...", "..#...", "######"];

#[test]
fn a_thrown_rock_flies_at_forty_and_tumbles_free_under_thirty() {
    // `Weapon.Thrown` sideways: vel = thrower vel * (0.2, 0.5) + (40 dir frc, 3 for a
    // rock); `ChangeMode(Thrown)`: layer 0, `setRotation` = throwDir, no spin, in
    // `Items`. No draws.
    let mut b = Bench::new(&FLAT, 21);
    let mut o = rock((2, 2));
    let mut w = b.place(&mut o);
    let state = b.rng.state();
    let room = Arc::clone(&b.room);
    let mut cx = ModeChange {
        room: &room,
        rng: &mut b.rng,
        update_seq: &mut b.seq,
    };
    let throw = Throw {
        by: THROWER,
        by_vel: Vec2::new(2.0, -4.0),
        pos: Vec2::new(50.0, 50.0),
        first_frame_trace_from_pos: Some(Vec2::new(40.0, 46.0)),
        dir: (1, 0),
        frc: 1.0,
        eu: w.even_update,
    };
    assert_eq!(w.thrown(throw, &mut o, &mut cx), SoundId::SlugcatThrowRock);
    assert_eq!(b.rng.state(), state);
    assert_eq!(w.chunk().vel, Vec2::new(0.4 + 40.0, -2.0 + 3.0));
    assert_eq!((w.mode, w.collision_layer), (WeaponMode::Thrown, 0));
    assert_eq!(w.set_rotation, Some(Vec2::new(1.0, 0.0)));
    assert_eq!((w.rotation_speed, w.in_front_of_objects), (0.0, 1));
    assert_eq!(
        (w.change_dir_counter, w.override_exit_thrown_speed),
        (3, 0.0)
    );
    assert_eq!(w.thrown_by, Some(THROWER));
    // Each thrown tick: out of the player's reach for 40 ticks, no draws, water
    // retardation immunity 0.9. It lands, the floor's friction slows it, and the first
    // tick that starts under 30 px a tick spins it twice (`SetRandomSpin`, then
    // `ChangeMode(Free)`'s): two draws. The trace branch still runs and sets the 40.
    let mut ticks = 0;
    loop {
        let before = b.rng.clone();
        let speed = w.chunk().vel.length();
        b.step(&mut w, &mut o);
        ticks += 1;
        assert!(ticks < 100);
        if w.mode == WeaponMode::Thrown {
            assert!(speed >= 30.0);
            assert_eq!(b.rng.state(), before.state());
            // `PlayerCarryableItem.Update` counts it down in the same update.
            assert_eq!(w.forbidden_to_player, 39);
            assert_eq!(w.body.params.water_retardation_immunity, 0.9);
            continue;
        }
        assert!(speed < 30.0, "{speed}");
        let mut expect = before;
        expect.value();
        expect.value();
        assert_eq!(b.rng.state(), expect.state());
        assert_eq!(w.forbidden_to_player, 39);
        assert_eq!(w.collision_layer, 2);
        break;
    }
    assert!(ticks > 3);
    assert_eq!(w.chunk().contact().y, -1, "skidding on the floor");
}

#[test]
fn a_rock_thrown_at_a_wall_hits_it_with_seven_sparks() {
    // The chunk touches the wall (`ContactPoint == throwDir`) and bounces off at 16 px
    // a tick. The next tick it first tumbles under the exit speed (`SetRandomSpin`,
    // `ChangeMode(Free)`: two spins), then `HitWall`s: in a viewed room seven sparks
    // (DegToVec(value * 360) * 10 value each, then the spark's own draws),
    // the camera's bump (no draws), `Rock_Hit_Wall`, `SetRandomSpin` (`ChangeMode(Free)` is already done), 10 ticks
    // out of reach, counted down once in the same update.
    let mut b = Bench::new(&WALL, 4).with_sounds(&[SoundId::RockHitWall]);
    let mut o = rock((3, 2));
    let mut w = b.place(&mut o);
    b.throw(&mut w, &mut o, Vec2::new(70.0, 50.0), (1, 0));
    while w.chunk().contact().x != 1 {
        b.step(&mut w, &mut o);
        assert!(b.tick < 10);
    }
    let mut expect = b.rng.clone();
    b.calls.clear();
    b.step(&mut w, &mut o);
    // `room.ScreenMovement(firstChunk.pos, throwDir * 1.5, 0)`, after the sparks.
    let [crate::RoomCall::ScreenMovement(m)] = b.calls[..] else {
        panic!("one screen movement: {:?}", b.calls);
    };
    assert_eq!((m.bump, m.shake), (Vec2::new(1.5, 0.0), 0.0));
    assert!(m.source.is_some_and(|p| p.x > 60.0));
    expect.value();
    expect.value();
    for _ in 0..7 {
        expect.value();
        expect.value();
        Spark::new(Vec2::ZERO, Vec2::ZERO, [1.0; 4], 2, 4, &mut expect);
    }
    let mut replay = Sounds::new(Arc::clone(&b.table));
    replay.play_on_object(
        SoundId::RockHitWall,
        Vec2::ZERO,
        false,
        1.0,
        1.0,
        &mut expect,
    );
    expect.value();
    assert_eq!(b.rng.state(), expect.state());
    assert_eq!(b.cosmetics.len(), 7);
    assert_eq!(b.played(), [SoundId::RockHitWall]);
    assert_eq!((w.mode, w.forbidden_to_player), (WeaponMode::Free, 9));
}

#[test]
fn a_spear_thrown_close_to_a_wall_sticks_and_makes_poles() {
    // 65 px from the wall, within 140: `Spear.Update` draws `value < 0.33` but sticks
    // anyway. stuckInWall = MiddleOfTile(6, 2) = (130, 50); `ChangeMode(StuckInWall)`
    // draws the cycles, Range(3, 7) (positive: thrown sideways), sees the open left side
    // and turns the spear into the wall, 5 px out: (125, 50); `Spear_Stick_In_Wall`.
    let mut b = Bench::new(&WALL, 8).with_sounds(&[SoundId::SpearStickInWall]);
    let mut o = spear((3, 2), 0);
    let mut w = b.place(&mut o);
    b.throw(&mut w, &mut o, Vec2::new(70.0, 50.0), (1, 0));
    let mut before = b.rng.clone();
    while w.mode == WeaponMode::Thrown {
        before = b.rng.clone();
        b.step(&mut w, &mut o);
        assert!(b.tick < 10);
    }
    let mut expect = before;
    expect.value();
    let cycles = expect.range_i32(3, 7);
    let mut replay = Sounds::new(Arc::clone(&b.table));
    replay.play_on_object(
        SoundId::SpearStickInWall,
        Vec2::ZERO,
        false,
        1.0,
        1.0,
        &mut expect,
    );
    assert_eq!(b.rng.state(), expect.state());
    assert_eq!(w.mode, WeaponMode::StuckInWall);
    assert_eq!(o.spear().map(|s| s.stuck_in_wall_cycles), Some(cycles));
    assert_eq!(
        w.spear().and_then(|s| s.stuck_in_wall),
        Some(Vec2::new(125.0, 50.0))
    );
    assert_eq!(w.set_rotation, Some(Vec2::new(1.0, 0.0)));
    assert_eq!((w.vibrate, w.collision_layer), (10, 0));
    assert!(!w.chunk().collide_with_terrain);
    assert_eq!(b.played(), [SoundId::SpearStickInWall]);
    // The next update writes the poles: the stuck tile and its open neighbour.
    b.step(&mut w, &mut o);
    assert!(b.room.tile(6, 2).horizontal_pole());
    assert!(b.room.tile(5, 2).horizontal_pole());
    assert!(!b.room.tile(7, 2).horizontal_pole());
    assert_eq!(w.chunk().pos, Vec2::new(125.0, 50.0));
}

/// Throws a spear (explosive or not) from the middle of `tile` right into the wall
/// next to it, after `setup`; returns the spear and its object after the throw ends.
fn throw_at_wall(
    rows: &[&str],
    tile: (i32, i32),
    explosive: bool,
    setup: fn(&mut Bench),
) -> (Weapon, AbstractObject) {
    let mut b = Bench::new(rows, 2);
    setup(&mut b);
    let mut o = spear(tile, 0);
    if let Some(s) = o.spear_mut() {
        s.explosive = explosive;
    }
    let mut w = b.place(&mut o);
    let at = Vec2::middle_of_tile(tile);
    b.throw(&mut w, &mut o, at, (1, 0));
    b.fly(&mut w, &mut o);
    (w, o)
}

fn no_setup(_: &mut Bench) {}

#[test]
fn spears_never_stick_in_the_rooms_outer_tile_ring() {
    // The abstract tile (0, 2) is on the ring (`x <= 0`): no stick (the game logs
    // "Spear prevented from stabbing the edge of a room"); the next tick it `HitWall`s.
    let (w, _) = throw_at_wall(&EDGE, (0, 2), false, no_setup);
    assert_eq!(w.mode, WeaponMode::Free);
    // One tile in, the same throw sticks.
    let (w, _) = throw_at_wall(&INNER, (1, 2), false, no_setup);
    assert_eq!(w.mode, WeaponMode::StuckInWall);
    // An explosive spear is exempt from the ring rule; it keeps no cycles or poles.
    let (w, o) = throw_at_wall(&EDGE, (0, 2), true, no_setup);
    assert_eq!(w.mode, WeaponMode::StuckInWall);
    assert_eq!(o.spear().map(|s| s.stuck_in_wall_cycles), Some(0));
    assert!(!w.spear().is_some_and(|s| s.add_poles));
}

#[test]
fn zones_stuck_spears_and_moving_shelter_doors_keep_a_spear_out() {
    // `NoSpearStickZone`: DistLess(MiddleOfTile(pos), zone, rad).
    let (w, _) = throw_at_wall(&INNER, (1, 2), false, |b| {
        b.zones.push((Vec2::new(30.0, 50.0), 1.0));
    });
    assert_eq!(w.mode, WeaponMode::Free);
    let (w, _) = throw_at_wall(&INNER, (1, 2), false, |b| {
        b.zones.push((Vec2::new(300.0, 50.0), 100.0));
    });
    assert_eq!(w.mode, WeaponMode::StuckInWall, "a zone out of reach");
    // Another spear stuck on that abstract tile.
    let (w, _) = throw_at_wall(&INNER, (1, 2), false, |b| b.stuck.push((1, 2)));
    assert_eq!(w.mode, WeaponMode::Free);
    // A shelter door closing or opening.
    let (w, _) = throw_at_wall(&INNER, (1, 2), false, |b| b.door_moving = true);
    assert_eq!(w.mode, WeaponMode::Free);
}

#[test]
fn a_far_spear_sticks_a_third_of_the_time_and_a_belly_slide_one_always() {
    // 40 x 8 with a wall from x 25: 450 px away, beyond 140, within 560.
    let open = ".".repeat(40);
    let wall = format!("{}{}", ".".repeat(25), "#".repeat(15));
    let floor = "#".repeat(40);
    let mut rows = vec![open];
    rows.extend(std::iter::repeat_n(wall, 6));
    rows.push(floor);
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut sticks = 0;
    for seed in 0..12 {
        let mut b = Bench::new(&rows, seed);
        let mut o = spear((2, 4), 0);
        let mut w = b.place(&mut o);
        b.throw(&mut w, &mut o, Vec2::new(50.0, 90.0), (1, 0));
        let mut draws = b.rng.clone();
        while w.mode == WeaponMode::Thrown {
            draws = b.rng.clone();
            b.step(&mut w, &mut o);
        }
        let first = draws.value();
        if w.mode == WeaponMode::StuckInWall {
            sticks += 1;
            assert!(first < 0.33);
        }
    }
    assert!(sticks > 0 && sticks < 12, "{sticks}");
    // `alwaysStickInWalls` (set by the belly-slide throw after `Thrown`) always sticks.
    for seed in 0..12 {
        let mut b = Bench::new(&rows, seed);
        let mut o = spear((2, 4), 0);
        let mut w = b.place(&mut o);
        b.throw(&mut w, &mut o, Vec2::new(50.0, 90.0), (1, 0));
        w.set_always_stick_in_walls();
        b.fly(&mut w, &mut o);
        assert_eq!(w.mode, WeaponMode::StuckInWall, "seed {seed}");
    }
}

#[test]
fn circles_collision_time_is_the_first_touch() {
    // A circle of radius 5 moving 40 px right from (0, 0) toward one of radius 5 at
    // (30, 0) touches it after 20 px: t = 0.5.
    let t = Vec2::ZERO.circles_collision_time(Vec2::new(30.0, 0.0), Vec2::new(40.0, 0.0), 5.0, 5.0);
    assert!((t - 0.5).abs() < 1e-3, "{t}");
    let miss =
        Vec2::ZERO.circles_collision_time(Vec2::new(30.0, 30.0), Vec2::new(40.0, 0.0), 5.0, 5.0);
    assert!(miss.is_nan());
}
