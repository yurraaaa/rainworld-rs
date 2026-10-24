//! The bullet drips in play: `RoomRain.Update`'s upkeep of their count, a drip's turn
//! (the retry on a failed strike, the trace and what a hit does). Expected values from
//! `RoomRain.cs` (`Update`), `BulletDrip.cs` (`Update`, `Strike`) and `SharedPhysics.cs`
//! (`TraceProjectileAgainstBodyChunks`), replayed by hand on a second generator.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, DangerType, RoomSettings};

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::creature::lizard::perception_turn::tests::game_seeded;
use crate::tests::room_with_water;
use crate::world::BulletDrip;
use crate::world::World;
use crate::{ChunkOwner, RainWorldGame, Vec2};
use crate::{Sounds, Start};

/// A 30 x 6 room open at the top; the two walls are the only columns with a roof tile.
const OPEN: [&str; 6] = [
    "#............................#",
    "#............................#",
    "#............................#",
    "#............................#",
    "#............................#",
    "##############################",
];

fn open_game() -> RainWorldGame {
    game_seeded(&OPEN, (3, 1), 5)
}

fn drips(g: &RainWorldGame) -> &[BulletDrip] {
    &g.world()
        .realized(0)
        .unwrap()
        .room_rain
        .as_ref()
        .unwrap()
        .bullet_drips
}

fn set_density(g: &mut RainWorldGame, density: f32, rain_intensity: f32) {
    g.world_mut().rain_cycle.global_rain.bullet_rain_density = density;
    g.world_mut()
        .realized_mut(0)
        .unwrap()
        .settings
        .rain_intensity = rain_intensity;
}

fn rain_order(g: &RainWorldGame) -> u64 {
    g.world()
        .realized(0)
        .unwrap()
        .room_rain
        .as_ref()
        .unwrap()
        .update_order
}

fn add_ready_drip(g: &mut RainWorldGame) -> u64 {
    let order = g.world_mut().next_update_seq();
    let mut d = BulletDrip::new(order);
    d.delay = 0;
    g.world_mut()
        .realized_mut(0)
        .unwrap()
        .room_rain
        .as_mut()
        .unwrap()
        .bullet_drips
        .push(d);
    order
}

fn drip(g: &RainWorldGame, order: u64) -> BulletDrip {
    drips(g)
        .iter()
        .find(|d| d.update_order == order)
        .unwrap()
        .clone()
}

/// `RoomRain.Update` keeps `(int)(TileWidth * bulletRainDensity * RainIntensity)` drips:
/// one added per tick until there, in the update list in order of joining.
#[test]
fn the_upkeep_adds_one_drip_per_tick_up_to_the_count() {
    let mut g = open_game();
    // (int)(30 * 0.5 * 0.5) = 7 (7.5 truncates).
    set_density(&mut g, 0.5, 0.5);
    let order = rain_order(&g);
    for n in 1..=9 {
        g.run_room_rain(0, order);
        assert_eq!(drips(&g).len(), n.min(7), "tick {n}");
    }
    let orders: Vec<u64> = drips(&g).iter().map(|d| d.update_order).collect();
    assert!(orders.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(g.room_bullet_drip_objects(0).len(), 7);
}

/// Above the count the oldest is destroyed, one per tick.
#[test]
fn the_upkeep_destroys_the_oldest_first() {
    let mut g = open_game();
    set_density(&mut g, 1.0, 1.0);
    let order = rain_order(&g);
    for _ in 0..30 {
        g.run_room_rain(0, order);
    }
    assert_eq!(drips(&g).len(), 30);
    let orders: Vec<u64> = drips(&g).iter().map(|d| d.update_order).collect();
    // (int)(30 * 0.1 * 1) = 3.
    set_density(&mut g, 0.1, 1.0);
    g.run_room_rain(0, order);
    assert_eq!(drips(&g).len(), 29);
    assert_eq!(drips(&g)[0].update_order, orders[1]);
    for _ in 0..40 {
        g.run_room_rain(0, order);
    }
    let left: Vec<u64> = drips(&g).iter().map(|d| d.update_order).collect();
    assert_eq!(left, orders[27..]);
}

/// A density of 0 (no bullet rain) keeps none.
#[test]
fn no_density_no_drips() {
    let mut g = open_game();
    let order = rain_order(&g);
    for _ in 0..5 {
        g.run_room_rain(0, order);
    }
    assert!(drips(&g).is_empty());
}

/// A seed whose first `Range(0, 30)` draw is a wall column (roof tile solid) or not.
fn seed_with_column(wall: bool) -> i32 {
    (0..10_000)
        .find(|&s| {
            let c = UnityRandom::new(s).range_i32(0, 30);
            (c == 0 || c == 29) == wall
        })
        .unwrap()
}

/// A strike on a column with a roof tile draws once and leaves `delay` alone, so the drip
/// strikes again every tick (user decision, D50): 1 draw per tick, `delay` counting down.
#[test]
fn a_failed_strike_retries_and_draws_every_tick() {
    let mut g = open_game();
    let order = add_ready_drip(&mut g);
    let seed = seed_with_column(true);
    g.rng = UnityRandom::new(seed);
    let mut e = UnityRandom::new(seed);
    g.run_bullet_drip(0, order);
    e.range_i32(0, 30);
    let d = drip(&g, order);
    assert_eq!((d.delay, d.falling), (-1, 1.0));
    assert_eq!(g.rng.state(), e.state(), "one draw");
    // The next tick strikes again, whatever column the stream gives.
    g.run_bullet_drip(0, order);
    let wall = matches!(e.range_i32(0, 30), 0 | 29);
    let d = drip(&g, order);
    if wall {
        assert_eq!((d.delay, d.falling), (-2, 1.0));
        assert_eq!(g.rng.state(), e.state(), "one draw again");
    } else {
        assert_eq!(d.falling, 0.0);
    }
}

/// An open column: the setup's draws (column, three values, the delay) and the strike
/// sound (`Sounds::default()` has no table, so none) leave the drip falling.
#[test]
fn a_strike_in_an_open_column_restarts_the_fall() {
    let mut g = open_game();
    let order = add_ready_drip(&mut g);
    let seed = seed_with_column(false);
    g.rng = UnityRandom::new(seed);
    set_density(&mut g, 0.5, 1.0);
    g.run_bullet_drip(0, order);
    let d = drip(&g, order);
    let mut e = UnityRandom::new(seed);
    let column = e.range_i32(0, 30);
    // Open interior: rainReach 0, so row -10.
    let x = (-10.0_f32).lerp(10.0, e.value());
    assert_eq!(
        d.pos,
        Vec2::new(column as f32 * 20.0 + 10.0 + x, -190.0 + 10.0)
    );
    for _ in 0..2 {
        e.value();
    }
    e.range_i32(0, 30);
    assert_eq!(g.rng.state(), e.state());
    assert_eq!((d.falling, d.last_falling, d.move_tip), (0.0, 0.0, true));
}

use crate::FloatExt;

/// The first layer-1 chunk the path from `skyPos` to `pos` crosses (radius 0.5,
/// `0 < t < 1`), by `CirclesCollisionTime`.
fn first_hit(g: &RainWorldGame, d: &BulletDrip) -> Option<(usize, f32)> {
    let mut best: Option<(usize, f32)> = None;
    for i in 0.. {
        let Some(c) = g.electric_chunk(ChunkOwner::Player, i) else {
            break;
        };
        let t = d
            .sky_pos
            .circles_collision_time(c.pos, d.pos - d.sky_pos, 0.5, c.rad);
        if t > 0.0 && t < 1.0 && best.is_none_or(|(_, b)| t < b) {
            best = Some((i, t));
        }
    }
    best
}

/// A drop whose path crosses the slugcat: `pos` moves to the collision point, the chunk's
/// `vel.y` drops by `2 / mass`, and the creature is stunned by `Range(0, 4)` (one more
/// draw after the setup's five).
#[test]
fn a_hit_pushes_the_chunk_down_and_stuns_the_creature() {
    let mut found = None;
    for seed in 0..20_000 {
        let mut g = open_game();
        let order = add_ready_drip(&mut g);
        g.rng = UnityRandom::new(seed);
        let (room, reach) = {
            let r = g.world().realized(0).unwrap();
            (
                r.room.clone(),
                r.room_rain.as_ref().unwrap().rain_reach.clone(),
            )
        };
        let mut probe = BulletDrip::new(0);
        let mut rng = UnityRandom::new(seed);
        if !probe.strike_setup(&room, &reach, 0.0, 0.0, &mut rng) {
            continue;
        }
        if let Some((i, t)) = first_hit(&g, &probe) {
            found = Some((seed, g, order, probe, i, t));
            break;
        }
    }
    let (seed, mut g, order, probe, i, t) = found.expect("some seed hits the slugcat");
    let before = g.electric_chunk(ChunkOwner::Player, i).unwrap();
    g.run_bullet_drip(0, order);
    let d = drip(&g, order);
    assert_eq!(d.pos, probe.sky_pos.lerp(probe.pos, t));
    let after = g.electric_chunk(ChunkOwner::Player, i).unwrap();
    assert_eq!(after.vel.y, before.vel.y - 2.0 / before.mass);
    let mut e = UnityRandom::new(seed);
    e.range_i32(0, 30);
    for _ in 0..3 {
        e.value();
    }
    e.range_i32(0, 60);
    e.range_i32(0, 4);
    // `Player.Stun`'s own two range draws (the unstanding and the back spear).
    e.range_i32(5, 12);
    e.range_i32(40, 80);
    assert_eq!(
        g.rng.state(),
        e.state(),
        "five setup draws, the stun's, Player.Stun's"
    );
}

fn water_game() -> RainWorldGame {
    let room = room_with_water(&OPEN, 3);
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        danger_type: Some(DangerType::Rain),
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let campaign = &SlugcatCampaign::SURVIVOR;
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let mut g = RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some((5, 1)),
        },
        UnityRandom::new(5),
        Sounds::default(),
    );
    g.tick_until_ready(&mut ());
    g
}

/// A drop landing under the surface: `pos.y = FloatWaterLevel - 30`, then
/// `WaterfallHitSurface(pos, 0, 1)` makes its draws (a `value < flow` roll per point from
/// the one at or left of `x` to the one past it, and two more where it passes), and
/// `Small_Object_Into_Water_Fast` plays.
#[test]
fn a_drop_under_the_surface_stirs_the_water() {
    let mut g = water_game();
    let order = add_ready_drip(&mut g);
    let seed = seed_with_column(false);
    g.rng = UnityRandom::new(seed);
    let mut water = g.world().realized(0).unwrap().water.clone().unwrap();
    g.run_bullet_drip(0, order);
    let d = drip(&g, order);

    let mut e = UnityRandom::new(seed);
    e.range_i32(0, 30);
    for _ in 0..3 {
        e.value();
    }
    e.range_i32(0, 60);
    let level = water.level_at(d.pos.x);
    assert_eq!(d.pos.y, level - 30.0);
    water.waterfall_hit_surface(d.pos, 0.0, 1.0, &mut e);
    assert_eq!(g.rng.state(), e.state());
    assert_eq!(
        g.world()
            .realized(0)
            .unwrap()
            .water
            .as_ref()
            .unwrap()
            .point(3),
        water.point(3)
    );
}

/// A hit and a submerged landing in one strike: the collision point is
/// `Lerp(skyPos, pos, t)` from the `pos` the setup made, taken before the water entry moves
/// `pos.y` to the surface minus 30; the push still lands on the hit chunk.
#[test]
fn a_hit_point_is_taken_before_the_water_entry_moves_the_drop() {
    let mut found = None;
    for seed in 0..20_000 {
        let mut g = water_game();
        let order = add_ready_drip(&mut g);
        g.rng = UnityRandom::new(seed);
        let (room, reach) = {
            let r = g.world().realized(0).unwrap();
            (
                r.room.clone(),
                r.room_rain.as_ref().unwrap().rain_reach.clone(),
            )
        };
        let mut probe = BulletDrip::new(0);
        if !probe.strike_setup(&room, &reach, 0.0, 0.0, &mut UnityRandom::new(seed)) {
            continue;
        }
        if let Some((i, t)) = first_hit(&g, &probe) {
            found = Some((g, order, probe, i, t));
            break;
        }
    }
    let (mut g, order, probe, i, t) = found.expect("some seed hits the slugcat");
    let level = g
        .world()
        .realized(0)
        .unwrap()
        .water
        .as_ref()
        .unwrap()
        .level_at(probe.pos.x);
    assert!(probe.pos.y < level, "the strike lands under the surface");
    let before = g.electric_chunk(ChunkOwner::Player, i).unwrap();
    g.run_bullet_drip(0, order);
    assert_eq!(drip(&g, order).pos, probe.sky_pos.lerp(probe.pos, t));
    let after = g.electric_chunk(ChunkOwner::Player, i).unwrap();
    assert_eq!(after.vel.y, before.vel.y - 2.0 / before.mass);
}
