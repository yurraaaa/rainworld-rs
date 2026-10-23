//! `StarMatrix`: the star count, draw counts and orders of the constructor and `Update`, the
//! z rotation that never moves, the projection. Expected values follow the decompiled
//! `StarMatrix.cs`.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;

use super::StarMatrix;
use crate::{Vec2, Vec3};

/// `(int)(Pow(Max(0, Lerp(rad, 500, 0.3)), 2) * pi / 4000)` with `rad = Rad + 50`.
#[test]
fn the_star_count_follows_the_radius() {
    let mut rng = UnityRandom::new(1);
    // rad 150 -> Lerp(150, 500, 0.3) = 255 -> 255^2 * pi / 4000 = 51.07.
    let m = StarMatrix::new(3, Vec2::new(10.0, 20.0), 100.0, &mut rng);
    assert_eq!((m.stars.len(), m.rad, m.update_order), (51, 150.0, 3));
    // rad 50 -> 50 + 0.3 * 450 = 185 -> 26.9.
    let m = StarMatrix::new(0, Vec2::ZERO, 0.0, &mut rng);
    assert_eq!(m.stars.len(), 26);
    // Sprites: a star, its connections and its rings; first sprites run on.
    let mut next = 0;
    for star in &m.stars {
        assert_eq!(star.first_sprite, next);
        assert_eq!(
            star.total_sprites,
            1 + star.connections.len() + star.circles
        );
        next += star.total_sprites;
    }
    assert_eq!(m.total_sprites, next);
}

/// Per star: the position (3 draws), the rings roll (1, +2 above it), the wanted
/// connections (1 past the tenth), a `Range(0, index)` per try, and one roll per kept
/// connection. Replayed on a second stream from the finished stars.
#[test]
fn constructor_draws_per_star() {
    for seed in 0..10 {
        let before = UnityRandom::new(seed);
        let mut rng = before.clone();
        let m = StarMatrix::new(0, Vec2::ZERO, 100.0, &mut rng);
        let mut probe = before.clone();
        for (index, star) in m.stars.iter().enumerate() {
            let [x, y, z] = probe.inside_unit_sphere();
            assert_eq!(star.position, Vec3::new(x, y, z));
            if probe.value() >= 0.5 {
                let top = probe.range_i32(1, 4);
                let rings = probe.range_i32(0, top) as usize;
                assert_eq!(star.circles, rings);
            } else {
                assert_eq!(star.circles, 0);
            }
            let wanted = if index > 10 { probe.range_i32(1, 3) } else { 0 };
            let mut found = Vec::new();
            for _ in 0..wanted * 100 {
                let other = probe.range_i32(0, index as i32) as usize;
                if m.stars[other].position.distance(star.position) < 0.4 && !found.contains(&other)
                {
                    found.push(other);
                }
                if found.len() as i32 >= wanted {
                    break;
                }
            }
            assert_eq!(found, star.connections, "seed {seed} star {index}");
            for on in &star.connections_on {
                assert_eq!(*on, probe.value() < 0.5);
            }
        }
        assert_eq!(probe, rng, "seed {seed}");
        for star in &m.stars {
            assert!(star.position.length() <= 1.0);
            assert!(star.connections.iter().all(|&c| c < star.index));
        }
    }
}

#[test]
fn the_first_ten_stars_have_no_connections() {
    let mut rng = UnityRandom::new(2);
    let m = StarMatrix::new(0, Vec2::ZERO, 200.0, &mut rng);
    assert!(m.stars.iter().take(11).all(|s| s.connections.is_empty()));
    assert!(m.stars.iter().skip(11).any(|s| !s.connections.is_empty()));
}

/// An update draws: the focus roll (1; +1 for the star and +3 when it lies within 0.6 of
/// the centre), the three rotation rolls (1 each, +2 on a hit) and each star's switch
/// roll (1; +2 on a hit with connections).
#[test]
fn update_draws_in_order() {
    let mut rng = UnityRandom::new(4);
    let mut m = StarMatrix::new(0, Vec2::ZERO, 100.0, &mut rng);
    for _ in 0..300 {
        let stars = m.stars.clone();
        let mut probe = rng.clone();
        if probe.value() < 0.025 {
            let pick = probe.range_i32(0, stars.len() as i32) as usize;
            if stars[pick].position.length() < 0.6 {
                probe.inside_unit_sphere();
            }
        }
        for _ in 0..3 {
            if probe.value() < 0.05 {
                probe.value();
                probe.value();
            }
        }
        for star in &stars {
            if probe.value() < 0.05 && !star.connections.is_empty() {
                probe.range_i32(0, star.connections.len() as i32);
                probe.value();
            }
        }
        m.update(&mut rng);
        assert_eq!(probe, rng);
    }
}

/// `zRotatVelVel += zRotatVelVel` (it should be `zRotatVel`): the z rotation never moves
/// while the others do.
#[test]
fn the_z_rotation_never_moves() {
    let mut rng = UnityRandom::new(5);
    let mut m = StarMatrix::new(0, Vec2::ZERO, 100.0, &mut rng);
    let mut moved = false;
    for _ in 0..4000 {
        m.update(&mut rng);
        assert_eq!((m.z_rotat, m.z_rotat_vel), (0.0, 0.0));
        moved |= m.x_rotat != 0.0 || m.y_rotat != 0.0;
    }
    assert!(moved);
}

#[test]
fn an_unrotated_star_projects_to_its_offset_from_the_focus() {
    let mut rng = UnityRandom::new(6);
    let mut m = StarMatrix::new(0, Vec2::ZERO, 100.0, &mut rng);
    m.focus = Vec3::new(0.1, -0.2, 0.3);
    m.last_focus = m.focus;
    let s = m.stars[3].position;
    let got = m.on_plane_pos(3, 0.5);
    assert!((got.x - (s.x - 0.1)).abs() < 1e-6 && (got.y - (s.y + 0.2)).abs() < 1e-6);
    // A quarter turn about x: y' = y cos - z sin = -z, so the plane's y is -(z - 0.3).
    m.x_rotat = 0.25;
    m.last_x_rotat = 0.25;
    let got = m.on_plane_pos(3, 0.0);
    assert!((got.y - -(s.z - 0.3)).abs() < 1e-5, "{got:?}");
    assert!((got.x - (s.x - 0.1)).abs() < 1e-5);
}

/// The focus drifts to `getToFocus` by `MoveTowards 0.002` then `Lerp 0.007`.
#[test]
fn the_focus_drifts_toward_its_target() {
    let mut rng = UnityRandom::new(7);
    let mut m = StarMatrix::new(0, Vec2::ZERO, 100.0, &mut rng);
    m.get_to_focus = Vec3::new(0.5, 0.0, 0.0);
    // A chance roll may retarget after the drift; the drift itself happens first.
    m.update(&mut rng);
    let want = 0.002 + (0.5 - 0.002) * 0.007;
    assert!((m.focus.x - want).abs() < 1e-7, "{}", m.focus.x);
    assert_eq!(m.last_focus, Vec3::default());
}
