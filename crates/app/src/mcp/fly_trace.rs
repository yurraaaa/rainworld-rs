//! The realized bat flies over a route through the Outskirts against the independent
//! reference model (`rainworld-re/tools/overseer_model/fly_model.py`, trace format
//! "flies-realized-1"): the tutorial route until the player is in SU_A41 (SU_A42's five
//! flies realized next door, not viewed), then into SU_A42 and about it among the
//! flies, then out through SU_A41 to SU_C04 (SU_A42 abstracted) and back (its flies
//! realized again).

use std::collections::HashMap;

use rw_graphics::FlyGraphics;
use rw_sim::creature::fly::{FlyView, PlayerBody};
use rw_sim::creature::{AbstractCreature, EntityId};
use rw_sim::{
    CallExtra, CreatureVessel, FliesCall, FliesCallRecord, GraphicsCallRecord, PlayerSnapshot,
    ReleaseReason, SoundId, Vec2, WaterDrip,
};
use serde_json::{Value, json};

use super::Session;
use super::params::{InputParam, NewGameParams, SetPlayerParams, StepParams};
use super::tutorial_route::{RouteView, TutorialRoute};

const ROUTE_MAX_TICKS: u64 = 20_000;

/// The sounds a fly, its vessel or the slugcat's hands play.
const FLY_SOUNDS: [SoundId; 20] = [
    SoundId::FlyCaught,
    SoundId::FlyCaughtDead,
    SoundId::SlugcatBiteFly,
    SoundId::SlugcatFinalBiteFly,
    SoundId::SlugcatLoseGrasp,
    SoundId::SlugcatPickUpCreature,
    SoundId::SlugcatThrowCreature,
    SoundId::SlugcatLayDownCreature,
    SoundId::SlugcatSwitchHandsInit,
    SoundId::SlugcatSwitchHandsComplete,
    SoundId::SmallNpcEnterShortcut,
    SoundId::SmallNpcTickAlongInShortcut,
    SoundId::SmallNpcExitShortcut,
    SoundId::BatEmergeFromGrass,
    SoundId::BatDiveIntoGrass,
    SoundId::BatIdleFlyingSounds,
    SoundId::FlyWingFlap,
    SoundId::BatAfraidFlyingSounds,
    SoundId::BatHoverFlyLoop,
    SoundId::BatAttatchToChain,
];

/// The run with every flies' call and the flies' graphics after each.
pub(super) struct FlyTrace {
    pub calls: Vec<FliesCallRecord>,
    pub graphics: Vec<Vec<FlyGraphics>>,
    pub ctor_rng: [[u32; 4]; 2],
    session: Session,
    names: HashMap<i32, String>,
    /// `CeilingDrips` of every room realized during the run, by name.
    ceiling_drips: serde_json::Map<String, Value>,
}

impl FlyTrace {
    /// The recording session of a new game started by `start`.
    fn start(start: impl FnOnce(&mut Session)) -> Option<Self> {
        let mut session = Session::installed()?;
        start(&mut session);
        let game = session.scene_mut()?.game_mut();
        game.record_fly_graphics();
        let sim = game.sim_mut();
        sim.record_flies_calls(true);
        let ctor_rng = sim.flies_ctor_rng();
        Some(Self {
            calls: Vec::new(),
            graphics: Vec::new(),
            ctor_rng,
            session,
            names: HashMap::new(),
            ceiling_drips: serde_json::Map::new(),
        })
    }

    /// The whole tutorial route (food 0) from SU_C04 into the shelter SU_S01; SU_A42's
    /// five flies are realized while the player is in SU_A41, never viewed.
    pub(super) fn tutorial() -> Option<Self> {
        let mut t = Self::new_survivor()?;
        let mut route = TutorialRoute::new();
        let mut state = t.session.state().expect("state");
        while !route.done() && state.tick < ROUTE_MAX_TICKS {
            let input = route.input(&RouteView::of(&state));
            t.step(input);
            state = t.session.state().expect("state");
        }
        assert!(route.done(), "the route reached the shelter");
        Some(t)
    }

    fn new_survivor() -> Option<Self> {
        Self::start(|s| {
            s.new_game(&NewGameParams {
                slugcat: "White".to_owned(),
                seed: Some(1),
            })
            .expect("new game");
        })
    }

    /// A game started in `room` (seed 1).
    pub(super) fn loaded(room: &str) -> Option<Self> {
        Self::start(|s| {
            s.load_room(&super::params::LoadRoomParams {
                name: room.to_owned(),
                seed: Some(1),
                spawn_tile: None,
                slugcat: None,
            })
            .expect("load room");
        })
    }

    /// A game started in `start`, `wait` ticks there, then through the exit into `room`.
    pub(super) fn entered(start: &str, wait: u64, room: &str) -> Option<Self> {
        let mut t = Self::loaded(start)?;
        t.wait(wait);
        assert!(t.go_through(room), "into {room}");
        Some(t)
    }

    /// A game started in SU_A41 (SU_A42's five flies realized next door), then
    /// `a42_ticks` ticks walking about SU_A42 among them, out to SU_C04 (SU_A42
    /// abstracted) and back in.
    pub(super) fn run(a42_ticks: u64) -> Option<Self> {
        let mut t = Self::entered("SU_A41", 200, "SU_A42")?;
        t.wander(a42_ticks);
        assert!(t.go_through("SU_A41"), "back to SU_A41");
        assert!(t.go_through("SU_C04"), "on to SU_C04");
        t.wait(600);
        assert!(t.go_through("SU_A41"), "back to SU_A41 again");
        assert!(t.go_through("SU_A42"), "back into SU_A42");
        t.wait(300);
        Some(t)
    }

    /// A game started in SU_A23, then `ticks` ticks about the swarm room SU_A24 (nine
    /// flies, whose swarm behaviour sends some out through its exits), out through SU_A23,
    /// SU_A22 and SU_A43 to SU_A41 (SU_A24 abstracted) and back in.
    pub(super) fn run_a24(ticks: u64) -> Option<Self> {
        let mut t = Self::entered("SU_A23", 100, "SU_A24")?;
        t.wander(ticks);
        for room in ["SU_A23", "SU_A22", "SU_A43", "SU_A41"] {
            assert!(t.go_through(room), "out to {room}");
        }
        t.wait(600);
        for room in ["SU_A43", "SU_A22", "SU_A23", "SU_A24"] {
            assert!(t.go_through(room), "back to {room}");
        }
        t.wait(300);
        Some(t)
    }

    /// A game loaded in SU_A41 as `slugcat` (seed 1), then into SU_A42, the player put
    /// in the pocket under the flies (tile 37, 7) until it holds one, tapping pick-up
    /// while one is in reach.
    pub(super) fn caught(slugcat: &str) -> Option<Self> {
        let mut t = Self::start(|s| {
            s.load_room(&super::params::LoadRoomParams {
                name: "SU_A41".to_owned(),
                slugcat: Some(slugcat.to_owned()),
                seed: Some(1),
                spawn_tile: None,
            })
            .expect("load room");
        })?;
        t.wait(200);
        assert!(t.go_through("SU_A42"), "into SU_A42");
        t.wait(100);
        for k in 0..3000u64 {
            let state = t.session.state().expect("state");
            let player = state.player.as_ref().expect("player");
            if player.grasps.iter().any(Option::is_some) {
                t.wait(10);
                return Some(t);
            }
            if k % 40 == 0 {
                t.under_a_fly();
            }
            t.step(InputParam {
                pickup: k % 40 < 8 && k % 2 == 1,
                ..InputParam::default()
            });
        }
        None
    }

    /// Puts the player just under the camera room's first fly in open air (a dev move:
    /// the flies keep away from a slugcat).
    fn under_a_fly(&mut self) {
        let Some(pos) = self
            .fly_views()
            .into_iter()
            .map(|(_, pos)| pos)
            .find(|&pos| {
                let tile = pos.tile();
                let open = self.session.tiles(&super::params::TilesParams {
                    rect: Some([tile.0, tile.1 - 2, tile.0, tile.1]),
                });
                open.is_ok_and(|t| !t.lines().skip(2).any(|l| l.contains('#')))
            })
        else {
            return;
        };
        self.session
            .set_player(&SetPlayerParams {
                pos: Some([pos.x, pos.y - 25.0]),
                ..Default::default()
            })
            .expect("set player");
    }

    /// Holds pick-up, standing still, until a fly is eaten (food or a quarter point
    /// more), then lets go and waits 150 ticks for the food meter.
    pub(super) fn eat(&mut self) -> bool {
        let start = self
            .session
            .state()
            .expect("state")
            .player
            .map_or(0, |p| p.food * 4 + p.quarter_food);
        let eaten = self.step_until(
            InputParam {
                pickup: true,
                ..InputParam::default()
            },
            400,
            |s| {
                s.player
                    .as_ref()
                    .is_some_and(|p| p.food * 4 + p.quarter_food > start)
            },
        );
        self.wait(150);
        eaten
    }

    /// Walks along SU_A42's floor to its exit (the shaft at tile 10) and down it, until
    /// the player is out of the tubes in `room`.
    pub(super) fn walk_out(&mut self, room: &str) -> bool {
        for k in 0..3000u64 {
            let state = self.session.state().expect("state");
            let out = state.player.as_ref().is_some_and(|p| !p.in_shortcut);
            if state.player_room == room && state.room == room && out && state.lobby.is_none() {
                self.wait(40);
                return true;
            }
            let Some(input) = Self::walk_out_input(&state, k) else {
                return false;
            };
            self.step(input);
        }
        false
    }

    /// Tick `k` of [`Self::walk_out`].
    pub(super) fn walk_out_input(state: &super::reply::State, k: u64) -> Option<InputParam> {
        let p = state.player.as_ref()?;
        let tx = p.chunks[0].tile[0];
        Some(if p.in_shortcut || state.player_room != "SU_A42" {
            InputParam::default()
        } else if tx > 11 {
            InputParam {
                x: -1,
                jump: k % 40 < 10,
                ..InputParam::default()
            }
        } else if tx < 10 {
            InputParam {
                x: 1,
                ..InputParam::default()
            }
        } else {
            InputParam {
                y: -1,
                ..InputParam::default()
            }
        })
    }

    /// [`Self::eat`], which must succeed.
    pub(super) fn eaten(mut self) -> Self {
        assert!(self.eat(), "the fly is eaten");
        self
    }

    /// A fly caught in SU_A42 as the Survivor, carried down the exit to SU_A41 and
    /// thrown there.
    pub(super) fn carried() -> Option<Self> {
        let mut t = Self::caught("White")?;
        assert!(t.walk_out("SU_A41"), "carried out to SU_A41");
        t.throw();
        Some(t)
    }

    /// Taps throw.
    pub(super) fn throw(&mut self) {
        self.step(InputParam {
            throw: true,
            ..InputParam::default()
        });
        self.wait(80);
    }

    pub(super) fn wait(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.step(InputParam::default());
        }
    }

    /// Walks left and right, jumping now and then.
    fn wander(&mut self, ticks: u64) {
        for k in 0..ticks {
            self.wander_tick(k);
        }
    }

    /// Tick `k` of [`Self::wander`].
    pub(super) fn wander_tick(&mut self, k: u64) {
        let x = if (k / 240).is_multiple_of(2) { 1 } else { -1 };
        self.step(InputParam {
            x,
            y: 0,
            jump: k % 97 < 6,
            map: false,
            pickup: false,
            throw: false,
        });
    }

    /// One tick with `input`, collecting the calls and graphics.
    pub(super) fn step(&mut self, input: InputParam) {
        self.session
            .step(&StepParams {
                ticks: Some(1),
                input: Some(input),
            })
            .expect("steps");
        let Some(game) = self.session.scene_mut().map(|s| s.game_mut()) else {
            return;
        };
        self.graphics.extend(game.take_fly_graphics());
        self.calls.extend(game.sim_mut().take_flies_calls());
        assert_eq!(
            self.calls.len(),
            self.graphics.len(),
            "a graphics snapshot per call"
        );
        self.note_ceiling_drips();
    }

    /// Records the realized rooms' `CeilingDrips` (the flies' impacts read them).
    fn note_ceiling_drips(&mut self) {
        let Some(scene) = self.session.scene_mut() else {
            return;
        };
        let world = scene.game().sim().world();
        let first = world.first_room_index();
        let rooms: Vec<(i32, f32)> = (0..world.number_of_rooms())
            .filter_map(|i| {
                let i = first + i32::try_from(i).ok()?;
                Some((i, world.realized(i)?.settings.ceiling_drips))
            })
            .collect();
        for (i, drips) in rooms {
            let name = self.name(i);
            self.ceiling_drips.insert(name, json!(drips));
        }
    }

    /// Steps with `input` until `done` holds of the state (at most `limit` ticks);
    /// whether it did.
    pub(super) fn step_until(
        &mut self,
        input: InputParam,
        limit: u64,
        done: impl Fn(&super::reply::State) -> bool,
    ) -> bool {
        for _ in 0..limit {
            self.step(input);
            if done(&self.session.state().expect("state")) {
                return true;
            }
        }
        false
    }

    /// Puts the player on the camera room's exit to `room` and holds its input until it
    /// is out of the tubes there.
    fn go_through(&mut self, room: &str) -> bool {
        let exits = self.session.exits().expect("exits");
        let Some(exit) = exits
            .exits
            .iter()
            .find(|e| e.destination.as_deref() == Some(room))
        else {
            return false;
        };
        let (Some(tile), Some([dx, dy])) = (exit.entrance_tile, exit.enter_input) else {
            return false;
        };
        self.session
            .set_player(&SetPlayerParams {
                tile: Some(tile),
                ..Default::default()
            })
            .expect("set player");
        let input = InputParam {
            x: i8::try_from(dx).unwrap_or(0),
            y: i8::try_from(dy).unwrap_or(0),
            jump: false,
            map: false,
            pickup: false,
            throw: false,
        };
        for _ in 0..1200 {
            self.step(input);
            let s = self.session.state().expect("state");
            let out = s.player.as_ref().is_some_and(|p| !p.in_shortcut);
            if s.player_room == room && s.room == room && out && s.lobby.is_none() {
                for _ in 0..20 {
                    self.step(InputParam::default());
                }
                return true;
            }
        }
        false
    }

    fn name(&mut self, room: i32) -> String {
        if let Some(n) = self.names.get(&room) {
            return n.clone();
        }
        let n = self
            .session
            .scene_mut()
            .and_then(|s| {
                s.game()
                    .sim()
                    .world()
                    .abstract_room(room)
                    .map(|r| r.name.clone())
            })
            .unwrap_or_default();
        self.names.insert(room, n.clone());
        n
    }

    fn sounds(&mut self) -> Value {
        let Some(scene) = self.session.scene_mut() else {
            return json!({});
        };
        let table = std::sync::Arc::clone(scene.game_mut().sim_mut().sounds_mut().table());
        let mut out = serde_json::Map::new();
        for id in FLY_SOUNDS {
            let Some(trigger) = table.trigger(id.name()) else {
                continue;
            };
            let samples: Vec<Value> = trigger
                .samples
                .iter()
                .map(|s| {
                    let n = table.clip_at(s.clip).map_or(0, |c| c.variations.len());
                    let exists: Vec<bool> = (0..n)
                        .map(|v| table.clip_at(s.clip).is_some_and(|c| c.variation_exists(v)))
                        .collect();
                    json!(exists)
                })
                .collect();
            out.insert(
                id.name().to_owned(),
                json!({"silent_chance": trigger.silent_chance, "play_all": trigger.play_all,
                       "samples": samples}),
            );
        }
        Value::Object(out)
    }

    fn call_name(&mut self, c: &FliesCallRecord) -> (&'static str, Option<String>, Option<usize>) {
        match c.call {
            FliesCall::Update => ("update", None, None),
            FliesCall::Ready(r) => ("ready", Some(self.name(r)), None),
            FliesCall::Abstractize(r) => ("abstractize", Some(self.name(r)), None),
            FliesCall::Realize { room, id } => ("realize", Some(self.name(room)), Some(id.0)),
            FliesCall::RoomsAi(r) => ("rooms_ai", Some(self.name(r)), None),
            FliesCall::Fly(id) => ("fly", None, Some(id.0)),
            FliesCall::Hover(id) => ("hover", None, Some(id.0)),
            FliesCall::Vessel(id) => ("vessel", None, Some(id.0)),
            FliesCall::Lobby(id) => ("lobby", None, Some(id.0)),
            FliesCall::NowViewed { room, id } => ("now_viewed", Some(self.name(room)), Some(id.0)),
            FliesCall::NoLongerViewed(r) => ("no_longer_viewed", Some(self.name(r)), None),
            FliesCall::Catch => ("catch", None, None),
            FliesCall::GrabUpdate => ("grab_update", None, None),
            FliesCall::Looker => ("looker", None, None),
            FliesCall::Hand(_) => ("hand", None, None),
            FliesCall::ModuleUpdated => ("module_updated", None, None),
            FliesCall::PlayerRelease => ("player_release", None, None),
            FliesCall::Drip(id) => ("drip", None, usize::try_from(id).ok()),
            FliesCall::ImpactDrips => ("impact_drips", None, None),
            FliesCall::CarriedPull(id) => ("carried_pull", None, Some(id.0)),
            FliesCall::CarriedSuckedIn(id) => ("carried_sucked_in", None, Some(id.0)),
            FliesCall::CarriedMove(id) => ("carried_move", None, Some(id.0)),
            FliesCall::CarriedSpitOut(id) => ("carried_spit_out", None, Some(id.0)),
        }
    }

    /// Whether the call reads the player `p` (and reports it after).
    fn reads_player(call: FliesCall) -> bool {
        matches!(
            call,
            FliesCall::Catch
                | FliesCall::GrabUpdate
                | FliesCall::Looker
                | FliesCall::Hand(_)
                | FliesCall::ModuleUpdated
                | FliesCall::PlayerRelease
        )
    }

    /// The player `p` of a record (the state before the call).
    fn player_json(&mut self, p: &PlayerSnapshot, eu: bool) -> Value {
        let room = p.room.map(|r| self.name(r));
        let chunks: Vec<Value> = p
            .chunks
            .iter()
            .map(|c| {
                json!({"pos": Self::vec(c.pos), "last_pos": Self::vec(c.last_pos),
                       "vel": Self::vec(c.vel), "rad": c.rad, "mass": c.mass})
            })
            .collect();
        let input: Vec<Value> = p
            .input
            .iter()
            .map(|i| json!([i.x, i.y, i.jump, i.throw, i.pickup]))
            .collect();
        json!({
            "id": p.id.0, "room": room, "chunks": chunks, "submersion": p.submersion,
            "input": input, "conscious": p.conscious, "dead": p.dead, "stun": p.stun,
            "animation": p.animation.value(), "body_mode": format!("{:?}", p.body_mode),
            "standing": p.standing, "flip_direction": p.flip_direction,
            "roll_direction": p.roll_direction, "roll_counter": p.roll_counter,
            "food": p.food, "quarter_food": p.quarter_food, "max_food": p.max_food,
            "food_to_hibernate": p.food_to_hibernate,
            "grab": Self::grab_json(p),
            "grasps": p.grasps.map(|g| g.map(|id| id.0)),
            "graphics": p.graphics, "hud": p.hud, "eu": eu,
        })
    }

    fn grab_json(p: &PlayerSnapshot) -> Value {
        let g = &p.grab;
        json!({
            "eat_counter": g.eat_counter, "want_to_pick_up": g.want_to_pick_up,
            "no_pick_up_on_release": g.no_pick_up_on_release,
            "dont_grab_stuff": g.dont_grab_stuff,
            "switch_hands_counter": g.switch_hands_counter,
            "switch_hands_process": g.switch_hands_process,
            "want_to_throw": g.want_to_throw,
            "swallow_and_regurgitate_counter": g.swallow_and_regurgitate_counter,
            "pick_up_candidate": g.pick_up_candidate.map(|c| c.0),
        })
    }

    /// `expected.state.player`: the player after the call.
    fn player_after_json(p: &PlayerSnapshot) -> Value {
        let mut v = Self::grab_json(p);
        v["food"] = json!(p.food);
        v["quarter_food"] = json!(p.quarter_food);
        v["grasps"] = json!(p.grasps.map(|g| g.map(|id| id.0)));
        v["roll_counter"] = json!(p.roll_counter);
        v["vel0"] = Self::vec(p.chunks[0].vel);
        v
    }

    fn drip_json(&mut self, d: &WaterDrip) -> Value {
        json!({
            "room": self.name(d.room), "pos": Self::vec(d.pos), "last_pos": Self::vec(d.last_pos),
            "last_last_pos": Self::vec(d.last_last_pos),
            "last_last_last_pos": Self::vec(d.last_last_last_pos), "vel": Self::vec(d.vel),
            "life": d.life, "last_life": d.last_life, "life_time": d.life_time,
            "random_lightness": d.random_lightness,
            "last_random_lightness": d.last_random_lightness, "must_exit": d.must_exit,
            "slated": d.slated,
        })
    }

    /// The fields a call read besides the common ones.
    fn extra_json(&mut self, c: &FliesCallRecord, e: &mut Value) {
        match &c.extra {
            CallExtra::None => {}
            CallExtra::Catch { single_room_world } => {
                e["num9_story"] = json!(true);
                e["single_room_world"] = json!(single_room_world);
            }
            CallExtra::GrabUpdate { head_pos } => e["head_pos"] = Self::opt_vec(*head_pos),
            CallExtra::ModuleUpdated {
                actually_viewed,
                hands,
            } => {
                e["actually_viewed"] = json!(actually_viewed);
                e["hands"] = hands.map_or(Value::Null, |h| {
                    json!(h.map(|(pos, vel)| json!({"pos": Self::vec(pos), "vel": Self::vec(vel)})))
                });
            }
            CallExtra::PlayerRelease { grasp, reason } => {
                e["grasp"] = json!(grasp);
                e["reason"] = json!(match reason {
                    ReleaseReason::Stun => "stun",
                    ReleaseReason::Die => "die",
                    ReleaseReason::Teleport => "teleport",
                });
            }
            CallExtra::Drip {
                room,
                glitter,
                float_water_level,
                before,
            } => {
                e["room"] = json!(self.name(*room));
                e["water_glitter"] = json!([glitter.last, glitter.cycle]);
                e["float_water_level"] = json!(float_water_level);
                e["before"] = self.drip_json(before);
            }
            CallExtra::ImpactDrips {
                room,
                impact,
                pos,
                rad,
                mass,
                ceiling_drips,
                ..
            } => {
                e["room"] = json!(self.name(*room));
                e["pos"] = Self::vec(*pos);
                e["rad"] = json!(rad);
                e["mass"] = json!(mass);
                e["direction"] = json!([impact.direction.x, impact.direction.y]);
                e["speed"] = json!(impact.speed);
                e["first_contact"] = json!(impact.first_contact);
                e["ceiling_drips"] = json!(ceiling_drips);
            }
            CallExtra::CarriedPull { target, radius } => {
                e["target"] = Self::vec(*target);
                e["radius"] = json!(radius);
            }
            CallExtra::CarriedMove { to } => {
                e["to"] = json!([self.name(to.room), to.tile.0, to.tile.1, to.node]);
            }
            CallExtra::CarriedSpitOut { pos, room } => {
                e["pos"] = json!([pos.0, pos.1]);
                e["room"] = json!(self.name(*room));
            }
        }
    }

    /// Merges a graphics call's recorded input into the entry (the looker's objects
    /// name their rooms) and returns its output for `expected.state.player`.
    fn graphics_json(&mut self, g: &GraphicsCallRecord, e: &mut Value) -> Value {
        let input: Value = serde_json::from_str(&g.input).expect("graphics input");
        if let Value::Object(map) = input {
            for (k, v) in map {
                e[k] = v;
            }
        }
        // The body the graphics read (before this tick's shortcut pull, if any).
        if let Some(Value::Array(chunks)) = e.as_object_mut().and_then(|m| m.remove("p_chunks")) {
            e["expected"]["state"]["player"]["vel0"] = chunks[0]["vel"].clone();
            for (k, c) in chunks.into_iter().enumerate() {
                for key in ["pos", "last_pos", "vel"] {
                    e["p"]["chunks"][k][key] = c[key].clone();
                }
            }
        }
        if let Some(room) = e.as_object_mut().and_then(|m| m.remove("p_room")) {
            let name = room
                .as_i64()
                .and_then(|r| i32::try_from(r).ok())
                .map(|r| self.name(r));
            e["p"]["room"] = json!(name);
        }
        if let Some(objects) = e.get_mut("objects").and_then(Value::as_array_mut) {
            let rooms: Vec<Option<i32>> = objects
                .iter()
                .map(|o| o["room"].as_i64().and_then(|r| i32::try_from(r).ok()))
                .collect();
            for (o, room) in objects.iter_mut().zip(rooms) {
                o["room"] = json!(room.map(|r| self.names.get(&r).cloned().unwrap_or_default()));
            }
        }
        serde_json::from_str(&g.output).expect("graphics output")
    }

    fn vec(v: Vec2) -> Value {
        json!([v.x, v.y])
    }

    fn opt_vec(v: Option<Vec2>) -> Value {
        v.map_or(Value::Null, Self::vec)
    }

    fn fly_graphics_json(g: &FlyGraphics) -> Value {
        let lb = &g.lower_body;
        json!({
            "culled": g.culled, "last_culled": g.last_culled,
            "lower_body": {"pos": Self::vec(lb.pos), "last_pos": Self::vec(lb.last_pos),
                           "vel": Self::vec(lb.vel)},
            "wings": [[g.wings[0][0], g.wings[0][1]], [g.wings[1][0], g.wings[1][1]]],
            "steering": g.horizontal_steering_compensation,
            "last_steering": g.last_horizontal_steering_compensation,
            "wing_sync": g.insect_flight_wing_sync,
            "death_wings": [g.death_wing_positions[0], g.death_wing_positions[1]],
        })
    }

    fn fly_json(
        &mut self,
        c: &AbstractCreature,
        vessel: Option<(CreatureVessel, bool)>,
        graphics: Option<&FlyGraphics>,
    ) -> Value {
        let Some(f) = c.fly() else {
            return Value::Null;
        };
        let ai = &f.ai;
        let threats: Vec<Value> = ai
            .threats
            .iter()
            .map(|t| {
                json!([
                    t.crit.0,
                    t.ticks_since_seen,
                    t.visual_contact,
                    t.currently_scared_of,
                    t.ticks_to_forget,
                    Self::vec(t.last_seen_pos),
                    t.delete_me_next_frame
                ])
            })
            .collect();
        let room = f.room.map(|r| self.name(r));
        let abs_room = self.name(c.pos.room);
        let vessel = match vessel {
            Some((v, in_lobby)) => {
                let (pos, last) = v.tiles();
                json!({"room": self.name(v.room), "pos": [pos.0, pos.1],
                       "last_pos": [last.0, last.1], "entrance_node": v.entrance_node,
                       "in_lobby": in_lobby})
            }
            None => Value::Null,
        };
        let chunk = f.chunk();
        json!({
            "room": room,
            "pos": Self::vec(chunk.pos), "last_pos": Self::vec(chunk.last_pos),
            "vel": Self::vec(chunk.vel),
            "dead": f.dead, "stun": f.stun, "shortcut_delay": f.shortcut_delay,
            "entering_shortcut": f.entering_shortcut.map(|(x, y)| json!([x, y])),
            "in_shortcut": f.in_shortcut, "drown": f.drown, "flap": f.flap,
            "flap_speed": f.flap_speed, "flap_depth": f.flap_depth,
            "last_flap_depth": f.last_flap_depth,
            "mov_mode": f.mov_mode.map(|m| m.value()),
            "burrow_or_hang_spot": Self::opt_vec(f.burrow_or_hang_spot),
            "dir": Self::vec(f.dir), "wall_avoidance": f.wall_avoidance,
            "chain_variation": f.chain_behavior_variation,
            "grasp": c.grasps.first().copied().flatten().map(|g| g.grabbed.0),
            "grabbed_by": c.grabbed_by.iter().map(|g| g.grabber.0).collect::<Vec<_>>(),
            "abs": [abs_room, c.pos.tile.0, c.pos.tile.1, c.pos.node],
            "ai": {
                "behavior": ai.behavior.map(|b| b.value()),
                "local_goal": Self::vec(ai.local_goal),
                "following": ai.following_dijkstra_map, "leave_room": ai.leave_room_dijkstra,
                "other_fly": ai.other_fly.map(|o| o.0),
                "lowest_other_pos": Self::vec(ai.lowest_other_pos), "afraid": ai.afraid,
                "panic_flee": ai.panic_flee, "flee_from_rain": ai.flee_from_rain,
                "drop_status": ai.drop_status.map(|d| d.value()),
                "stuck_counter": ai.stuck_counter, "stuck_pos": Self::vec(ai.stuck_pos),
                "get_back_to_ground": ai.get_back_to_ground,
                "no_swarm_counter": ai.no_swarm_counter, "flock": ai.flock,
                "threats": threats,
            },
            "graphics": graphics.map_or(Value::Null, Self::fly_graphics_json),
            "hover": f.hover.map_or(Value::Null, |h| {
                json!({"alive": h.alive, "slated": h.slated, "still_playing": h.still_playing})
            }),
            "vessel": vessel,
            "bites": f.bites, "eaten": f.eaten, "ever_caught": f.ever_been_caught_by_player,
            "even_update": f.even_update, "set_pos": Self::opt_vec(chunk.set_pos),
            "slated": f.slated,
        })
    }

    /// The state after call `i` as the model writes it.
    pub(super) fn state_json(&mut self, i: usize) -> Value {
        let Some(state) = self.calls[i].state.clone() else {
            return Value::Null;
        };
        let graphics = self.graphics[i].clone();
        let mut flies = serde_json::Map::new();
        let mut flocks = serde_json::Map::new();
        for (id, c, vessel) in &state.flies {
            let has = c.fly().is_some_and(|f| f.graphics);
            let g = graphics.iter().find(|g| has && g.id == c.id);
            let v = self.fly_json(c, *vessel, g);
            flies.insert(id.0.to_string(), v);
            if let Some(f) = c.fly()
                && let Some(b) = state.flocks.get(f.ai.flock)
            {
                flocks.insert(
                    f.ai.flock.to_string(),
                    json!({
                        "sin_cycle": b.sin_cycle, "life": b.life, "leader": b.leader.0,
                        "dom": b.dom, "distance_to_others": b.distance_to_others,
                        "collective": b.collective_movement, "drop": b.drop,
                        "form_chains": b.form_chains, "sin_freq": b.swarm_flight_sin_freq,
                        "sin_ampl": b.swarm_flight_sin_ampl,
                        "vertical_displace": b.swarm_flight_vertical_displace,
                        "flock_dir": Self::vec(b.flock_dir),
                        "exaggerate": b.exaggerate_direction_of_others,
                        "life_time": b.life_time,
                    }),
                );
            }
        }
        let mut rooms = serde_json::Map::new();
        for (r, ai) in &state.rooms_ai {
            let name = self.name(*r);
            rooms.insert(
                name,
                json!({"flies": ai.flies.iter().map(|f| f.0).collect::<Vec<_>>(),
                       "in_hive": ai.in_hive.iter().map(|f| f.0).collect::<Vec<_>>()}),
            );
        }
        let mut drips = serde_json::Map::new();
        for d in &state.drips {
            let v = self.drip_json(d);
            drips.insert(d.id.to_string(), v);
        }
        let mut out = json!({"flies": flies, "flocks": flocks, "rooms_ai": rooms, "drips": drips});
        if let Some(p) = &state.player
            && Self::reads_player(self.calls[i].call)
        {
            out["player"] = Self::player_after_json(p);
        }
        out
    }

    fn body_json(&mut self, p: &PlayerBody) -> Value {
        let room = p.room.map(|r| self.name(r));
        json!({"room": room, "chunks": [Self::vec(p.chunks[0]), Self::vec(p.chunks[1])],
               "main": 0})
    }

    /// The model's input: the constructor's random state and every call with what it
    /// read and the port's results as `expected`.
    pub(super) fn model_input(&mut self) -> Value {
        let sounds = self.sounds();
        let mut ticks = Vec::new();
        for i in 0..self.calls.len() {
            let c = self.calls[i].clone();
            let (call, room, id) = self.call_name(&c);
            let realized: Vec<Value> = c
                .realized
                .iter()
                .map(|&(r, loaded)| json!([self.name(r), loaded]))
                .collect();
            let mut creatures = serde_json::Map::new();
            for (r, list) in &c.world.creatures {
                let rows: Vec<Value> = list
                    .iter()
                    .map(|(cid, kind, realized)| json!([cid.0, kind.value(), realized]))
                    .collect();
                creatures.insert(self.name(*r), json!(rows));
            }
            let mut bodies = serde_json::Map::new();
            if let Some(p) = &c.world.player {
                bodies.insert(p.id.0.to_string(), self.body_json(p));
            }
            let camera_room = self.name(c.camera.room);
            let player_room = self.name(c.world.player_room);
            let state = self.state_json(i);
            let mut e = json!({
                "tick": c.tick, "call": call, "rng": c.rng_before,
                "rain": {"timer": c.rain_timer, "cycle_length": c.cycle_length},
                "realized": realized,
                "camera": {"room": camera_room, "current": c.camera.current,
                           "next": c.camera.next},
                "creatures": creatures, "bodies": bodies, "player_followed": player_room,
                "new_ids": c.new_ids.iter().map(|n| n.0).collect::<Vec<_>>(),
                "expected": {"rng_after": c.rng_after, "state": state},
            });
            if let Some(room) = room {
                e["room"] = json!(room);
            }
            if let Some(id) = id {
                e["id"] = json!(id);
            }
            e["eu"] = json!(c.eu);
            if let Some(p) = c.player.as_ref().filter(|_| Self::reads_player(c.call)) {
                e["p"] = self.player_json(p, c.eu);
            }
            if !c.new_drip_ids.is_empty() {
                e["new_drip_ids"] = json!(c.new_drip_ids);
            }
            self.extra_json(&c, &mut e);
            if let Some(g) = &c.graphics {
                let out = self.graphics_json(g, &mut e);
                if let Value::Object(out) = out {
                    for (k, v) in out {
                        e["expected"]["state"]["player"][k] = v;
                    }
                }
            }
            ticks.push(e);
        }
        let campaign = self.session.scene_mut().map(|s| s.game().sim().campaign());
        let slugcat = campaign.map(|c| c.name.value().to_owned());
        let food_to_hibernate = campaign.map_or(4, |c| c.stats.food_to_hibernate);
        json!({
            "format": "flies-realized-2", "region": "SU", "food_to_hibernate": food_to_hibernate,
            "slugcat": slugcat, "ceiling_drips": self.ceiling_drips,
            "ctor": {"rng": self.ctor_rng[0], "expected": {"rng_after": self.ctor_rng[1]}},
            "sounds": sounds,
            "ticks": ticks,
        })
    }

    /// The realized flies in the camera's room now: entity id and position.
    pub(super) fn fly_views(&mut self) -> Vec<(EntityId, Vec2)> {
        let Some(scene) = self.session.scene_mut() else {
            return Vec::new();
        };
        let sim = scene.game().sim();
        let world = sim.world();
        world
            .creatures()
            .filter_map(|(id, _)| world.realized_fly_view(id))
            .filter(|v: &FlyView<'_>| v.fly.room == Some(sim.camera_room()))
            .map(|v| (v.id, v.fly.pos()))
            .collect()
    }

    pub(super) fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }
}

#[test]
#[ignore = "writes the realized flies' model input; run with FLIES_TRACE=<path> [FLY_TICKS=<n>]"]
fn dump_fly_trace() {
    let Some(path) = std::env::var_os("FLIES_TRACE") else {
        return;
    };
    let ticks = std::env::var("FLY_TICKS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2500);
    let run = match std::env::var("FLY_ROUTE").as_deref() {
        Ok("a24") => FlyTrace::run_a24(ticks),
        Ok("tutorial") => FlyTrace::tutorial(),
        Ok("eat") => FlyTrace::caught("White").map(FlyTrace::eaten),
        Ok("eat_red") => FlyTrace::caught("Red").map(FlyTrace::eaten),
        Ok("carry") => FlyTrace::carried(),
        _ => FlyTrace::run(ticks),
    };
    let Some(mut t) = run else {
        return;
    };
    let input = t.model_input();
    std::fs::write(path, serde_json::to_string(&input).expect("json")).expect("write");
}
