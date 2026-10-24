//! `RoomRain`'s simulation half: the room's rain object with what `GenerateShelterTex`
//! hands it (`rainReach`, `splashTiles`, `splashes`), its `intensity`, the push properties
//! and the per-chunk steps of `ThrowAroundObjects` and `CreatureSmashedInGround`. The game's
//! side (making it in `Room.Loaded`, its turn in the update list, reaching the room's
//! bodies and creatures) is `game/room_rain_turn.rs`.
//!
//! `FloodLevel` and the tests of `Update`'s flood arm are here; the arm itself (the water
//! level, `GeneralUpsetSurface`, `AddWater`) is the game side's (D47).
//!
//! Not ported here: the bullet drips (R5), the sound loops, the sprites and the shader
//! (R3), and the DLC and Remix branches (D45, D47).
//!
//! Mirrors: RoomRain.cs (RoomRain, GenerateShelterTex, Update, ThrowAroundObjects, CreatureSmashedInGround, InsidePushAround, OutsidePushAround, FloodLevel)

use std::sync::Arc;

use rw_assets::{DangerType, Room};
use rw_random::UnityRandom;

use crate::world::{GlobalRain, World};
use crate::{BodyChunk, FloatExt, Vec2, Water};

/// `RoomRain.InsidePushAround` and `OutsidePushAround`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PushAround {
    pub inside: f32,
    pub outside: f32,
}

/// A chunk found by `ThrowAroundObjects` to be pushed: the push `num` and whether its
/// tile lies in the open (`flag`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChunkPush {
    pub num: f32,
    pub outside: bool,
    pub tile: (i32, i32),
}

/// `RoomRain`.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomRain {
    pub update_order: u64,
    pub danger_type: DangerType,
    /// Per column, the row of the first solid tile from the top (0 for a column without
    /// one).
    pub rain_reach: Arc<[i32]>,
    /// The first solid tile of each column that is not at the room's top row.
    pub splash_tiles: Vec<(i32, i32)>,
    pub splashes: i32,
    pub last_intensity: f32,
    pub intensity: f32,
}

impl RoomRain {
    /// The constructor: `dangerType` is the settings' (a shelter's is `Flood`);
    /// `GenerateShelterTex`'s `rainReach` and `splashTiles` (`HasAnySolid` is `Tile.Solid`,
    /// D7); `splashes = splashTiles.Count * 2 / cameraPositions.Length`.
    #[must_use]
    pub fn new(update_order: u64, room: &Room, settings_danger: DangerType, shelter: bool) -> Self {
        let (mut reach, mut splash_tiles) = (vec![0; room.width() as usize], Vec::new());
        for x in 0..room.width() {
            for y in (0..room.height()).rev() {
                if room.tile(x, y).is_solid() {
                    if y < room.height() - 1 {
                        splash_tiles.push((x, y));
                    }
                    reach[x as usize] = y;
                    break;
                }
            }
        }
        let cameras = room.cameras().len().max(1) as i32;
        Self {
            update_order,
            danger_type: if shelter {
                DangerType::Flood
            } else {
                settings_danger
            },
            rain_reach: reach.into(),
            splashes: splash_tiles.len() as i32 * 2 / cameras,
            splash_tiles,
            // `intensity` is 0 here, so the constructor's test picks 1.
            last_intensity: 1.0,
            intensity: 0.0,
        }
    }

    /// `InsidePushAround` and `OutsidePushAround` of this room.
    #[must_use]
    pub fn push_around(&self, global: &GlobalRain, rain_intensity: f32) -> PushAround {
        let inside = match self.danger_type {
            DangerType::AerieBlizzard => global.inside_push_around(),
            DangerType::Rain => global.inside_push_around() * rain_intensity,
            _ => 0.0,
        };
        let outside = match self.danger_type {
            DangerType::AerieBlizzard => global.outside_push_around(),
            DangerType::Rain | DangerType::FloodAndRain => {
                global.outside_push_around() * rain_intensity
            }
            _ => 0.0,
        };
        PushAround { inside, outside }
    }

    /// `Update`'s first arm: `intensity` follows `globalRain.Intensity` in the rain
    /// types (a `Flood` room's stays 0), is capped by the room's `RainIntensity` (1 for
    /// an `AerieBlizzard`), then `lastIntensity = intensity` (the game assigns it after
    /// the update, so it always equals `intensity`).
    pub fn update_intensity(&mut self, global_intensity: f32, rain_intensity: f32) {
        if matches!(
            self.danger_type,
            DangerType::Rain | DangerType::FloodAndRain | DangerType::AerieBlizzard
        ) {
            self.intensity = self.intensity.lerp(global_intensity, 0.2);
        }
        let cap = if self.danger_type == DangerType::AerieBlizzard {
            1.0
        } else {
            rain_intensity
        };
        self.intensity = self.intensity.min(cap);
        self.last_intensity = self.intensity;
    }

    /// `FloodLevel` (base arm; this rain's `dangerType`, a shelter's being `Flood`): -100
    /// without a water object, `originalWaterLevel` in a room that does not flood, else
    /// `originalWaterLevel + globalRain.flood`.
    #[must_use]
    pub fn flood_level(&self, water: Option<&Water>, flood: f32) -> f32 {
        let Some(water) = water else {
            return -100.0;
        };
        if Self::floods(self.danger_type) {
            water.original_level() + flood
        } else {
            water.original_level()
        }
    }

    /// `Flood` or `FloodAndRain`.
    #[must_use]
    pub fn floods(danger: DangerType) -> bool {
        matches!(danger, DangerType::Flood | DangerType::FloodAndRain)
    }

    /// The test of `Update`'s flood arm (base): the room's settings flood and the rain
    /// has come (`TimeUntilRain <= 0`), the flood has started, or the room has level
    /// water (`defaultWaterLevel > -1`).
    #[must_use]
    pub fn flood_arm_runs(
        settings_danger: Option<DangerType>,
        time_until_rain: i32,
        flood: f32,
        default_water_level: Option<i32>,
    ) -> bool {
        settings_danger.is_some_and(Self::floods)
            && (time_until_rain <= 0 || flood > 0.0 || default_water_level.is_some_and(|l| l > -1))
    }

    /// `rainReach[Custom.IntClamp(x, 0, TileWidth - 1)]`.
    #[must_use]
    pub fn reach_at(&self, x: i32) -> i32 {
        self.rain_reach[x.clamp(0, self.rain_reach.len() as i32 - 1) as usize]
    }

    /// The tile a chunk's random point falls in (two draws, x then y).
    fn jittered_tile(chunk_pos: Vec2, rad: f32, rng: &mut UnityRandom) -> (i32, i32) {
        let x = (-rad).lerp(rad, rng.value());
        let y = (-rad).lerp(rad, rng.value());
        (chunk_pos + Vec2::new(x, y)).tile()
    }

    /// `ThrowAroundObjects`' start for one chunk: the jittered tile (2 draws), the push
    /// `num` (the inside value, or the larger of the two where the tile is above the rain
    /// reach), weakened toward the water surface (`water_level` is `FloatWaterLevel` at the chunk, in a
    /// room with water). `None` when `num` is not above 0: the
    /// chunk gets nothing more.
    pub fn chunk_push(
        &self,
        push: PushAround,
        water_level: Option<f32>,
        chunk: &BodyChunk,
        rng: &mut UnityRandom,
    ) -> Option<ChunkPush> {
        let tile = Self::jittered_tile(chunk.pos, chunk.rad, rng);
        let mut num = push.inside;
        let mut outside = false;
        if self.reach_at(tile.0) < tile.1 {
            outside = true;
            num = push.outside.max(push.inside);
        }
        if let Some(level) = water_level {
            num *= chunk.pos.y.inverse_lerp(level - 100.0, level);
        }
        (num > 0.0).then_some(ChunkPush { num, outside, tile })
    }

    /// The velocity push of a chunk: resting on something (`ContactPoint.y < 0`) it is
    /// thrown up and sideways away from the rain's edge (two draws), else it is pushed
    /// down (one draw).
    pub fn push_velocity(
        &self,
        room: &Room,
        chunk: &mut BodyChunk,
        p: &ChunkPush,
        rng: &mut UnityRandom,
    ) {
        if chunk.contact().y < 0 {
            let (x, y) = p.tile;
            let mut side = 0;
            if self.reach_at(x - 1) >= y && !room.tile(x - 1, y).is_solid() {
                side -= 1;
            }
            if self.reach_at(x + 1) >= y && !room.tile(x + 1, y).is_solid() {
                side += 1;
            }
            let angle = (-30.0_f32).lerp(30.0, rng.value()) + (side * 16) as f32;
            let reach = if p.outside { 9.0 } else { 4.0 };
            let size = rng.value() * reach * p.num;
            chunk.vel += Vec2::from_degrees(angle) * size / chunk.mass;
        } else {
            chunk.vel.y -= rng.value().mathf_pow(5.0) * 16.5 * p.num / chunk.mass;
        }
    }

    /// A creature chunk's stun roll (one draw, a second for the length): the stun in ticks
    /// when `value^1.2 * 2 * bodyChunks.Length < num`.
    pub fn stun_roll(num: f32, owner_chunks: usize, rng: &mut UnityRandom) -> Option<i32> {
        if rng.value().mathf_pow(1.2) * 2.0 * (owner_chunks as f32) < num {
            Some(rng.range_i32(1, 1 + (9.0 * num) as i32))
        } else {
            None
        }
    }

    /// The kill roll: with `num` over 0.5 and `rainDeath` over 1, one draw below 0.025.
    pub fn kill_roll(num: f32, rain_death: f32, rng: &mut UnityRandom) -> bool {
        num > 0.5 && rain_death > 1.0 && rng.value() < 0.025
    }

    /// The push every pushed chunk ends with (two draws): up-and-sideways by the inside
    /// push, even when that is 0.
    pub fn final_push(chunk: &mut BodyChunk, inside: f32, rng: &mut UnityRandom) {
        let dir = Vec2::from_degrees(90.0_f32.lerp(270.0, rng.value()));
        chunk.vel += dir * (rng.value() * 5.0 * inside);
    }

    /// What `Creature.TerrainImpact` hands `CreatureSmashedInGround` while the global
    /// rain pushes (`AnyPushAround`).
    #[must_use]
    pub fn smash_view(&self, global: &GlobalRain, rain_intensity: f32) -> Option<SmashView> {
        global.any_push_around().then(|| SmashView {
            rain_reach: Arc::clone(&self.rain_reach),
            push: self.push_around(global, rain_intensity),
        })
    }
}

/// `CreatureSmashedInGround` with what it reads of the room rain.
#[derive(Debug, Clone, PartialEq)]
pub struct SmashView {
    rain_reach: Arc<[i32]>,
    push: PushAround,
}

impl SmashView {
    /// `CreatureSmashedInGround(crit, speed)` for a creature whose chunks are at
    /// `chunks` (position, radius): what it adds to `rainDeath`. Under 2.5 nothing, no
    /// draws; else a random chunk (`Range`) and its jitter (2 draws) pick the tile. The
    /// speed is positive, so `InverseLerp(-2.5, -15, speed)` is always 0 and nothing is
    /// added (as in the game).
    pub fn smashed_in_ground(
        &self,
        chunks: &[(Vec2, f32)],
        speed: f32,
        rng: &mut UnityRandom,
    ) -> f32 {
        if speed < 2.5 {
            return 0.0;
        }
        let mut a = self.push.inside;
        if chunks.is_empty() {
            return 0.0;
        }
        let (pos, rad) = chunks[rng.range_i32(0, chunks.len() as i32) as usize];
        let tile = RoomRain::jittered_tile(pos, rad, rng);
        let reach = self.rain_reach[tile.0.clamp(0, self.rain_reach.len() as i32 - 1) as usize];
        if reach < tile.1 {
            a = self.push.outside.max(self.push.inside);
        }
        speed.inverse_lerp(-2.5, -15.0) * a.lerp(1.0, 0.5) * 0.65 / chunks.len() as f32
    }
}

impl World {
    /// What `Creature.TerrainImpact` hands `CreatureSmashedInGround` in room `index`:
    /// `room.game.globalRain.AnyPushAround && room.roomRain != null`.
    #[must_use]
    pub(crate) fn room_rain_smash(&self, index: i32) -> Option<SmashView> {
        let r = self.realized(index)?;
        r.room_rain
            .as_ref()?
            .smash_view(&self.rain_cycle.global_rain, r.settings.rain_intensity)
    }

    /// The rain's part of `Creature.TerrainImpact` for a realized creature `id` (not the
    /// player, whose body does it inline): on a first contact with a floor,
    /// `CreatureSmashedInGround` adds to its `rainDeath`. `chunks` are every chunk's
    /// position and radius at the impact.
    pub(crate) fn creature_smashed_in_ground(
        &mut self,
        index: i32,
        id: crate::creature::CreatureId,
        impact: &crate::TerrainImpact,
        chunks: &[(Vec2, f32)],
        rng: &mut UnityRandom,
    ) {
        if !impact.first_contact || impact.direction.y >= 0 {
            return;
        }
        if let Some(view) = self.room_rain_smash(index) {
            let added = view.smashed_in_ground(chunks, impact.speed, rng);
            self.creature_mut(id).rain_death += added;
        }
    }
}

#[cfg(test)]
#[path = "tests/room_rain.rs"]
mod tests;
