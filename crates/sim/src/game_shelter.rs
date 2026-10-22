//! The game's side of shelters: the `ShelterDoor` `Room.Loaded` adds, its update in the
//! room, the player's sleep and hibernation checks, `FoodInRoom`, `KillAllHostiles` and
//! `DoorClosed`.
//!
//! Mirrors: ShelterDoor.cs, Room.cs (Loaded), Player.cs (sleep checks)

use std::sync::Arc;

use rw_assets::creatures::CreatureSpawner;
use rw_assets::{CreatureType, Room, SlugcatName, Terrain};

use super::RainWorldGame;
use crate::creature::{CreatureId, WorldCoordinate};
use crate::shelter::{SessionEnd, ShelterDoor};
use crate::{Input, Player, ShelterCheck, TileExt};

impl RainWorldGame {
    /// `Room.Loaded`'s `new ShelterDoor(this)` for a shelter that finished loading (the
    /// ancient More Slugcats shelters are not ported).
    pub(super) fn add_shelter_door(&mut self, index: i32) {
        self.shelter_doors
            .retain(|d| d.room != index && self.world.realized(d.room).is_some());
        let Some(data) = self.world.abstract_room(index) else {
            return;
        };
        if !data.shelter() || data.ancient_shelter {
            return;
        }
        let Some(realized) = self.world.realized(index) else {
            return;
        };
        let slugcat_in_room = self
            .world
            .room_creatures(index)
            .iter()
            .any(|&c| self.world.creature(c).kind() == CreatureType::Slugcat);
        let mut door = ShelterDoor::new(
            index,
            &realized.room,
            &realized.settings,
            self.world.mods(),
            self.world.rain_cycle.timer,
            slugcat_in_room,
            &mut self.rng,
        );
        door.update_order = self.world.next_update_seq();
        self.shelter_doors.push(door);
    }

    /// The door of shelter `room`, while it is realized.
    #[must_use]
    pub fn shelter_door(&self, room: i32) -> Option<&ShelterDoor> {
        self.shelter_doors
            .iter()
            .find(|d| d.room == room && self.world.realized(room).is_some())
    }

    /// The regions whose shelters started closing since the last call (`Close` adds the
    /// world to `miscProgressionData.regionsVisited` for the story's slugcat).
    pub fn take_closed_shelter_regions(&mut self) -> Vec<String> {
        std::mem::take(&mut self.closed_shelter_regions)
    }

    /// The shelters the player was spat out into since the last call
    /// (`progression.TempDiscoverShelter`).
    pub fn take_entered_shelters(&mut self) -> Vec<String> {
        std::mem::take(&mut self.entered_shelters)
    }

    /// The player constructor's `sleepCounter = 100` in a shelter whose room holds no
    /// creature but slugcats.
    pub(super) fn player_wakes_in_shelter(&mut self, player: &mut Player) {
        let room = self.pos().room;
        if !self.world.abstract_room(room).is_some_and(|r| r.shelter()) {
            return;
        }
        let others = self
            .world
            .room_creatures(room)
            .iter()
            .any(|&c| self.world.creature(c).kind() != CreatureType::Slugcat);
        if !others {
            player.start_asleep();
        }
    }

    /// `SleepUpdate` at the start of the player's update in room `index`.
    pub(super) fn player_sleep_update(&mut self, room: &Room, input: Input) {
        let tile = self.pos().tile;
        if let Some(p) = self.player.as_mut() {
            p.sleep_update(room, input, tile, &mut self.rng);
        }
    }

    /// The part of `Player.Update` between the bat-fly catch and the creature update:
    /// `touchedNoInputCounter`, the camera clamp, then the shelter block, which may
    /// close the door.
    pub(super) fn player_shelter_step(&mut self, index: i32, room: &Room) {
        let check = self.shelter_check(index);
        let Some(p) = self.player.as_mut() else {
            return;
        };
        p.count_untouched_input();
        p.clamp_to_cameras(room);
        if p.shelter_check(room, &check) {
            self.close_shelter(index);
        }
    }

    fn shelter_check(&self, index: i32) -> ShelterCheck {
        let stats = self.character_stats();
        let rain = &self.world.rain_cycle;
        let pos = self.pos();
        let entrance = self
            .world
            .realized(index)
            .and_then(|r| r.room.shortcut_map().leading_to_node(0).map(|s| s.start));
        let ancient = self
            .world
            .abstract_room(index)
            .is_some_and(|r| r.ancient_shelter);
        ShelterCheck {
            in_shelter: self.world.story && self.shelter_door(index).is_some(),
            food_in_room: self.food_in_room_count(index),
            food_to_hibernate: stats.food_to_hibernate,
            max_food: stats.max_food,
            malnourished: self.story.malnourished,
            rain_over: rain.timer > rain.cycle_length,
            far_from_entrance: entrance.is_some_and(|e| pos.tile.manhattan(e) > 6),
            inside_range: ShelterDoor::coord_inside_shelter_range(pos.tile, ancient),
        }
    }

    /// `session.characterStats`, the slugcat's `slugcatStats` (its malnourished variant
    /// in a malnourished cycle until a full stomach cures it).
    #[must_use]
    pub fn character_stats(&self) -> crate::campaign::SlugcatStats {
        self.character_stats
    }

    /// `ShelterDoor.Close` of shelter `index`.
    fn close_shelter(&mut self, index: i32) {
        let Some(door) = self.shelter_doors.iter_mut().find(|d| d.room == index) else {
            return;
        };
        if !door.is_closing() {
            log::info!("shelter closing");
            self.closed_shelter_regions
                .push(self.world.name().to_owned());
        }
        door.close();
    }

    /// `ShelterDoor.Update` of room `index`'s door: the AI remap after opening, the stun
    /// of creatures in a closed or newly open shelter, `DoorClosed`, the sleep and
    /// starvation marks, the close tiles, and `KillAllHostiles`' check.
    pub(super) fn update_shelter_door(&mut self, index: i32) {
        let Some(k) = self.shelter_doors.iter().position(|d| d.room == index) else {
            return;
        };
        self.run_shelter_ai_mapper(index, k);
        let timer = self.world.rain_cycle.timer;
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        self.shelter_doors[k].update_loop_emitters(sounds);
        let step = self.shelter_doors[k].update(timer, sounds, &mut self.rng);
        if step.stun_creatures {
            self.stun_shelter_creatures(index);
        }
        if step.closed {
            self.door_closed(index);
        }
        let stats = self.character_stats();
        let player_realized = self.player.is_some();
        if step.sleep_when_still
            && player_realized
            && self.food_in_room_count(index) >= stats.food_to_hibernate
            && let Some(p) = self.player.as_mut()
        {
            p.set_sleep_when_still();
        }
        let needed = if self.story.malnourished {
            stats.max_food
        } else {
            1
        };
        if step.starve_check && player_realized && self.food_in_room_count(index) < needed {
            log::info!("starving");
            self.session_end.get_or_insert(SessionEnd::Starve);
        }
        if let Some(solid) = step.close_tiles {
            self.set_close_tiles(index, k, solid);
        }
        if self.shelter_doors[k].kill_hostiles {
            self.kill_all_hostiles(index, k);
        }
    }

    /// `openTime < 400`: every realized creature but the slugcat and its friends is
    /// stunned for 20 ticks (the flies and the lizards; the overseers have no stun in the
    /// port). A creature with a social memory gets a relationship with the player
    /// (`GetOrInitiateRelationship`); one that likes the player is stunned only while
    /// the door has been open under 120 ticks and is not closing.
    fn stun_shelter_creatures(&mut self, index: i32) {
        let (open_time, closing) = self
            .shelter_doors
            .iter()
            .find(|d| d.room == index)
            .map_or((0, false), |d| (d.open_time, d.is_closing()));
        let player = self.world.creature(self.player_id).id;
        for id in self.world.room_creatures(index).to_vec() {
            let in_room = match &self.world.creature(id).realized {
                Some(crate::creature::RealizedCreature::Fly(f)) => f.room == Some(index),
                Some(crate::creature::RealizedCreature::Lizard(l)) => l.room == Some(index),
                _ => false,
            };
            if !in_room {
                continue;
            }
            let mut friend = false;
            if let Some(m) = &mut self.world.creature_mut(id).state.social_memory {
                let r = m.get_or_initiate(player);
                friend = r.like >= 0.5 || r.temp_like > 0.5;
            }
            if !friend || (open_time < 120 && !closing) {
                if self.world.creature(id).lizard().is_some() {
                    self.lizard_stun(id, 20);
                } else {
                    self.world.stun_creature(id, 20);
                }
            }
        }
    }

    fn set_close_tiles(&mut self, index: i32, k: usize, solid: bool) {
        let tiles = self.shelter_doors[k].close_tiles;
        let terrain = if solid { Terrain::Solid } else { Terrain::Air };
        if let Some(r) = self.world.realized_mut(index) {
            let room = Arc::make_mut(&mut r.room);
            for (x, y) in tiles {
                room.set_terrain(x, y, terrain);
            }
        }
    }

    /// The door's `aiMapper`: 100 steps a tick; once done, in a room ready for AI, the
    /// room's AI map takes its tiles and every path finder in the room resets
    /// (`UpdatePathfindingCreatures`).
    pub(super) fn run_shelter_ai_mapper(&mut self, index: i32, k: usize) {
        let Some(mapper) = self.shelter_doors[k].ai_mapper.as_mut() else {
            return;
        };
        if mapper.done() {
            if !self
                .world
                .realized(index)
                .is_some_and(crate::world::RealizedRoom::ready_for_ai)
            {
                return;
            }
            let map = mapper.take_map();
            self.shelter_doors[k].ai_mapper = None;
            if let (Some(map), Some(r)) = (map, self.world.realized_mut(index)) {
                r.ai_map = Some(Arc::new(map));
            }
            let rng = &mut self.rng;
            self.world
                .with_path_finders(|pfs, w| pfs.reset_room(w, index, rng));
            return;
        }
        let Some(r) = self.world.realized(index) else {
            return;
        };
        let Some(old) = r.ai_map.clone() else {
            return;
        };
        let room = Arc::clone(&r.room);
        mapper.update(100, &old, &room);
    }

    /// `KillAllHostiles`: each hostile creature in the shelter's range dies if realized
    /// (its state first, so `AbstractCreature.Die` adds no respawn) and a big one goes back
    /// to its spawner's den (`Move`); the check goes on while one of them was alive.
    /// Lizards are not hostile: they stay in the shelter. The list is walked by index
    /// as the game does, so the creature after one moved out is skipped this tick.
    fn kill_all_hostiles(&mut self, index: i32, k: usize) {
        let ancient = self
            .world
            .abstract_room(index)
            .is_some_and(|r| r.ancient_shelter);
        let mut alive = false;
        let mut i = 0;
        while let Some(&id) = self.world.room_creatures(index).get(i) {
            i += 1;
            let c = self.world.creature(id);
            if !ShelterDoor::hostile_for_shelter(c.kind())
                || !ShelterDoor::coord_inside_shelter_range(c.pos.tile, ancient)
            {
                continue;
            }
            alive |= c.state.alive;
            if c.is_realized() {
                self.world.creature_mut(id).state.alive = false;
                self.realized_creature_die(id);
            }
            let c = self.world.creature(id);
            if ShelterDoor::big_for_shelter(c.kind()) {
                log::info!("removed giant creature {:?} from shelter", c.id);
                if let Some(den) = self.world.spawner(c.id).map(CreatureSpawner::den) {
                    let den = WorldCoordinate::new(den.room, -1, -1, den.node);
                    self.world.move_creature(id, den);
                }
                // No big creature has a body in the port: nothing to `Destroy`.
            }
        }
        if !alive {
            self.shelter_doors[k].kill_hostiles = false;
            log::info!("killed all hostiles in shelter");
        }
    }

    /// `DoorClosed`: `Win(FoodInRoom < foodToHibernate)` when every player lives, else
    /// the death screen.
    fn door_closed(&mut self, index: i32) {
        let alive = self.world.creature(self.player_id).state.alive;
        let end = if alive {
            let food = self.food_in_room_count(index);
            SessionEnd::Win {
                malnourished: food < self.character_stats().food_to_hibernate,
            }
        } else {
            SessionEnd::Death
        };
        // Called every tick while closed, as in the game; the switch happens once
        // (`end_session` returns while a process is upcoming, as `Win` does).
        self.session_end.get_or_insert(end);
    }

    /// `Player.FoodInRoom(eatAndDestroy)` in the player's room: the food in the stomach
    /// plus the flies held and (counted again, as the game does with Remix off) the
    /// flies in the room that are dead or held by the player. Eating destroys the
    /// counted flies.
    pub fn food_in_room(&mut self, eat_and_destroy: bool) -> i32 {
        let room = self.pos().room;
        self.food_in_room_of(room, eat_and_destroy)
    }

    fn food_in_room_count(&self, room: i32) -> i32 {
        let mut counted = Vec::new();
        self.food_in_room_walk(room, false, &mut counted)
    }

    fn food_in_room_of(&mut self, room: i32, eat_and_destroy: bool) -> i32 {
        let mut eaten = Vec::new();
        let food = self.food_in_room_walk(room, eat_and_destroy, &mut eaten);
        if eat_and_destroy {
            for (fly, grasp) in eaten {
                if let Some(g) = grasp {
                    self.player_release_grasp(g);
                }
                self.world.fly_at(fly).slated = true;
                self.world.remove_entity(fly);
            }
        }
        food
    }

    /// The counting of `FoodInRoom`, listing the flies it counts (with the grasp holding
    /// them) in order. Eating, a held fly is gone before the room's flies are counted.
    fn food_in_room_walk(
        &self,
        room: i32,
        eat: bool,
        eaten: &mut Vec<(CreatureId, Option<usize>)>,
    ) -> i32 {
        let stomach = self.food_in_stomach().cast_signed();
        let max_food = self.max_food().cast_signed();
        if self.campaign.name == SlugcatName::Red || stomach >= max_food {
            return stomach;
        }
        let mut food = stomach;
        let mut quarters = 0;
        let grasps = self.player_grasps();
        for (i, grasp) in grasps.iter().enumerate() {
            let Some(fly) = grasp
                .and_then(crate::item::Entity::creature)
                .filter(|&c| self.world.creature(c).fly().is_some())
            else {
                continue;
            };
            quarters += self.fly_nourishment();
            while quarters >= 4 {
                food += 1;
                quarters -= 4;
            }
            eaten.push((fly, Some(i)));
            if food >= max_food {
                return food;
            }
        }
        let stats = self.character_stats();
        if food >= stats.food_to_hibernate {
            return food;
        }
        for &c in self.world.room_creatures(room).iter().rev() {
            let creature = self.world.creature(c);
            let Some(f) = creature.fly().filter(|f| f.room == Some(room) && !f.slated) else {
                continue;
            };
            let held = creature
                .grabbed_by
                .first()
                .is_some_and(|g| g.grabber == self.player_id);
            if !held && !f.dead {
                continue;
            }
            if eat && eaten.iter().any(|&(e, _)| e == c) {
                continue;
            }
            quarters += self.fly_nourishment();
            while quarters >= 4 {
                food += 1;
                quarters -= 4;
            }
            eaten.push((c, None));
            if food >= stats.food_to_hibernate {
                return food;
            }
        }
        food
    }
}
