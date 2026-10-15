//! The bat flies' world process and quantified placement over the tutorial route (a new
//! Survivor game, seed 1) against the independent reference model
//! (`rainworld-re/tools/overseer_model/flies_model.py`; its output trimmed into
//! `tests/data/flies_route_model.json` with `--expected`).

use rw_sim::{FliesCall, FliesCallRecord};
use serde_json::{Value, json};

use super::Session;
use super::params::{NewGameParams, StepParams};
use super::tutorial_route::{RouteView, TutorialRoute};

const ROUTE_MAX_TICKS: u64 = 20_000;

/// The route's run with every flies' call recorded.
struct FliesTrace {
    calls: Vec<FliesCallRecord>,
    ctor_rng: [[u32; 4]; 2],
    session: Session,
}

impl FliesTrace {
    fn run(food: i32, pit_first: bool) -> Option<Self> {
        let mut session = Session::installed()?;
        session
            .new_game(&NewGameParams {
                slugcat: "White".to_owned(),
                seed: Some(1),
            })
            .expect("new game");
        let sim = session.scene_mut()?.game_mut().sim_mut();
        sim.record_flies_calls(true);
        let ctor_rng = sim.flies_ctor_rng();
        session
            .set_player(&super::params::SetPlayerParams {
                food: Some(food),
                ..Default::default()
            })
            .expect("food");
        let mut route = if pit_first {
            TutorialRoute::pit_first()
        } else {
            TutorialRoute::new()
        };
        let mut state = session.state().expect("state");
        let mut calls = Vec::new();
        while !route.done() && state.tick < ROUTE_MAX_TICKS {
            let input = route.input(&RouteView::of(&state));
            state = session
                .step(&StepParams {
                    ticks: Some(1),
                    input: Some(input),
                })
                .expect("steps");
            calls.extend(session.scene_mut()?.game_mut().sim_mut().take_flies_calls());
        }
        assert!(route.done(), "the route reached the shelter");
        Some(Self {
            calls,
            ctor_rng,
            session,
        })
    }

    fn name(&mut self, room: i32) -> String {
        self.session
            .scene_mut()
            .and_then(|s| {
                s.game()
                    .sim()
                    .world()
                    .abstract_room(room)
                    .map(|r| r.name.clone())
            })
            .unwrap_or_default()
    }

    /// Counts as the model writes them: `[room, node, count]` sorted by name.
    fn counts(&mut self, c: &FliesCallRecord) -> Value {
        let mut rows: Vec<(String, i32, i32)> = c
            .counts
            .iter()
            .map(|&(r, n, k)| (self.name(r), n, k))
            .collect();
        rows.sort();
        json!(
            rows.iter()
                .map(|(r, n, k)| json!([r, n, k]))
                .collect::<Vec<_>>()
        )
    }

    fn creatures(&mut self, c: &FliesCallRecord) -> Value {
        let mut rows: Vec<(String, usize)> = c
            .creatures
            .iter()
            .map(|&(r, n)| (self.name(r), n))
            .collect();
        rows.sort();
        json!(rows.iter().map(|(r, n)| json!([r, n])).collect::<Vec<_>>())
    }

    fn placed(c: &FliesCallRecord) -> Value {
        json!(
            c.placed
                .iter()
                .map(|p| json!({"tile": [p.tile.0, p.tile.1], "node": p.node}))
                .collect::<Vec<_>>()
        )
    }

    fn call_name(&mut self, c: &FliesCallRecord) -> (&'static str, Option<String>) {
        match c.call {
            FliesCall::Update => ("update", None),
            FliesCall::Ready(r) => ("ready", Some(self.name(r))),
            FliesCall::Abstractize(r) => ("abstractize", Some(self.name(r))),
        }
    }

    /// The model's input: the constructor's random state and every call with what it
    /// read and the port's results as `expected`.
    fn model_input(&mut self) -> Value {
        let calls = std::mem::take(&mut self.calls);
        let mut ticks = Vec::new();
        for c in &calls {
            let (call, room) = self.call_name(c);
            let realized: Vec<Value> = c
                .realized
                .iter()
                .map(|&(r, loaded)| json!([self.name(r), loaded]))
                .collect();
            let mut e = json!({
                "tick": c.tick, "call": call, "rng": c.rng_before,
                "rain": {"timer": c.rain_timer, "cycle_length": c.cycle_length},
                "realized": realized,
                "expected": {
                    "rng_after": c.rng_after, "counts": self.counts(c),
                    "creatures": self.creatures(c),
                },
            });
            if let Some(room) = room {
                e["room"] = json!(room);
            }
            if call == "ready" {
                e["expected"]["placed"] = Self::placed(c);
            }
            ticks.push(e);
        }
        self.calls = calls;
        json!({
            "region": "SU", "food_to_hibernate": 4,
            "ctor": {"rng": self.ctor_rng[0], "expected": {"rng_after": self.ctor_rng[1]}},
            "ticks": ticks,
        })
    }
}

impl FliesTrace {
    /// The port's run against the model's fixture: every call's code, tick and newest
    /// random word, and at the fixture's states the full random state, the stored counts,
    /// the fly creatures and the placed flies. The first difference, if any.
    fn first_mismatch(&mut self, expected: &Value) -> Option<String> {
        let calls = std::mem::take(&mut self.calls);
        let rows = expected["calls"].as_array().expect("calls");
        let mut found = None;
        if rows.len() != calls.len() {
            found = Some(format!("calls: port {}, model {}", calls.len(), rows.len()));
        }
        for (i, (c, row)) in calls.iter().zip(rows).enumerate() {
            let code = match c.call {
                FliesCall::Update => "u",
                FliesCall::Ready(_) => "r",
                FliesCall::Abstractize(_) => "a",
            };
            let got = json!([c.tick, code, c.rng_after[3]]);
            if found.is_none() && &got != row {
                found = Some(format!("call {i}: port {got}, model {row}"));
            }
        }
        for s in expected["states"].as_array().expect("states") {
            let i = usize::try_from(s["index"].as_u64().expect("index")).expect("index");
            let Some(c) = calls.get(i) else { break };
            let mut got = json!({
                "index": i, "tick": c.tick, "rng_after": c.rng_after,
                "counts": self.counts(c), "creatures": self.creatures(c),
            });
            if !c.placed.is_empty() {
                got["placed"] = Self::placed(c);
            }
            if found.is_none() && &got != s {
                found = Some(format!("state {i}: port {got}\n model {s}"));
            }
        }
        self.calls = calls;
        found
    }
}

/// The tutorial route's flies against the reference model (the fixture's source:
/// `flies_model.py --expected` on `dump_flies_trace`'s input).
#[test]
fn flies_route_matches_reference_model() {
    let expected: Value =
        serde_json::from_str(include_str!("../../tests/data/flies_route_model.json"))
            .expect("model output");
    let Some(mut t) = FliesTrace::run(0, false) else {
        return;
    };
    assert_eq!(
        json!(t.ctor_rng[1]),
        expected["ctor"]["rng_after"],
        "constructor"
    );
    let mismatch = t.first_mismatch(&expected);
    assert!(mismatch.is_none(), "{}", mismatch.unwrap_or_default());
    let placed: usize = t.calls.iter().map(|c| c.placed.len()).sum();
    println!(
        "flies route: {} calls compared ({} states, {placed} flies placed), 0 mismatches",
        t.calls.len(),
        expected["states"].as_array().map_or(0, Vec::len)
    );
    let mut wrong = expected.clone();
    wrong["states"][1]["counts"][0][2] =
        json!(wrong["states"][1]["counts"][0][2].as_i64().expect("count") + 1);
    assert!(
        t.first_mismatch(&wrong).is_some(),
        "a perturbed count is caught"
    );
    let mut wrong = expected;
    wrong["calls"][100][2] = json!(0);
    assert!(
        t.first_mismatch(&wrong).is_some(),
        "a perturbed draw is caught"
    );
}

#[test]
#[ignore = "writes the flies model's input for the tutorial route; run with FLIES_TRACE=<path>"]
fn dump_flies_trace() {
    let Some(path) = std::env::var_os("FLIES_TRACE") else {
        return;
    };
    let Some(mut t) = FliesTrace::run(0, false) else {
        return;
    };
    let input = t.model_input();
    std::fs::write(path, serde_json::to_string(&input).expect("json")).expect("write");
}
