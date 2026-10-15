//! The guide overseer in a new Survivor game against the independent reference model's
//! output (`tests/data/overseer_su_c04.json`).

use rw_sim::{OverseerCall, OverseerCallRecord};
use serde_json::{Value, json};

use rw_sim::creature::CreatureId;
use rw_sim::creature::overseer::hologram::{
    Controller, HologramKind, HologramPart, InstructionKind, ObjectPointerKind, OverseerHologram,
    PartKind, PointerKind,
};

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

    fn set_food(&mut self, food: i32) {
        self.set_player(&super::params::SetPlayerParams {
            food: Some(food),
            ..Default::default()
        })
        .expect("food");
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

/// The tutorial route's run (a new Survivor game, seed 1, `TutorialRoute` through
/// SU_C04, SU_A41, SU_A43 and SU_A22 into SU_S01) with every guide call recorded.
struct RouteTrace {
    calls: Vec<OverseerCallRecord>,
    session: Session,
}

impl RouteTrace {
    /// With `food` pips in the stomach from the start (the fed run shows the shelter
    /// pointers and the super jump).
    fn run(food: i32, pit_first: bool) -> Option<Self> {
        let mut session = Session::new_survivor(1)?;
        session.set_food(food);
        let place = session
            .scene_mut()
            .and_then(|sc| sc.game().sim().last_overseer_place().cloned())
            .expect("the guide was placed while the first room loaded");
        let guide = place.creature;
        let mut calls = vec![place];
        let mut route = if pit_first {
            super::tutorial_route::TutorialRoute::pit_first()
        } else {
            super::tutorial_route::TutorialRoute::new()
        };
        let mut state = session.state().expect("state");
        while !route.done() && state.tick < ROUTE_MAX_TICKS {
            let input = route.input(&super::tutorial_route::RouteView::of(&state));
            state = session
                .step(&StepParams {
                    ticks: Some(1),
                    input: Some(input),
                })
                .expect("steps");
            calls.extend(
                session
                    .overseer_calls()
                    .into_iter()
                    .filter(|c| c.creature == guide),
            );
        }
        assert!(route.done(), "the route reached the shelter");
        Some(Self { calls, session })
    }

    /// Hologram ids in creation order from 0, as the model numbers them.
    fn hologram_index(calls: &[OverseerCallRecord], id: u64) -> usize {
        let mut seen: Vec<u64> = Vec::new();
        for c in calls {
            for h in &c.holograms {
                if !seen.contains(&h.id) {
                    seen.push(h.id);
                }
            }
            if let OverseerCall::Hologram(hid) = c.call
                && !seen.contains(&hid)
            {
                seen.push(hid);
            }
        }
        seen.iter()
            .position(|&h| h == id)
            .expect("a recorded hologram")
    }
}

#[test]
#[ignore = "writes the reference model's input for the tutorial route; run with ROUTE_TRACE=<path>"]
fn dump_route_trace() {
    let Some(path) = std::env::var_os("ROUTE_TRACE") else {
        return;
    };
    let food = std::env::var("ROUTE_FOOD")
        .ok()
        .and_then(|f| f.parse().ok())
        .unwrap_or(0);
    let pit_first = std::env::var_os("ROUTE_PIT").is_some();
    let Some(mut t) = RouteTrace::run(food, pit_first) else {
        return;
    };
    let input = t.model_input();
    std::fs::write(path, serde_json::to_string(&input).expect("json")).expect("write");
}

impl RouteTrace {
    /// The model's per-call input (its README's "Input (per-call trace)").
    fn model_input(&mut self) -> Value {
        let calls = std::mem::take(&mut self.calls);
        let place = calls[0].clone();
        let guide = place.creature;
        let before = place.abstract_before.as_ref().expect("abstract creature");
        let ai = before.ai.as_ref().expect("abstract AI");
        let oa = before.overseer_ai().expect("overseer AI");
        let room = self.session.room_label(before.pos.room);
        let mut ticks = Vec::new();
        for c in &calls[1..] {
            let mut e = self.session.model_tick(c, guide);
            e["room"] = json!(self.session.room_label(c.pos_before.room));
            match c.call {
                OverseerCall::Update => e["call"] = json!("update"),
                OverseerCall::Hologram(id) => {
                    e["call"] = json!("hologram");
                    e["hologram_id"] = json!(Self::hologram_index(&calls, id));
                }
                OverseerCall::TeleportIn => {
                    e["call"] = json!("teleport_in");
                    e["pos"] = Session::coordinate(c.pos_before);
                    e["viewed"] = json!(c.viewed);
                }
                OverseerCall::Place => continue,
            }
            if let Some(p) = c.player {
                let player = &mut e["player"];
                player["chunk1"]["contact_y"] = json!(p.hips_contact_y);
                player["standing"] = json!(p.standing);
                player["animation"] = json!(format!("{:?}", p.animation));
                player["input0"] = json!({
                    "x": p.input_x, "y": p.input_y, "jmp": p.jump[0], "pckp": false, "thrw": false,
                });
                player["food"] = json!(p.food);
                player["max_food"] = json!(7);
                player["food_to_hibernate"] = json!(4);
                player["grabbed"] = json!(false);
                player["room_null"] = json!(p.in_shortcut);
            }
            ticks.push(e);
        }
        self.calls = calls;
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
                "lastRoom": Session::coordinate(oa.last_room),
                "destination": Session::coordinate(ai.destination),
                "time_buffer": ai.time_buffer,
            },
            "place": {"rng": place.rng_before, "viewed": place.viewed, "tick": place.tick},
            "expected_place": {"rng_after": place.rng_after},
            "ticks": ticks,
        })
    }
}

const ROUTE_MAX_TICKS: u64 = 20_000;

/// The tutorial route (a new Survivor game, seed 1, through SU_C04, SU_A41, SU_A43 and
/// SU_A22 into SU_S01) against the reference model's output for the port's per-call
/// trace of it (`make_expected.py` format 2): every guide and hologram call's random
/// state and rain timer, the full random state, the guide and its holograms (with their
/// parts) where the model kept a state, the tutorial texts and the arrivals.
#[test]
fn route_matches_reference_model() {
    let expected: Value =
        serde_json::from_str(include_str!("../../tests/data/overseer_route_model.json"))
            .expect("model output");
    ModelCompare::route(0, false, &expected);
}

/// The same with 4 food pips from the start and a drop into SU_A43's pit before the
/// leap: the shelter pointers (the shelter finder's door in SU_A43, `FORCEPOINT` in
/// SU_A22), "Leap the gap", the super-jump lesson and the rain brought in.
#[test]
fn fed_route_matches_reference_model() {
    let expected: Value = serde_json::from_str(include_str!(
        "../../tests/data/overseer_route_fed_model.json"
    ))
    .expect("model output");
    ModelCompare::route(4, true, &expected);
}

/// The port's guide and holograms in the model's state format.
struct ModelCompare;

impl ModelCompare {
    /// Runs the route (with `food`, by the pit if `pit_first`) against `expected`.
    fn route(food: i32, pit_first: bool, expected: &Value) {
        let Some(mut session) = Session::new_survivor(1) else {
            return;
        };
        session.set_food(food);
        let place = session
            .scene_mut()
            .and_then(|sc| sc.game().sim().last_overseer_place().cloned())
            .expect("the guide was placed");
        assert_eq!(
            json!(place.rng_after),
            expected["place"]["rng_after"],
            "placement draws"
        );
        let guide = place.creature;
        let states = expected["states"].as_array().expect("states");
        let mut next_state = 0;
        let mut calls: Vec<OverseerCallRecord> = Vec::new();
        let mut compared_states = 0;
        let mut route = if pit_first {
            super::tutorial_route::TutorialRoute::pit_first()
        } else {
            super::tutorial_route::TutorialRoute::new()
        };
        let mut state = session.state().expect("state");
        let mut messages = Vec::new();
        while !route.done() && state.tick < ROUTE_MAX_TICKS {
            let input = route.input(&super::tutorial_route::RouteView::of(&state));
            state = session
                .step(&StepParams {
                    ticks: Some(1),
                    input: Some(input),
                })
                .expect("steps");
            let start = calls.len();
            calls.extend(
                session
                    .overseer_calls()
                    .into_iter()
                    .filter(|c| c.creature == guide),
            );
            while let Some(want) = states.get(next_state) {
                let index = usize::try_from(want["index"].as_u64().expect("index")).expect("index");
                if index >= calls.len() {
                    break;
                }
                assert!(index >= start, "state {index} compared late");
                ModelCompare::state(&mut session, &calls, index, guide, want);
                compared_states += 1;
                next_state += 1;
            }
        }
        assert!(route.done(), "the route reached the shelter");
        let rows = expected["calls"].as_array().expect("calls");
        assert_eq!(calls.len(), rows.len(), "calls");
        for (i, (c, row)) in calls.iter().zip(rows).enumerate() {
            let (code, rain) = match c.call {
                OverseerCall::Update => ("u", Some(c.rain_timer_after)),
                OverseerCall::Hologram(_) => ("h", None),
                OverseerCall::TeleportIn => ("t", None),
                OverseerCall::Place => ("p", None),
            };
            let mut got = vec![json!(c.tick), json!(code), json!(c.rng_after[3])];
            got.extend(rain.map(|r| json!(r)));
            assert_eq!(&Value::Array(got), row, "call {i}");
        }
        for a in expected["arrivals"].as_array().expect("arrivals") {
            let i = usize::try_from(a["index"].as_u64().expect("index")).expect("index");
            assert_eq!(json!(calls[i].rng_after), a["rng_after"], "arrival {i}");
            let after = calls[i].after.as_ref().expect("recorded");
            let b = after.overseer_body().expect("realized");
            let path: Vec<[i32; 2]> = b.zip_path[..b.zip_path_count]
                .iter()
                .map(|&t| Session::tile(t))
                .collect();
            assert_eq!(json!(path), a["zipPath"], "arrival {i} zip path");
        }
        println!(
            "route: {} calls compared ({} guide updates, {} hologram updates, {} arrivals), \
             {compared_states} states, 0 mismatches",
            calls.len(),
            calls
                .iter()
                .filter(|c| c.call == OverseerCall::Update)
                .count(),
            calls
                .iter()
                .filter(|c| matches!(c.call, OverseerCall::Hologram(_)))
                .count(),
            calls
                .iter()
                .filter(|c| c.call == OverseerCall::TeleportIn)
                .count(),
        );
        for (i, c) in calls.iter().enumerate() {
            for m in &c.messages {
                messages.push(json!({
                    "index": i, "tick": c.tick,
                    "messages": [{"text": m.text, "wait": m.wait, "time": m.time, "hideHud": m.hide_hud}],
                }));
            }
        }
        assert_eq!(
            Value::Array(messages),
            expected["messages"],
            "tutorial texts"
        );
    }

    /// State `want` (after entry `index`): the full random state and the holograms after
    /// that call; the guide after an update (the tick's last call into it) or an
    /// arrival.
    fn state(
        session: &mut Session,
        calls: &[OverseerCallRecord],
        index: usize,
        guide: CreatureId,
        want: &Value,
    ) {
        let c = &calls[index];
        let at = format!("state {index} (tick {})", c.tick);
        assert_eq!(json!(c.rng_after), want["rng_after"], "{at}: random state");
        let current = c
            .current_hologram
            .and_then(|id| c.holograms.iter().find(|h| h.id == id));
        let hologram = current.map(|h| Self::hologram(session, calls, h));
        Self::same(
            hologram.as_ref().unwrap_or(&Value::Null),
            &want["hologram"],
            &format!("{at}: hologram"),
        );
        let orphans: Vec<Value> = c
            .holograms
            .iter()
            .filter(|h| Some(h.id) != c.current_hologram)
            .map(|h| Self::hologram(session, calls, h))
            .collect();
        Self::same(
            &Value::Array(orphans),
            &want["orphan_holograms"],
            &format!("{at}: orphans"),
        );
        let creature = match c.call {
            OverseerCall::Update => {
                let scene = session.scene_mut().expect("scene");
                Some(scene.game().sim().world().creature(guide).clone())
            }
            OverseerCall::TeleportIn => c.after.clone(),
            _ => None,
        };
        if let Some(creature) = creature {
            let hand = session
                .scene_mut()
                .map_or(0.0, |sc| sc.game().sim().guide_state().hand_holding);
            let got = Self::guide(session, &creature, hand);
            Expect::fields(&got, &want["guide"], &format!("{at}: guide"));
            if let Some(t) = want["guide"]["tutorial"].as_object() {
                for (k, v) in t {
                    assert_eq!(
                        Expect::f32s(&got["tutorial"][k]),
                        Expect::f32s(v),
                        "{at}: tutorial {k}"
                    );
                }
            }
        }
    }

    fn same(got: &Value, want: &Value, at: &str) {
        assert_eq!(Expect::f32s(got), Expect::f32s(want), "{at}");
    }

    fn guide(session: &mut Session, c: &rw_sim::creature::AbstractCreature, hand: f32) -> Value {
        let b = c.overseer_body().expect("realized");
        let oa = c.overseer_ai().expect("overseer AI");
        let ai = oa.real_ai.as_deref().expect("OverseerAI");
        let t = ai.tutorial.as_ref();
        json!({
            "room": session.room_label(c.pos.room),
            "mode": format!("{:?}", b.mode),
            "extended": b.extended,
            "pos": b.chunk().pos.to_array(),
            "vel": b.chunk().vel.to_array(),
            "rootPos": b.root_pos.to_array(),
            "rootDir": b.root_dir.to_array(),
            "hoverTile": Session::tile(b.hover_tile),
            "nextHoverTile": Session::tile(b.next_hover_tile),
            "rootTile": Session::tile(b.root_tile),
            "tempHoverTile": Session::tile(ai.temp_hover_tile),
            "zipProgs": b.zip_progs,
            "zipPathCount": b.zip_path_count,
            "lookAt": ai.look_at.to_array(),
            "bringUpLens": ai.bring_up_lens,
            "stationaryCounter": b.stationary_counter,
            "conversationDelay": b.conversation_delay,
            "abstract": {"playerGuideCounter": oa.player_guide_counter},
            "goToPlayer": oa.go_to_player,
            "destination": Session::coordinate(c.ai.as_ref().expect("ai").destination),
            "handHolding": hand,
            "tutorial": t.map(|t| json!({
                "encounterCounter": t.encounter_counter,
                "boxTrouble": t.box_trouble,
                "poleTrouble": t.pole_trouble,
                "superJumpTrouble": t.super_jump_trouble,
                "batTrouble": t.bat_trouble,
                "fruitTrouble": t.fruit_trouble,
                "playerHasJumpedOverBox": t.player_has_jumped_over_box,
                "playerHasClimbedPole": t.player_has_climbed_pole,
                "playerHasMadeSuperJump": t.player_has_made_super_jump,
                "superJumpAttemptTime": t.super_jump_attempt_time,
                "messagesDisplayed": t.messages_displayed,
                "pauseRain": t.pause_rain,
                "bringInRain": t.bring_in_rain,
                "displayRunMessage": t.display_run_message,
                "lastFoodInStomach": t.last_food_in_stomach,
            })),
        })
    }

    fn hologram(
        session: &mut Session,
        calls: &[OverseerCallRecord],
        h: &OverseerHologram,
    ) -> Value {
        let mut v = json!({
            "id": RouteTrace::hologram_index(calls, h.id),
            "message": format!("{:?}", h.message),
            "kind": Self::kind_name(h),
            "fade": h.fade,
            "lastFade": h.last_fade,
            "pos": h.pos.to_array(),
            "displayTile": Session::tile(h.display_tile),
            "tempDisplayTile": Session::tile(h.temp_display_tile),
            "stillRelevant": h.still_relevant,
            "importance": h.importance,
            "lookAtCommunicationCreature": h.look_at_communication_creature,
            "lookAtCommCritCounter": h.look_at_comm_crit_counter,
            "pushAroundPos": h.push_around_pos.to_array(),
            "overseerSitStill": h.overseer_sit_still,
            "slated": h.slated,
            "room": session.room_label(h.room),
        });
        match &h.kind {
            HologramKind::Door { pointer, door } => {
                v["door"] = json!(door.door);
                v["doorPos"] = json!(door.door_pos.to_array());
                v["pointerNecessary"] = json!(door.pointer_necessary);
                if let PointerKind::Shelter {
                    forcepoint,
                    showing_a_shelter_door,
                    currently_showing_to_shelter,
                } = pointer
                {
                    v["FORCEPOINT"] = json!(forcepoint);
                    v["showingAShelterDoor"] = json!(showing_a_shelter_door);
                    v["currentlyShowingToShelter"] = json!(currently_showing_to_shelter);
                }
            }
            HologramKind::Instruction(i) => {
                v["direction"] = json!([i.direction.0, i.direction.1]);
                v["showJump"] = json!(i.show_jump);
                v["showPickup"] = json!(i.show_pickup);
                v["showThrow"] = json!(i.show_throw);
                v["hideInputs"] = json!(i.hide_inputs);
                v["directionOnly"] = json!(i.direction_only);
                v["closeToPos"] = json!(i.close_to_pos.to_array());
                v["controller"] = i.controller.as_ref().map_or(Value::Null, Self::controller);
                v["waitMeter"] = i.wait_meter.as_ref().map_or(Value::Null, |m| {
                    json!({
                        "filled": m.filled,
                        "visible": m.visible,
                        "visibility": m.visibility,
                        "blink": m.blink,
                        "blinkCounter": m.blink_counter,
                        "offset": m.offset.to_array(),
                    })
                });
            }
            _ => {}
        }
        v["parts"] = h.parts.iter().map(Self::part).collect();
        v
    }

    fn controller(c: &Controller) -> Value {
        match c {
            Controller::GetUpOnFirstBox => json!({"kind": "GetUpOnFirstBox"}),
            Controller::ClimbPole => json!({"kind": "ClimbPole"}),
            Controller::SuperJump {
                in_position_counter,
                move_keys_up,
                jump_direction,
                ..
            } => json!({
                "kind": "SuperJump",
                "inPositionCounter": in_position_counter,
                "moveKeysUp": move_keys_up,
                "jumpDirection": jump_direction,
            }),
            other => json!({"kind": format!("{other:?}")}),
        }
    }

    fn kind_name(h: &OverseerHologram) -> &'static str {
        match &h.kind {
            HologramKind::Door { pointer, .. } => match pointer {
                PointerKind::Bat { .. } => "BatPointer",
                PointerKind::Shelter { .. } => "ShelterPointer",
                PointerKind::Direction { .. } => "DirectionPointer",
            },
            HologramKind::Object { pointer } => match pointer {
                ObjectPointerKind::Creature { .. } => "CreaturePointer",
                ObjectPointerKind::Food { .. } => "FoodPointer",
            },
            HologramKind::Image(_) => "OverseerImage",
            HologramKind::Angry { .. } => "AngryHologram",
            HologramKind::Instruction(i) => match i.kind() {
                InstructionKind::Keyboard => "KeyBoardInstruction",
                InstructionKind::GamePad => "GamePadInstruction",
                InstructionKind::Switch => "SwitchInstruction",
            },
        }
    }

    /// `[kind, partFade, fadeExponent, transform, visible, offset, rotation.z, ...]`.
    fn part(p: &HologramPart) -> Value {
        let (name, extra): (&str, Vec<Value>) = match &p.kind {
            PartKind::Key { button, .. } => (
                "KeyBoardKey",
                vec![
                    json!(button.pulsate),
                    json!(button.down),
                    json!(button.counter),
                    json!(button.time_up),
                    json!(button.time_down),
                ],
            ),
            PartKind::Arrow { dir } => ("Arrow", vec![json!(dir.to_array())]),
            PartKind::PlayerGhost { room_position, .. } => (
                "PlayerGhost",
                vec![json!(room_position.map(|p| p.to_array()))],
            ),
            PartKind::MeterBox { lit, .. } => ("MeterBox", vec![json!(lit)]),
            PartKind::Symbol(s) => ("Symbol", vec![json!(s.element)]),
            PartKind::LightEffect { .. } => ("HologramLightEffect", Vec::new()),
            other => {
                let name = format!("{other:?}");
                return json!([name]);
            }
        };
        let mut v = vec![
            json!(name),
            json!(p.part_fade),
            json!(p.fade_exponent),
            json!(p.transform),
            json!(p.visible),
            json!(p.offset.to_array()),
            json!(p.rotation),
        ];
        v.extend(extra);
        Value::Array(v)
    }
}

/// Screenshots of the traced run at a few ticks with the guide on screen, written to
/// `OVERSEER_SHOTS=<dir>` (`guide_<tick>.png`, and `_lens`/`_crop` variants zoomed in).
#[test]
#[ignore = "writes screenshots; run with OVERSEER_SHOTS=<dir>"]
fn overseer_screenshots() {
    let Some(dir) = std::env::var_os("OVERSEER_SHOTS") else {
        return;
    };
    let Some(mut s) = Session::new_survivor(1) else {
        return;
    };
    let shots = [2, 300, 840, 900, 1000, 1100, 1200, 1260, 1380, 1400, 1420];
    for tick in 0..=1420 {
        s.tick_with(Session::trace_input(tick));
        if shots.contains(&tick) {
            for (suffix, crop) in [("", None), ("_crop", Some(200.0))] {
                let path = std::path::Path::new(&dir).join(format!("guide_{tick}{suffix}.png"));
                let params = super::params::ScreenshotParams {
                    crop_radius: crop,
                    scale: crop.map(|_| 2.0),
                    save_path: Some(path.to_string_lossy().into_owned()),
                    ..super::params::ScreenshotParams::default()
                };
                if let Err(e) = s.screenshot(&params) {
                    eprintln!("no screenshot on tick {tick}: {e}");
                    return;
                }
            }
        }
    }
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
    let queued = messages.first().map_or(0, |m| m.0);
    assert_eq!(
        messages,
        [(
            queued,
            json!("You are hungry, find food"),
            json!(10),
            json!(160)
        )],
        "SU_C04's one message, once the player is past x 1450"
    );
    let queued = usize::try_from(queued).expect("tick");
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
    assert_eq!(prompts[queued + 1]["queued"], json!(1));
    assert_eq!(prompts[queued + 11]["showing"], json!("Nothing"));
    assert_eq!(
        prompts[queued + 12]["text"],
        json!("You are hungry, find food")
    );
    assert_eq!(prompts[queued + 232]["queued"], json!(1));
    assert_eq!(prompts[queued + 233]["queued"], json!(0));
    // `pauseRain` sets the timer to 2000 on every guide update with the player in its
    // room; once the player has gone ahead and the guide still waits in SU_C04, the
    // tutorial returns first and the cycle runs on.
    let rain: Vec<i64> = t
        .after
        .iter()
        .map(|a| a["rain"].as_i64().unwrap_or(-1))
        .collect();
    let arrived = usize::try_from(last).expect("tick") + 1;
    assert_eq!(rain[1], 1);
    assert!(rain[2..1285].iter().all(|&r| r == 2000));
    assert!(
        rain[1285..arrived]
            .iter()
            .zip(2001..)
            .all(|(&r, want)| r == want)
    );
    assert_eq!(
        rain[arrived], 2000,
        "pinned again in SU_A41 with the player"
    );
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
