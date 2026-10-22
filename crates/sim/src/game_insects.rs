//! The game's side of the cosmetic insects: the coordinator's `NowViewed` and
//! `NoLongerViewed` when camera 0 changes rooms, each insect's turn in its room's update
//! list, what the insects read of the room's creatures, and the fireflies' lights.
//!
//! The game changes the camera's room (`RoomCamera.ChangeRoom`, from
//! `ApplyPositionChange`) in the camera's update at the head of the tick after
//! `MoveCamera`; the insects are rolled there too, so their draws come before the
//! tick's rooms. A session's first room is shown the same way, on the first tick,
//! before the room has finished loading (`readyForAI` false).
//!
//! Mirrors: RoomCamera.cs (ChangeRoom: the rooms' NowViewed and NoLongerViewed), Room.cs (Update: the insects' turns), FireFly.cs (Update: its light)

use super::{RainWorldGame, RoomObject};
use crate::creature::{CreatureId, RealizedCreature};
use crate::insect::{
    CosmeticInsect, FireFlyLight, InsectBody, InsectCreature, InsectCreatureSlot, InsectCx,
    InsectWater,
};
use crate::{LightSource, LightTie, Vec2};

/// Camera 0 moved between rooms; the insects follow on the next tick's camera update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct InsectViewChange {
    /// The room the camera left (`None` for a session's first room).
    pub from: Option<i32>,
    pub to: i32,
}

impl RainWorldGame {
    /// `RoomCamera.ChangeRoom`'s insect part, pending since the camera moved: the old
    /// room's `NoLongerViewed`, then the new one's `NowViewed` (none for a move within
    /// one room).
    pub(super) fn apply_insect_view_change(&mut self) {
        let Some(change) = self.insect_view.take() else {
            return;
        };
        if change.from == Some(change.to) {
            return;
        }
        if let Some(from) = change.from
            && let Some(c) = self
                .world
                .realized_mut(from)
                .and_then(|r| r.insects.as_mut())
        {
            c.no_longer_viewed();
        }
        self.insects_now_viewed(change.to);
    }

    /// `InsectCoordinator.NowViewed` of room `index` (no ghost presence in the port).
    fn insects_now_viewed(&mut self, index: i32) {
        let creatures = self.insect_creatures(index);
        let region_si = self.insect_region_si();
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let Some(mut coord) = r.insects.take() else {
            return;
        };
        let Some(r) = self.world.realized(index) else {
            return;
        };
        let mut cx = InsectCx {
            room: &r.room,
            ai_map: r.ai_map.as_deref(),
            water: InsectWater::new(r.water.as_ref(), &r.room),
            time_until_rain: self.world.rain_cycle.time_until_rain(),
            cycle_start_up: self.world.rain_cycle.cycle_start_up(),
            viewed: false,
            darkness: 0.0,
            region_si,
            creatures: &creatures,
            rng: &mut self.rng,
        };
        let made = coord.now_viewed(&mut cx);
        let first = coord.insects.len() - made;
        for insect in &mut coord.insects[first..] {
            insect.update_order = self.world.next_update_seq();
        }
        if let Some(r) = self.world.realized_mut(index) {
            r.insects = Some(coord);
        }
    }

    fn insect_region_si(&self) -> bool {
        !self.world.single_room() && self.world.name() == "SI"
    }

    /// The insects in room `index`'s update list.
    pub(super) fn room_insect_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world
            .realized(index)
            .and_then(|r| r.insects.as_ref())
            .map_or_else(Vec::new, |c| {
                c.insects
                    .iter()
                    .filter(|i| !i.base.slated)
                    .map(|i| (i.update_order, RoomObject::Insect(i.update_order)))
                    .collect()
            })
    }

    /// The `Update` of the insect that joined room `index` at `order`, reading the room's
    /// creatures as `creatures` (made at its turn when `None`), then its firefly light's
    /// step; a destroyed insect leaves the room.
    pub(super) fn run_insect(
        &mut self,
        index: i32,
        order: u64,
        creatures: &mut Option<Vec<InsectCreatureSlot>>,
    ) {
        if creatures.is_none() {
            *creatures = Some(self.insect_creatures(index));
        }
        let region_si = self.insect_region_si();
        let viewed = index == self.camera_room;
        let darkness = if viewed { self.camera_darkness } else { 0.0 };
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let Some(mut coord) = r.insects.take() else {
            return;
        };
        let Some(i) = coord.insects.iter().position(|i| i.update_order == order) else {
            if let Some(r) = self.world.realized_mut(index) {
                r.insects = Some(coord);
            }
            return;
        };
        if let Some(r) = self.world.realized(index) {
            let mut cx = InsectCx {
                room: &r.room,
                ai_map: r.ai_map.as_deref(),
                water: InsectWater::new(r.water.as_ref(), &r.room),
                time_until_rain: self.world.rain_cycle.time_until_rain(),
                cycle_start_up: self.world.rain_cycle.cycle_start_up(),
                viewed,
                darkness,
                region_si,
                creatures: creatures.as_deref().unwrap_or(&[]),
                rng: &mut self.rng,
            };
            coord.insects[i].update(&mut cx);
        }
        self.fire_fly_light_step(index, &mut coord.insects[i]);
        if coord.insects[i].base.slated {
            coord.insects.remove(i);
        }
        if let Some(r) = self.world.realized_mut(index) {
            r.insects = Some(coord);
        }
    }

    /// What a firefly's update asked of its `LightSource`: made with `room.AddObject`
    /// (a new place in the update list), set, or destroyed.
    fn fire_fly_light_step(&mut self, index: i32, insect: &mut CosmeticInsect) {
        let order = insect.update_order;
        let InsectBody::FireFly(f) = &mut insect.body else {
            return;
        };
        let Some(step) = f.light_step.take() else {
            return;
        };
        let (pos, alpha, rad) = match step {
            FireFlyLight::Create { pos, alpha, rad } => {
                let id = self.world.next_update_seq();
                let light = LightSource::new(
                    id,
                    index,
                    pos,
                    false,
                    f.color,
                    Some(LightTie::Insect(order)),
                );
                if let Some(r) = self.world.realized_mut(index) {
                    r.light_sources.push(light);
                }
                f.light = Some(id);
                (pos, alpha, rad)
            }
            FireFlyLight::Set { pos, alpha, rad } => (pos, alpha, rad),
            FireFlyLight::Destroy => {
                if let Some(id) = f.light.take()
                    && let Some(l) = self.world.light_source_mut(id)
                {
                    l.slated = true;
                }
                return;
            }
        };
        if let Some(id) = f.light
            && let Some(l) = self.world.light_source_mut(id)
        {
            l.set_pos = Some(pos);
            l.set_alpha = Some(alpha);
            l.set_rad = Some(rad);
        }
    }

    /// Whether the insect a light is tied to has left room `index`'s update list.
    pub(super) fn insect_tie_gone(&self, index: i32, order: u64) -> bool {
        !self
            .world
            .realized(index)
            .and_then(|r| r.insects.as_ref())
            .is_some_and(|c| {
                c.insects
                    .iter()
                    .any(|i| i.update_order == order && !i.base.slated)
            })
    }

    /// `room.abstractRoom.creatures` of room `index` with the bodies the insects read.
    pub(super) fn insect_creatures(&self, index: i32) -> Vec<InsectCreatureSlot> {
        self.world
            .room_creatures(index)
            .iter()
            .map(|&id| InsectCreatureSlot {
                id,
                realized: self.insect_creature(id, index),
            })
            .collect()
    }

    fn insect_creature(&self, id: CreatureId, index: i32) -> Option<InsectCreature> {
        let c = self.world.creature(id);
        let small = c.template.small_creature;
        match c.realized.as_ref()? {
            RealizedCreature::Player => {
                let p = self.player.as_ref()?;
                let chunks: Vec<(Vec2, f32)> =
                    p.body.chunks.iter().map(|ch| (ch.pos, ch.rad)).collect();
                let main = &p.body.chunks[0];
                Some(InsectCreature {
                    id,
                    dead: p.is_dead(),
                    in_room: !p.in_shortcut() && self.pos().room == index,
                    slated: p.is_destroyed(),
                    danger_pos: main.pos,
                    total_mass: self.world.player_mass,
                    main_pos: main.pos,
                    main_last_pos: main.last_pos,
                    chunks,
                    small,
                    malnourished_player: p.malnourished(),
                })
            }
            RealizedCreature::Fly(f) => {
                let main = f.chunk();
                Some(InsectCreature {
                    id,
                    dead: f.dead,
                    in_room: f.room == Some(index) && !f.in_shortcut,
                    slated: f.slated,
                    danger_pos: main.pos,
                    total_mass: f.body.chunks.iter().map(|ch| ch.mass).sum(),
                    main_pos: main.pos,
                    main_last_pos: main.last_pos,
                    chunks: f.body.chunks.iter().map(|ch| (ch.pos, ch.rad)).collect(),
                    small,
                    malnourished_player: false,
                })
            }
            RealizedCreature::Lizard(l) => {
                let main = l.head();
                Some(InsectCreature {
                    id,
                    dead: l.dead,
                    in_room: l.room == Some(index) && !l.in_shortcut,
                    slated: l.slated,
                    danger_pos: main.pos,
                    total_mass: l.total_mass(),
                    main_pos: main.pos,
                    main_last_pos: main.last_pos,
                    chunks: l.chunks().iter().map(|ch| (ch.pos, ch.rad)).collect(),
                    small,
                    malnourished_player: false,
                })
            }
            RealizedCreature::Overseer(b) => {
                let main = b.chunk();
                Some(InsectCreature {
                    id,
                    dead: b.dead,
                    in_room: b.in_room && !b.in_shortcut && c.pos.room == index,
                    slated: b.slated,
                    danger_pos: main.pos,
                    total_mass: main.mass,
                    main_pos: main.pos,
                    main_last_pos: main.last_pos,
                    chunks: vec![(main.pos, main.rad)],
                    small,
                    malnourished_player: false,
                })
            }
        }
    }
}
