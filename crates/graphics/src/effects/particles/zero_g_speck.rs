//! `BlinkSpeck`'s and `CorruptionSpore`'s sprites in `Midground`: a `pixel` whose grey
//! blinks (toward the colour of the light on it in a dark room), and a `Pebble` disc that
//! spins about its vertical axis in blue shades. Both are hidden while `reset`. The
//! spore's pebble is picked when its sprite is made (a draw on the cosmetic stream), so
//! the caller keeps it. Watcher's calcified and sentient spores are not ported (D29).
//!
//! Mirrors: BlinkSpeck.cs (InitiateSprites, DrawSprites), CorruptionSpore.cs (InitiateSprites, DrawSprites)

use rw_random::UnityRandom;
use rw_sim::{BlinkState, FloatExt, SpeckKind, SporeState, Vec2, ZeroGSpeck};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};
use crate::core::math::Mathf;
use crate::item::weapon::PEBBLES;

/// A speck's sprite.
pub trait SpeckSprites {
    /// `InitiateSprites`' cosmetic draw: a spore's `Pebble{n}` (`Random.Range(1, 15)`),
    /// nothing for a speck.
    fn pick_pebble(&self, rng: &mut UnityRandom) -> Option<usize>;

    /// `DrawSprites` at `t` between ticks; `pebble` is what [`Self::pick_pebble`] gave.
    /// Nothing while `reset`.
    fn draw(&self, t: f32, pebble: Option<usize>) -> DrawList;
}

impl SpeckSprites for ZeroGSpeck {
    fn pick_pebble(&self, rng: &mut UnityRandom) -> Option<usize> {
        match self.kind {
            SpeckKind::Blink(_) => None,
            SpeckKind::CorruptionSpore(_) => usize::try_from(rng.range_i32(1, 15) - 1).ok(),
        }
    }

    fn draw(&self, t: f32, pebble: Option<usize>) -> DrawList {
        if self.reset || self.slated {
            return DrawList::default();
        }
        let pos = self.last_pos.lerp(self.pos, t);
        let sprite = match &self.kind {
            SpeckKind::Blink(b) => b.sprite(pos, t),
            SpeckKind::CorruptionSpore(s) => s.sprite(pebble, pos, t),
        };
        DrawList {
            items: vec![DrawItem::Sprite(sprite)],
        }
    }
}

/// A `BlinkSpeck`'s sprite.
trait BlinkSprite {
    fn sprite(&self, pos: Vec2, t: f32) -> Sprite;
}

impl BlinkSprite for BlinkState {
    fn sprite(&self, pos: Vec2, t: f32) -> Sprite {
        let b = self;
        let level = b.last_blink.lerp(b.blink, t);
        let grey = Color::rgb(level, level, level);
        let color = if b.room_darkness == 0.0 {
            grey
        } else {
            grey.lerp(
                Color::from(b.last_col).lerp(Color::from(b.col), t),
                b.room_darkness,
            )
        };
        Sprite {
            index: 0,
            element: "pixel",
            pos,
            rotation: 0.0,
            scale: Vec2::new(1.0, 1.0),
            anchor: Vec2::new(0.5, 0.5),
            color,
            shader: Shader::Basic,
            layer: Layer::Midground,
        }
    }
}

/// A `CorruptionSpore`'s sprite.
trait SporeSprite {
    fn sprite(&self, pebble: Option<usize>, pos: Vec2, t: f32) -> Sprite;
}

impl SporeSprite for SporeState {
    fn sprite(&self, pebble: Option<usize>, pos: Vec2, t: f32) -> Sprite {
        let s = self;
        let turn = s.last_y_rot.lerp(s.y_rot, t) * std::f32::consts::PI;
        Sprite {
            index: 0,
            element: PEBBLES[pebble.unwrap_or(0).min(PEBBLES.len() - 1)],
            pos,
            rotation: s.last_rot.lerp(s.rot, t),
            scale: Vec2::new(
                0.15_f32.lerp(0.35, s.scale) * turn.mathf_sin(),
                0.25_f32.lerp(0.55, s.scale),
            ),
            anchor: Vec2::new(0.5, 0.5),
            color: Color::rgb(0.0, 0.0, s.col),
            shader: Shader::Basic,
            layer: Layer::Midground,
        }
    }
}

#[cfg(test)]
#[path = "tests/zero_g_speck.rs"]
mod tests;
