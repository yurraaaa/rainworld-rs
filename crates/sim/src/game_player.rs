//! The slugcat's hands in the game: the bat-fly catch in `Player.Update` (87506) with
//! `Player.Collide` (88193), `GrabUpdate` (91722: eating, picking up, dropping,
//! throwing, switching hands), `GraphicsModuleUpdated` (87786) moving what the hands
//! hold, the grasps `Stun` and `Die` drop, the food the player eats, the flies carried
//! through shortcuts, and the `WaterDrip`s in the rooms' update lists. Base game, Remix
//! off: the only objects a room holds are flies and overseers, so only flies are held.

use std::sync::Arc;

use rw_assets::{Room, SlugcatName};

use super::{
    CallExtra, FliesCall, HudEvent, PlayerObserver, RainWorldGame, ReleaseReason, RoomObject,
};
use crate::creature::fly::FlyRoomTick;
use crate::creature::{CreatureId, GrabRequest, Shareability, WorldCoordinate};
use crate::player::ImpactDrips;
use crate::{Animation, FloatExt, Player, SoundId, Vec2, Water, WaterDrip};

/// `Player.ObjectGrabability` (the values a fly or an overseer can have).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Grabability {
    CantGrab,
    OneHand,
}

impl RainWorldGame {
    /// `game.evenUpdate`: toggled before the rooms update, false at construction, so true
    /// on the first tick (INFERRED: the port's ticks are the game's `Update` calls).
    #[must_use]
    pub fn even_update(&self) -> bool {
        self.tick.is_multiple_of(2)
    }

    /// `playerState.foodInStomach`.
    #[must_use]
    pub fn food_in_stomach(&self) -> i32 {
        self.world.creature(self.player_id).state.food_in_stomach
    }

    /// `playerState.quarterFoodPoints`.
    #[must_use]
    pub fn quarter_food_points(&self) -> i32 {
        self.world
            .creature(self.player_id)
            .state
            .quarter_food_points
    }

    /// `MaxFoodInStomach` (no Red's illness in a new game's first cycle).
    #[must_use]
    pub fn max_food(&self) -> i32 {
        self.campaign.stats.max_food
    }

    /// Sets the food in the player's stomach and clears the quarter points (a dev tool).
    pub fn set_food_in_stomach(&mut self, food: i32) {
        let max = self.max_food();
        let state = &mut self.world.creature_mut(self.player_id).state;
        state.food_in_stomach = food.clamp(0, max);
        state.quarter_food_points = 0;
    }

    /// `Player.AddFood` (no Red's illness, never malnourished in a new game).
    fn add_food(&mut self, add: i32) {
        let max = self.max_food();
        let state = &mut self.world.creature_mut(self.player_id).state;
        let add = add.min(max - state.food_in_stomach);
        state.food_in_stomach += add;
        if state.food_in_stomach >= max {
            state.quarter_food_points = 0;
        }
    }

    /// `Player.AddQuarterFood`.
    fn add_quarter_food(&mut self) {
        if self.food_in_stomach() >= self.max_food() {
            return;
        }
        let state = &mut self.world.creature_mut(self.player_id).state;
        state.quarter_food_points += 1;
        if state.quarter_food_points > 3 {
            state.quarter_food_points -= 4;
            self.add_food(1);
        }
    }

    /// `SlugcatStats.NourishmentOfObjectEaten` of a bat fly (`FoodPoints` 1) in quarter
    /// points: the Hunter gets the bare food points, the others four times as many.
    fn fly_nourishment(&self) -> i32 {
        if self.campaign.name == SlugcatName::Red {
            1
        } else {
            4
        }
    }

    /// The player's two grasps: what each holds.
    #[must_use]
    pub fn player_grasps(&self) -> [Option<CreatureId>; 2] {
        let grasps = &self.world.creature(self.player_id).grasps;
        [0, 1].map(|i| grasps.get(i).copied().flatten().map(|g| g.grabbed))
    }

    /// `Player.Grabability` of a creature in the room.
    fn grabability(&self, id: CreatureId) -> Grabability {
        let c = self.world.creature(id);
        match c.fly() {
            Some(f) => {
                let held_by_me = c
                    .grabbed_by
                    .first()
                    .is_some_and(|g| g.grabber == self.player_id);
                if held_by_me || f.shortcut_delay == 0 {
                    Grabability::OneHand
                } else {
                    Grabability::CantGrab
                }
            }
            None => Grabability::CantGrab,
        }
    }

    /// The auto-catch block of `Player.Update` (87506-87517): with a free hand and room
    /// in the stomach, the first fly of the room's `FliesRoomAI` within 20 px (10 in a
    /// narrow space; half that outside an active swarm room in a story) that the player
    /// can grab one-handed and nobody holds is `Collide`d with.
    pub(super) fn player_catch(&mut self, index: i32, observer: &mut dyn PlayerObserver) {
        let Some(flies) = self
            .world
            .realized(index)
            .and_then(|r| r.flies_room_ai.as_ref())
            .map(|a| a.flies.clone())
        else {
            return;
        };
        let before = self.flies_record_start();
        let grasps = self.player_grasps();
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let head = p.head().pos;
        let ready = (grasps[0].is_none() || grasps[1].is_none())
            && p.grab_state().dont_grab_stuff < 1
            && self.food_in_stomach() < self.max_food();
        if ready {
            let num9 = if self.world.single_room() || self.world.active_swarm_room(index) {
                1.0
            } else {
                0.5
            };
            let map = self.world.realized(index).and_then(|r| r.ai_map.clone());
            for fly in flies {
                let Some(f) = self.world.creature(fly).fly() else {
                    continue;
                };
                let (x, y) = f.pos().tile();
                let narrow = map.as_ref().is_some_and(|m| m.narrow_space(x, y));
                let reach = if narrow { 10.0 } else { 20.0 } * num9;
                if self.world.fly_auto_grabable(fly)
                    && head.dist_less(f.pos(), reach)
                    && self.grabability(fly) == Grabability::OneHand
                    && self.world.creature(fly).grabbed_by.is_empty()
                {
                    self.player_collide_fly(index, fly, observer);
                    break;
                }
            }
        }
        let single_room = self.world.single_room();
        self.flies_record_end_extra(
            before,
            FliesCall::Catch,
            CallExtra::Catch {
                single_room_world: single_room,
            },
            observer,
        );
    }

    /// `Player.Collide` with a fly: a belly slide stuns it 2 ticks; a conscious player
    /// with room in the stomach catches it in the first free hand (`Fly_Caught`, the
    /// first catch marking its swarm room for depletion, `SlugcatGrab`).
    fn player_collide_fly(
        &mut self,
        index: i32,
        fly: CreatureId,
        observer: &mut dyn PlayerObserver,
    ) {
        let Some(p) = self.player.as_mut() else {
            return;
        };
        if p.collide_with_creature() {
            let f = self.world.fly_at(fly);
            f.stun = f.stun.max(2);
        }
        let Some(p) = self.player.as_ref() else {
            return;
        };
        if !p.is_conscious() {
            return;
        }
        // The `else if (wantToPickUp > 0 && CanIPickThisUp)` branch cannot follow from the
        // catch, which already asked for room in the stomach and a one-handed grab.
        if self.food_in_stomach() >= self.max_food()
            || self.grabability(fly) != Grabability::OneHand
        {
            return;
        }
        let grasps = self.player_grasps();
        let Some(k) = (0..2).find(|&k| grasps[k].is_none()) else {
            return;
        };
        let f = self.world.fly_ref(fly);
        let id = if f.dead {
            SoundId::FlyCaughtDead
        } else {
            SoundId::FlyCaught
        };
        let pos = f.pos();
        self.play_in_room(index, id, pos);
        if !self.world.fly_ref(fly).ever_been_caught_by_player {
            self.world.fly_at(fly).ever_been_caught_by_player = true;
            if let Some(name) = self.world.deplete_swarm_room(index) {
                log::info!("swarm room {name} is a candidate for depletion");
            }
        }
        self.slugcat_grab(fly, k, observer);
    }

    /// `room.PlaySound(id, pos)` in room `index` (silent off camera).
    fn play_in_room(&mut self, index: i32, id: SoundId, pos: Vec2) {
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        sounds.play_at(id, pos, 1.0, 1.0, &mut self.rng);
    }

    /// `Player.SlugcatGrab` of a fly: an edible's pacifying grab, then the regular one
    /// (which finds the same object held and regrabs it through `Player.ReleaseGrasp`,
    /// whose `Slugcat_Lose_Grasp` has no sounds.txt line). `Fly.Grabbed` brings its
    /// sprites to the front.
    fn slugcat_grab(&mut self, fly: CreatureId, grasp: usize, observer: &mut dyn PlayerObserver) {
        let request = GrabRequest {
            grabber: self.player_id,
            grabbed: fly,
            grasp_used: grasp,
            chunk_grabbed: 0,
            shareability: Shareability::CanOnlyShareWithNonExclusive,
            dominance: 0.5,
            override_equally_dominant: false,
            pacifying: true,
        };
        if self.world.grab(request) {
            self.fly_grabbed_by_player(fly, observer);
        }
        if let Some(p) = self.player.as_mut() {
            let g = p.grab_mut();
            g.switch_hands_counter = 0;
            g.want_to_pick_up = 0;
            g.no_pick_up_on_release = 20;
        }
        // Not a cicada or a jet fish: pacifying.
        let request = GrabRequest {
            override_equally_dominant: true,
            ..request
        };
        if self.world.grab(request) {
            self.fly_grabbed_by_player(fly, observer);
        }
    }

    /// `Fly.Grabbed` by the player: `MoveObjectToContainer` of its graphics.
    fn fly_grabbed_by_player(&mut self, fly: CreatureId, observer: &mut dyn PlayerObserver) {
        if self.world.fly_ref(fly).graphics {
            observer.fly_to_front(self.world.creature(fly).id);
        }
    }

    /// `Player.ReleaseGrasp` (its `Slugcat_Lose_Grasp` draws nothing) then the base's.
    fn player_release_grasp(&mut self, grasp: usize) {
        self.world.release_grasp(self.player_id, grasp);
    }

    /// `Stun` or `Die` let go of everything (`LoseAllGrasps`), one record per grasp.
    pub(super) fn apply_player_releases(&mut self, observer: &mut dyn PlayerObserver) {
        let Some(p) = self.player.as_mut() else {
            return;
        };
        if !p.take_release_grasps() {
            return;
        }
        let reason = if p.is_dead() {
            ReleaseReason::Die
        } else {
            ReleaseReason::Stun
        };
        self.release_all_grasps(reason, observer);
    }

    /// Lets go of everything held, one record per grasp.
    pub(super) fn release_all_grasps(
        &mut self,
        reason: ReleaseReason,
        observer: &mut dyn PlayerObserver,
    ) {
        for grasp in 0..2 {
            if self.player_grasps()[grasp].is_none() {
                continue;
            }
            let before = self.flies_record_start();
            self.player_release_grasp(grasp);
            self.flies_record_end_extra(
                before,
                FliesCall::PlayerRelease,
                CallExtra::PlayerRelease { grasp, reason },
                observer,
            );
        }
    }

    /// The drips the player's terrain impacts threw this tick, into its room.
    pub(super) fn add_player_drips(&mut self, index: i32, observer: &mut dyn PlayerObserver) {
        let Some(p) = self.player.as_mut() else {
            return;
        };
        for ImpactDrips {
            impact,
            pos,
            rad,
            mass,
            ceiling_drips,
            rng,
            drips,
        } in p.take_drips()
        {
            let mut before = self.flies_record_start();
            if let Some(b) = &mut before {
                b.rng_before = rng[0];
            }
            for d in drips {
                self.world.add_drip(index, d);
            }
            self.flies_record_end_extra(
                before,
                FliesCall::ImpactDrips,
                CallExtra::ImpactDrips {
                    room: index,
                    impact,
                    pos,
                    rad,
                    mass,
                    ceiling_drips,
                    rng_after: rng[1],
                },
                observer,
            );
            // The drips were drawn inside the player's update, before what followed it.
            if let Some(last) = self.flies_calls.as_mut().and_then(|c| c.last_mut()) {
                last.rng_after = rng[1];
            }
        }
    }

    /// `Player.GrabUpdate` for a player holding nothing but flies, base game.
    pub(super) fn grab_update(
        &mut self,
        index: i32,
        room: &Arc<Room>,
        water: Option<&Water>,
        observer: &mut dyn PlayerObserver,
    ) {
        let before = self.flies_record_start();
        let eu = self.even_update();
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let input = *p.input_history();
        let neutral = input[0].x == 0
            && input[0].y == 0
            && !input[0].jump
            && !input[0].throw
            && p.submersion(water) < 0.5;
        let mut eating = false;
        let mut head_pos = None;
        self.switch_hands(index, input[0].pickup && !input[1].pickup);
        if neutral {
            let grasps = self.player_grasps();
            let edible = (0..2).find(|&i| grasps[i].is_some());
            let g = *self.player_ref().grab_state();
            if edible.is_some() && g.no_pick_up_on_release < 1 {
                if !input[0].pickup {
                    let held = (1..10).find(|&j| !input[j].pickup).unwrap_or(10);
                    if held > 1 && held < 10 {
                        self.pickup_pressed();
                    }
                }
            } else if input[0].pickup && !input[1].pickup {
                self.pickup_pressed();
            }
            let g = *self.player_ref().grab_state();
            let p = self.player_ref();
            let still = p.head().pos.dist_less(p.head().last_pos, 3.6);
            if let Some(e) = edible
                && g.want_to_pick_up < 1
                && (input[0].pickup || g.eat_counter <= 15)
                && p.is_conscious()
                && still
            {
                let fly = grasps[e].unwrap_or(self.player_id);
                observer.player_look_at(Some(fly));
                eating = true;
                if self.food_in_stomach() < self.max_food() {
                    if g.eat_counter < 1 {
                        self.player_mut().grab_mut().eat_counter = 15;
                        head_pos = self.bite_edible_object(index, eu, observer);
                    }
                } else if g.eat_counter < 20 && self.script_tick.hud_with_control_preset {
                    self.hud_events.push(HudEvent::RefuseFood);
                }
            }
        } else if input[0].pickup && !input[1].pickup {
            self.pickup_pressed();
        }
        let g = self.player_mut().grab_mut();
        if eating && g.eat_counter > 0 {
            g.eat_counter -= 1;
        } else if !eating && g.eat_counter < 40 {
            g.eat_counter += 1;
        }
        // Nothing held can be swallowed: the counter stays at 0.
        g.swallow_and_regurgitate_counter = 0;
        for i in 0..2 {
            let slated = self.player_grasps()[i]
                .is_some_and(|f| self.world.creature(f).fly().is_some_and(|f| f.slated));
            if slated {
                self.player_release_grasp(i);
            }
        }
        let candidate = if self.player_ref().grab_state().dont_grab_stuff < 1 {
            self.pickup_candidate(index, room, 20.0)
        } else {
            None
        };
        let g = self.player_mut().grab_mut();
        g.pick_up_candidate = candidate;
        for counter in [
            &mut g.switch_hands_counter,
            &mut g.want_to_pick_up,
            &mut g.want_to_throw,
            &mut g.no_pick_up_on_release,
        ] {
            if *counter > 0 {
                *counter -= 1;
            }
        }
        if input[0].throw && !input[1].throw {
            g.want_to_throw = 5;
        }
        if g.want_to_throw > 0
            && let Some(i) = (0..2).find(|&i| self.player_grasps()[i].is_some())
        {
            self.throw_object(index, i, observer);
            self.player_mut().grab_mut().want_to_throw = 0;
        }
        if self.player_ref().grab_state().want_to_pick_up > 0 {
            self.drop_or_pick_up(index, &input, observer);
        }
        self.flies_record_end_extra(
            before,
            FliesCall::GrabUpdate,
            CallExtra::GrabUpdate { head_pos },
            observer,
        );
    }

    fn player_ref(&self) -> &Player {
        self.player.as_ref().expect("the player updates")
    }

    fn player_mut(&mut self) -> &mut Player {
        self.player.as_mut().expect("the player updates")
    }

    /// `PickupPressed`.
    fn pickup_pressed(&mut self) {
        let g = self.player_mut().grab_mut();
        g.want_to_pick_up = 5;
        g.swallow_and_regurgitate_counter = 0;
    }

    /// The switch-hands press and its progress: a second press within 15 ticks while
    /// holding something starts it; halfway the grasps swap.
    fn switch_hands(&mut self, index: i32, pressed: bool) {
        let holding = self.player_grasps().iter().any(Option::is_some);
        let head = self.player_ref().head().pos;
        if pressed && self.player_ref().grab_state().switch_hands_process == 0.0 {
            if holding {
                let g = self.player_mut().grab_mut();
                if g.switch_hands_counter == 0 {
                    g.switch_hands_counter = 15;
                } else {
                    self.play_in_room(index, SoundId::SlugcatSwitchHandsInit, head);
                    let g = self.player_mut().grab_mut();
                    g.switch_hands_process = 0.01;
                    g.want_to_pick_up = 0;
                    g.no_pick_up_on_release = 20;
                }
            } else {
                self.player_mut().grab_mut().switch_hands_process = 0.0;
            }
        }
        let g = self.player_mut().grab_mut();
        if g.switch_hands_process > 0.0 {
            let last = g.switch_hands_process;
            g.switch_hands_process += 1.0 / 12.0;
            let now = g.switch_hands_process;
            if last < 0.5 && now >= 0.5 {
                self.play_in_room(index, SoundId::SlugcatSwitchHandsComplete, head);
                self.world.switch_grasps(self.player_id, 0, 1);
            }
            if now >= 1.0 {
                self.player_mut().grab_mut().switch_hands_process = 0.0;
            }
        }
    }

    /// `BiteEdibleObject`: the first held fly is bitten (`BiteFly` on the graphics with
    /// its drips, then `Fly.BitByPlayer`). Returns the graphics' head position.
    fn bite_edible_object(
        &mut self,
        index: i32,
        eu: bool,
        observer: &mut dyn PlayerObserver,
    ) -> Option<Vec2> {
        let grasps = self.player_grasps();
        let (hand, fly) = (0..2).find_map(|i| grasps[i].map(|f| (i, f)))?;
        let head = observer.player_bite_fly(hand, fly);
        if let Some(head) = head {
            self.bite_fly_drips(index, head);
        }
        self.bit_by_player(index, fly, hand, eu, observer);
        head
    }

    /// The drips of `PlayerGraphics.BiteFly`: the loop bound `Random.Range(0, 3)` is
    /// drawn again at every test.
    fn bite_fly_drips(&mut self, index: i32, head: Vec2) {
        let mut i = 0;
        while i < self.rng.range_i32(0, 3) {
            let dir = Vec2::from_degrees(self.rng.value() * 360.0);
            let vel = dir * (self.rng.value() * 4.0);
            let drip = WaterDrip::new(head, vel, false, &mut self.rng);
            self.world.add_drip(index, drip);
            i += 1;
        }
    }

    /// `Fly.BitByPlayer`: a bite less, dead from the first, the bite sound, moved to the
    /// player's main chunk; the last bite is `ObjectEaten`, the grasp let go and the fly
    /// gone three ticks later.
    fn bit_by_player(
        &mut self,
        index: i32,
        fly: CreatureId,
        hand: usize,
        eu: bool,
        observer: &mut dyn PlayerObserver,
    ) {
        self.world.fly_at(fly).bites -= 1;
        if !self.world.fly_ref(fly).dead {
            self.world.fly_die(fly);
        }
        let f = self.world.fly_ref(fly);
        let id = if f.bites == 0 {
            SoundId::SlugcatFinalBiteFly
        } else {
            SoundId::SlugcatBiteFly
        };
        let pos = f.pos();
        self.play_in_room(index, id, pos);
        let to = self.player_ref().head().pos;
        let f = self.world.fly_at(fly);
        let even = f.even_update;
        f.chunk_mut().move_from_outside_my_update(even, eu, to);
        if f.bites < 1 && f.eaten == 0 {
            self.object_eaten(fly, observer);
            self.player_release_grasp(hand);
            self.world.fly_at(fly).eaten = 3;
        }
    }

    /// `Player.ObjectEaten` of a fly in a story: look away, the nourishment in whole and
    /// quarter pips, then every grasp on it lets go.
    fn object_eaten(&mut self, fly: CreatureId, observer: &mut dyn PlayerObserver) {
        observer.player_look_at(None);
        let mut num = self.fly_nourishment();
        while num >= 4 {
            self.add_food(1);
            num -= 4;
        }
        while num > 0 {
            self.add_quarter_food();
            num -= 1;
        }
        log::info!(
            "ate a bat fly: food {} and {} quarters",
            self.food_in_stomach(),
            self.quarter_food_points()
        );
        let holders = self.world.creature(fly).grabbed_by.clone();
        for g in holders.iter().rev() {
            if g.grabber == self.player_id {
                self.player_release_grasp(g.grasp_used);
            } else {
                self.world.release_grasp(g.grabber, g.grasp_used);
            }
        }
    }

    /// `PickupCandidate(favorSpears)`: the nearest object within reach (40 px beyond its
    /// radius, and in sight or within 20) the player can pick up; objects on the facing
    /// side count 10 px nearer. Only flies can be picked up; the room's physical objects
    /// are in creation order.
    fn pickup_candidate(&self, index: i32, room: &Room, _favor_spears: f32) -> Option<CreatureId> {
        let p = self.player_ref();
        let head = p.head().pos;
        let flip = p.flip_direction();
        let held = self.player_grasps();
        let mut flies: Vec<(u64, CreatureId)> = self
            .world
            .creatures()
            .filter_map(|(id, c)| {
                let f = c.fly()?;
                (f.room == Some(index)).then_some((f.update_order, id))
            })
            .collect();
        flies.sort_unstable();
        let mut best = None;
        let mut best_dist = f32::MAX;
        for (_, id) in flies {
            let f = self.world.fly_ref(id);
            let pos = f.pos();
            let rad = f.chunk().rad;
            let in_reach = head.dist_less(pos, rad + 40.0)
                && (head.dist_less(pos, rad + 20.0)
                    || crate::room_ext::RoomExt::visual_contact(room, head, pos));
            let can = self.grabability(id) != Grabability::CantGrab && !held.contains(&Some(id));
            if !(in_reach && can) {
                continue;
            }
            let mut dist = head.distance(pos);
            if (pos.x < head.x) == (flip < 0) {
                dist -= 10.0;
            }
            if dist < best_dist {
                best = Some(id);
                best_dist = dist;
            }
        }
        best
    }

    /// `ThrowObject` of a held fly: `TossObject`, `dontGrabStuff` 15, a look at it, let
    /// go.
    fn throw_object(&mut self, index: i32, grasp: usize, observer: &mut dyn PlayerObserver) {
        let Some(fly) = self.player_grasps()[grasp] else {
            return;
        };
        self.toss_object(index, fly);
        self.player_mut().grab_mut().dont_grab_stuff = 15;
        observer.player_look_at(Some(fly));
        self.player_release_grasp(grasp);
        log::info!("threw a bat fly");
    }

    /// `TossObject` of a fly (`Slugcat_Throw_Creature`): a one-handed light toss along
    /// `ThrowDirection`, kept outside the body at head height.
    fn toss_object(&mut self, index: i32, fly: CreatureId) {
        let pos = self.world.fly_ref(fly).pos();
        self.play_in_room(index, SoundId::SlugcatThrowCreature, pos);
        let p = self.player_ref();
        let input = p.input_history()[0];
        let mass: f32 = self
            .world
            .fly_ref(fly)
            .body
            .chunks
            .iter()
            .map(|c| c.mass)
            .sum();
        let (mut angle, mut power) = (45.0, 4.0);
        if input.x != 0 && input.y == 0 {
            angle = mass.lerp_map(0.2, 0.3, 60.0, 50.0);
            power = mass.lerp_map_pow(0.2, 0.3, 12.5, 8.0, 2.0);
        } else if input.x != 0 && input.y == 1 {
            angle = 25.0;
            power = 9.0;
        } else if input.x == 0 && input.y == 1 {
            angle = 5.0;
            power = 8.0;
        }
        // A fly is one-handed.
        power *= 2.0;
        if input.x != 0 && input.y == 0 {
            angle = 70.0;
        }
        let flip_down = p.animation() == Animation::Flip && input.y < 0 && input.x == 0;
        if flip_down {
            angle = 180.0;
            power = 8.0;
        }
        let dir = p.throw_direction();
        let (head, hips) = (p.head().clone(), p.hips().clone());
        let total = p.total_mass();
        let f = self.world.fly_at(fly);
        let count = f.body.chunks.len() as f32;
        for c in &mut f.body.chunks {
            if flip_down {
                c.go_through_floors = true;
            }
        }
        if mass < total * 2.0 && dir != 0 {
            let edge = if dir < 0 {
                head.pos.x.min(hips.pos.x)
            } else {
                head.pos.x.max(hips.pos.x)
            };
            for c in &mut f.body.chunks {
                if dir < 0 {
                    c.pos.x = c.pos.x.min(edge - 8.0);
                    c.vel.x = c.vel.x.min(0.0);
                } else {
                    c.pos.x = c.pos.x.max(edge + 8.0);
                    c.vel.x = c.vel.x.max(0.0);
                }
            }
        }
        if mass < total * 0.75 {
            for c in &mut f.body.chunks {
                c.pos.y = head.pos.y;
            }
        }
        let carry = mass.lerp_map(0.2, 0.5, 0.6, 0.3);
        let push = (power / (mass.lerp(0.4, 0.2) * count)).clamp(4.0, 14.0);
        let toss = Vec2::from_degrees(angle * dir as f32) * push;
        for c in &mut f.body.chunks {
            if c.vel.y < 0.0 {
                c.vel.y = 0.0;
            }
            c.vel = (c.vel * 0.35).lerp(head.vel, carry);
            c.vel += toss;
        }
    }

    /// The end of `GrabUpdate` while `wantToPickUp` lasts: down held for 5 ticks (10 and
    /// no sideways input in deep water) lays the first held object down; otherwise the
    /// pick-up candidate is picked up in a free hand (a held fly let go first when both
    /// are full).
    fn drop_or_pick_up(
        &mut self,
        index: i32,
        input: &[crate::Input; crate::INPUT_HISTORY],
        observer: &mut dyn PlayerObserver,
    ) {
        let grasps = self.player_grasps();
        let p = self.player_ref();
        let drop = if p.animation() == Animation::DeepSwim {
            grasps.iter().any(Option::is_some) && input[..10].iter().all(|i| i.y <= -1 && i.x == 0)
        } else {
            input[..5].iter().all(|i| i.y <= -1)
        };
        if drop {
            if let Some(i) = (0..2).find(|&i| grasps[i].is_some()) {
                self.player_mut().grab_mut().want_to_pick_up = 0;
                self.release_object(index, i);
            }
            return;
        }
        let Some(candidate) = self.player_ref().grab_state().pick_up_candidate else {
            return;
        };
        if grasps.iter().all(Option::is_some) {
            let fly = grasps
                .iter()
                .position(|g| g.is_some_and(|f| self.world.creature(f).fly().is_some()));
            if let Some(i) = fly {
                self.player_release_grasp(i);
            }
        }
        let grasps = self.player_grasps();
        if let Some(k) = (0..2).find(|&k| grasps[k].is_none()) {
            let pos = self.world.fly_ref(candidate).pos();
            self.play_in_room(index, SoundId::SlugcatPickUpCreature, pos);
            self.slugcat_grab(candidate, k, observer);
            if self.world.fly_ref(candidate).graphics {
                observer.fly_to_front(self.world.creature(candidate).id);
            }
            log::info!("picked up a bat fly");
        }
        self.player_mut().grab_mut().want_to_pick_up = 0;
    }

    /// `ReleaseObject` of a fly: `Slugcat_Lay_Down_Creature`, let go.
    fn release_object(&mut self, index: i32, grasp: usize) {
        let Some(fly) = self.player_grasps()[grasp] else {
            return;
        };
        let pos = self.world.fly_ref(fly).pos();
        self.play_in_room(index, SoundId::SlugcatLayDownCreature, pos);
        self.player_release_grasp(grasp);
    }

    /// `Player.GraphicsModuleUpdated`: each held fly follows its hand (`MoveFromOutside
    /// MyUpdate` with the hand's velocity) while the room is viewed, else sits on the
    /// head chunk.
    pub(super) fn graphics_module_updated(&mut self, observer: &mut dyn PlayerObserver) {
        let grasps = self.player_grasps();
        if grasps.iter().all(Option::is_none) {
            return;
        }
        let before = self.flies_record_start();
        let eu = self.even_update();
        let hands = observer.player_hands();
        let p = self.player_ref();
        let (head_pos, head_vel) = (p.head().pos, p.head().vel);
        for (i, held) in grasps.iter().enumerate() {
            let Some(fly) = *held else { continue };
            let f = self.world.fly_at(fly);
            let even = f.even_update;
            let c = f.chunk_mut();
            match hands {
                Some(h) => {
                    c.vel = h[i].1;
                    c.move_from_outside_my_update(even, eu, h[i].0);
                }
                None => {
                    c.pos = head_pos;
                    c.vel = head_vel;
                }
            }
        }
        self.flies_record_end_extra(
            before,
            FliesCall::ModuleUpdated,
            CallExtra::ModuleUpdated {
                actually_viewed: hands.is_some(),
                hands,
            },
            observer,
        );
    }

    /// The carried objects in the entering branch of the player's `Creature.Update`:
    /// pulled toward the tube like the player's own chunks.
    pub(super) fn pull_carried(&mut self, observer: &mut dyn PlayerObserver) {
        let Some((target, radius)) = self.player.as_ref().and_then(Player::shortcut_pull) else {
            return;
        };
        for id in self.carried() {
            let before = self.flies_record_start();
            for c in &mut self.world.fly_at(id).body.chunks {
                c.vel *= 0.06;
                if c.pos.dist_less(target, radius) {
                    c.pos = target;
                    c.last_pos = target;
                } else {
                    c.last_pos = c.pos;
                    c.pos += c.pos.direction_to(target) * 4.5;
                }
            }
            self.flies_record_end_extra(
                before,
                FliesCall::CarriedPull(id),
                CallExtra::CarriedPull { target, radius },
                observer,
            );
        }
    }

    /// The realized flies stuck to the player (`GetAllConnectedObjects` but itself).
    fn carried(&self) -> Vec<CreatureId> {
        self.world
            .connected_objects(self.player_id)
            .into_iter()
            .filter(|&id| id != self.player_id && self.world.creature(id).fly().is_some())
            .collect()
    }

    /// `SuckedIntoShortCut` of the player: every connected object leaves the room with it.
    pub(super) fn carried_sucked_in(&mut self, observer: &mut dyn PlayerObserver) {
        for id in self.carried() {
            let before = self.flies_record_start();
            let f = self.world.fly_at(id);
            f.in_shortcut = true;
            f.room = None;
            self.flies_record_end_extra(
                before,
                FliesCall::CarriedSuckedIn(id),
                CallExtra::None,
                observer,
            );
        }
    }

    /// `AbstractPhysicalObject.Move` through the player's sticks: what it carries moves
    /// to `to` with it.
    pub(super) fn move_carried(&mut self, to: WorldCoordinate, observer: &mut dyn PlayerObserver) {
        let carried: Vec<CreatureId> = self
            .world
            .connected_objects(self.player_id)
            .into_iter()
            .filter(|&id| id != self.player_id)
            .collect();
        for id in carried {
            let from = self.world.creature(id).pos;
            if from.same_room_and_node(to) {
                continue;
            }
            let before = self.flies_record_start();
            self.world.move_creature(id, to);
            self.flies_record_end_extra(
                before,
                FliesCall::CarriedMove(id),
                CallExtra::CarriedMove { to },
                observer,
            );
        }
    }

    /// The base `Creature.SpitOutOfShortCut`'s loop for what the player carries: each
    /// object's chunks at the entrance (a draw each), added to the room, spat out
    /// itself (`Fly.SpitOutOfShortCut`) and `NewRoom`.
    pub(super) fn spit_out_carried(&mut self, observer: &mut dyn PlayerObserver) {
        // The head stops 2.5 px out of the entrance's middle: still on its tile.
        let Some(entrance) = self.player.as_ref().map(|p| p.head().pos.tile()) else {
            return;
        };
        let room = self.pos().room;
        let Some(map_room) = self.world.realized(room).map(|r| Arc::clone(&r.room)) else {
            return;
        };
        let middle = Vec2::middle_of_tile(entrance);
        let hole = map_room.shortcut_hole_direction(entrance.0, entrance.1);
        let hole = Vec2::new(hole.0 as f32, hole.1 as f32);
        for id in self.carried() {
            let before = self.flies_record_start();
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
            self.flies_record_end_extra(
                before,
                FliesCall::CarriedSpitOut(id),
                CallExtra::CarriedSpitOut {
                    pos: entrance,
                    room,
                },
                observer,
            );
        }
    }

    /// The `WaterDrip`s in room `index`'s update list.
    pub(super) fn room_drip_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world.realized(index).map_or_else(Vec::new, |r| {
            r.drips
                .iter()
                .filter(|d| !d.slated)
                .map(|d| (d.update_order, RoomObject::Drip(d.id)))
                .collect()
        })
    }

    /// A drip's slot in its room's update list; slated, it is cleaned out after.
    pub(super) fn run_drip(
        &mut self,
        index: i32,
        id: u64,
        room: &Room,
        observer: &mut dyn PlayerObserver,
    ) {
        let Some(realized) = self.world.realized(index) else {
            return;
        };
        let Some(pos) = realized.drips.iter().position(|d| d.id == id) else {
            return;
        };
        let glitter = realized.water_glitter;
        let level = realized
            .water
            .as_ref()
            .map_or(f32::MIN, |w| w.level_at(realized.drips[pos].pos.x));
        let drip_before = realized.drips[pos].clone();
        let before = self.flies_record_start();
        let Some(realized) = self.world.realized_mut(index) else {
            return;
        };
        let water = realized.water.as_ref();
        let drip = &mut realized.drips[pos];
        drip.update(room, water, glitter, &mut self.rng);
        let slated = drip.slated;
        self.flies_record_end_extra(
            before,
            FliesCall::Drip(id),
            CallExtra::Drip {
                room: index,
                glitter,
                float_water_level: level,
                before: Box::new(drip_before),
            },
            observer,
        );
        if slated && let Some(r) = self.world.realized_mut(index) {
            r.drips.retain(|d| d.id != id);
        }
    }

    /// The room's physical objects in its update list but the player, oldest first.
    pub(super) fn room_physical_objects(&self, index: i32) -> Vec<CreatureId> {
        let mut objects: Vec<(u64, CreatureId)> = self.world.room_overseers(index);
        for (id, c) in self.world.creatures() {
            if let Some(f) = c.fly().filter(|f| f.room == Some(index)) {
                objects.push((f.update_order, id));
            }
        }
        objects.sort_unstable();
        objects.into_iter().map(|(_, id)| id).collect()
    }
}
