//! Physics on the installed `Summit` test room (read-only). Set `RW_DIR` to the game
//! install directory (the one containing `RainWorld_Data`); skipped if absent.
//! Expected slope classes come from `rainworld-re/notes/slope-collision.md` section 8.

use rw_sim::Sounds;
use std::path::PathBuf;

use rw_assets::{Room, Slope, Terrain, tile_position};
use rw_random::UnityRandom;
use rw_sim::{
    Animation, BodyMode, HEAD, HIPS, Input, PhysicalObject, Player, Vec2,
    determine_safe_spawn_tile, place_in_room,
};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

fn summit() -> Option<Room> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let sa = root.join("RainWorld_Data").join("StreamingAssets");
    if !sa.join("levels").is_dir() {
        eprintln!("skipping: no levels dir under {}", sa.display());
        return None;
    }
    // Mapped, as a loaded room is from `ShortCutsReady` on.
    let room = Room::load_level(&sa, "Summit").expect("summit loads");
    room.map_shortcuts();
    Some(room)
}

#[test]
fn summit_slopes_identify_by_neighbour_rule() {
    let Some(room) = summit() else { return };
    let mut slopes: Vec<((i32, i32), Option<Slope>)> = Vec::new();
    for x in 0..room.width() {
        for y in 0..room.height() {
            if room.tile(x, y).terrain == Terrain::Slope {
                slopes.push(((x, y), room.identify_slope(x, y)));
            }
        }
    }
    // Hand-applied rule (slope-collision.md section 1) to the ASCII dump of summit.txt
    // (`rw_room.py --ascii`): (10,13) has right (11,13) and above (10,14) Solid, below
    // Air => DownLeft; (11,12) has right (12,12) and above (11,13) Solid, below (11,11)
    // Air => DownLeft. Both are the diagonal underside of the left block; the right
    // block mirrors them (x -> 47 - x) with the wall on the left => DownRight.
    assert_eq!(
        slopes,
        vec![
            ((10, 13), Some(Slope::DownLeft)),
            ((11, 12), Some(Slope::DownLeft)),
            ((36, 12), Some(Slope::DownRight)),
            ((37, 13), Some(Slope::DownRight)),
        ]
    );
}

#[test]
fn summit_spawned_slugcat_comes_to_rest_on_block_top() {
    let Some(room) = summit() else { return };
    // These `UnityRandom` seeds spawn on the open top of a block ((36,17) for 6 and 16,
    // (11,17) for 12; row 16 Solid). Most other small seeds pick a shortcut entrance.
    // Surface friction is the player's not-conscious 0.3: with the conscious 0.5 a
    // flat landing multiplies vel.x by clamp(2 * 0.5, 0, 1) = 1, so a bare body
    // without the Player's ground friction slides off the block. The conscious case is
    // `summit_spawned_player_rests_on_block_top_then_stands`.
    for seed in [6, 12, 16] {
        let mut rng = UnityRandom::new(seed);
        let tile = determine_safe_spawn_tile(&room, &mut rng);
        // The spawn predicate: inside, y > 0, not Solid, three Solid tiles below.
        assert!(room.contains(tile.0, tile.1) && tile.1 > 0);
        assert!(!room.tile(tile.0, tile.1).is_solid());
        assert!((-1..=1).all(|dx| room.tile(tile.0 + dx, tile.1 - 1).is_solid()));
        assert_eq!(
            room.tile(tile.0, tile.1).terrain,
            Terrain::Air,
            "seed {seed}"
        );
        assert_eq!(tile.1, 17, "seed {seed}");

        let mut body = PhysicalObject::slugcat();
        body.params.surface_friction = 0.3;
        place_in_room(&mut body, tile, &mut rng);
        for _ in 0..400 {
            body.update(&room);
        }
        for (i, c) in body.chunks.iter().enumerate() {
            let ctx = format!("seed {seed} spawn {tile:?} chunk {i}: {c:?}");
            assert_eq!(c.contact().y, -1, "{ctx}");
            assert_eq!(c.on_slope(), 0, "{ctx}");
            // Flat-floor snap on row 16: (16 + 1) * 20 + rad.
            assert_eq!(c.pos.y, 340.0 + c.rad, "{ctx}");
            // Supported: the vertical sweep covers columns under x +/- (rad - 1).
            let (x0, ty) = tile_position([c.pos.x - c.rad + 1.0, c.pos.y - c.rad - 1.0]);
            let (x1, _) = tile_position([c.pos.x + c.rad - 1.0, 0.0]);
            assert_eq!(ty, 16, "{ctx}");
            assert!((x0..=x1).any(|x| room.tile(x, ty).is_solid()), "{ctx}");
            assert!(c.vel.length() < 1e-3, "{ctx}");
        }
        let d = body.chunks[0].pos.distance(body.chunks[1].pos);
        assert!((d - 17.0).abs() < 1e-3, "seed {seed}: rod length {d}");
    }
}

// ---- Player ------------------------------------------------------------------------
// Expectations from `rainworld-re/notes/slugcat-movement.md` and `horizontal-beams.md`.

const UP: Input = Input::new(0, 1, false);
const DOWN: Input = Input::new(0, -1, false);
const RIGHT: Input = Input::new(1, 0, false);

/// A player with the hips centre at `hips` and the head 17 px above, at rest.
fn upright_player(hips: Vec2) -> Player {
    let mut p = Player::new();
    p.body.chunks[HIPS].reset_at(hips);
    p.body.chunks[HEAD].reset_at(hips + Vec2::new(0.0, 17.0));
    p
}

fn run(p: &mut Player, room: &Room, input: Input, ticks: usize) {
    for _ in 0..ticks {
        p.update(
            room,
            None,
            input,
            &mut UnityRandom::new(0),
            &mut Sounds::default(),
        );
    }
}

#[test]
fn summit_spawned_player_rests_on_block_top_then_stands() {
    let Some(room) = summit() else { return };
    for seed in [6, 12, 16] {
        // Spawn tile, facing and chunk offsets from one stream, in the game's order.
        let mut rng = UnityRandom::new(seed);
        let tile = determine_safe_spawn_tile(&room, &mut rng);
        let mut p = Player::create(&mut rng);
        p.place(tile, &mut rng);
        // Conscious friction 0.5: the Player ground friction stops the body.
        run(&mut p, &room, Input::NONE, 400);
        // The constructor leaves `standing` false, so with no input the body lies
        // down on all fours (Stand mode lowers it after 5 ticks).
        assert_eq!(p.body_mode(), BodyMode::Crawl, "seed {seed}");
        for (i, c) in p.chunks().iter().enumerate() {
            let ctx = format!("seed {seed} chunk {i}: {c:?}");
            assert_eq!(c.contact().y, -1, "{ctx}");
            // Flat-floor snap on row 16: (16 + 1) * 20 + rad.
            assert_eq!(c.pos.y, 340.0 + c.rad, "{ctx}");
            assert!(c.vel.length() < 1e-3, "{ctx}");
        }
        let d = p.chunks()[0].pos.distance(p.chunks()[1].pos);
        assert!((d - 17.0).abs() < 1e-3, "seed {seed}: rod length {d}");

        // An up press sets `standing`; Stand mode holds the head up (+1.5 / -4.5).
        p.update(
            &room,
            None,
            UP,
            &mut UnityRandom::new(0),
            &mut Sounds::default(),
        );
        run(&mut p, &room, Input::NONE, 100);
        assert_eq!(p.body_mode(), BodyMode::Stand, "seed {seed}");
        let (head, hips) = (p.head(), p.hips());
        assert_eq!(hips.contact().y, -1, "seed {seed}");
        assert!((hips.pos.y - 348.0).abs() < 1.0, "seed {seed}: {hips:?}");
        assert!(
            (head.pos.x - hips.pos.x).abs() < 1.0,
            "seed {seed}: upright"
        );
        assert!(head.pos.y - hips.pos.y > 16.0, "seed {seed}: upright");
        let before = p.chunks().to_vec();
        run(&mut p, &room, Input::NONE, 20);
        for (a, b) in before.iter().zip(p.chunks()) {
            assert!(
                a.pos.distance(b.pos) < 0.05,
                "seed {seed}: at rest {a:?} -> {b:?}"
            );
        }
    }
}

#[test]
fn summit_running_then_releasing_stops() {
    let Some(room) = summit() else { return };
    // Left pit: floor row 6 (top 140) for x = 15..20, middle block from x = 21 (420).
    let mut p = upright_player(Vec2::new(370.0, 148.0));
    p.update(
        &room,
        None,
        UP,
        &mut UnityRandom::new(0),
        &mut Sounds::default(),
    );
    run(&mut p, &room, Input::NONE, 40);
    assert_eq!(p.body_mode(), BodyMode::Stand);
    let start = p.hips().pos.x;
    run(&mut p, &room, RIGHT, 6);
    // Accelerating at 1.2 per tick toward the 4.2 / 4.0 caps.
    assert!(p.head().vel.x > 3.5, "{:?}", p.head());
    run(&mut p, &room, Input::NONE, 60);
    for c in p.chunks() {
        assert!(c.vel.x.abs() < 1e-3, "{c:?}");
    }
    let x = p.hips().pos.x;
    // Six ticks: 1.2 + 2.4 + 3.6 + 3 * 4 = 19.2 px at most before release, then
    // ground friction (x 0.646 per tick) and the feet pin stop it within ~12 px.
    assert!(
        x > start + 10.0 && x < start + 19.2 + 12.0,
        "start {start}, end {x}"
    );
    assert!(x < 420.0 - 8.0, "stopped before the block");
}

#[test]
fn summit_horizontal_beam_grab_shuffle_and_pull_up() {
    let Some(room) = summit() else { return };
    // Beam tiles (15..=17, 15) stick out right of the left block (Solid x <= 14);
    // the beam line is the tile middle, y = 310.
    for x in 15..=17 {
        assert!(room.tile(x, 15).horizontal_pole(), "beam at ({x}, 15)");
    }
    assert!(!room.tile(18, 15).horizontal_pole());

    // Falling with up held: grabbed when the head's sweep passes row 15.
    let mut p = upright_player(Vec2::new(312.0, 400.0));
    let mut grabbed = false;
    for _ in 0..40 {
        p.update(
            &room,
            None,
            UP,
            &mut UnityRandom::new(0),
            &mut Sounds::default(),
        );
        if p.animation() == Animation::HangFromBeam {
            grabbed = true;
            break;
        }
    }
    assert!(grabbed, "{:?}", p.chunks());
    assert_eq!(p.head().pos.y, 310.0);
    assert_eq!(p.head().vel.y, 0.0);
    run(&mut p, &room, Input::NONE, 30);
    assert_eq!(p.animation(), Animation::HangFromBeam);
    assert_eq!(p.body_mode(), BodyMode::ClimbingOnBeam);
    assert_eq!(p.head().pos.y, 310.0, "head pinned to the beam line");
    assert!(p.hips().pos.y < 300.0, "hips hang below");

    // Shuffle right: the head speeds up toward the steady shuffle (about 1.19 px/tick,
    // derived and checked on a long beam in the unit tests); this beam is too short to
    // reach it, so only the range is checked here.
    run(&mut p, &room, RIGHT, 10);
    let x0 = p.head().pos.x;
    run(&mut p, &room, RIGHT, 20);
    assert_eq!(p.animation(), Animation::HangFromBeam);
    let speed = (p.head().pos.x - x0) / 20.0;
    assert!((1.0..1.25).contains(&speed), "shuffle speed {speed}");
    assert_eq!(p.head().pos.y, 310.0);
    // At the beam's end (tile 18) the hands let go.
    let mut fell = false;
    for _ in 0..40 {
        p.update(
            &room,
            None,
            RIGHT,
            &mut UnityRandom::new(0),
            &mut Sounds::default(),
        );
        if p.animation() == Animation::None {
            fell = true;
            break;
        }
    }
    assert!(fell);
    assert_eq!(p.head().pos.x.div_euclid(20.0), 18.0);

    // Pull up: re-grab at x = 330, then an up press. Facing right with a beam tile to
    // the right: the hips swing up on that side (+2 vy, +0.5 vx per tick) until above
    // the head, then StandOnBeam with the hips 5 px above the beam line.
    let mut p = upright_player(Vec2::new(330.0, 400.0));
    run(&mut p, &room, UP, 40);
    assert_eq!(p.animation(), Animation::HangFromBeam);
    run(&mut p, &room, Input::NONE, 20);
    p.update(
        &room,
        None,
        UP,
        &mut UnityRandom::new(0),
        &mut Sounds::default(),
    );
    assert_eq!(p.animation(), Animation::GetUpOnBeam);
    let mut stood = false;
    for _ in 0..60 {
        p.update(
            &room,
            None,
            Input::NONE,
            &mut UnityRandom::new(0),
            &mut Sounds::default(),
        );
        if p.animation() == Animation::StandOnBeam {
            stood = true;
            break;
        }
    }
    assert!(stood, "{:?} {:?}", p.animation(), p.chunks());
    assert_eq!(p.hips().pos.y, 315.0);
    run(&mut p, &room, Input::NONE, 10);
    assert_eq!(p.animation(), Animation::StandOnBeam);
    assert_eq!(p.hips().pos.y, 315.0, "feet held on the beam");
    assert!(p.head().pos.y > p.hips().pos.y + 10.0);

    // Down press steps off: falls to the pit floor (top 140).
    p.update(
        &room,
        None,
        DOWN,
        &mut UnityRandom::new(0),
        &mut Sounds::default(),
    );
    assert_eq!(p.animation(), Animation::None);
    run(&mut p, &room, Input::NONE, 80);
    assert_eq!(p.hips().contact().y, -1);
    assert!((p.hips().pos.y - 148.0).abs() < 1.0, "{:?}", p.hips());
}

#[test]
fn summit_open_bottom_kills_below_minus_500() {
    let Some(room) = summit() else { return };
    // Columns 0..=11 are Air down to row 0 and the room has no water (falls.md
    // section 5): the limit is max(-1200 + 1, -500), checked before physics.
    assert_eq!(room.water_level(), None);
    assert!((0..room.height()).all(|y| !room.tile(5, y).is_solid()));
    assert!((0..room.height()).all(|y| !room.tile(4, y).is_solid()));
    let mut p = Player::new();
    p.body.chunks[HIPS].reset_at(Vec2::new(100.0, 200.0));
    p.body.chunks[HEAD].reset_at(Vec2::new(117.0, 200.0));
    let mut died = None;
    for tick in 1..200 {
        let start_y = p.head().pos.y;
        p.update(
            &room,
            None,
            Input::NONE,
            &mut UnityRandom::new(0),
            &mut Sounds::default(),
        );
        assert!(!p.is_stunned(), "nothing to hit");
        assert_eq!(
            p.is_dead(),
            start_y < -500.0,
            "tick {tick}: head at {start_y}"
        );
        if p.is_dead() {
            died = Some(tick);
            break;
        }
    }
    assert!(died.is_some(), "fell out: {:?}", p.head());
}

#[test]
fn summit_ledge_grab_on_the_left_block() {
    let Some(room) = summit() else { return };
    // The left block's outer face: column 10 is Solid on rows 14..=16 with open (10, 17)
    // above, (8, 16) open and nothing under column 9 down to the bottom.
    // ledge-grab.md section 1: head pressed right into the face above the middle of
    // row 16 (y 330), (8, 16), (9, 14) and (10, 17) open: LedgeGrab.
    assert!(room.tile(10, 16).is_solid() && !room.tile(10, 17).is_solid());
    assert!(!room.tile(9, 14).is_solid() && !room.tile(8, 16).is_solid());
    let mut p = upright_player(Vec2::new(189.0, 319.0));
    for c in &mut p.body.chunks {
        c.vel.x = 3.0;
    }
    p.update(
        &room,
        None,
        RIGHT,
        &mut UnityRandom::new(0),
        &mut Sounds::default(),
    );
    assert_eq!(p.head().contact().x, 1);
    assert_eq!(p.animation(), Animation::LedgeGrab);
    // Holding toward the wall hauls the body over the edge onto row 17 (top 340), a
    // two-tile step in front of the next wall (column 12).
    let mut ticks = 0;
    while p.body_mode() != BodyMode::Stand {
        p.update(
            &room,
            None,
            RIGHT,
            &mut UnityRandom::new(0),
            &mut Sounds::default(),
        );
        ticks += 1;
        assert!(ticks < 40, "{:?}", p.chunks());
    }
    assert_eq!(p.hips().contact().y, -1);
    assert!((p.hips().pos.y - 348.0).abs() < 1.0, "{:?}", p.hips());
    assert!(p.hips().pos.x > 200.0);
}
