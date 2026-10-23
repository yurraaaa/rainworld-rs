//! The slugcat eating a dead creature it holds in hand 0, base game with Remix off:
//! the meat block of `GrabUpdate` (`eatMeat` counting while pick-up is held, the
//! Hunter's back spear locked, the body tossed aside once eaten or the stomach is
//! full, or when pick-up is let go after 60 ticks), `CanEatMeat` and `EatMeatUpdate`
//! (the chewing shake, a bite every 15 ticks after 40 taking one `meatLeft` for one
//! food pip, with drips and `Slugcat_Eat_Meat_A`/`_B`). Only the Hunter eats lizards;
//! the others eat only centipedes. Behaviour: `rainworld-re/notes/items.md` section 14.
//!
//! Mirrors: Player.cs (GrabUpdate, CanEatMeat, EatMeatOmnivoreGreenList, EatMeatUpdate)

use rw_assets::{CreatureType, SlugcatName};

use crate::creature::CreatureId;
use crate::game::{PlayerObserver, RainWorldGame};
use crate::item::Entity;
use crate::{FloatExt, SoundId, Vec2, WaterDrip};

impl RainWorldGame {
    /// `EatMeatOmnivoreGreenList`: the centipedes (any slugcat eats a dead one).
    fn eat_meat_omnivore_green_list(kind: CreatureType) -> bool {
        matches!(
            kind,
            CreatureType::Centipede | CreatureType::Centiwing | CreatureType::RedCentipede
        )
    }

    /// `CanEatMeat`: a dead centipede, or for the Hunter any dead creature that is not
    /// an `IPlayerEdible` (a fly).
    pub(crate) fn can_eat_meat(&self, id: CreatureId) -> bool {
        let c = self.world.creature(id);
        let dead = !c.state.alive || c.lizard().is_some_and(|l| l.dead);
        if Self::eat_meat_omnivore_green_list(c.kind()) && dead {
            return true;
        }
        if self.world.player_edible(Entity::Creature(id)) || !dead {
            return false;
        }
        self.campaign.name == SlugcatName::Red
    }

    /// The meat block of `GrabUpdate` (hand 0 only, Remix off): with pick-up held on
    /// a creature it can eat that has meat, `eatMeat` counts up and `EatMeatUpdate`
    /// runs, the back spear locked; every 80 ticks a body with no meat left (or a full
    /// stomach) is tossed and let go. Let go of pick-up after 60 ticks of it, the body
    /// is tossed too. Otherwise `eatMeat` eases down, capped at 50. Returns whether
    /// `GrabUpdate` returns here.
    pub(crate) fn eat_meat_grab_update(
        &mut self,
        index: i32,
        pickup: bool,
        observer: &mut dyn PlayerObserver,
    ) -> bool {
        let held = self.player_grasp(0);
        let edible = held.and_then(|g| g.grabbed.creature()).is_some_and(|c| {
            self.can_eat_meat(c) && self.world.creature(c).template.meat_points() > 0
        });
        if pickup && edible {
            self.player_mut().grab_mut().eat_meat += 1;
            self.eat_meat_update(index, observer);
            self.set_spear_increment(false);
            self.lock_spear_on_back();
            let eat_meat = self.player_ref().grab_state().eat_meat;
            if let Some(g) = self.player_grasp(0)
                && eat_meat % 80 == 0
            {
                let left = g
                    .grabbed
                    .creature()
                    .map_or(0, |c| self.world.creature(c).state.meat_left);
                if left <= 0 || self.food_in_stomach() >= self.max_food() {
                    self.toss_eaten_body(index);
                }
            }
            return true;
        }
        if !pickup && held.is_some() && self.player_ref().grab_state().eat_meat > 60 {
            self.toss_eaten_body(index);
            return true;
        }
        let g = self.player_mut().grab_mut();
        g.eat_meat = (g.eat_meat - 1).clamp(0, 50);
        false
    }

    /// `eatMeat` and `wantToPickUp` cleared, hand 0's body tossed and let go, the
    /// slugcat standing.
    fn toss_eaten_body(&mut self, index: i32) {
        let g = self.player_mut().grab_mut();
        g.eat_meat = 0;
        g.want_to_pick_up = 0;
        self.toss_object(index, 0);
        self.player_release_grasp(0);
        self.player_mut().set_standing(true);
        log::info!("tossed the eaten body");
    }

    /// `EatMeatUpdate(0)` once `eatMeat` passes 20: down on all fours, blinking, the
    /// main chunk shaken every 5 ticks (a direction draw) and pulled toward the held
    /// chunk's mass centre with its neighbours (the hips pushed away). With graphics,
    /// meat left and room in the stomach: the graphics' head kept at the held chunk's
    /// radius (or shaken every 5 ticks, a draw); past 40 ticks every 15th (at 3) is a
    /// bite: the main chunk jerked (a draw), the held chunk tugged, 1 to 3 drips (a
    /// `Range` draw, then 8 draws and the drip's own each), `meatLeft` down one, one
    /// food pip and `Slugcat_Eat_Meat_B`; the other 15th ticks play `_A`.
    fn eat_meat_update(&mut self, index: i32, observer: &mut dyn PlayerObserver) {
        let Some(g) = self.player_grasp(0) else {
            return;
        };
        let Some(prey) = g.grabbed.creature() else {
            return;
        };
        let eat_meat = self.player_ref().grab_state().eat_meat;
        if eat_meat <= 20 {
            return;
        }
        let p = self.player_mut();
        p.set_standing(false);
        p.blink(5);
        if eat_meat % 5 == 0 {
            let v = Vec2::random_unit(&mut self.rng) * 3.0;
            let main = &mut self.player_mut().body.chunks[0];
            main.pos += v;
            main.vel += v;
        }
        let Some(centre) = self.held_mass_centre(g.grabbed, g.chunk_grabbed) else {
            return;
        };
        let p = self.player_mut();
        let dir = p.body.chunks[0].pos.direction_to(centre);
        p.body.chunks[0].vel += dir * 0.5;
        p.body.chunks[1].vel -= dir * 0.6;
        let meat_left = self.world.creature(prey).state.meat_left;
        if !observer.has_player_graphics()
            || meat_left <= 0
            || self.food_in_stomach() >= self.max_food()
        {
            return;
        }
        let Some(chunk) = self
            .world
            .body_chunks(g.grabbed)
            .and_then(|c| c.get(g.chunk_grabbed))
            .cloned()
        else {
            return;
        };
        let head = observer.player_hands().map_or(Vec2::ZERO, |h| h.head);
        if !chunk.pos.dist_less(head, chunk.rad) {
            let dv = chunk.pos.direction_to(head) * (chunk.rad - chunk.pos.distance(head));
            observer.nudge_player_head(dv);
        } else if eat_meat % 5 == 3 {
            let dv = Vec2::random_unit(&mut self.rng) * 4.0;
            observer.nudge_player_head(dv);
        }
        if eat_meat > 40 && eat_meat % 15 == 3 {
            let jerk = Vec2::from_degrees((-90.0f32).lerp(90.0, self.rng.value())) * 4.0;
            self.player_mut().body.chunks[0].pos += jerk;
            let main = self.player_ref().head().pos;
            if let Some(c) = self.world.chunk_mut(g.grabbed, g.chunk_grabbed) {
                c.vel += centre.direction_to(main) * 0.9 / c.mass;
            }
            self.eat_meat_drips(index, &chunk, centre, (main + head) / 2.0);
            let id = self.world.creature(prey).id;
            self.add_eat(id);
            self.world.creature_mut(prey).state.meat_left -= 1;
            self.add_food(1);
            self.play_on_player(index, SoundId::SlugcatEatMeatB);
            log::info!(
                "bit off meat: {} left, food {}",
                self.world.creature(prey).state.meat_left,
                self.food_in_stomach()
            );
        } else if eat_meat % 15 == 3 {
            self.play_on_player(index, SoundId::SlugcatEatMeatA);
        }
    }

    /// `EatMeatUpdate`'s mass centre of the held chunk and the chunks connected to it
    /// (`bodyChunkConnections`, active or not).
    fn held_mass_centre(&self, e: Entity, held: usize) -> Option<Vec2> {
        let chunks = self.world.body_chunks(e)?;
        let c = chunks.get(held)?;
        let (mut sum, mut mass) = (c.pos * c.mass, c.mass);
        for k in self.world.chunk_connections(e) {
            let other = if k.chunk1 == held {
                k.chunk2
            } else if k.chunk2 == held {
                k.chunk1
            } else {
                continue;
            };
            sum += chunks[other].pos * chunks[other].mass;
            mass += chunks[other].mass;
        }
        Some(sum / mass)
    }

    /// The bite's drips: `Random.Range(0, 3) + 1` of them between the held chunk and
    /// the main chunk, thrown out of the body toward `mouth` (the main chunk and the
    /// graphics' head halfway).
    fn eat_meat_drips(&mut self, index: i32, chunk: &crate::BodyChunk, centre: Vec2, mouth: Vec2) {
        let main = self.player_ref().head().pos;
        let eg = self.player_ref().effective_room_gravity();
        let mut n = self.rng.range_i32(0, 3);
        while n >= 0 {
            let rng = &mut self.rng;
            let pos = chunk.pos.lerp(main, rng.value())
                + Vec2::random_unit(rng) * (chunk.rad * rng.value());
            let out = Vec2::random_unit(rng) * (6.0 * rng.value());
            let toward = centre.direction_to(mouth) * (7.0 * rng.value());
            let side = Vec2::from_degrees((-90.0f32).lerp(90.0, rng.value()));
            let vel = out + toward + side * (rng.value() * eg * 7.0);
            let drip = WaterDrip::new(pos, vel, false, &mut self.rng);
            self.world.add_drip(index, drip);
            n -= 1;
        }
    }
}

#[cfg(test)]
#[path = "tests/eat_meat_turn.rs"]
mod tests;
