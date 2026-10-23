//! `MiniFly`, the black dash of the `Flies` effect: it wanders toward open air near the
//! floor and now and then buzzes around a corpse or a malnourished slugcat.
//!
//! Mirrors: MiniFly.cs

use super::{FOUR_DIRECTIONS, InsectBase, InsectBehavior, InsectCreature, InsectCx};
use crate::{FloatExt, RoomExt, Vec2};

/// `MiniFly`.
#[derive(Debug, Clone, PartialEq)]
pub struct MiniFly {
    pub dir: Vec2,
    pub last_last_pos: Vec2,
    /// `buzzAroundCorpse`.
    pub buzz: Option<BuzzTarget>,
}

/// The body chunk a mini fly buzzes around.
#[derive(Debug, Clone, PartialEq)]
pub struct BuzzTarget {
    pub creature: InsectCreature,
    pub chunk: usize,
    /// Its creature still has a body (`abstractCreature.realizedCreature`).
    pub realized: bool,
}

impl BuzzTarget {
    /// Reads the creature as the room lists it now: gone from the list, it is in another
    /// room; listed without a body, it was abstracted.
    fn refresh(&mut self, cx: &InsectCx) {
        match cx.creatures.iter().find(|c| c.id == self.creature.id) {
            Some(slot) => match &slot.realized {
                Some(now) => self.creature.clone_from(now),
                None => self.realized = false,
            },
            None => self.creature.in_room = false,
        }
    }

    fn chunk(&self) -> (Vec2, f32) {
        self.creature
            .chunks
            .get(self.chunk)
            .copied()
            .unwrap_or_default()
    }
}

impl MiniFly {
    #[must_use]
    pub fn new(pos: Vec2) -> Self {
        Self {
            dir: Vec2::ZERO,
            last_last_pos: pos,
            buzz: None,
        }
    }

    /// `Update`.
    pub fn update(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        if let Some(t) = &mut self.buzz {
            t.refresh(cx);
        }
        b.vel *= 0.85;
        b.vel += self.dir * 2.4;
        let to = Vec2::from_degrees(cx.value() * 360.0) * cx.value().mathf_pow(0.75);
        let t = cx.value().mathf_pow(1.5);
        self.dir = self.dir.lerp(to, t);
        if b.want_to_burrow {
            self.dir = self.dir.lerp(Vec2::new(0.0, -1.0), 0.1);
        } else if let Some(target) = self.buzz.clone() {
            let viable = target.realized.then_some(&target.creature);
            if !Self::viable_for_buzzaround(viable, b, cx) {
                self.buzz = None;
            } else {
                let (cpos, rad) = target.chunk();
                let above = cpos + Vec2::new(0.0, rad + 30.0);
                let reach = rad + 20.0 + cx.value().mathf_pow(2.0) * 100.0;
                if !b.pos.dist_less(above, reach) {
                    b.vel += b.pos.direction_to(above) * (0.4 * cx.value());
                    let to = b.pos.direction_to(cpos + Vec2::new(0.0, rad));
                    self.dir = self.dir.lerp(to, cx.value().mathf_pow(2.0) * 0.65);
                }
            }
        } else if let Some(s) = b.swarm.filter(|_| b.out_of_bounds()) {
            let t = b.pos.distance(s.center).inverse_lerp(s.rad, s.rad + 100.0);
            self.dir = self.dir.lerp(b.pos.direction_to(s.center), t);
        } else {
            let tile = b.pos.tile();
            let mut best = Self::tile_score(tile, cx);
            let mut step = (0, 0);
            for (dx, dy) in FOUR_DIRECTIONS {
                let far = (tile.0 + dx * 3, tile.1 + dy * 3);
                if !cx.solid_tile((tile.0 + dx, tile.1 + dy)) && Self::tile_score(far, cx) > best {
                    best = Self::tile_score(far, cx);
                    step = (dx, dy);
                }
            }
            b.vel += Vec2::new(step.0 as f32, step.1 as f32) * 0.4;
            if cx.value() < 1.0 / 120.0 && !cx.creatures.is_empty() {
                let i = cx.rng.range_i32(0, cx.creatures.len() as i32) as usize;
                if let Some(c) = cx.creatures[i].realized.clone() {
                    let first = c.chunks.first().map_or(c.main_pos, |&(p, _)| p);
                    if b.pos.dist_less(first, 250.0 + 250.0 * cx.value())
                        && Self::viable_for_buzzaround(Some(&c), b, cx)
                        && cx.room.visual_contact(b.pos, first)
                    {
                        let chunk = cx.rng.range_i32(0, c.chunks.len() as i32) as usize;
                        self.buzz = Some(BuzzTarget {
                            creature: c,
                            chunk,
                            realized: true,
                        });
                    }
                }
            }
        }
        if cx.water.submerged(b.pos) {
            b.pos.y = cx.water.level(b.pos);
        }
        self.last_last_pos = b.last_pos;
        self.base_update(b, cx);
        if !cx.viewed {
            b.destroy();
        }
    }

    /// `ViableForBuzzaround`: a realized creature, then a draw, then dead (or a
    /// malnourished slugcat), a second draw when the fly has a swarm, still here, and
    /// not small.
    fn viable_for_buzzaround(
        c: Option<&InsectCreature>,
        b: &InsectBase,
        cx: &mut InsectCx,
    ) -> bool {
        let Some(c) = c else {
            return false;
        };
        if cx.value() <= 0.000_833_333_35 {
            return false;
        }
        if !(c.dead || c.malnourished_player) {
            return false;
        }
        if let Some(s) = b.swarm {
            // `DistLess(center, center, Rad * (1 + value))`: always true, one draw.
            let _ = s.center.dist_less(s.center, s.rad * (1.0 + cx.value()));
        }
        !c.slated && c.in_room && !c.small
    }

    /// `TileScore`: one draw while `readyForAI` and the tile is in the room.
    fn tile_score(tile: (i32, i32), cx: &mut InsectCx) -> f32 {
        let Some(map) = cx.ai_map else {
            return 0.0;
        };
        if !cx.inside(tile) {
            return 0.0;
        }
        let v = cx.rng.value();
        v / (map.floor_altitude(tile.0, tile.1) - 4).abs() as f32
            / (map.terrain_proximity(tile.0, tile.1) - 4).abs() as f32
    }
}

impl InsectBehavior for MiniFly {
    fn reset(&mut self, b: &mut InsectBase, pos: Vec2, cx: &mut InsectCx) {
        b.reset(pos, cx.rng);
        self.last_last_pos = pos;
    }

    fn emerge_from_ground(&mut self, _b: &mut InsectBase, _pos: Vec2, _cx: &mut InsectCx) {
        self.dir = Vec2::new(0.0, 1.0);
    }

    /// `MiniFly` has no creature avoider and keeps the base `Act`.
    fn act(&mut self, b: &mut InsectBase, cx: &mut InsectCx) {
        b.act(cx);
    }
}
