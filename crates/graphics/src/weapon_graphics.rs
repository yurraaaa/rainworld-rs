//! `Rock.InitiateSprites` / `DrawSprites` / `ApplyPalette` (247100) and `Spear`'s
//! (271997): a pebble (with a thrown rock's trail) or a small spear in the `Items`
//! container, in the palette's black, turned by the item's rotation. A spear pivots on
//! its tip while thrown or stuck in a creature. `DrawSprites` draws a vibrating item's
//! jitter and a blinking one's flicker from the cosmetic stream (docs/DIVERGENCES.md D1).

use std::borrow::Cow;

use rw_random::UnityRandom;
use rw_sim::creature::EntityId;
use rw_sim::item::{Weapon, WeaponKind, WeaponMode};
use rw_sim::{Vec2, Water};

use crate::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, Sprite};
use crate::math::CustomVec;

/// `Pebble1` to `Pebble14` (`rainWorld` atlas).
pub const PEBBLES: [&str; 14] = [
    "Pebble1", "Pebble2", "Pebble3", "Pebble4", "Pebble5", "Pebble6", "Pebble7", "Pebble8",
    "Pebble9", "Pebble10", "Pebble11", "Pebble12", "Pebble13", "Pebble14",
];

pub const SMALL_SPEAR: &str = "SmallSpear";

const TRAIL: [[u16; 3]; 1] = [[0, 1, 2]];

/// The sprites of a realized rock or spear.
pub trait WeaponSprites {
    /// `DrawSprites` at `t` between ticks: `id` is the abstract object's (it picks the
    /// pebble), `black` the palette's `blackColor`, `water` the room's (the blink colour
    /// is darker under it). `None` for a weapon in another room than `room`.
    fn draw(
        &self,
        id: EntityId,
        room: i32,
        t: f32,
        black: Color,
        water: Option<&Water>,
        rng: &mut UnityRandom,
    ) -> Option<DrawList>;
}

/// A rock's pebble element.
pub trait PebbleElement {
    /// `"Pebble" + Random.Range(1, 15)` under `InitState(ID.RandomSeed)`; the global
    /// state is put back, so it draws nothing from the stream.
    fn pebble(self) -> &'static str;
}

impl PebbleElement for EntityId {
    fn pebble(self) -> &'static str {
        let mut rng = UnityRandom::new(self.random_seed());
        let n = usize::try_from(rng.range_i32(1, 15) - 1).unwrap_or(0);
        PEBBLES[n.min(PEBBLES.len() - 1)]
    }
}

impl WeaponSprites for Weapon {
    fn draw(
        &self,
        id: EntityId,
        room: i32,
        t: f32,
        black: Color,
        water: Option<&Water>,
        rng: &mut UnityRandom,
    ) -> Option<DrawList> {
        if self.room != room {
            return None;
        }
        let c = self.chunk();
        let mut at = c.last_pos.lerp(c.pos, t);
        if self.vibrate > 0 {
            let dir = Vec2::random_unit(rng);
            at += dir * (2.0 * rng.value());
        }
        let rotation = Vec2::ZERO.aim_to(self.last_rotation.slerp(self.rotation, t));
        let layer = if self.in_front_of_objects == 0 {
            Layer::Background
        } else {
            Layer::Items
        };
        // `blinkColor`: white, or nearly black under water.
        let blink = if water.is_some_and(|w| c.pos.y < w.level_at(c.pos.x)) {
            Color::rgb(0.0, 1.0 / 255.0, 0.0)
        } else {
            Color::WHITE
        };
        let mut items = Vec::new();
        match &self.kind {
            WeaponKind::Rock => {
                let color = if self.blink > 1 && rng.value() < 0.5 {
                    blink
                } else {
                    black
                };
                items.push(DrawItem::Sprite(Sprite {
                    index: 0,
                    element: id.pebble(),
                    pos: at,
                    rotation,
                    scale: Vec2::new(1.0, 1.0),
                    anchor: Vec2::new(0.5, 0.5),
                    color,
                    shader: Shader::Basic,
                    layer,
                }));
                if self.mode == WeaponMode::Thrown {
                    let tail = self.tail_pos.lerp(c.last_pos, t);
                    let perp = (at - tail).normalized().perpendicular();
                    items.push(DrawItem::Mesh(Mesh {
                        index: 1,
                        vertices: vec![at + perp * 3.0, at - perp * 3.0, tail],
                        uvs: Vec::new(),
                        triangles: Cow::Borrowed(&TRAIL),
                        color: black,
                        vertex_colors: Vec::new(),
                        shader: Shader::Basic,
                        layer,
                    }));
                }
            }
            WeaponKind::Spear(s) => {
                let color = if self.blink > 0 && rng.value() < 0.5 {
                    blink
                } else {
                    black
                };
                let from = if s.last_pivot_at_tip { 0.85 } else { 0.5 };
                let to = if s.pivot_at_tip { 0.85 } else { 0.5 };
                items.push(DrawItem::Sprite(Sprite {
                    index: 0,
                    element: SMALL_SPEAR,
                    pos: at,
                    rotation,
                    scale: Vec2::new(1.0, 1.0),
                    anchor: Vec2::new(0.5, from + (to - from) * t.clamp(0.0, 1.0)),
                    color,
                    shader: Shader::Basic,
                    layer,
                }));
            }
        }
        Some(DrawList { items })
    }
}

#[cfg(test)]
mod tests {
    use rw_random::UnityRandom;
    use rw_sim::creature::EntityId;

    use super::{PEBBLES, PebbleElement};

    #[test]
    fn the_pebble_follows_the_id_and_leaves_the_stream_alone() {
        // `InitState(seed); Range(1, 15)` on a generator of its own.
        let id = EntityId::new(-3, 1042);
        let mut expect = UnityRandom::new(id.random_seed());
        let n = expect.range_i32(1, 15);
        assert_eq!(id.pebble(), PEBBLES[usize::try_from(n - 1).unwrap()]);
        assert_eq!(id.pebble(), format!("Pebble{n}"));
    }
}
