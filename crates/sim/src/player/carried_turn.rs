//! What a creature's vessel carries through the shortcuts: everything stuck to it
//! (`GetAllConnectedObjects`), the held player among it. `AbstractCreature
//! .FollowedByCamera` through what it carries, `AbstractPhysicalObject.Move`'s sticks
//! moving with the vessel out of the lobby, the base `Creature.SpitOutOfShortCut` loop's
//! turn for a carried player (`Player.SpitOutOfShortCut(false)`, `NewRoom`), and the
//! player killed in a den it was dragged into (`AbstractCreature.IsEnteringDen`'s
//! `realizedCreature.Die()`). Behaviour: `rainworld-re/notes/lizard-ai.md` section 10.8.
//!
//! Mirrors: AbstractCreature.cs (FollowedByCamera, IsEnteringDen), AbstractPhysicalObject.cs (Move), Creature.cs (SpitOutOfShortCut), Player.cs (SpitOutOfShortCut, Die), ShortcutHandler.cs (Update)

use std::sync::Arc;

use rw_assets::Room;

use crate::Vec2;
use crate::creature::fly::FlyRoomTick;
use crate::creature::{CreatureId, WorldCoordinate};
use crate::game::{PlayerObserver, RainWorldGame};
use crate::item::Entity;

impl RainWorldGame {
    /// `FollowedByCamera(0)`: camera 0 follows the player, so a creature is followed
    /// when it is the player or carries it (any of its connected objects).
    pub(crate) fn followed_by_camera(&self, id: CreatureId) -> bool {
        self.player.is_some()
            && (id == self.player_id
                || self
                    .world
                    .connected_objects(Entity::Creature(id))
                    .contains(&Entity::Creature(self.player_id)))
    }

    /// The stick loop of `AbstractPhysicalObject.Move(coord)` after creature `id` moved:
    /// every object connected to it that is not already at `coord`'s room and node
    /// moves there too (the player with what it carries).
    pub(crate) fn move_stuck_objects(&mut self, id: CreatureId, coord: WorldCoordinate) {
        for e in self.world.connected_objects(Entity::Creature(id)) {
            if e == Entity::Creature(id) || self.world.entity_pos(e).same_room_and_node(coord) {
                continue;
            }
            match e {
                Entity::Creature(o) if o == self.player_id => self.set_pos(coord),
                Entity::Creature(o) => self.world.move_creature(o, coord),
                Entity::Object(o) => self.world.move_object(o, coord),
            }
        }
    }

    /// The carried player's turn in the base `Creature.SpitOutOfShortCut` loop of the
    /// creature whose vessel reached entrance `pos` of room `room`: its chunks at the
    /// entrance (a draw each), `AddObject` (the end of the update list),
    /// `Player.SpitOutOfShortCut(false)` (the hole positions, `cantBeGrabbedCounter`,
    /// `ReportRoomVisited`, a shelter discovered) and `NewRoom`, the graphics reset by
    /// both.
    pub(crate) fn carried_player_spat_out(
        &mut self,
        room: i32,
        pos: (i32, i32),
        map_room: &Room,
        observer: &mut dyn PlayerObserver,
    ) {
        let Some(player) = self.player.as_mut() else {
            return;
        };
        player.spit_out_of_shortcut(map_room, pos, &mut self.rng);
        self.player_seq = self.world.next_update_seq();
        self.report_room_visited(room);
        if let Some(r) = self.world.abstract_room(room).filter(|r| r.shelter()) {
            self.entered_shelters.push(r.name.clone());
        }
        if let Some(player) = self.player.as_ref() {
            observer.spat_out(player, &mut self.rng);
        }
    }

    /// A fly stuck to a creature spat out of a shortcut (the base `SpitOutOfShortCut`
    /// loop): its chunks at the entrance's `middle` (a draw each), added to `room`,
    /// `Fly.SpitOutOfShortCut` out of the `hole`, and `NewRoom` when it comes from
    /// elsewhere.
    pub(crate) fn stuck_fly_spat_out(
        &mut self,
        id: CreatureId,
        room: i32,
        middle: Vec2,
        hole: Vec2,
        observer: &mut dyn PlayerObserver,
    ) {
        let mut cx = FlyRoomTick {
            rng: &mut self.rng,
            observer: &mut *observer,
            camera_room: self.camera_room,
        };
        for c in &mut self.world.fly_at(id).body.chunks {
            let p = middle + Vec2::random_unit(cx.rng);
            c.hard_set_position(p);
            c.vel = Vec2::ZERO;
        }
        let came_from = self.world.fly_ref(id).room;
        self.world.fly_add_object(id, room, &mut cx);
        self.world.fly_spat_out(id, middle, hole, &mut cx);
        if came_from != Some(room) {
            self.world.fly_new_room(id, room, &mut cx);
        }
    }

    /// `AbstractCreature.IsEnteringDen` of the creature carrying the player into a den in
    /// room `room` killed the abstract player (`abstract_die`, the base `Creature.Die`'s
    /// world part): `Player.Die` of the body out in the tubes, `GameOver(null)` and
    /// `UI_Slugcat_Die` at its main chunk in that room (played at once: the body no
    /// longer updates).
    pub(crate) fn player_killed_in_den(&mut self, room: i32) {
        let in_den = self.world.creature(self.player_id).in_den;
        let Some(player) = self.player.as_mut().filter(|p| !p.is_dead()) else {
            return;
        };
        if !in_den || player.carried_by().is_none() {
            return;
        }
        player.die(&mut self.rng);
        let sounds = if room == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        player.flush_sounds(&mut self.rng, sounds);
        log::info!("the player was dragged into a den and died");
        let Some(map_room) = self.world.realized(room).map(|r| Arc::clone(&r.room)) else {
            return;
        };
        self.game_over_on_death(&map_room);
        self.apply_player_releases();
    }
}

#[cfg(test)]
#[path = "tests/carried_turn.rs"]
mod tests;
