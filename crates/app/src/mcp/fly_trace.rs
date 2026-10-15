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
use rw_sim::{CreatureVessel, FliesCall, FliesCallRecord, SoundId, Vec2};
use serde_json::{Value, json};

use super::Session;
use super::params::{InputParam, NewGameParams, SetPlayerParams, StepParams};
use super::tutorial_route::{RouteView, TutorialRoute};

const ROUTE_MAX_TICKS: u64 = 20_000;

/// The sounds a fly or its vessel plays.
const FLY_SOUNDS: [SoundId; 10] = [
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
        });
    }

    /// One tick with `input`, collecting the calls and graphics.
    fn step(&mut self, input: InputParam) {
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
        }
    }

    fn vec(v: Vec2) -> Value {
        json!([v.x, v.y])
    }

    fn opt_vec(v: Option<Vec2>) -> Value {
        v.map_or(Value::Null, Self::vec)
    }

    fn graphics_json(g: &FlyGraphics) -> Value {
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
            "graphics": graphics.map_or(Value::Null, Self::graphics_json),
            "hover": f.hover.map_or(Value::Null, |h| {
                json!({"alive": h.alive, "slated": h.slated, "still_playing": h.still_playing})
            }),
            "vessel": vessel,
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
        json!({"flies": flies, "flocks": flocks, "rooms_ai": rooms})
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
            ticks.push(e);
        }
        json!({
            "format": "flies-realized-1", "region": "SU", "food_to_hibernate": 4,
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
        _ => FlyTrace::run(ticks),
    };
    let Some(mut t) = run else {
        return;
    };
    let input = t.model_input();
    std::fs::write(path, serde_json::to_string(&input).expect("json")).expect("write");
}
