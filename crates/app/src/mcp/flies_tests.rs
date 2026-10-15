//! The bat flies (the world process, quantified placement and the realized flies) against
//! the independent reference model (`rainworld-re/tools/overseer_model/fly_model.py`; its
//! output trimmed into `tests/data/flies_*_model.json` with `--expected`), over three
//! runs: the tutorial route, SU_A42 among its flies, and the swarm room SU_A24 whose flies
//! leave and enter it through the shortcuts.

use rw_sim::FliesCall;
use serde_json::{Value, json};

use super::fly_trace::FlyTrace;

impl FlyTrace {
    /// The fixture's call code for call `i`.
    fn code(&self, i: usize) -> &'static str {
        match self.calls[i].call {
            FliesCall::Update => "u",
            FliesCall::Ready(_) => "r",
            FliesCall::Realize { .. } => "z",
            FliesCall::Abstractize(_) => "a",
            FliesCall::RoomsAi(_) => "m",
            FliesCall::Fly(_) => "f",
            FliesCall::Hover(_) => "h",
            FliesCall::Vessel(_) => "v",
            FliesCall::Lobby(_) => "l",
            FliesCall::NowViewed { .. } => "n",
            FliesCall::NoLongerViewed(_) => "x",
            FliesCall::Catch => "c",
            FliesCall::GrabUpdate => "g",
            FliesCall::Looker => "k",
            FliesCall::Hand(_) => "j",
            FliesCall::ModuleUpdated => "M",
            FliesCall::PlayerRelease => "p",
            FliesCall::Drip(_) => "w",
            FliesCall::ImpactDrips => "i",
            FliesCall::CarriedPull(_) => "P",
            FliesCall::CarriedSuckedIn(_) => "S",
            FliesCall::CarriedMove(_) => "V",
            FliesCall::CarriedSpitOut(_) => "O",
        }
    }

    /// The fixture's rows (format 2): per tick with calls, `[tick, the calls' codes, FNV-1a
    /// of each call's newest random word]`.
    fn tick_rows(&self) -> Vec<Value> {
        let mut rows: Vec<(u64, String, Fnv1a)> = Vec::new();
        for (i, c) in self.calls.iter().enumerate() {
            if rows.last().is_none_or(|r| r.0 != c.tick) {
                rows.push((c.tick, String::new(), Fnv1a::new()));
            }
            let row = rows.last_mut().expect("a row");
            row.1.push_str(self.code(i));
            row.2.push(c.rng_after[3]);
        }
        rows.into_iter()
            .map(|(tick, codes, hash)| json!([tick, codes, hash.0]))
            .collect()
    }

    /// The port's run against the model's fixture: every tick's calls (their codes and
    /// newest random words), and at the fixture's states the full random state and every field the
    /// port writes of the flies, their flocks and the rooms' fly AIs. The first
    /// difference, if any.
    fn first_mismatch(&mut self, expected: &Value) -> Option<String> {
        if json!(self.ctor_rng[1]) != expected["ctor"]["rng_after"] {
            return Some("constructor".to_owned());
        }
        let rows = expected["calls"].as_array().expect("calls");
        let ticks = self.tick_rows();
        if let Some((got, want)) = ticks.iter().zip(rows).find(|(got, want)| got != want) {
            return Some(format!("tick row: port {got}, model {want}"));
        }
        if rows.len() != ticks.len() {
            return Some(format!("ticks: port {}, model {}", ticks.len(), rows.len()));
        }
        for s in expected["states"].as_array().expect("states") {
            let i = usize::try_from(s["index"].as_u64().expect("index")).expect("index");
            if json!(self.calls[i].rng_after) != s["rng_after"] {
                return Some(format!("state {i}: random state"));
            }
            let got = self.state_json(i);
            if let Some(d) = StateDiff::first(&got, &s["state"], &format!("state {i}")) {
                return Some(d);
            }
        }
        None
    }

    /// Asserts the run matches `expected` and that perturbed copies of it do not.
    fn assert_matches(&mut self, expected: &Value, what: &str) {
        let mismatch = self.first_mismatch(expected);
        assert!(mismatch.is_none(), "{}", mismatch.unwrap_or_default());
        let states = expected["states"].as_array().map_or(0, Vec::len);
        println!(
            "{what}: {} calls compared ({states} states), 0 mismatches",
            self.calls.len()
        );
        let mut wrong = expected.clone();
        wrong["calls"][100][2] = json!(0);
        assert!(
            self.first_mismatch(&wrong).is_some(),
            "a perturbed draw is caught"
        );
        let mut wrong = expected.clone();
        wrong["calls"]
            .as_array_mut()
            .expect("calls")
            .pop()
            .expect("a call");
        assert!(
            self.first_mismatch(&wrong).is_some(),
            "a missing call is caught"
        );
        let mut wrong = expected.clone();
        let fly = wrong["states"]
            .as_array_mut()
            .expect("states")
            .iter_mut()
            .rev()
            .find_map(|s| {
                s["state"]["flies"]
                    .as_object_mut()
                    .and_then(|f| f.values_mut().find(|f| f["pos"].is_array()))
            })
            .expect("a realized fly in a state");
        fly["pos"][0] = json!(fly["pos"][0].as_f64().expect("x") + 0.001);
        assert!(
            self.first_mismatch(&wrong).is_some(),
            "a perturbed fly position is caught"
        );
    }

    /// How many calls of each code the run made.
    fn count(&self, code: &str) -> usize {
        (0..self.calls.len())
            .filter(|&i| self.code(i) == code)
            .count()
    }
}

/// 32-bit FNV-1a over little-endian `u32` words.
struct Fnv1a(u32);

impl Fnv1a {
    fn new() -> Self {
        Self(0x811C_9DC5)
    }

    fn push(&mut self, word: u32) {
        for b in word.to_le_bytes() {
            self.0 = (self.0 ^ u32::from(b)).wrapping_mul(0x0100_0193);
        }
    }
}

/// Field-by-field comparison of the port's state with the model's.
struct StateDiff;

impl StateDiff {
    /// The first field the port writes that the model's state lacks or holds otherwise:
    /// floats as f32 (0 equal to -0), the rest exactly.
    fn first(port: &Value, model: &Value, at: &str) -> Option<String> {
        match (port, model) {
            (Value::Object(p), Value::Object(m)) => p.iter().find_map(|(k, v)| match m.get(k) {
                Some(w) => Self::first(v, w, &format!("{at}/{k}")),
                None => Some(format!("{at}/{k}: missing in the model")),
            }),
            (Value::Array(p), Value::Array(m)) if p.len() == m.len() => p
                .iter()
                .zip(m)
                .enumerate()
                .find_map(|(i, (v, w))| Self::first(v, w, &format!("{at}[{i}]"))),
            (Value::Number(p), Value::Number(m)) if p.is_f64() || m.is_f64() => {
                let (p, m) = (p.as_f64().map(|f| f as f32), m.as_f64().map(|f| f as f32));
                let same = p == m || p.zip(m).is_some_and(|(p, m)| p.is_nan() && m.is_nan());
                (!same).then(|| format!("{at}: port {p:?}, model {m:?}"))
            }
            _ => (port != model).then(|| format!("{at}: port {port}, model {model}")),
        }
    }
}

/// The tutorial route (a new Survivor game, seed 1, food 0, SU_C04 to the shelter SU_S01):
/// the world process every tick, the abstract flies' moves, and SU_A42's five flies
/// realized (never viewed) while the player is in SU_A41.
#[test]
fn flies_route_matches_reference_model() {
    let expected: Value =
        serde_json::from_str(include_str!("../../tests/data/flies_route_model.json"))
            .expect("model output");
    let Some(mut t) = FlyTrace::tutorial() else {
        return;
    };
    t.assert_matches(&expected, "flies route");
    assert!(t.count("r") > 0, "rooms ready for their flies");
    assert!(t.count("f") > 1000, "the realized flies update");
}

/// The tutorial route into SU_A41, 400 ticks walking about SU_A42 among its five flies
/// (threats, fleeing, burrowing), out to SU_C04 (SU_A42 abstracted) and back in.
#[test]
fn flies_in_su_a42_match_reference_model() {
    let expected: Value =
        serde_json::from_str(include_str!("../../tests/data/flies_a42_model.json"))
            .expect("model output");
    let Some(mut t) = FlyTrace::run(400) else {
        return;
    };
    t.assert_matches(&expected, "SU_A42");
    assert!(t.count("n") > 0 && t.count("x") > 0, "viewed and left");
    assert!(
        t.count("a") > 0 && t.count("h") > 0,
        "abstracted; hover sounds"
    );
}

/// A game in SU_A23, 3000 ticks about the swarm room SU_A24, whose flies leave it and
/// arrive through its exits (vessels, the lobby), out to SU_A41 and back.
#[test]
fn flies_through_shortcuts_match_reference_model() {
    let expected: Value =
        serde_json::from_str(include_str!("../../tests/data/flies_a24_model.json"))
            .expect("model output");
    let Some(mut t) = FlyTrace::run_a24(3000) else {
        return;
    };
    t.assert_matches(&expected, "SU_A24");
    assert!(
        t.count("v") > 0 && t.count("l") > 0,
        "flies in the shortcuts"
    );
    assert!(t.count("z") > 0, "abstract flies realized");
}

impl FlyTrace {
    /// Whether a fly's first bite kills it: the first state with 2 bites left has it
    /// dead, the one before it alive.
    fn first_bite_kills(&self) -> bool {
        let mut alive_before = None;
        for c in &self.calls {
            let Some(state) = &c.state else { continue };
            for (_, creature, _) in &state.flies {
                let Some(f) = creature.fly() else { continue };
                match f.bites {
                    3 => alive_before = Some(!f.dead),
                    2 => return f.dead && alive_before == Some(true),
                    _ => {}
                }
            }
        }
        false
    }

    /// The (food, quarter points) the player went through, in order.
    fn foods(&self) -> Vec<(i32, i32)> {
        let mut out: Vec<(i32, i32)> = Vec::new();
        for p in self.calls.iter().filter_map(|c| c.player.as_ref()) {
            let food = (p.food, p.quarter_food);
            if out.last() != Some(&food) {
                out.push(food);
            }
        }
        out
    }

    /// The model's fixture with the food of its last state with a player changed.
    fn with_wrong_food(expected: &Value) -> Value {
        let mut wrong = expected.clone();
        let player = wrong["states"]
            .as_array_mut()
            .expect("states")
            .iter_mut()
            .rev()
            .find_map(|s| s["state"].get_mut("player").filter(|p| p.is_object()))
            .expect("a state with the player");
        player["food"] = json!(player["food"].as_i64().expect("food") + 1);
        wrong
    }
}

/// A Survivor in SU_A42 (entered from SU_A41) put under the flies until it holds one
/// (it takes a second one with its other hand in the same tick), then eating: three
/// bites each (the first kills the fly), `WaterDrip`s, the flies destroyed, a full food
/// pip each (four quarter points) and the food meter's new pips. With the eat counter
/// still low the second fly is eaten without holding pick-up, as in `GrabUpdate`.
#[test]
fn flies_eaten_match_reference_model() {
    let expected: Value =
        serde_json::from_str(include_str!("../../tests/data/flies_eat_model.json"))
            .expect("model output");
    let Some(mut t) = FlyTrace::caught("White").map(FlyTrace::eaten) else {
        return;
    };
    t.assert_matches(&expected, "eating");
    assert!(
        t.first_mismatch(&FlyTrace::with_wrong_food(&expected))
            .is_some(),
        "a wrong food count is caught"
    );
    assert!(t.count("g") > 0 && t.count("j") > 0 && t.count("w") > 0);
    assert!(t.first_bite_kills(), "the first bite kills the fly");
    let state = t.session_mut().state().expect("state");
    let player = state.player.expect("player");
    assert_eq!(
        (player.food, player.quarter_food),
        (2, 0),
        "a full pip each"
    );
    assert_eq!(t.foods(), [(0, 0), (1, 0), (2, 0)], "food 0, 1, 2");
    let hud = t.session_mut().hud_state().expect("hud");
    assert_eq!(hud.food_meter.show_count, 2, "the meter shows the pips");
}

/// The same as the Hunter: a bat fly is a single quarter point.
#[test]
fn hunter_eats_a_quarter_pip() {
    let expected: Value =
        serde_json::from_str(include_str!("../../tests/data/flies_eat_red_model.json"))
            .expect("model output");
    let Some(mut t) = FlyTrace::caught("Red").map(FlyTrace::eaten) else {
        return;
    };
    t.assert_matches(&expected, "the Hunter eating");
    let state = t.session_mut().state().expect("state");
    let player = state.player.expect("player");
    assert_eq!(t.foods(), [(0, 0), (0, 1), (0, 2)], "a quarter pip per fly");
    assert_eq!((player.food, player.quarter_food), (0, 2));
    let hud = t.session_mut().hud_state().expect("hud");
    assert_eq!(
        (hud.food_meter.show_count, hud.food_meter.quarter_pips),
        (0, 2),
        "the meter shows the quarter pips"
    );
}

/// A fly caught in SU_A42, carried down its exit to SU_A41 (pulled in, out of the room,
/// moved with the player, spat out with it) and thrown there.
#[test]
fn carried_fly_matches_reference_model() {
    let expected: Value =
        serde_json::from_str(include_str!("../../tests/data/flies_carry_model.json"))
            .expect("model output");
    let Some(mut t) = FlyTrace::carried() else {
        return;
    };
    t.assert_matches(&expected, "carrying");
    for code in ["P", "S", "V", "O", "M"] {
        assert!(t.count(code) > 0, "{code} calls");
    }
    let state = t.session_mut().state().expect("state");
    assert_eq!(state.player_room, "SU_A41");
    let player = state.player.expect("player");
    assert_eq!(
        player.grasps,
        [None, Some(39)],
        "the first hand's fly thrown"
    );
}
