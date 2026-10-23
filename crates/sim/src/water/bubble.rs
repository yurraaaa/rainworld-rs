//! `Bubble`: an air bubble rising through a room's water, a `CosmeticSprite` in its
//! room's update list. The water makes them (the bottom roll, bodies crossing the
//! surface, their `BubbleEmitter`s), so do the slugcat's lungs and deep-swim boosts and
//! a drowning lizard. Constructor and update draw on the shared stream; the sprite is
//! `rw-graphics`'. Behaviour: `rainworld-re/surveys/room-effects/water-effects.md` W6.
//! Remix off and no DLC: the inverted-water and `PointSubmerged` branches are left out
//! (docs/divergences/ D11).
//!
//! Mirrors: Bubble.cs

use rw_assets::{Room, Terrain};
use rw_random::UnityRandom;

use crate::{FloatExt, Vec2, Water};

/// `Bubble.Mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BubbleMode {
    /// Swelling on the floor it was born on, still.
    Growing,
    /// Rising and wobbling.
    Free,
    /// Caught under a ceiling tile, shrinking while it slides sideways.
    Trapped,
}

impl BubbleMode {
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Growing => "Growing",
            Self::Free => "Free",
            Self::Trapped => "Trapped",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        [Self::Growing, Self::Free, Self::Trapped]
            .into_iter()
            .find(|m| m.value() == value)
    }
}

/// One `Bubble`.
#[derive(Debug, Clone, PartialEq)]
pub struct Bubble {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub vel: Vec2,
    pub full_size: f32,
    pub size: f32,
    pub last_size: f32,
    pub growth_speed: f32,
    /// `fakeWaterBubble`: not popped at the surface or by walls (gates' fake water).
    pub fake_water_bubble: bool,
    pub age: i32,
    pub mode: BubbleMode,
    /// A free bubble starts at size 0 and jumps to full size on its first update.
    pub init_pop: bool,
    pub do_not_slow: bool,
    pub ignore_walls: bool,
}

impl Bubble {
    /// The constructor: `fullSize = Lerp(0.5, 1.5, value)`, `growthSpeed = 1 /
    /// Lerp(5, 120, value)` (two draws); a bottom bubble starts growing, any other free.
    pub fn new(
        pos: Vec2,
        vel: Vec2,
        bottom_bubble: bool,
        fake_water_bubble: bool,
        rng: &mut UnityRandom,
    ) -> Self {
        let full_size = 0.5.lerp(1.5, rng.value());
        let growth_speed = 1.0 / 5.0.lerp(120.0, rng.value());
        Self {
            pos,
            // `CosmeticSprite.lastPos` is never set before the first update.
            last_pos: Vec2::ZERO,
            vel,
            full_size,
            size: 0.0,
            last_size: 0.0,
            growth_speed,
            fake_water_bubble,
            age: 0,
            mode: if bottom_bubble {
                BubbleMode::Growing
            } else {
                BubbleMode::Free
            },
            init_pop: !bottom_bubble,
            do_not_slow: false,
            ignore_walls: false,
        }
    }

    /// `Bubble.Update` then `CosmeticSprite.Update`; true once destroyed. Growing: one
    /// draw (two when it stops growing); trapped: two; free: two. Above
    /// `FloatWaterLevel - 10` it shrinks to nothing, then goes.
    pub fn update(&mut self, room: &Room, water: Option<&Water>, rng: &mut UnityRandom) -> bool {
        self.last_size = self.size;
        if !self.do_not_slow {
            self.vel *= 0.8;
        }
        let mut destroyed = match self.mode {
            BubbleMode::Growing => {
                self.grow(rng);
                false
            }
            BubbleMode::Trapped => self.trapped(room, rng),
            BubbleMode::Free => self.free(room, rng),
        };
        let level = water.map_or(f32::MIN, |w| w.level_at(self.pos.x));
        if self.pos.y > level - 10.0 && !self.fake_water_bubble {
            if self.size == 0.0 {
                destroyed = true;
            } else {
                self.size = 0.0;
            }
        }
        self.last_pos = self.pos;
        self.pos += self.vel;
        destroyed
    }

    fn grow(&mut self, rng: &mut UnityRandom) {
        self.vel = Vec2::ZERO;
        if self.size < 1.0 && rng.value() > 0.0125 {
            self.size = (self.size + self.growth_speed).clamp(0.0, 1.0);
            self.pos.y += self.full_size * self.growth_speed * 10.0;
        } else if rng.value() < 0.05 {
            self.mode = BubbleMode::Free;
        }
    }

    /// Under a ceiling: a sideways wobble and a push to a free side, held just under the
    /// tile above; free again once that tile opens, gone once shrunk away.
    fn trapped(&mut self, room: &Room, rng: &mut UnityRandom) -> bool {
        self.vel.x += (-0.2).lerp(0.2, rng.value());
        let side = if rng.value() < 0.5 { -1.0 } else { 1.0 };
        let beside = self.pos + Vec2::new(20.0 * side, 10.0);
        if room.tile_at(beside.to_array()).terrain != Terrain::Solid {
            self.vel.x += side;
        }
        self.pos.y = Vec2::middle_of_tile(self.pos.tile()).y + 9.0;
        let above = self.pos + Vec2::new(0.0, 20.0);
        if room.tile_at(above.to_array()).terrain != Terrain::Solid {
            self.mode = BubbleMode::Free;
        }
        self.size -= self.growth_speed;
        self.size <= 0.0
    }

    /// Rising with a random push (`DegToVec(-90 + 180 value) * value * 1.2`), gone after
    /// 600 ticks; rising into a solid tile traps it, moving sideways into one puts it back
    /// at the edge, starting inside one pops it.
    fn free(&mut self, room: &Room, rng: &mut UnityRandom) -> bool {
        if self.size == 0.0 && self.init_pop {
            self.size = 1.0;
            self.init_pop = false;
        }
        // `WaterViscosity` is 0: no base-game room sets it (D11).
        let dir = Vec2::from_degrees(-90.0 + 180.0 * rng.value());
        self.vel += dir * (rng.value() * 1.2);
        self.age += 1;
        let mut destroyed = self.age > 600;
        let solid = room.tile_at(self.pos.to_array()).terrain == Terrain::Solid;
        if solid && !self.fake_water_bubble && !self.ignore_walls {
            if room.tile_at(self.last_pos.to_array()).terrain == Terrain::Solid {
                destroyed = true;
            } else {
                let (tile, last) = (self.pos.tile(), self.last_pos.tile());
                if tile.1 == last.1 + 1 {
                    self.mode = BubbleMode::Trapped;
                    self.vel.y = 0.0;
                    self.pos.y = Vec2::middle_of_tile(tile).y - 11.0;
                } else if tile.0 != last.0 {
                    let edge = if self.pos.x < self.last_pos.x {
                        -10.0
                    } else {
                        10.0
                    };
                    self.pos.x = Vec2::middle_of_tile(last).x + edge;
                }
            }
        }
        destroyed
    }
}

#[cfg(test)]
#[path = "tests/bubble.rs"]
mod tests;
