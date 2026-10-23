//! `Room.Loaded`'s `ElectricDeath`, its turn in the update list (draw order and count),
//! the strikes and the creatures' electric `Violence`, the `SparkFlash` objects.
//! Expected values from `Room.cs` (`Loaded`), `ElectricDeath.cs` (`Update`,
//! `AIMapReady`, `SparkFlash`), `Creature.cs` (`Violence`) and `Player.cs` (`Stun`), the
//! slugcat template (`baseDamageResistance` 1, `baseStunResistance` 1,
//! `instantDeathDamageLimit` 1) and the green lizard breed (`toughness` 2.5, so a damage
//! resistance of 5), replayed by hand on a second generator.

use std::sync::Arc;

use rw_assets::{ClipNames, RoomEffect, RoomEffectType, SoundTable};
use rw_random::UnityRandom;

use crate::game::lizard_perception::tests::{HALL, game};
use crate::{CosmeticKind, FloatExt, RainWorldGame, SoundEvent, SoundId, Sounds, Vec2};

impl RainWorldGame {
    fn with_electric_death(&mut self, amount: f32) -> u64 {
        self.world_mut()
            .realized_mut(0)
            .expect("room")
            .settings
            .effects = vec![RoomEffect {
            kind: RoomEffectType::ElectricDeath.value().to_owned(),
            amount,
        }];
        self.add_electric_deaths(0);
        self.world().realized(0).unwrap().electric_deaths[0].update_order
    }

    /// The rain cycle `ticks` past the end of the cycle.
    fn cycle_end_plus(&mut self, ticks: i32) {
        let r = &mut self.world_mut().rain_cycle;
        r.timer = r.cycle_length + ticks;
    }

    fn death(&self) -> &crate::ElectricDeath {
        &self.world().realized(0).unwrap().electric_deaths[0]
    }

    /// Every chunk of room 0's `physicalObjects` in order, with whether its owner is a
    /// creature (the player's first: its layer comes before the items').
    fn room_chunks(&self) -> Vec<(bool, crate::BodyChunk)> {
        let seq = self.player_seq;
        let mut chunks = Vec::new();
        for (_, _, owner) in self.world().physical_objects(0, Some(seq)) {
            let (creature, list) = match owner {
                crate::ChunkOwner::Player => {
                    (true, self.player.as_ref().unwrap().body.chunks.to_vec())
                }
                crate::ChunkOwner::Entity(e) => (
                    matches!(e, crate::item::Entity::Creature(_)),
                    self.world().surface_chunks(e).unwrap().to_vec(),
                ),
            };
            chunks.extend(list.into_iter().map(|c| (creature, c)));
        }
        chunks
    }

    fn count_cosmetics(&self, f: fn(&CosmeticKind) -> bool) -> usize {
        self.world()
            .realized(0)
            .unwrap()
            .cosmetics
            .iter()
            .filter(|c| f(&c.kind))
            .count()
    }

    /// `sounds.txt` with the two one-shot lines and the two loops, one loose sample each.
    fn with_sound_table(&mut self) {
        let text = "Volume: 6\nVolume exponent: 1.8\n\
            Death_Lightning_Spark_Object : snap2/vol=0.7\n\
            Death_Lightning_Spark_Spontaneous : snap2/vol=0.5\n\
            Death_Lightning_Heavy_Lightning_LOOP : loop2/vol=0.8\n\
            Death_Lightning_Early_Sizzle_LOOP : loop1/vol=0.6\n";
        let names = ClipNames::new(
            ["snap2".to_owned(), "loop1".to_owned(), "loop2".to_owned()],
            None,
        );
        let table = Arc::new(SoundTable::parse(text, &names).unwrap());
        self.sounds = Sounds::new(Arc::clone(&table));
        self.silent = Sounds::new(table);
    }
}

/// One `Update` at full intensity replayed from the decompile on a second generator.
struct Model {
    e: UnityRandom,
    /// The draws a one-shot sound makes (7 with a table, 0 without).
    sound: usize,
    strikes: Vec<(f32, f32, f32)>,
    /// Per chunk of the room, in `physicalObjects` order.
    kick: Vec<Vec2>,
    dead: bool,
    stun: i32,
    wall: Option<(Vec2, f32)>,
    wall_flash: bool,
}

impl Model {
    /// The three chained one-in-three tests of the colour block, each hit redrawing one
    /// component (the branches differ only in which).
    #[allow(clippy::if_same_then_else)]
    fn colour_chain(e: &mut UnityRandom) {
        if e.value() < 1.0 / 3.0 {
            e.value();
        } else if e.value() < 1.0 / 3.0 {
            e.value();
        } else if e.value() < 1.0 / 3.0 {
            e.value();
        }
    }

    fn skip_sound(&mut self) {
        for _ in 0..self.sound {
            self.e.value();
        }
    }

    /// `ElectricDeath.Update` with the first tick's loops (no draws without a loop
    /// sound table), room area `area`, wall `tiles`, the room's chunks `chunks` (whether
    /// its owner is a creature, and the chunk).
    fn run(
        seed: i32,
        sound: usize,
        area: f32,
        tiles: &[(i32, i32)],
        chunks: &[(bool, crate::BodyChunk)],
        long_after: bool,
    ) -> Self {
        let mut m = Self {
            e: UnityRandom::new(seed),
            sound,
            strikes: Vec::new(),
            kick: vec![Vec2::ZERO; chunks.len()],
            dead: false,
            stun: 0,
            wall: None,
            wall_flash: false,
        };
        // The spontaneous spark: the gate (one draw), then the tile, the spot, the size.
        if m.e.value() < area.inverse_lerp(1000.0, 9120.0) {
            let n = i32::try_from(tiles.len()).unwrap();
            let tile = tiles[m.e.range_i32(0, n) as usize];
            let x = (-10.0f32).lerp(10.0, m.e.value());
            let y = (-10.0f32).lerp(10.0, m.e.value());
            let size = m.e.value();
            let pos = Vec2::new(
                tile.0 as f32 * 20.0 + 10.0 + x,
                tile.1 as f32 * 20.0 + 10.0 + y,
            );
            // On screen: `SparkFlash`'s lifetime draw.
            m.e.value();
            m.wall_flash = true;
            m.skip_sound();
            m.wall = Some((pos, size));
        }
        // Eight flashes reset, the colour block.
        for _ in 0..32 {
            m.e.value();
        }
        m.e.value();
        Self::colour_chain(&mut m.e);
        if m.e.value() >= 0.5 {
            return m;
        }
        for (i, (creature, c)) in chunks.iter().enumerate() {
            if m.e.value() >= 0.5 {
                continue;
            }
            let k = c.contact();
            if k.x == 0 && k.y == 0 {
                continue;
            }
            let power = m.e.value().powf(0.9);
            m.e.value();
            let dir = Vec2::from_degrees(m.e.value() * 360.0);
            let contact = Vec2::new(f32::from(k.x), f32::from(k.y));
            m.kick[i] = -(contact + dir).normalized() * (22.0 * power / c.mass);
            m.skip_sound();
            let stun_bonus = power * 40.0;
            if !creature {
                m.strikes.push((power, 0.0, stun_bonus));
                continue;
            }
            let mut damage = power * 1.8;
            if long_after {
                damage = m.e.value().powf(0.9) * 2.0;
            }
            // `Stun((int)(damage * 30 + stunBonus))`: two range draws.
            m.e.range_i32(5, 12);
            m.e.range_i32(40, 80);
            m.stun = m.stun.max((damage * 30.0 + stun_bonus) as i32);
            m.dead |= damage >= 1.0;
            m.strikes.push((power, damage, stun_bonus));
        }
        m
    }
}

/// `Room.Loaded` makes an `ElectricDeath` per effect in the update list, and
/// `AIMapReady` lists the tiles with `getTerrainProximity == 1`: the ring of floor tiles
/// beside the walls of the 28 x 4 hall (60 of them), x outer, y inner.
#[test]
fn loaded_makes_it_with_the_wall_tiles() {
    let mut g = game(&HALL, (3, 1));
    let before = g.world().update_seq();
    let order = g.with_electric_death(0.5);
    assert_eq!(order, before + 1);
    let d = g.death();
    assert_eq!(d.amount, 0.5);
    let tiles = d.close_to_wall_tiles.clone().expect("listed");
    let mut want = Vec::new();
    for x in 0..30 {
        for y in 0..6 {
            let inside = (1..=28).contains(&x) && (1..=4).contains(&y);
            if inside && (x == 1 || x == 28 || y == 1 || y == 4) {
                want.push((x, y));
            }
        }
    }
    assert_eq!(want.len(), 60);
    assert_eq!(tiles, want);
}

/// Before the rain's last 400 ticks `Intensity` is 0 and the object does nothing: no
/// draws, no loops.
#[test]
fn nothing_happens_at_intensity_zero() {
    let mut g = game(&HALL, (3, 1));
    let order = g.with_electric_death(1.0);
    g.world_mut().rain_cycle.timer = g.world().rain_cycle.cycle_length - 400;
    let state = g.rng.state();
    g.run_electric_death(0, order);
    assert_eq!(g.rng.state(), state);
    assert!(g.death().heavy_loop.is_none());
    assert_eq!(g.death().sin, 0.0);
}

/// One `Update` at intensity 1 in the 30 x 6 hall (area 180, so the spark gate is `0 *
/// 1`): the gate's draw, eight resets of four, the colour block, the strike gate; and
/// when it passes, per chunk of the player a gate and, for a touching one, the power, the
/// flash's lifetime, the kick's direction and the player's `Stun` (two ranges). The
/// slugcat dies at damage 1 (`instantDeathDamageLimit`), else it is stunned for
/// `(int)(damage * 30 + stunBonus)` and kicked off the wall.
#[test]
fn an_update_draws_in_the_games_order_and_strikes_the_player() {
    let (mut struck_any, mut killed, mut survived_hit) = (false, false, false);
    for seed in 0..80 {
        let mut g = game(&HALL, (3, 1));
        let order = g.with_electric_death(1.0);
        g.cycle_end_plus(1200);
        let chunks = g.room_chunks();
        let tiles = g.death().close_to_wall_tiles.clone().unwrap();
        let m = Model::run(seed, 0, 180.0, &tiles, &chunks, false);
        g.rng = UnityRandom::new(seed);
        g.run_electric_death(0, order);
        assert_eq!(g.rng.state(), m.e.state(), "seed {seed}");
        let strikes: Vec<_> = g
            .death()
            .strikes
            .iter()
            .map(|s| (s.power, s.damage, s.stun_bonus))
            .collect();
        assert_eq!(strikes, m.strikes, "seed {seed}");
        assert_eq!(
            chunks[0].1.pos,
            g.player.as_ref().unwrap().body.chunks[0].pos
        );
        assert_eq!(
            g.count_cosmetics(|k| matches!(k, CosmeticKind::SparkFlash(_))),
            m.strikes.len()
        );
        let p = g.player.as_ref().unwrap();
        assert_eq!(p.is_dead(), m.dead, "seed {seed}");
        if !m.strikes.is_empty() {
            struck_any = true;
            assert_eq!(p.stun_ticks(), m.stun, "seed {seed}");
            for (i, c) in p.body.chunks.iter().enumerate() {
                assert_eq!(c.vel, chunks[i].1.vel + m.kick[i], "seed {seed} chunk {i}");
            }
        }
        killed |= m.dead;
        survived_hit |= !m.dead && !m.strikes.is_empty();
        assert_eq!(g.death().sin, 0.1, "sin += Intensity * 0.1");
        assert_eq!(g.death().last_sin, 0.0);
    }
    assert!(struck_any && killed && survived_hit);
}

/// At `TimeUntilRain <= -1500` the damage is `Pow(value, 0.9) * 2` (one more draw after
/// the sound), instead of `power * 1.8`.
#[test]
fn long_after_the_rain_the_damage_is_rolled() {
    let mut rolled = 0;
    for seed in 0..60 {
        let mut g = game(&HALL, (3, 1));
        let order = g.with_electric_death(1.0);
        g.cycle_end_plus(1500);
        let chunks = g.room_chunks();
        let tiles = g.death().close_to_wall_tiles.clone().unwrap();
        let m = Model::run(seed, 0, 180.0, &tiles, &chunks, true);
        g.rng = UnityRandom::new(seed);
        g.run_electric_death(0, order);
        assert_eq!(g.rng.state(), m.e.state(), "seed {seed}");
        let got: Vec<_> = g
            .death()
            .strikes
            .iter()
            .map(|s| (s.power, s.damage, s.stun_bonus))
            .collect();
        assert_eq!(got, m.strikes, "seed {seed}");
        assert_eq!(g.player.as_ref().unwrap().is_dead(), m.dead);
        rolled += got.len();
    }
    assert!(rolled > 5);
}

/// Below intensity 0.5 nothing is struck and the strike gate draws nothing; the flashes
/// and the colour still draw.
#[test]
fn no_strikes_below_half_intensity() {
    let mut g = game(&HALL, (3, 1));
    let order = g.with_electric_death(1.0);
    let cl = g.world().rain_cycle.cycle_length;
    g.world_mut().rain_cycle.timer = cl - 400 + 640;
    let i = g.death().intensity(&g.world().rain_cycle);
    assert!(i > 0.0 && i < 0.5, "{i}");
    g.rng = UnityRandom::new(12);
    let mut e = UnityRandom::new(12);
    g.run_electric_death(0, order);
    // The wall gate, the flashes and the colour chain; no strike gate.
    for _ in 0..34 {
        e.value();
    }
    Model::colour_chain(&mut e);
    assert_eq!(g.rng.state(), e.state());
    assert!(g.death().strikes.is_empty());
    assert!(g.player.as_ref().is_some_and(|p| !p.is_dead()));
}

/// A green lizard on the floor in the strike takes `Violence` with the electric damage:
/// its health falls by `damage / 5` (`toughness * 2`, no electric resistance) and it is
/// stunned.
#[test]
fn a_lizard_takes_electric_violence() {
    let mut hurt = 0;
    for seed in 0..60 {
        let mut g = game(&HALL, (20, 1));
        let l = g.still_lizard((5, 1), 1.0, 1);
        let order = g.with_electric_death(1.0);
        g.cycle_end_plus(1200);
        g.rng = UnityRandom::new(seed);
        g.run_electric_death(0, order);
        let strikes: Vec<_> = g
            .death()
            .strikes
            .iter()
            .filter(|s| s.pos.x < 200.0)
            .copied()
            .collect();
        let health = g.world.creature(l).state.health;
        let want: f32 = 1.0 - strikes.iter().map(|s| s.damage / 5.0).sum::<f32>();
        assert!((health - want).abs() < 1e-5, "seed {seed}: {health} {want}");
        // `Lizard.Stun((int)((damage * 30 + stunBonus) / 2.5 * ...))`, the factor at least 1.
        if strikes
            .iter()
            .any(|s| (s.damage * 30.0 + s.stun_bonus) / 2.5 >= 1.0)
        {
            hurt += 1;
            assert!(g.world.lizard_ref(l).stun > 0, "seed {seed}");
        }
    }
    assert!(hurt > 3);
}

/// A `SparkFlash` object adds a green spark to the room each update and is destroyed
/// when its life has run out for two ticks.
#[test]
fn a_spark_flash_in_the_room_sheds_sparks() {
    let mut g = game(&HALL, (3, 1));
    let flash = crate::SparkFlash::new(Vec2::new(300.0, 60.0), 0.3, &mut g.rng);
    g.world_mut()
        .add_cosmetic(0, CosmeticKind::SparkFlash(flash));
    let id = g.world().realized(0).unwrap().cosmetics.last().unwrap().id;
    let sparks = |g: &RainWorldGame| g.count_cosmetics(|k| matches!(k, CosmeticKind::Spark(_)));
    assert_eq!(sparks(&g), 0);
    let state = g.rng.state();
    g.run_cosmetic(0, id);
    assert_eq!(sparks(&g), 1);
    assert_ne!(g.rng.state(), state);
    let mut updates = 1;
    while g
        .world()
        .realized(0)
        .unwrap()
        .cosmetics
        .iter()
        .any(|c| c.id == id)
    {
        g.run_cosmetic(0, id);
        updates += 1;
        assert!(updates < 40);
    }
    let want = flash.life_time.ceil() as i32 + 2;
    assert!((want - 1..=want).contains(&updates), "{updates} {want}");
    // A spark each update (the destroying one too).
    assert_eq!(sparks(&g) as i32, updates);
}

/// A chunk's strike plays `Death_Lightning_Spark_Object` on it with the power as volume
/// (the usual seven one-shot draws, after the flash and the kick's direction, before the
/// damage), and a wall spark plays the spontaneous one at its spot with its size (seven
/// more, after the flash's lifetime), in a room big enough to spark (40 x 30 tiles, the
/// gate `InverseLerp(1000, 9120, 1200)`).
#[test]
fn sounds_make_their_draws_in_place() {
    let rows: Vec<String> = (0..30)
        .map(|y| {
            if y == 0 || y == 29 {
                "#".repeat(40)
            } else {
                format!("#{}#", ".".repeat(38))
            }
        })
        .collect();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let (mut walls, mut objects) = (0, 0);
    for seed in 0..600 {
        let mut g = game(&rows, (3, 1));
        g.with_sound_table();
        let order = g.with_electric_death(1.0);
        g.cycle_end_plus(1200);
        let chunks = g.room_chunks();
        let tiles = g.death().close_to_wall_tiles.clone().unwrap();
        let m = Model::run(seed, 7, 1200.0, &tiles, &chunks, false);
        g.rng = UnityRandom::new(seed);
        g.sounds.take_events();
        g.run_electric_death(0, order);
        assert_eq!(g.rng.state(), m.e.state(), "seed {seed}");
        let played = |id: SoundId| {
            g.sounds
                .events()
                .iter()
                .filter_map(|e| match e {
                    SoundEvent::Play(p) if p.id == id => Some(p.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let wall = played(SoundId::DeathLightningSparkSpontaneous);
        assert_eq!(wall.len(), usize::from(m.wall.is_some()), "seed {seed}");
        if let (Some((pos, size)), Some(p)) = (m.wall, wall.first()) {
            walls += 1;
            assert_eq!(p.emitter_volume, size);
            assert_eq!(p.source, crate::SoundSource::Position(pos));
            assert!(m.wall_flash);
            assert_eq!(
                g.count_cosmetics(|k| matches!(k, CosmeticKind::SparkFlash(_))),
                1 + m.strikes.len()
            );
        }
        let object = played(SoundId::DeathLightningSparkObject);
        assert_eq!(object.len(), m.strikes.len(), "seed {seed}");
        for (p, s) in object.iter().zip(&m.strikes) {
            objects += 1;
            assert_eq!(p.emitter_volume, s.0);
        }
    }
    assert!(walls > 3 && objects > 10, "{walls} {objects}");
}

/// The two loops start on the third tick with any intensity (the first makes them at
/// volume 0): the heavy loop's volume is the intensity, the early sizzle's is
/// `I^0.1 * Lerp(0.5 + 0.5 sin(sin * 2 pi), 0, I^8)`; `sin` advances by `I * 0.1`.
#[test]
fn the_loops_start_and_follow_the_intensity() {
    let mut g = game(&HALL, (3, 1));
    g.with_sound_table();
    let order = g.with_electric_death(1.0);
    let cl = g.world().rain_cycle.cycle_length;
    g.world_mut().rain_cycle.timer = cl + 200;
    let i = g.death().intensity(&g.world().rain_cycle);
    g.run_electric_death(0, order);
    let d = g.death();
    assert_eq!(d.heavy_loop.as_ref().unwrap().volume(), 0.0);
    assert!(d.heavy_loop.as_ref().unwrap().emitter.is_none());
    g.run_electric_death(0, order);
    let d = g.death();
    assert_eq!(d.heavy_loop.as_ref().unwrap().volume(), i);
    let sizzle_sin = 0.1 * i;
    let want = i.powf(0.1)
        * (0.5 + 0.5 * (sizzle_sin * std::f32::consts::PI * 2.0).sin()).lerp(0.0, i.powf(8.0));
    assert!((d.sizzle_loop.as_ref().unwrap().volume() - want).abs() < 1e-5);
    assert!((d.sin - 2.0 * 0.1 * i).abs() < 1e-6);
    // The third tick starts the emitters (volume above 0 after a 0): two loops.
    g.run_electric_death(0, order);
    assert_eq!(g.death().emitters().count(), 2);
    let started: Vec<_> = g
        .sounds
        .events()
        .iter()
        .filter_map(|e| match e {
            SoundEvent::Play(p) if p.looped => Some(p.id),
            _ => None,
        })
        .collect();
    // `DynamicSoundLoop.Update` starts a loop whose volume rose from 0 twice in that tick
    // (once for the changed `sound`, once for the rise: `lastSound` is cleared while it
    // has no emitter), the first emitter forgotten.
    assert_eq!(
        started,
        vec![
            SoundId::DeathLightningHeavyLightningLoop,
            SoundId::DeathLightningHeavyLightningLoop,
            SoundId::DeathLightningEarlySizzleLoop,
            SoundId::DeathLightningEarlySizzleLoop,
        ]
    );
}
