//! The guide overseer in a new Survivor game against the independent reference model's
//! output (`tests/data/overseer_su_c04.json`).

use rw_sim::{OverseerCall, OverseerCallRecord};
use serde_json::{Value, json};

use rw_sim::creature::CreatureId;

use super::Session;
use super::params::{InputParam, NewGameParams, StepParams};

impl Session {
    fn new_survivor(seed: u64) -> Option<Self> {
        let mut s = Self::installed()?;
        s.new_game(&NewGameParams {
            slugcat: "White".to_owned(),
            seed: Some(seed),
        })
        .expect("new game");
        s.scene_mut()?
            .game_mut()
            .sim_mut()
            .record_overseer_calls(true);
        Some(s)
    }

    fn tick_with(&mut self, input: InputParam) {
        self.step(&StepParams {
            ticks: Some(1),
            input: Some(input),
        })
        .expect("steps");
    }

    fn overseer_calls(&mut self) -> Vec<OverseerCallRecord> {
        self.scene_mut()
            .expect("scene")
            .game_mut()
            .sim_mut()
            .take_overseer_calls()
    }
}

/// The traced run: the guide's calls and its state after each tick (index = ticks run).
struct Trace {
    guide: CreatureId,
    calls: Vec<OverseerCallRecord>,
    after: Vec<Value>,
    session: Session,
}

impl Trace {
    /// A new Survivor game (seed 1) run for `ticks` with [`Session::trace_input`].
    fn run(ticks: u32) -> Option<Self> {
        let mut session = Session::new_survivor(1)?;
        let place = session
            .scene_mut()
            .and_then(|sc| sc.game().sim().last_overseer_place().cloned())
            .expect("the guide was placed while the first room loaded");
        let guide = place.creature;
        let mut calls = vec![place];
        let mut after = vec![session.guide_snapshot(guide)];
        for tick in 0..ticks {
            session.tick_with(Session::trace_input(tick));
            calls.extend(session.overseer_calls());
            after.push(session.guide_snapshot(guide));
        }
        Some(Self {
            guide,
            calls,
            after,
            session,
        })
    }

    fn call(&self, tick: u64) -> &OverseerCallRecord {
        self.calls
            .iter()
            .find(|c| c.creature == self.guide && c.call == OverseerCall::Update && c.tick == tick)
            .unwrap_or_else(|| panic!("no guide update on tick {tick}"))
    }
}

#[test]
#[ignore = "writes the reference model's input; run with OVERSEER_TRACE=<path>"]
fn dump_overseer_trace() {
    let Some(path) = std::env::var_os("OVERSEER_TRACE") else {
        return;
    };
    let Some(mut t) = Trace::run(TRACE_TICKS) else {
        return;
    };
    let mut input = t.session.model_input(&t.calls);
    input["actual"] = Value::Array(std::mem::take(&mut t.after));
    std::fs::write(path, serde_json::to_string_pretty(&input).expect("json")).expect("write");
}

/// The guide in SU_C04 from its placement to its zip out after the player, against the
/// reference model's output for the same inputs (`tests/data/overseer_su_c04_model.json`,
/// see `rainworld-re/notes/overseer.md`): the random stream after placement and after
/// every update (every draw, in order), the rain timer, and the body, tutorial and
/// graphics state, all exact.
#[test]
fn guide_matches_reference_model() {
    let expected: Value =
        serde_json::from_str(include_str!("../../tests/data/overseer_su_c04_model.json"))
            .expect("model output");
    let ticks = expected["ticks"].as_array().expect("ticks");
    let last = expected["teleport"]["tick"].as_u64().expect("tick");
    let Some(mut t) = Trace::run(u32::try_from(last).expect("ticks") + 1) else {
        return;
    };
    let place = &t.calls[0];
    assert_eq!(json!(place.rng_after), expected["place"]["rng_after"]);
    Expect::fields(&t.after[0], &expected["place"], "place");
    let mut messages = Vec::new();
    for e in ticks {
        let tick = e["tick"].as_u64().expect("tick");
        let call = t.call(tick);
        assert_eq!(
            json!(call.rng_after),
            e["rng_after"],
            "draws on tick {tick}"
        );
        assert_eq!(
            json!(call.rain_timer_after),
            e["rain_timer_after"],
            "rain on tick {tick}"
        );
        let after = &t.after[usize::try_from(tick).expect("tick") + 1];
        Expect::fields(after, e, &format!("tick {tick}"));
        if let Some(g) = e.get("graphics") {
            Expect::fields(&after["graphics"], g, &format!("graphics on tick {tick}"));
        }
        for m in e
            .get("messages")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            messages.push((
                tick,
                m["text"].clone(),
                m["wait"].clone(),
                m["time"].clone(),
            ));
        }
    }
    assert_eq!(
        messages,
        [(
            807,
            json!("You are hungry, find food"),
            json!(10),
            json!(160)
        )],
        "SU_C04's one message, once the player is past x 1450"
    );
    // The HUD's TextPrompt queues it that tick, shows it after its 10 ticks' wait and
    // removes it once its time is up and it has faded out.
    // Zipped out after the player, it waits in the lobby as a `TeleportationVessel`
    // and arrives in SU_A41 at (room, -1, -1, entrance node), then `TeleportingIntoRoom`
    // places it and zips it in from SU_C04.
    let tp = &expected["teleport"];
    let arrival = t
        .calls
        .iter()
        .find(|c| c.call == OverseerCall::TeleportIn)
        .expect("the guide teleported in");
    assert_eq!(json!(arrival.tick), tp["tick"]);
    assert_eq!(arrival.pos_before.tile, (-1, -1));
    let (room, node) = (arrival.pos_before.room, arrival.pos_before.node);
    assert_eq!(json!(t.session.room_label(room)), tp["room"]);
    assert_eq!(node, 2, "the node of its destination (250, 19, 9, 2)");
    assert_eq!(json!(arrival.rng_after), tp["rng_after"], "arrival draws");
    let after = arrival.after.as_ref().expect("recorded");
    let b = after.overseer_body().expect("realized");
    let arrived = json!({
        "mode": format!("{:?}", b.mode),
        "hoverTile": Session::tile(b.hover_tile),
        "rootTile": Session::tile(b.root_tile),
        "pos": b.chunk().pos.to_array(),
        "zipProgSpeed": b.zip_prog_speed,
        "zipPath": b.zip_path[..b.zip_path_count].iter().map(|&t| Session::tile(t)).collect::<Vec<_>>(),
    });
    Expect::fields(&arrived, tp, "arrival");
    let prompts: Vec<&Value> = t.after.iter().map(|a| &a["prompt"]).collect();
    assert_eq!(prompts[808]["queued"], json!(1));
    assert_eq!(prompts[818]["showing"], json!("Nothing"));
    assert_eq!(prompts[819]["text"], json!("You are hungry, find food"));
    assert_eq!(prompts[1039]["queued"], json!(1));
    assert_eq!(prompts[1040]["queued"], json!(0));
    // `pauseRain` sets the timer to 2000 on every guide update with the player in its
    // room; once the player has gone ahead and the guide still waits in SU_C04, the
    // tutorial returns first and the cycle runs on.
    let rain: Vec<i64> = t
        .after
        .iter()
        .map(|a| a["rain"].as_i64().unwrap_or(-1))
        .collect();
    assert_eq!(rain[1], 1);
    assert!(rain[2..1285].iter().all(|&r| r == 2000));
    assert!(
        rain[1285..1374]
            .iter()
            .zip(2001..)
            .all(|(&r, want)| r == want)
    );
    assert_eq!(rain[1374], 2000, "pinned again in SU_A41 with the player");
}

/// Field-by-field comparison of a snapshot with the model's values.
struct Expect;

impl Expect {
    /// Every field of `expected` that the snapshot also has must be equal; floats as
    /// the f32 the port holds.
    fn fields(actual: &Value, expected: &Value, at: &str) {
        let Some(fields) = expected.as_object() else {
            return;
        };
        for (k, want) in fields {
            let got = match k.as_str() {
                "zipProgs0" => &actual["zipProgs"][0],
                "playerGuideCounter" => &actual["abstract"]["playerGuideCounter"],
                "encounterCounter" | "pauseRain" => &actual["tutorial"][k],
                "myceliaTips" => {
                    let tips: Vec<Value> = actual["mycelia"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|m| {
                            m["points"]
                                .as_array()
                                .and_then(|p| p.last())
                                .cloned()
                                .unwrap_or_default()
                        })
                        .collect();
                    assert_eq!(
                        Self::f32s(&Value::Array(tips)),
                        Self::f32s(want),
                        "{at}: {k}"
                    );
                    continue;
                }
                "tick" | "rng_after" | "rain_timer_after" | "messages" | "graphics" | "draws"
                | "place_attempts" | "room" => continue,
                _ => &actual[k],
            };
            assert_eq!(Self::f32s(got), Self::f32s(want), "{at}: {k}");
        }
    }

    /// Non-integer numbers rounded to f32 (`serde_json` parses decimals only to within
    /// an ulp of the f64; the values are f32s).
    fn f32s(v: &Value) -> Value {
        match v {
            Value::Number(n) if n.is_f64() => json!(n.as_f64().map(|f| f as f32)),
            Value::Array(a) => Value::Array(a.iter().map(Self::f32s).collect()),
            Value::Object(o) => {
                Value::Object(o.iter().map(|(k, v)| (k.clone(), Self::f32s(v))).collect())
            }
            other => other.clone(),
        }
    }
}

/// Ticks of the traced run: standing up, then walking right and jumping now and then.
const TRACE_TICKS: u32 = 2000;

impl Session {
    fn trace_input(tick: u32) -> InputParam {
        InputParam {
            x: i8::from(tick > 400),
            jump: tick > 400 && tick % 40 < 12,
            ..InputParam::default()
        }
    }

    /// The guide's state after a tick, in the model output's names.
    fn guide_snapshot(&mut self, guide: CreatureId) -> Value {
        let tick = self.scene_mut().map_or(0, |sc| sc.game().ticks());
        let prompt = self.hud_state().ok().map(|h| h.text_prompt).map(
            |t| json!({"showing": t.showing, "text": t.text, "show": t.show, "queued": t.queued}),
        );
        let scene = self.scene_mut().expect("scene");
        let game = scene.game();
        let world = game.sim().world();
        let c = world.creature(guide);
        let rain = world.rain_cycle.timer;
        let Some(b) = c.overseer_body() else {
            return json!({"tick": tick, "realized": false, "prompt": prompt, "rain": rain});
        };
        let oa = c.overseer_ai().expect("overseer AI");
        let ai = oa.real_ai.as_deref().expect("OverseerAI");
        let t = ai.tutorial.as_ref();
        let graphics = game.overseer_graphics(c.id).map(|g| {
            json!({
                "culled": g.culled,
                "holoLensUp": g.holo_lens_up,
                "convoMode": g.convo_mode,
                "myceliaMovements": g.mycelia_movements,
                "holoRotations": g.holo.rotations,
                "holoMovementsCount": g.holo.movements.len(),
                "mycelia": g.mycelia.iter().map(|m| json!({
                    "points": m.points.iter().map(|p| p[0].to_array()).collect::<Vec<_>>(),
                    "vels": m.points.iter().map(|p| p[2].to_array()).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
                "usePos": g.at.use_pos.to_array(),
                "useDir": [g.at.use_dir.x, g.at.use_dir.y, g.at.use_dir.z],
            })
        });
        json!({
            "tick": tick,
            "realized": true,
            "in_room": b.in_room,
            "room": c.pos.room,
            "mode": format!("{:?}", b.mode),
            "extended": b.extended,
            "hoverTile": Self::tile(b.hover_tile),
            "nextHoverTile": Self::tile(b.next_hover_tile),
            "rootTile": Self::tile(b.root_tile),
            "tempHoverTile": Self::tile(ai.temp_hover_tile),
            "pos": b.chunk().pos.to_array(),
            "vel": b.chunk().vel.to_array(),
            "lastPos": b.chunk().last_pos.to_array(),
            "rootPos": b.root_pos.to_array(),
            "rootDir": b.root_dir.to_array(),
            "zipProgs": b.zip_progs,
            "zipProgSpeed": b.zip_prog_speed,
            "zipPathCount": b.zip_path_count,
            "zipPath": b.zip_path[..b.zip_path_count].iter().map(|&t| Self::tile(t)).collect::<Vec<_>>(),
            "lookAt": ai.look_at.to_array(),
            "slowLookAt": ai.slow_look_at.to_array(),
            "lookAtAdd": ai.look_at_add.to_array(),
            "targetStationary": ai.target_stationary,
            "stationaryCounter": b.stationary_counter,
            "conversationDelay": b.conversation_delay,
            "bringUpLens": ai.bring_up_lens,
            "abstract": {
                "goToPlayer": oa.go_to_player,
                "playerGuideCounter": oa.player_guide_counter,
                "destination": Self::coordinate(c.ai.as_ref().expect("ai").destination),
                "lastRooms": oa.last_rooms,
            },
            "tutorial": t.map(|t| json!({
                "encounterCounter": t.encounter_counter,
                "boxTrouble": t.box_trouble,
                "messagesDisplayed": t.messages_displayed,
                "pauseRain": t.pause_rain,
            })),
            "graphics": graphics,
            "prompt": prompt,
            "rain": rain,
        })
    }

    fn room_label(&mut self, index: i32) -> String {
        self.scene_mut()
            .and_then(|s| {
                s.game()
                    .sim()
                    .world()
                    .abstract_room(index)
                    .map(|r| r.name.clone())
            })
            .unwrap_or_default()
    }

    fn tile(t: (i32, i32)) -> [i32; 2] {
        [t.0, t.1]
    }

    fn coordinate(c: rw_sim::WorldCoordinate) -> Value {
        json!([c.room, c.tile.0, c.tile.1, c.node])
    }

    /// The reference model's input (its README) from the recorded calls of the first
    /// overseer placed.
    fn model_input(&mut self, calls: &[OverseerCallRecord]) -> Value {
        let place = calls
            .iter()
            .find(|c| c.call == OverseerCall::Place)
            .expect("an overseer was placed");
        let guide = place.creature;
        let before = place.abstract_before.as_ref().expect("abstract creature");
        let ai = before.ai.as_ref().expect("abstract AI");
        let oa = before.overseer_ai().expect("overseer AI");
        let room = self.room_label(before.pos.room);
        let ticks: Vec<Value> = calls
            .iter()
            .filter(|c| c.creature == guide && c.call == OverseerCall::Update)
            .map(|c| self.model_tick(c, guide))
            .collect();
        json!({
            "room": room,
            "slugcat": "White",
            "guide": {
                "player_guide_counter": oa.player_guide_counter,
                "seed_number": before.id.random_seed(),
            },
            "abstract": {
                "tile": [before.pos.tile.0, before.pos.tile.1],
                "node": before.pos.node,
                "lastRooms": oa.last_rooms,
                "lastRoom": Self::coordinate(oa.last_room),
                "destination": Self::coordinate(ai.destination),
                "time_buffer": ai.time_buffer,
            },
            "place": {"rng": place.rng_before, "viewed": place.viewed, "tick": place.tick},
            "teleport": calls
                .iter()
                .find(|c| c.creature == guide && c.call == OverseerCall::TeleportIn)
                .map(|c| self.teleport_json(c)),
            "expected_place": {"rng_after": place.rng_after},
            "ticks": ticks,
        })
    }

    /// The guide's `TeleportingIntoRoom` after zipping out of SU_C04, with the port's
    /// result.
    fn teleport_json(&mut self, c: &OverseerCallRecord) -> Value {
        let after = c.after.as_ref().expect("recorded");
        let b = after.overseer_body().expect("realized");
        json!({
            "room": self.room_label(c.pos_before.room),
            "pos": Self::coordinate(c.pos_before),
            "rng": c.rng_before,
            "viewed": c.viewed,
            "tick": c.tick,
            "expected": {
                "rng_after": c.rng_after,
                "mode": format!("{:?}", b.mode),
                "hoverTile": Self::tile(b.hover_tile),
                "rootTile": Self::tile(b.root_tile),
                "pos": b.chunk().pos.to_array(),
                "zipProgSpeed": b.zip_prog_speed,
                "zipPath": b.zip_path[..b.zip_path_count].iter().map(|&t| Self::tile(t)).collect::<Vec<_>>(),
            },
        })
    }

    fn model_tick(&mut self, c: &OverseerCallRecord, guide: rw_sim::creature::CreatureId) -> Value {
        let creatures: Vec<&str> = c
            .creatures
            .iter()
            .map(|&id| {
                if id == guide {
                    "guide"
                } else if id == c.player_id {
                    "player"
                } else {
                    "other"
                }
            })
            .collect();
        let player = c
            .player
            .map_or(json!({"realized": false, "in_room": false}), |p| {
                json!({
                    "room": self.room_label(c.player_pos.room),
                    "realized": true,
                    "in_room": p.in_room,
                    "chunk0": {"pos": p.pos.to_array(), "last_pos": p.last_pos.to_array()},
                    "chunk1": {"pos": p.hips.to_array(), "last_pos": c.player_hips_last.to_array()},
                    "dead": p.dead,
                    "food": p.food,
                    "jmp": p.jump,
                    "entering_shortcut": p.entering_shortcut.map(|(x, y)| [x, y]),
                    "karma": 0,
                    "abs_tile": [c.player_pos.tile.0, c.player_pos.tile.1],
                    "abs_node": c.player_pos.node,
                })
            });
        json!({
            "tick": c.tick,
            "rng": c.rng_before,
            "player": player,
            "creatures": creatures,
            "entities": creatures.len(),
            "camera": {
                "room": self.room_label(c.camera.room),
                "current": c.camera.current,
                "next": c.camera.next,
            },
            "hud": c.hud,
            "rain": {"timer": c.rain_timer_before, "cycle_length": c.cycle_length},
            "sound_draws": c.viewed,
            "expected": {"rng_after": c.rng_after, "rain_timer_after": c.rain_timer_after},
        })
    }
}
