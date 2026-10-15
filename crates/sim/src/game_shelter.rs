//! The game's side of shelters: the `ShelterDoor` `Room.Loaded` adds, its update in the
//! room, the player's sleep and hibernation checks, `FoodInRoom` and `DoorClosed`.

use std::sync::Arc;

use rw_assets::{CreatureType, Room, SlugcatName, Terrain};

use super::RainWorldGame;
use crate::creature::CreatureId;
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
    /// creature but slugcats, and its stats' malnourishment.
    pub(super) fn player_wakes_in_shelter(&mut self, player: &mut Player) {
        player.set_malnourished(self.story.malnourished);
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
        let stats = self.player_stats();
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

    /// The slugcat's `slugcatStats` (its malnourished variant when malnourished).
    fn player_stats(&self) -> crate::campaign::SlugcatStats {
        if self.story.malnourished {
            self.campaign.stats.malnourished()
        } else {
            self.campaign.stats
        }
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

    /// `ShelterDoor.Update` of room `index`'s door: the stun of creatures in a closed or
    /// newly open shelter, `DoorClosed`, the sleep and starvation marks, the close
    /// tiles, and `KillAllHostiles`' check.
    pub(super) fn update_shelter_door(&mut self, index: i32) {
        let Some(k) = self.shelter_doors.iter().position(|d| d.room == index) else {
            return;
        };
        let timer = self.world.rain_cycle.timer;
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let step = self.shelter_doors[k].update(timer, sounds, &mut self.rng);
        if step.stun_creatures {
            self.stun_shelter_creatures(index);
        }
        if step.closed {
            self.door_closed(index);
        }
        let stats = self.player_stats();
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
    /// stunned for 20 ticks (the flies; the overseers have no stun in the port).
    fn stun_shelter_creatures(&mut self, index: i32) {
        let flies: Vec<CreatureId> = self
            .world
            .room_creatures(index)
            .iter()
            .copied()
            .filter(|&c| {
                self.world
                    .creature(c)
                    .fly()
                    .is_some_and(|f| f.room == Some(index))
            })
            .collect();
        for id in flies {
            let f = self.world.fly_at(id);
            f.stun = f.stun.max(20);
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

    /// `KillAllHostiles`: while a hostile creature is alive in the shelter the check
    /// goes on (no hostile creature is realized in the port, so none is killed or moved
    /// back to its den).
    fn kill_all_hostiles(&mut self, index: i32, k: usize) {
        let ancient = self
            .world
            .abstract_room(index)
            .is_some_and(|r| r.ancient_shelter);
        let alive = self.world.room_creatures(index).iter().any(|&c| {
            let c = self.world.creature(c);
            ShelterDoor::hostile_for_shelter(c.kind())
                && ShelterDoor::coord_inside_shelter_range(c.pos.tile, ancient)
                && c.state.alive
        });
        if !alive {
            self.shelter_doors[k].kill_hostiles = false;
        }
    }

    /// `DoorClosed`: `Win(FoodInRoom < foodToHibernate)` when every player lives, else
    /// the death screen.
    fn door_closed(&mut self, index: i32) {
        let alive = self.world.creature(self.player_id).state.alive;
        let end = if alive {
            let food = self.food_in_room_count(index);
            SessionEnd::Win {
                malnourished: food < self.player_stats().food_to_hibernate,
            }
        } else {
            SessionEnd::Death
        };
        log::info!("shelter closed: {end:?}");
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
        let stomach = self.food_in_stomach();
        if self.campaign.name == SlugcatName::Red || stomach >= self.max_food() {
            return stomach;
        }
        let mut food = stomach;
        let mut quarters = 0;
        let grasps = self.player_grasps();
        for (i, grasp) in grasps.iter().enumerate() {
            let Some(fly) = grasp.filter(|&c| self.world.creature(c).fly().is_some()) else {
                continue;
            };
            quarters += self.fly_nourishment();
            while quarters >= 4 {
                food += 1;
                quarters -= 4;
            }
            eaten.push((fly, Some(i)));
            if food >= self.max_food() {
                return food;
            }
        }
        let stats = self.player_stats();
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
