//! A realized room's `Water.Update` with the room's physical objects: every body's chunks
//! in `physicalObjects` order (layer 0: flies, overseers, thrown and stuck weapons;
//! layer 1: lizards and the player; layer 2: the other weapons; each layer in joining
//! order), and the splashes and bubbles it makes added to the update list.
//!
//! Mirrors: Water.cs (Update), Room.cs (physicalObjects)

use rw_random::UnityRandom;

use crate::creature::RealizedCreature;
use crate::item::Entity;
use crate::water::waterfall::FallObject;
use crate::world::{RealizedRoom, World};
use crate::{BodyChunk, ChunkOwner, CosmeticKind, Sounds, SurfaceChunk, WaterObject};

/// A physical object of a room: its collision layer, place in that layer, and owner.
pub(crate) type LayerEntry = (i32, u64, ChunkOwner);

impl World {
    /// `Water.Update` of room `index`. `player` is the player's body in this room with
    /// its place in the update list (`None` when it is elsewhere or in a shortcut).
    /// Bodies keep the `splash_stop` the water sets; the splashes and bubbles join the
    /// room.
    pub(crate) fn update_water(
        &mut self,
        index: i32,
        player: Option<(u64, &mut [BodyChunk])>,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) {
        if self.realized(index).is_none_or(|r| r.water.is_none()) {
            return;
        }
        let player_seq = player.as_ref().map(|(seq, _)| *seq);
        let mut copies: Vec<(ChunkOwner, usize, BodyChunk)> = Vec::new();
        for (_, _, owner) in self.physical_objects(index, player_seq) {
            let chunks = match owner {
                ChunkOwner::Player => player.as_ref().map(|(_, c)| &c[..]),
                ChunkOwner::Entity(e) => self.surface_chunks(e),
                ChunkOwner::Segment(s) => self.segment_chunks(s),
            };
            for (i, c) in chunks.into_iter().flatten().enumerate() {
                copies.push((owner, i, c.clone()));
            }
        }
        let (screen_shake, micro_screen_shake) = (
            self.rain_cycle.screen_shake(),
            self.rain_cycle.micro_screen_shake(),
        );
        let Some(RealizedRoom {
            room,
            water: Some(water),
            ..
        }) = self.realized_mut(index)
        else {
            return;
        };
        water.rumble.screen_shake = screen_shake;
        water.rumble.micro_screen_shake = micro_screen_shake;
        let chunks = copies.iter_mut().map(|(owner, index, chunk)| SurfaceChunk {
            owner: *owner,
            index: *index,
            chunk,
        });
        let made = water.update(room, chunks, rng, sounds);
        let mut player = player.map(|(_, c)| c);
        for (owner, i, copy) in copies {
            let chunk = match owner {
                ChunkOwner::Player => player.as_deref_mut().and_then(|c| c.get_mut(i)),
                ChunkOwner::Entity(e) => self.surface_chunks_mut(e).and_then(|c| c.get_mut(i)),
                ChunkOwner::Segment(s) => self.segment_chunks_mut(s).and_then(|c| c.get_mut(i)),
            };
            if let Some(c) = chunk {
                c.splash_stop = copy.splash_stop;
            }
        }
        for o in made {
            let kind = match o {
                WaterObject::Bubble(b) => CosmeticKind::Bubble(b),
                WaterObject::Splash(s) => CosmeticKind::Splash(s),
            };
            self.add_cosmetic(index, kind);
        }
    }

    /// `WaterFall.Update` of the fall at `order` in room `index`. `player` is the player's
    /// body in this room with its place in the update list (`None` when it is elsewhere,
    /// in a shortcut or holding up); the fall's drips and bubbles join the room.
    pub(crate) fn update_waterfall(
        &mut self,
        index: i32,
        order: u64,
        player: Option<(u64, &mut [BodyChunk])>,
        rng: &mut UnityRandom,
    ) {
        let player_seq = player.as_ref().map(|(seq, _)| *seq);
        let mut copies: Vec<(ChunkOwner, usize, BodyChunk)> = Vec::new();
        for (_, _, owner) in self.physical_objects(index, player_seq) {
            let chunks = match owner {
                ChunkOwner::Player => player.as_ref().map(|(_, c)| &c[..]),
                ChunkOwner::Entity(e) => self.surface_chunks(e),
                ChunkOwner::Segment(s) => self.segment_chunks(s),
            };
            for (i, c) in chunks.into_iter().flatten().enumerate() {
                copies.push((owner, i, c.clone()));
            }
        }
        let Some(RealizedRoom {
            water, fixtures, ..
        }) = self.realized_mut(index)
        else {
            return;
        };
        let Some(fall) = fixtures
            .waterfalls
            .iter_mut()
            .find(|w| w.update_order == order)
        else {
            return;
        };
        let made = fall.update(water.as_mut(), rng);
        if let Some(w) = water.as_ref() {
            fall.push_chunks(w, copies.iter_mut().map(|(_, _, c)| c));
        }
        let mut player = player.map(|(_, c)| c);
        for (owner, i, copy) in copies {
            let chunk = match owner {
                ChunkOwner::Player => player.as_deref_mut().and_then(|c| c.get_mut(i)),
                ChunkOwner::Entity(e) => self.surface_chunks_mut(e).and_then(|c| c.get_mut(i)),
                ChunkOwner::Segment(s) => self.segment_chunks_mut(s).and_then(|c| c.get_mut(i)),
            };
            if let Some(c) = chunk {
                c.vel = copy.vel;
            }
        }
        for o in made {
            match o {
                FallObject::Drip(d) => {
                    self.add_drip(index, d);
                }
                FallObject::Bubble(b) => self.add_cosmetic(index, CosmeticKind::Bubble(b)),
            }
        }
    }

    /// Room `index`'s `physicalObjects`, layer by layer in joining order; the player at
    /// `player_seq` when it is here.
    pub(crate) fn physical_objects(&self, index: i32, player_seq: Option<u64>) -> Vec<LayerEntry> {
        let mut list: Vec<LayerEntry> = Vec::new();
        for (id, c) in self.creatures() {
            let owner = ChunkOwner::Entity(Entity::Creature(id));
            if let Some(f) = c.fly().filter(|f| f.room == Some(index)) {
                list.push((0, f.update_order, owner));
            } else if let Some(l) = c.lizard().filter(|l| l.room == Some(index) && !l.slated) {
                list.push((1, l.update_order, owner));
            }
        }
        list.extend(
            self.room_overseers(index)
                .into_iter()
                .map(|(seq, id)| (0, seq, ChunkOwner::Entity(Entity::Creature(id)))),
        );
        if let Some(seq) = player_seq {
            list.push((1, seq, ChunkOwner::Player));
        }
        for (id, o) in self.objects() {
            if let Some(w) = o.realized.as_deref().filter(|w| w.room == index) {
                let owner = ChunkOwner::Entity(Entity::Object(id));
                list.push((w.collision_layer, w.layer_order, owner));
            }
        }
        if let Some(c) = self.realized(index).and_then(|r| r.coral.as_ref()) {
            for (stem, s) in c.stems.iter().enumerate() {
                for (segment, g) in s.segments.iter().enumerate() {
                    let owner = ChunkOwner::Segment(crate::SegmentRef {
                        room: index,
                        stem: stem as u32,
                        segment: segment as u32,
                    });
                    list.push((0, g.update_order, owner));
                }
            }
        }
        list.sort_by_key(|&(layer, seq, _)| (layer, seq));
        list
    }

    /// The chunks of a coral stem's segment.
    pub fn segment_chunks(&self, s: crate::SegmentRef) -> Option<&[BodyChunk]> {
        let stem = self
            .realized(s.room)?
            .coral
            .as_ref()?
            .stems
            .get(s.stem as usize)?;
        Some(&stem.segments.get(s.segment as usize)?.body.chunks)
    }

    pub(crate) fn segment_chunks_mut(&mut self, s: crate::SegmentRef) -> Option<&mut [BodyChunk]> {
        let stem = self
            .realized_mut(s.room)?
            .coral
            .as_mut()?
            .stems
            .get_mut(s.stem as usize)?;
        Some(&mut stem.segments.get_mut(s.segment as usize)?.body.chunks)
    }

    /// The chunks of a realized fly, lizard, overseer or item.
    pub(crate) fn surface_chunks(&self, e: Entity) -> Option<&[BodyChunk]> {
        match e {
            Entity::Creature(id) => match &self.creature(id).realized {
                Some(RealizedCreature::Overseer(b)) => Some(&b.body.chunks[..]),
                _ => self.body_chunks(e),
            },
            Entity::Object(_) => self.body_chunks(e),
        }
    }

    pub(crate) fn surface_chunks_mut(&mut self, e: Entity) -> Option<&mut [BodyChunk]> {
        let overseer = matches!(e, Entity::Creature(id)
            if matches!(self.creature(id).realized, Some(RealizedCreature::Overseer(_))));
        if !overseer {
            return self.body_chunks_mut(e);
        }
        match &mut self.creature_mut(e.creature()?).realized {
            Some(RealizedCreature::Overseer(b)) => Some(&mut b.body.chunks[..]),
            _ => None,
        }
    }
}
