//! Swimming in the installed arena level `WaterReactor` (read-only): water row 5
//! (surface 110 px), water in front of the terrain, no wave keys in its settings (root
//! defaults). Set `RW_DIR` to the game install directory; skipped if absent.

use rw_sim::Sounds;
use std::path::PathBuf;

use rw_assets::{Room, RoomSettings};
use rw_random::UnityRandom;
use rw_sim::{Animation, BodyMode, Input, Player, Water};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

fn waterreactor() -> Option<Room> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let sa = root.join("RainWorld_Data").join("StreamingAssets");
    if !sa.is_dir() {
        eprintln!("skipping: no {}", sa.display());
        return None;
    }
    Some(Room::load_level(&sa, "WaterReactor").expect("waterreactor loads"))
}

/// Dropped from tile (16, 10) into the left pool (surface 110 px, floor 60 px), left
/// alone for 200 ticks, then holding down until it drowns. Checks the mode changes and
/// breath timing of `water.md` sections 3-5 against the installed room geometry.
#[test]
fn waterreactor_fall_float_dive_and_drown() {
    let Some(room) = waterreactor() else { return };
    assert_eq!(room.water_level(), Some(5));
    let mut water = Water::new(&room, &RoomSettings::ANCESTOR).expect("water");
    let mut rng = UnityRandom::new(1);
    let mut p = Player::create(&mut rng);
    p.place((16, 10), &mut rng);
    let down = Input::new(0, -1, false);
    let mut first_out_of_air = None;
    let mut deepest_low_air = f32::MAX;
    let mut resurfaced = false;
    let mut death = None;
    for t in 0..2000 {
        let input = if t < 200 { Input::NONE } else { down };
        water.update(
            &room,
            p.body.chunks.iter_mut(),
            &mut rng,
            &mut Sounds::default(),
        );
        p.update(&room, Some(&water), input, &mut rng, &mut Sounds::default());
        if (100..200).contains(&t) {
            // Floating: surface swimming, the head held up around 15 px over 110.
            assert_eq!(p.animation(), Animation::SurfaceSwim, "tick {t}");
            let y = p.head().pos.y;
            assert!((105.0..130.0).contains(&y), "tick {t}: head {y}");
            assert_eq!(p.air_in_lungs(), 1.0);
        }
        if t == 220 {
            // Down dives (10 px is deep enough while diving).
            assert_eq!(p.body_mode(), BodyMode::Swimming);
            assert_eq!(p.animation(), Animation::DeepSwim);
            assert!(p.air_in_lungs() < 1.0);
        }
        if t > 220 && p.air_in_lungs() < 0.5 {
            deepest_low_air = deepest_low_air.min(p.head().pos.y);
            resurfaced |= p.animation() == Animation::SurfaceSwim;
        }
        if p.air_in_lungs() == 0.0 && first_out_of_air.is_none() {
            first_out_of_air = Some(t);
        }
        if p.is_dead() {
            death = Some(t);
            break;
        }
    }
    // Held down, the body dives to the pool's floor. Short of air, the swim direction
    // bends toward up (`Vector3.Slerp` of straight down toward straight up turns about
    // the x axis, so the push shrinks and then points up): the drowning body rises to
    // the surface, gasps, and drowns there.
    let (zero, dead) = (
        first_out_of_air.expect("ran out of air"),
        death.expect("drowned"),
    );
    assert!(deepest_low_air < 75.0, "dived: {deepest_low_air}");
    assert!(resurfaced, "the bend brought it back up");
    assert!(zero < dead, "{zero} {dead}");
}
