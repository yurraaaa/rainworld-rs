//! The slugcat's hands in the game: the bat-fly catch in `Player.Update` (87506) with
//! `Player.Collide` (88193), `GrabUpdate` (91722: eating, swallowing, picking up,
//! dropping, throwing, switching hands), `GraphicsModuleUpdated` (87786) moving what the
//! hands hold, the grasps `Stun` and `Die` drop, the food the player eats, the objects
//! carried through shortcuts, and the `WaterDrip`s in the rooms' update lists. Base game,
//! Remix off: the objects a room holds are flies, overseers, rocks and spears. Picking
//! items up, laying them down and swallowing them are in `game_items.rs`.
//!
//! Mirrors: Player.cs (Update, Collide, GrabUpdate, GraphicsModuleUpdated, JumpOnChunk), WaterDrip.cs

use std::sync::Arc;

use rw_assets::{Room, SlugcatName};

use super::items::Grabability;
use super::{HudEvent, PlayerObserver, RainWorldGame, RoomObject};
use crate::creature::fly::MovementMode;
use crate::creature::overseer::EdibleView;
use crate::creature::{
    CreatureId, EntityId, GrabRequest, RealizedCreature, Shareability, WorldCoordinate,
};
use crate::item::Entity;
use crate::player::{HeavyCarry, JumpChunkBody};
use crate::{Animation, CosmeticKind, FloatExt, Player, SoundId, Vec2, Water, WaterDrip};

impl RainWorldGame {
    /// `game.evenUpdate`: toggled before the rooms update, false at construction, so true
    /// on the first tick (INFERRED: the port's ticks are the game's `Update` calls).
    #[must_use]
    pub fn even_update(&self) -> bool {
        self.tick.is_multiple_of(2)
    }

    /// `playerState.foodInStomach`.
    #[must_use]
    pub fn food_in_stomach(&self) -> u32 {
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

    /// `MaxFoodInStomach` (no Red's illness in the base game's story sessions).
    #[must_use]
    pub fn max_food(&self) -> u32 {
        self.campaign.stats.max_food.unsigned_abs()
    }

    /// Sets the food in the player's stomach and clears the quarter points (a dev tool).
    pub fn set_food_in_stomach(&mut self, food: u32) {
        let max = self.max_food();
        let state = &mut self.world.creature_mut(self.player_id).state;
        state.food_in_stomach = food.min(max);
        state.quarter_food_points = 0;
    }

    /// `Player.AddFood` (no Red's illness; the base game only adds). A full stomach cures
    /// a malnourished slugcat (`SetMalnourished(false)`); the save state stays
    /// malnourished until the cycle ends.
    pub(crate) fn add_food(&mut self, add: u32) {
        let max = self.max_food();
        let state = &mut self.world.creature_mut(self.player_id).state;
        let add = add.min(max.saturating_sub(state.food_in_stomach));
        state.food_in_stomach += add;
        if state.food_in_stomach >= max {
            state.quarter_food_points = 0;
        }
        self.session_food += add.cast_signed();
        let malnourished = self.player.as_ref().is_some_and(Player::malnourished);
        if malnourished && self.food_in_stomach() >= max {
            self.set_malnourished(false);
        }
    }

    /// `Player.SetMalnourished(m)`: the session's `characterStats` made anew and the
    /// player's chunk masses from them.
    fn set_malnourished(&mut self, m: bool) {
        log::info!(
            "{}",
            if m {
                "NOW MALNOURISHED"
            } else {
                "NO LONGER MALNOURISHED"
            }
        );
        self.character_stats = if m {
            self.campaign.stats.malnourished()
        } else {
            self.campaign.stats
        };
        if let Some(p) = self.player.as_mut() {
            p.set_stats(self.character_stats);
            self.world.player_mass = p.total_mass();
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
    pub(super) fn fly_nourishment(&self) -> i32 {
        if self.campaign.name == SlugcatName::Red {
            1
        } else {
            4
        }
    }

    /// The player's two grasps: what each holds.
    #[must_use]
    pub fn player_grasps(&self) -> [Option<Entity>; 2] {
        let grasps = &self.world.creature(self.player_id).grasps;
        [0, 1].map(|i| grasps.get(i).copied().flatten().map(|g| g.grabbed))
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
                    && self.grabability(Entity::Creature(fly)) == Grabability::OneHand
                    && self.world.creature(fly).grabbed_by.is_empty()
                {
                    self.player_collide_fly(index, fly, observer);
                    break;
                }
            }
        }
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
        if let Some(stun) = p.collide_with_creature() {
            let f = self.world.fly_at(fly);
            f.stun = f.stun.max(stun);
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
            || self.grabability(Entity::Creature(fly)) != Grabability::OneHand
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
        self.slugcat_grab(Entity::Creature(fly), k, observer);
    }

    /// `room.PlaySound(id, pos)` in room `index` (silent off camera).
    pub(super) fn play_in_room(&mut self, index: i32, id: SoundId, pos: Vec2) {
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        sounds.play_at(id, pos, 1.0, 1.0, &mut self.rng);
    }

    /// `room.PlaySound(id, chunk)` on an object's chunk at `pos` in room `index`.
    pub(super) fn play_on_object(&mut self, index: i32, id: SoundId, pos: Vec2) {
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        sounds.play_on_object(id, pos, false, 1.0, 1.0, &mut self.rng);
    }

    /// `room.PlaySound(id, mainBodyChunk)` of the player in room `index`.
    pub(super) fn play_on_player(&mut self, index: i32, id: SoundId) {
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        sounds.play_on_chunk(id, 0, false, 1.0, 1.0, &mut self.rng);
    }

    /// `Player.SlugcatGrab`: an edible's pacifying grab first, then the regular one
    /// (which, for a fly, finds the same object held and regrabs it through
    /// `Player.ReleaseGrasp`, whose `Slugcat_Lose_Grasp` has no sounds.txt line) by the
    /// chunk nearest the main chunk for a dragged object, else chunk 0. Not a cicada or
    /// a jet fish, so the regular grab pacifies. `Fly.Grabbed` brings its sprites to the
    /// front.
    pub(super) fn slugcat_grab(
        &mut self,
        obj: Entity,
        grasp: usize,
        observer: &mut dyn PlayerObserver,
    ) {
        let chunk_grabbed = if self.grabability(obj) == Grabability::Drag {
            self.nearest_chunk_to_main(obj)
        } else {
            0
        };
        let request = GrabRequest {
            grabber: self.player_id,
            grabbed: obj,
            grasp_used: grasp,
            chunk_grabbed: 0,
            shareability: Shareability::CanOnlyShareWithNonExclusive,
            dominance: 0.5,
            override_equally_dominant: false,
            pacifying: true,
        };
        if self.world.player_edible(obj) && self.world.grab(request, &mut self.rng) {
            self.grabbed_by_player(obj, observer);
        }
        if let Some(p) = self.player.as_mut() {
            let g = p.grab_mut();
            g.switch_hands_counter = 0;
            g.want_to_pick_up = 0;
            g.no_pick_up_on_release = 20;
        }
        let request = GrabRequest {
            chunk_grabbed,
            override_equally_dominant: true,
            ..request
        };
        if self.world.grab(request, &mut self.rng) {
            self.grabbed_by_player(obj, observer);
        }
    }

    /// The index of `obj`'s chunk nearest the player's main chunk (the first of equals).
    pub(super) fn nearest_chunk_to_main(&self, obj: Entity) -> usize {
        let main = self.player_ref().head().pos;
        let mut best = (0, f32::MAX);
        for (i, c) in self
            .world
            .body_chunks(obj)
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            if main.dist_less(c.pos, best.1) {
                best = (i, main.distance(c.pos));
            }
        }
        best.0
    }

    /// `Fly.Grabbed` by the player: `MoveObjectToContainer` of its graphics.
    fn grabbed_by_player(&mut self, obj: Entity, observer: &mut dyn PlayerObserver) {
        if let Entity::Creature(fly) = obj
            && self.world.creature(fly).fly().is_some_and(|f| f.graphics)
        {
            observer.fly_to_front(self.world.creature(fly).id);
        }
    }

    /// `Player.ReleaseGrasp` (its `Slugcat_Lose_Grasp` draws nothing) then the base's.
    pub(super) fn player_release_grasp(&mut self, grasp: usize) {
        self.world.release_grasp(self.player_id, grasp);
    }

    /// `Stun` or `Die` let go of everything (`LoseAllGrasps`), one record per grasp, and
    /// dropped the spear on the back (`DropSpear`).
    pub(super) fn apply_player_releases(&mut self) {
        let Some(p) = self.player.as_mut() else {
            return;
        };
        if p.take_release_grasps() {
            self.release_all_grasps();
        }
        self.apply_back_spear_drop();
    }

    /// Lets go of everything held.
    pub(super) fn release_all_grasps(&mut self) {
        for grasp in 0..2 {
            if self.player_grasps()[grasp].is_none() {
                continue;
            }
            self.player_release_grasp(grasp);
        }
    }

    /// The drips (impacts, skids, corridor shoots) and bubbles the player made this tick,
    /// into its room in the order made.
    pub(super) fn add_player_drips(&mut self, index: i32) {
        let Some(p) = self.player.as_mut() else {
            return;
        };
        let mut bubbles = p.take_bubbles().into_iter().peekable();
        for (i, d) in p.take_drips().into_iter().enumerate() {
            while let Some((_, b)) = bubbles.next_if(|&(before, _)| before <= i) {
                self.world.add_cosmetic(index, CosmeticKind::Bubble(b));
            }
            self.world.add_drip(index, d);
        }
        for (_, b) in bubbles {
            self.world.add_cosmetic(index, CosmeticKind::Bubble(b));
        }
    }

    /// `Player.GrabUpdate` (base game, Remix off), the Hunter's spear on the back first.
    pub(super) fn grab_update(
        &mut self,
        index: i32,
        room: &Arc<Room>,
        water: Option<&Water>,
        observer: &mut dyn PlayerObserver,
    ) {
        let eu = self.even_update();
        self.spear_on_back_update(index, eu, observer);
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
        let mut swallowing = false;
        self.switch_hands(index, input[0].pickup && !input[1].pickup);
        if neutral {
            let grasps = self.player_grasps();
            let edible = (0..2).find(|&i| grasps[i].is_some_and(|e| self.world.player_edible(e)));
            let stomach = self.player_ref().object_in_stomach().is_some();
            let full = self.food_in_stomach() >= self.max_food();
            let can_put = self.can_put_spear_to_back();
            // The first held spear to stash or object to swallow (`num5`, `num8`).
            let (mut to_back, mut swallowable) = (None, None);
            if (edible.is_none() || full) && (!stomach || can_put) {
                for (i, held) in grasps.iter().enumerate() {
                    let Some(e) = *held else { continue };
                    if can_put && self.world.is_spear(e) {
                        to_back = Some(i);
                        break;
                    }
                    if self.world.can_be_swallowed(e) {
                        swallowable = Some(i);
                        break;
                    }
                }
            }
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
            if input[0].pickup {
                if to_back.is_some() || self.can_retrieve_spear_from_back() {
                    self.set_spear_increment(true);
                } else if swallowable.is_some() || stomach {
                    swallowing = true;
                }
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
                observer.player_look_at(grasps[e]);
                eating = true;
                if self.food_in_stomach() < self.max_food() {
                    swallowing = false;
                    self.set_spear_increment(false);
                    if g.eat_counter < 1 {
                        self.player_mut().grab_mut().eat_counter = 15;
                        self.bite_edible_object(index, eu, observer);
                    }
                } else if g.eat_counter < 20 && self.script_tick.hud_with_control_preset {
                    self.hud_events.push(HudEvent::RefuseFood);
                }
            }
        } else if input[0].pickup && !input[1].pickup {
            self.pickup_pressed();
        } else if input[0].pickup {
            let held_spear = self.can_put_spear_to_back()
                && self
                    .player_grasps()
                    .iter()
                    .flatten()
                    .any(|&e| self.world.is_spear(e));
            if held_spear || self.can_retrieve_spear_from_back() {
                self.set_spear_increment(true);
            }
        }
        if self.eat_meat_grab_update(index, input[0].pickup, observer) {
            return;
        }
        let g = self.player_mut().grab_mut();
        if eating && g.eat_counter > 0 {
            g.eat_counter -= 1;
        } else if !eating && g.eat_counter < 40 {
            g.eat_counter += 1;
        }
        if swallowing {
            self.swallow_and_regurgitate_update(index, observer);
        } else {
            self.player_mut().grab_mut().swallow_and_regurgitate_counter = 0;
        }
        for i in 0..2 {
            let slated = self.player_grasps()[i].is_some_and(|e| {
                e.creature()
                    .and_then(|f| self.world.creature(f).fly())
                    .is_some_and(|f| f.slated)
            });
            if slated {
                self.player_release_grasp(i);
            }
        }
        self.update_pickup_candidate(index, room);
        let g = self.player_mut().grab_mut();
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
    }

    pub(super) fn player_ref(&self) -> &Player {
        self.player.as_ref().expect("the player updates")
    }

    pub(super) fn player_mut(&mut self) -> &mut Player {
        self.player.as_mut().expect("the player updates")
    }

    /// `PickupPressed`.
    fn pickup_pressed(&mut self) {
        let g = self.player_mut().grab_mut();
        g.want_to_pick_up = 5;
        g.swallow_and_regurgitate_counter = 0;
    }

    /// The switch-hands press and its progress: a second press within 15 ticks while
    /// holding something (not a two-handed or dragged object in hand 0) starts it;
    /// halfway the grasps swap.
    fn switch_hands(&mut self, index: i32, pressed: bool) {
        let grasps = self.player_grasps();
        let mut holding = grasps.iter().any(Option::is_some);
        if grasps[0].is_some_and(|e| {
            matches!(
                self.grabability(e),
                Grabability::TwoHands | Grabability::Drag
            )
        }) {
            holding = false;
        }
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
    /// its drips, then `Fly.BitByPlayer`).
    fn bite_edible_object(&mut self, index: i32, eu: bool, observer: &mut dyn PlayerObserver) {
        let grasps = self.player_grasps();
        let Some((hand, fly)) = (0..2).find_map(|i| {
            grasps[i]
                .filter(|&e| self.world.player_edible(e))
                .and_then(Entity::creature)
                .map(|f| (i, f))
        }) else {
            return;
        };
        self.world.set_kill_tag(fly, self.player_id);
        let head = observer.player_bite_fly(hand, fly);
        if let Some(head) = head {
            self.bite_fly_drips(index, head);
        }
        self.bit_by_player(index, fly, hand, eu, observer);
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
            self.world.fly_die(fly, &mut self.rng);
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

    /// `SessionRecord.AddEat`.
    pub(super) fn add_eat(&mut self, id: EntityId) {
        if !self.eaten.contains(&id) {
            self.eaten.push(id);
        }
    }

    /// `Player.ObjectEaten` of a fly in a story: look away, the nourishment in whole and
    /// quarter pips, then every grasp on it lets go.
    fn object_eaten(&mut self, fly: CreatureId, observer: &mut dyn PlayerObserver) {
        observer.player_look_at(None);
        let id = self.world.creature(fly).id;
        self.add_eat(id);
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
        self.lock_spear_on_back();
        let holders = self.world.creature(fly).grabbed_by.clone();
        for g in holders.iter().rev() {
            if g.grabber == self.player_id {
                self.player_release_grasp(g.grasp_used);
            } else {
                self.world.release_grasp(g.grabber, g.grasp_used);
            }
        }
    }

    /// The `IPlayerEdible`s of room `index`'s `physicalObjects` (only flies, in creation
    /// order) as the tutorial's Eat instruction reads them (233759, 235452).
    pub(crate) fn room_edibles(&self, index: i32) -> Vec<EdibleView> {
        let mut flies: Vec<(u64, EdibleView)> = self
            .world
            .creatures()
            .filter_map(|(_, c)| {
                let f = c.fly().filter(|f| f.room == Some(index) && !f.slated)?;
                let edible = EdibleView {
                    pos: f.pos(),
                    free: c.grabbed_by.is_empty() && f.mov_mode != Some(MovementMode::Burrow),
                    held_by_player: c
                        .grabbed_by
                        .first()
                        .is_some_and(|g| g.grabber == self.player_id),
                };
                Some((f.update_order, edible))
            })
            .collect();
        flies.sort_unstable_by_key(|&(seq, _)| seq);
        flies.into_iter().map(|(_, e)| e).collect()
    }

    /// `ThrowObject`: `AerobicIncrease(0.75)`, a weapon thrown or anything else tossed,
    /// `dontGrabStuff` 15, a look at it, a carried item forbidden to the player, let go.
    pub(super) fn throw_object(
        &mut self,
        index: i32,
        grasp: usize,
        observer: &mut dyn PlayerObserver,
    ) {
        let Some(obj) = self.player_grasps()[grasp] else {
            return;
        };
        self.player_mut().aerobic_increase(0.75);
        match obj {
            Entity::Object(id) => self.throw_weapon(index, grasp, id, observer),
            Entity::Creature(_) => self.toss_object(index, grasp),
        }
        self.player_mut().grab_mut().dont_grab_stuff = 15;
        observer.player_look_at(Some(obj));
        if let Entity::Object(id) = obj {
            self.world.forbid(id);
        }
        self.player_release_grasp(grasp);
        log::info!("threw a held object");
    }

    /// `TossObject` of a held creature (`Slugcat_Throw_Creature` on the held chunk): a
    /// fly is tossed light along `ThrowDirection`, kept outside the body at head height;
    /// a dragged body only gets a push on the held chunk.
    pub(super) fn toss_object(&mut self, index: i32, grasp: usize) {
        let Some(g) = self.player_grasp(grasp) else {
            return;
        };
        let obj = g.grabbed;
        let pos = self.grabbed_chunk_pos(grasp).unwrap_or_default();
        self.play_on_object(index, SoundId::SlugcatThrowCreature, pos);
        let grabability = self.grabability(obj);
        let heavy = self.heavy_carry(obj);
        let mass = self.world.total_mass(obj);
        let p = self.player_ref();
        let input = p.input_history()[0];
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
        if grabability == Grabability::OneHand {
            power *= 2.0;
            if input.x != 0 && input.y == 0 {
                angle = 70.0;
            }
        }
        let flip_down = p.animation() == Animation::Flip && input.y < 0 && input.x == 0;
        if flip_down {
            angle = 180.0;
            power = 8.0;
        }
        let dir = p.throw_direction();
        let (head, hips) = (p.head().clone(), p.hips().clone());
        let total = p.total_mass();
        let Some(chunks) = self.world.body_chunks_mut(obj) else {
            return;
        };
        let count = chunks.len() as f32;
        for c in chunks.iter_mut() {
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
            for c in chunks.iter_mut() {
                if dir < 0 {
                    c.pos.x = c.pos.x.min(edge - 8.0);
                    c.vel.x = c.vel.x.min(0.0);
                } else {
                    c.pos.x = c.pos.x.max(edge + 8.0);
                    c.vel.x = c.vel.x.max(0.0);
                }
            }
        }
        if !heavy && mass < total * 0.75 {
            for c in chunks.iter_mut() {
                c.pos.y = head.pos.y;
            }
        }
        if grabability == Grabability::Drag {
            let c = &mut chunks[g.chunk_grabbed];
            c.vel += Vec2::from_degrees(angle * dir as f32) * power / c.mass.max(0.5);
            return;
        }
        let carry = mass.lerp_map(0.2, 0.5, 0.6, 0.3);
        let push = (power / (mass.lerp(0.4, 0.2) * count)).clamp(4.0, 14.0);
        let toss = Vec2::from_degrees(angle * dir as f32) * push;
        for c in chunks.iter_mut() {
            if c.vel.y < 0.0 {
                c.vel.y = 0.0;
            }
            c.vel = (c.vel * 0.35).lerp(head.vel, carry);
            c.vel += toss;
        }
    }

    /// `Player.GraphicsModuleUpdated`: what the graphics' hands did to the body, then
    /// each held object: a heavy carry hangs from the main chunk on a rod (`HeavyCarry`
    /// below), anything else follows its hand (`MoveFromOutsideMyUpdate` with the hand's
    /// velocity, a weapon turned the way it is held, `GetHeldItemDirection`) while the
    /// room is viewed, else sits on the head chunk.
    pub(super) fn graphics_module_updated(&mut self, observer: &mut dyn PlayerObserver) {
        let hands = observer.player_hands();
        if let Some(h) = hands {
            self.player_mut().body.chunks[0].vel += h.body_push;
        }
        self.spear_on_back_graphics_updated(hands);
        let grasps = self.player_grasps();
        if grasps.iter().all(Option::is_none) {
            return;
        }
        let eu = self.even_update();
        let p = self.player_ref();
        let (head_pos, head_vel) = (p.head().pos, p.head().vel);
        for (i, held) in grasps.iter().enumerate() {
            let Some(obj) = *held else { continue };
            if self.heavy_carry(obj) {
                self.heavy_carry_rod(i);
                continue;
            }
            if let (Some(h), Entity::Object(id)) = (hands, obj)
                && let Some(overlap) = h.overlap[i]
            {
                self.world.change_overlap(id, overlap);
            }
            let even = self.world.realized_even_update(obj);
            let Some(c) = self.world.chunk_mut(obj, 0) else {
                continue;
            };
            match hands {
                Some(h) => {
                    c.vel = h.hands[i].1;
                    c.move_from_outside_my_update(even, eu, h.hands[i].0);
                    if let Entity::Object(id) = obj {
                        let dir = self.held_item_direction(i, h.spear_dir);
                        self.world.set_held_rotation(id, dir);
                    }
                }
                None => {
                    c.pos = head_pos;
                    c.vel = head_vel;
                }
            }
        }
    }

    /// What hand 0 carries heavily, lent to the player's update in room `index`
    /// ([`HeavyCarry`]): its mass, whether it is dragged, its gravity and chunks.
    pub(super) fn heavy_carry_loan(&self, index: i32) -> Option<HeavyCarry> {
        let g = self.player_grasp(0)?;
        if !self.heavy_carry(g.grabbed) {
            return None;
        }
        Some(HeavyCarry {
            total_mass: self.world.total_mass(g.grabbed),
            drag: self.grabability(g.grabbed) == Grabability::Drag,
            gravity: self.world.object_gravity(g.grabbed),
            grabbed_chunk: g.chunk_grabbed,
            chunks: self.world.body_chunks(g.grabbed)?.to_vec(),
            ai_map: self.world.realized(index).and_then(|r| r.ai_map.clone()),
        })
    }

    /// Hand 0's held chunk (position, radius) when it is a heavy carry.
    pub(super) fn heavy_carry_chunk(&self) -> Option<(Vec2, f32)> {
        let g = self.player_grasp(0)?;
        if !self.heavy_carry(g.grabbed) {
            return None;
        }
        let c = self.world.body_chunks(g.grabbed)?.get(g.chunk_grabbed)?;
        Some((c.pos, c.rad))
    }

    /// The lent object's chunks back from the player's update.
    /// The masses `JumpOnChunk` reads of the player's jump chunk, when its owner is
    /// realized in room `index` (`jumpChunk.owner.room`: a creature sucked into a
    /// shortcut has none).
    pub(super) fn jump_chunk_body(&self, index: i32) -> Option<JumpChunkBody> {
        let c = self.player.as_ref()?.jump_chunk()?;
        let room = match c.owner {
            Entity::Creature(id) => self.world.physical_room(id),
            Entity::Object(_) => Some(self.world.entity_pos(c.owner).room),
        };
        if room != Some(index) {
            return None;
        }
        Some(JumpChunkBody {
            total_mass: self.world.total_mass(c.owner),
            mass: self.world.body_chunks(c.owner)?.get(c.index)?.mass,
        })
    }

    /// `JumpOnChunk`'s push on the other object: the chunk's `vel.y` drop and, for a
    /// creature, `Stun(1)`.
    pub(super) fn apply_jump_chunk_push(&mut self) {
        let Some(push) = self.player.as_mut().and_then(Player::take_jump_chunk_push) else {
            return;
        };
        if let Some(c) = self.world.chunk_mut(push.chunk.owner, push.chunk.index) {
            c.vel.y -= push.vel_y;
        }
        if let Entity::Creature(id) = push.chunk.owner {
            self.creature_stun(id, 1);
        }
    }

    pub(super) fn return_heavy_carry(&mut self) {
        let Some(carry) = self.player.as_mut().and_then(Player::take_heavy_carry) else {
            return;
        };
        let Some(g) = self.player_grasp(0) else {
            return;
        };
        if let Some(chunks) = self.world.body_chunks_mut(g.grabbed) {
            chunks.clone_from_slice(&carry.chunks);
        }
    }

    /// The heavy-carry branch of `GraphicsModuleUpdated` for hand `i`: the held chunk and
    /// the main chunk kept `5 + rad` apart (none while chewing, `eatMeat` 25 and up), the
    /// held chunk moving `1 - share` of the error where `share` is its mass's part of
    /// both chunks' (halved for an object lighter than the slugcat; all of it while the
    /// player enters a shortcut, and then only a stretched rod pulls); on a horizontal
    /// beam the held chunk is held up against 0.75 of its gravity; a dragged object
    /// further than twice the rod plus 30 is let go.
    fn heavy_carry_rod(&mut self, i: usize) {
        let Some(g) = self.player_grasp(i) else {
            return;
        };
        let room = self.pos().room;
        let water = self.world.realized(room).and_then(|r| r.water.as_ref());
        let Some(chunk) = self
            .world
            .body_chunks(g.grabbed)
            .and_then(|c| c.get(g.chunk_grabbed))
        else {
            return;
        };
        let (pos, mass, rad) = (chunk.pos, chunk.mass, chunk.rad);
        let submersion = water.map_or(0.0, |w| w.submersion(chunk));
        let gravity = self.world.object_gravity(g.grabbed);
        let total = self.world.total_mass(g.grabbed);
        let drag = self.grabability(g.grabbed) == Grabability::Drag;
        let p = self.player_ref();
        let entering = p.entering_shortcut().is_some();
        let main = p.head();
        let dir = main.pos.direction_to(pos);
        let dist = main.pos.distance(pos);
        let rod = (5.0 + rad) * (p.grab_state().eat_meat as f32).inverse_lerp(25.0, 15.0);
        let mut share = mass / (main.mass + mass);
        if entering {
            share = 0.0;
        } else if total < p.total_mass() {
            share /= 2.0;
        }
        let beam = p.body_mode() == crate::BodyMode::ClimbingOnBeam
            && !matches!(p.animation(), Animation::BeamTip | Animation::StandOnBeam);
        if !entering || dist > rod {
            let main = &mut self.player_mut().body.chunks[0];
            let push = dir * ((dist - rod) * share);
            main.pos += push;
            main.vel += push;
            if let Some(c) = self.world.chunk_mut(g.grabbed, g.chunk_grabbed) {
                let pull = dir * ((dist - rod) * (1.0 - share));
                c.pos -= pull;
                c.vel -= pull;
            }
        }
        if beam && let Some(c) = self.world.chunk_mut(g.grabbed, g.chunk_grabbed) {
            c.vel.y += gravity * (1.0 - submersion) * 0.75;
        }
        if drag && dist > rod * 2.0 + 30.0 {
            self.player_release_grasp(i);
        }
    }

    /// The carried objects in the entering branch of the player's `Creature.Update`:
    /// pulled toward the tube like the player's own chunks.
    pub(super) fn pull_carried(&mut self) {
        let Some((target, radius)) = self.player.as_ref().and_then(Player::shortcut_pull) else {
            return;
        };
        for e in self.carried() {
            let Some(chunks) = self.world.body_chunks_mut(e) else {
                continue;
            };
            for c in chunks {
                c.vel *= 0.06;
                if c.pos.dist_less(target, radius) {
                    c.pos = target;
                    c.last_pos = target;
                } else {
                    c.last_pos = c.pos;
                    c.pos += c.pos.direction_to(target) * 4.5;
                }
            }
        }
    }

    /// The realized objects stuck to the player (`GetAllConnectedObjects` but itself).
    fn carried(&self) -> Vec<Entity> {
        self.world
            .connected_objects(Entity::Creature(self.player_id))
            .into_iter()
            .filter(|&e| match e {
                Entity::Creature(id) => {
                    let c = self.world.creature(id);
                    c.fly().is_some() || c.lizard().is_some()
                }
                Entity::Object(id) => self.world.object(id).realized.is_some(),
            })
            .collect()
    }

    /// `SuckedIntoShortCut` of the player: every connected object leaves the room with it.
    pub(super) fn carried_sucked_in(&mut self) {
        for e in self.carried() {
            match e {
                Entity::Creature(id) => match &mut self.world.creature_mut(id).realized {
                    Some(RealizedCreature::Fly(f)) => {
                        f.in_shortcut = true;
                        f.room = None;
                    }
                    Some(RealizedCreature::Lizard(l)) => {
                        l.in_shortcut = true;
                        l.leave_room();
                    }
                    _ => {}
                },
                Entity::Object(id) => self.world.object_leaves_room(id),
            }
        }
    }

    /// `AbstractPhysicalObject.Move` through the player's sticks: what it carries moves
    /// to `to` with it.
    pub(super) fn move_carried(&mut self, to: WorldCoordinate) {
        let carried: Vec<Entity> = self
            .world
            .connected_objects(Entity::Creature(self.player_id))
            .into_iter()
            .filter(|&e| e != Entity::Creature(self.player_id))
            .collect();
        for e in carried {
            let from = self.world.entity_pos(e);
            if from.same_room_and_node(to) {
                continue;
            }
            match e {
                Entity::Creature(id) => self.world.move_creature(id, to),
                Entity::Object(id) => self.world.move_object(id, to),
            }
        }
    }

    /// The base `Creature.SpitOutOfShortCut`'s loop for what the player carries: each
    /// object's chunks at the entrance (a draw each), added to the room, a fly spat out
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
        for e in self.carried() {
            match e {
                Entity::Creature(id) if self.world.creature(id).lizard().is_some() => {
                    self.stuck_lizard_spat_out(id, room, middle, entrance, &map_room, observer);
                }
                Entity::Creature(id) => self.stuck_fly_spat_out(id, room, middle, hole, observer),
                Entity::Object(id) => self.world.spit_out_object(id, room, middle, &mut self.rng),
            }
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
    pub(super) fn run_drip(&mut self, index: i32, id: u64, room: &Room) {
        let Some(realized) = self.world.realized(index) else {
            return;
        };
        let Some(pos) = realized.drips.iter().position(|d| d.id == id) else {
            return;
        };
        let glitter = realized.water_glitter;
        let Some(realized) = self.world.realized_mut(index) else {
            return;
        };
        let water = realized.water.as_ref();
        let drip = &mut realized.drips[pos];
        drip.update(room, water, glitter, &mut self.rng);
        let slated = drip.slated;
        if slated && let Some(r) = self.world.realized_mut(index) {
            r.drips.retain(|d| d.id != id);
        }
    }

    /// The room's physical objects in its update list but the player, oldest first.
    pub(super) fn room_physical_objects(&self, index: i32) -> Vec<Entity> {
        let mut objects: Vec<(u64, Entity)> = self
            .world
            .room_overseers(index)
            .into_iter()
            .map(|(seq, id)| (seq, Entity::Creature(id)))
            .collect();
        for (id, c) in self.world.creatures() {
            if let Some(f) = c.fly().filter(|f| f.room == Some(index)) {
                objects.push((f.update_order, Entity::Creature(id)));
            }
        }
        objects.extend(self.world.room_weapon_entities(index));
        objects.sort_unstable_by_key(|&(seq, _)| seq);
        objects.into_iter().map(|(_, id)| id).collect()
    }
}

#[cfg(test)]
#[path = "game_player_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "game_jump_chunk_tests.rs"]
mod jump_chunk_tests;
