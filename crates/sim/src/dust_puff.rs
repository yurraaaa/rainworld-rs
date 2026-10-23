//! `RoofTopView.DustpuffSpawner` and the `DustPuff`s it makes: the player's chunks
//! landing on or running along a floor raise puffs of dust that drift up and fade. The
//! spawner is made by `RoofTopView`'s constructor (and by a `Dustpuffs` effect, which no
//! base room has). Draws on the shared stream (D1): one for each chunk on the floor
//! that is not landing (the 1-in-10 test), then, for the puff that test lets through,
//! one for its size and one for its life time; one for a landing's life time.
//! Sprites: `rw-graphics` `dust_puff.rs`.
//!
//! Mirrors: RoofTopView.cs (DustpuffSpawner, DustPuff), CosmeticSprite.cs (Update)

use rw_random::UnityRandom;

use crate::{BodyChunk, FloatExt, Vec2};

/// A `DustPuff` in a room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct DustPuff {
    /// Its place in the room's update list.
    pub update_order: u64,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub life: f32,
    pub last_life: f32,
    pub life_time: f32,
    pub size: f32,
    /// `motion` (`(0.5, 0.25)`; `room.snow` zeroes it, which no base room has).
    pub motion: Vec2,
    pub slated: bool,
}

impl DustPuff {
    /// `new DustPuff(pos, size)`: its life time draws once.
    pub fn new(pos: Vec2, size: f32, rng: &mut UnityRandom) -> Self {
        Self {
            update_order: 0,
            pos,
            last_pos: pos,
            life: 1.0,
            last_life: 1.0,
            life_time: 40.0_f32.lerp(120.0, rng.value()) * 0.5_f32.lerp(1.5, size),
            size,
            motion: Vec2::new(0.5, 0.25),
            slated: false,
        }
    }

    /// `Update`: `CosmeticSprite.Update` (no velocity), the drift (`motion`'s x up, its y
    /// sideways), the life; destroyed the tick after the life passed 0.
    pub fn update(&mut self) {
        self.last_pos = self.pos;
        self.pos.y += self.motion.x;
        self.pos.x += self.motion.y;
        self.last_life = self.life;
        self.life -= 1.0 / self.life_time;
        if self.last_life < 0.0 {
            self.slated = true;
        }
    }
}

/// `DustpuffSpawner`, a room object with no state of its own.
#[derive(Debug, Clone, PartialEq)]
pub struct DustpuffSpawner {
    /// Its place in the room's update list.
    pub update_order: u64,
}

impl DustpuffSpawner {
    /// `Update` for one player's chunks (in `bodyChunks` order): the puffs made, in order,
    /// each with its constructor's draw made.
    pub fn update(chunks: &[BodyChunk], rng: &mut UnityRandom) -> Vec<DustPuff> {
        let mut puffs = Vec::new();
        for c in chunks.iter().filter(|c| c.contact().y < 0) {
            let at = c.pos + Vec2::new(0.0, -c.rad);
            let fall = c.last_pos.y - c.pos.y;
            if c.last_contact().y >= 0 && fall > 5.0 {
                puffs.push(DustPuff::new(at, fall.lerp_map(5.0, 10.0, 0.5, 1.0), rng));
            } else if rng.value() < 0.1 && (c.last_pos.x - c.pos.x).abs() > 3.0 {
                let size = 0.25 * rng.value();
                puffs.push(DustPuff::new(at, size, rng));
            }
        }
        puffs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Contact;

    fn chunk(contact_y: i8, last_contact_y: i8, last_pos: Vec2, pos: Vec2) -> BodyChunk {
        let mut c = BodyChunk::new(pos, 6.0, 0.4);
        c.last_pos = last_pos;
        c.set_contacts(
            Contact { x: 0, y: contact_y },
            Contact {
                x: 0,
                y: last_contact_y,
            },
        );
        c
    }

    fn draws(before: &UnityRandom, after: &UnityRandom) -> usize {
        let mut probe = before.clone();
        (0..20)
            .find(|_| {
                if probe == *after {
                    return true;
                }
                probe.next_u32();
                false
            })
            .expect("within 20 draws")
    }

    #[test]
    fn a_landing_makes_one_puff_with_one_draw() {
        // Fell 7.5 px onto the floor: size LerpMap(7.5, 5, 10, 0.5, 1) = 0.75.
        let c = chunk(-1, 0, Vec2::new(100.0, 57.5), Vec2::new(100.0, 50.0));
        let mut rng = UnityRandom::new(9);
        let before = rng.clone();
        let puffs = DustpuffSpawner::update(&[c], &mut rng);
        assert_eq!(draws(&before, &rng), 1);
        assert_eq!(puffs.len(), 1);
        assert_eq!(puffs[0].pos, Vec2::new(100.0, 44.0));
        assert_eq!(puffs[0].size, 0.75);
        let v = UnityRandom::new(9).value();
        assert_eq!(
            puffs[0].life_time,
            40.0_f32.lerp(120.0, v) * 0.5_f32.lerp(1.5, 0.75)
        );
    }

    #[test]
    fn running_on_the_floor_draws_the_test_and_a_puff_makes_two_more() {
        let c = chunk(-1, -1, Vec2::new(96.0, 50.0), Vec2::new(100.0, 50.0));
        let (mut puffed, mut missed) = (false, false);
        for seed in 0..40 {
            let mut rng = UnityRandom::new(seed);
            let before = rng.clone();
            let first = rng.clone().value();
            let puffs = DustpuffSpawner::update(std::slice::from_ref(&c), &mut rng);
            if first < 0.1 {
                // The size draw (0.25 * value), then the life time's.
                assert_eq!((draws(&before, &rng), puffs.len()), (3, 1), "seed {seed}");
                assert!(puffs[0].size < 0.25);
                puffed = true;
            } else {
                assert_eq!((draws(&before, &rng), puffs.len()), (1, 0), "seed {seed}");
                missed = true;
            }
        }
        assert!(puffed && missed);
    }

    #[test]
    fn slow_chunks_and_chunks_off_the_floor_make_no_puffs() {
        // On the floor but moved 3 px (not more): only the 1-in-10 draw.
        let slow = chunk(-1, -1, Vec2::new(97.0, 50.0), Vec2::new(100.0, 50.0));
        let airborne = chunk(0, -1, Vec2::new(90.0, 80.0), Vec2::new(100.0, 50.0));
        let mut rng = UnityRandom::new(4);
        let before = rng.clone();
        assert!(DustpuffSpawner::update(&[airborne], &mut rng).is_empty());
        assert_eq!(rng, before);
        let _ = DustpuffSpawner::update(&[slow], &mut rng);
        assert_eq!(draws(&before, &rng), 1);
    }

    #[test]
    fn a_puff_drifts_fades_and_goes_the_tick_after_its_life_passes_zero() {
        let mut rng = UnityRandom::new(2);
        let mut p = DustPuff::new(Vec2::new(10.0, 20.0), 0.5, &mut rng);
        p.update();
        // `motion.x` moves it up, `motion.y` sideways.
        assert_eq!(p.pos, Vec2::new(10.25, 20.5));
        assert_eq!(p.last_pos, Vec2::new(10.0, 20.0));
        assert_eq!((p.last_life, p.life), (1.0, 1.0 - 1.0 / p.life_time));
        let mut ticks = 1;
        while !p.slated {
            p.update();
            ticks += 1;
            assert!(ticks < 400);
        }
        // Destroyed when the life before the tick's decrement was already below 0.
        assert!(p.last_life < 0.0 && p.life < p.last_life);
    }
}
