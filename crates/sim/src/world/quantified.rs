//! Quantified creatures and a realized room: `Room.PlaceQuantifiedCreaturesInRoom` when
//! the room is ready for AI, and `AbstractRoom.CountQuantifiedCreatures` when it is
//! abstracted again. Base game; the Fly's placement is ported, the other quantified types
//! (leeches, sea leeches, spiders) are not placed yet (docs/DIVERGENCES.md D14).

use rw_assets::CreatureType;

use super::{FliesTick, World};
use crate::FloatExt;
use crate::creature::fly::FlyRoomTick;
use crate::creature::{CreatureId, CreatureTemplate, WorldCoordinate};

/// One placed quantified creature (for traces and tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedQuantified {
    pub creature: CreatureId,
    pub tile: (i32, i32),
    pub node: i32,
}

impl World {
    /// `Room.ReadyForAI`'s quantified part: each quantified type with any counted in
    /// room `room` (the counts as stored: the room is not fully loaded yet) is placed.
    pub fn place_quantified_creatures(
        &mut self,
        room: i32,
        cx: &mut FliesTick<'_>,
    ) -> Vec<PlacedQuantified> {
        let mut placed = Vec::new();
        let mut kinds: Vec<CreatureType> = Vec::new();
        for q in self.room_quantified(room) {
            if !kinds.contains(&q.kind) {
                kinds.push(q.kind);
            }
        }
        kinds.sort_by_key(|&k| k != CreatureType::Fly);
        for kind in kinds {
            let nodes = self.abstract_room(room).map_or(0, |r| r.nodes.len());
            let total: i32 = (0..nodes)
                .map(|n| self.quantified_count(room, kind, i32::try_from(n).unwrap_or(-1)))
                .sum();
            if total <= 0 {
                continue;
            }
            if kind == CreatureType::Fly {
                placed.extend(self.place_flies(room, cx));
            } else {
                log::warn!("placing quantified {kind} is not ported");
            }
        }
        placed
    }

    /// `PlaceQuantifiedCreaturesInRoom(Fly)`: per node (its common index read as the
    /// Fly's own, as the game does), the accessible tiles with an exit distance to it;
    /// a random one per fly there, improved by 20 tries on `PlaceQCScore`; then each a new
    /// `AbstractCreature` in the room, whose `RealizeInRoom` draws its node, realizes the
    /// `Fly` and places it on its tile.
    fn place_flies(&mut self, room: i32, cx: &mut FliesTick<'_>) -> Vec<PlacedQuantified> {
        let mut placed = Vec::new();
        let Some(nodes) = self.abstract_room(room).map(|r| r.nodes.len()) else {
            return placed;
        };
        for node in 0..nodes {
            let node = i32::try_from(node).unwrap_or(-1);
            let count = self.quantified_count(room, CreatureType::Fly, node);
            let Some(map) = self.realized(room).and_then(|r| r.ai_map.as_ref()) else {
                return placed;
            };
            let Some(fly) = map.fly() else { return placed };
            let mut candidates: Vec<(i32, i32)> = fly
                .accessible_tiles()
                .iter()
                .copied()
                .filter(|&(x, y)| fly.distance_to_exit(x, y, node) > 0)
                .collect();
            if candidates.is_empty() {
                candidates.push(self.local_coordinate_of_node(room, node, cx.rng));
            }
            let len = i32::try_from(candidates.len()).unwrap_or(i32::MAX);
            let mut tiles: Vec<(i32, i32)> = (0..count)
                .map(|_| candidates[usize::try_from(cx.rng.range_i32(0, len)).unwrap_or(0)])
                .collect();
            for tile in &mut tiles {
                for _ in 0..20 {
                    let pick = candidates[usize::try_from(cx.rng.range_i32(0, len)).unwrap_or(0)];
                    if self.place_score(room, pick, node) > self.place_score(room, *tile, node) {
                        *tile = pick;
                    }
                }
            }
            for tile in tiles {
                let id = cx.ids.new_id(-1);
                let template = CreatureTemplate::of(CreatureType::Fly, self.mods);
                let c = self.new_creature(
                    template,
                    WorldCoordinate::new(room, tile.0, tile.1, -1),
                    id,
                    cx.session,
                    cx.ids,
                    cx.rng,
                );
                let cid = self.insert_creature(c);
                self.add_entity(cid);
                let node = self.realize_in_room_node(cid, cx);
                self.realize_fly(cid, cx.rng);
                let mut place = FlyRoomTick {
                    rng: &mut *cx.rng,
                    observer: &mut *cx.observer,
                    camera_room: cx.camera_room,
                };
                self.place_fly(cid, room, &mut place);
                placed.push(PlacedQuantified {
                    creature: cid,
                    tile,
                    node,
                });
            }
        }
        placed
    }

    /// `AbstractCreature.RealizeInRoom` up to `Realize`: an undefined node is drawn among
    /// the nodes the creature maps.
    fn realize_in_room_node(&mut self, id: CreatureId, cx: &mut FliesTick<'_>) -> i32 {
        let c = self.creature(id);
        if c.pos.node_defined() {
            return c.pos.node;
        }
        let template = c.template;
        let relevant: Vec<usize> = self
            .abstract_room(c.pos.room)
            .map(|r| {
                r.nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| template.maps_node(n.kind))
                    .map(|(i, _)| i)
                    .collect()
            })
            .unwrap_or_default();
        let specific = cx
            .rng
            .range_i32(0, i32::try_from(relevant.len()).unwrap_or(i32::MAX));
        let node = usize::try_from(specific)
            .ok()
            .and_then(|s| relevant.get(s))
            .map_or(-1, |&n| i32::try_from(n).unwrap_or(-1));
        self.creature_mut(id).pos.node = node;
        node
    }

    /// `PlaceQCScore` for the Fly: visibility, terrain proximity and floor altitude,
    /// divided by every node's exit distance (+ 10) and once more, nearly, by the node's.
    fn place_score(&self, room: i32, (x, y): (i32, i32), node: i32) -> f32 {
        let Some(map) = self.realized(room).and_then(|r| r.ai_map.as_ref()) else {
            return 0.0;
        };
        let Some(fly) = map.fly() else { return 0.0 };
        let mut score = 0.0f32;
        score += map.visibility(x, y) as f32;
        score += map.terrain_proximity(x, y) as f32 * 0.2;
        score += map.floor_altitude(x, y) as f32 * 0.1;
        let nodes = self.abstract_room(room).map_or(0, |r| r.nodes.len());
        for i in 0..nodes {
            let i = i32::try_from(i).unwrap_or(-1);
            score /= (fly.distance_to_exit(x, y, i) + 10) as f32;
        }
        score / ((fly.distance_to_exit(x, y, node) + 10) as f32).lerp(1.0, 0.95)
    }

    /// `AbstractRoom.Abstractize`'s quantified part: `CountQuantifiedCreatures` (every
    /// count reset, then each creature that `Quantify`s counted at its node), and those
    /// creatures removed. Only a creature holding something quantifies, and no
    /// quantified creature holds anything yet; one without a node would draw its node
    /// (`AddQuantifiedCreature(-1)`), which the realizer's call here cannot make.
    pub(crate) fn count_quantified_creatures(&mut self, room: i32) {
        let Some(l) = self.local(room) else { return };
        for q in &mut self.lists[l].quantified {
            q.count = 0;
        }
        let ids: Vec<CreatureId> = self.room_creatures(room).to_vec();
        for &id in &ids {
            let c = self.creature(id);
            if c.quantify() {
                let (node, kind) = (c.pos.node, c.kind());
                if node < 0 {
                    log::warn!("a quantified {kind} without a node is not counted");
                    continue;
                }
                self.add_quantified_at(room, node, kind);
            }
        }
        for &id in ids.iter().rev() {
            if self.creature(id).template.quantified && self.creature(id).quantify() {
                self.creature_mut(id).slated_for_deletion = true;
                self.lists[l].creatures.retain(|&c| c != id);
            }
        }
    }
}
