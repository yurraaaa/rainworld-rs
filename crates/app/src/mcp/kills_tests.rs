//! The cycle's kills after a continued cycle, on a constructed save in temporary
//! directories: the sleep screen's trophies (`SleepScreenKills`) and `AppendKills` in
//! the save. The kills are put in the session record as `PlayerSessionRecord.AddKill`
//! leaves them (the kills themselves are rw-sim's tests). Skipped without the installed
//! game. With `KILL_SHOTS=<dir>` the screen is also saved as `kills.png` there.

use std::path::Path;

use rw_assets::CreatureType;
use rw_save::PlayerProgression;
use rw_sim::creature::{EntityId, SessionKill};

use super::passage_tests::{SAVE, WHITE, continued_with};
use crate::saves::SaveConfig;

#[test]
fn the_sleep_screen_shows_kill_trophies_and_the_save_counts_the_kills() {
    let Some((mut s, assets, port)) = continued_with(SAVE) else {
        return;
    };
    let game = s.frontend_mut().unwrap().game_mut().expect("the game");
    let session = &mut game.scene.game_mut().sim_mut().world_mut().session;
    for (kind, n) in [
        (CreatureType::Fly, 900),
        (CreatureType::Overseer, 901),
        (CreatureType::Overseer, 902),
    ] {
        session.kills.push(SessionKill {
            kind,
            int_data: 0,
            id: EntityId::new(-1, n),
            lizard: false,
        });
    }
    session.peaceful = false;
    let screen = s.sleep();
    // A fly is quantified: no trophy. The two overseers collapse.
    let kills = screen.kills.expect("the kills display");
    let kinds: Vec<&str> = kills.trophies.iter().map(|t| t.0.as_str()).collect();
    assert_eq!(kinds, ["Overseer", "Overseer"]);
    assert!(kills.trophies.iter().all(|t| !t.2));
    // The wait of 40, then a trophy every eighth tick.
    let mut frames = 0;
    let done = loop {
        let state = s.screen();
        frames += 1;
        let kills = state.kills.expect("the kills display");
        if kills.counted_and_done {
            break kills;
        }
        assert!(frames < 200, "never counted");
    };
    assert!(done.trophies.iter().all(|t| t.2));
    assert!(frames > 40, "{frames}");
    if let Some(dir) = std::env::var_os("KILL_SHOTS") {
        s.frames(10, &[]);
        s.process_shot(Path::new(&dir), "kills");
    }
    // `AppendKills` at `Win`: by symbol, each record once.
    let sav = port.path().join("sav");
    let mut p = PlayerProgression::load(&sav, SaveConfig::salt(&assets)).expect("loads");
    let state = p.peek_save_state(WHITE).expect("reads").expect("a save");
    assert_eq!(
        state.kills,
        [
            ("Fly-Creature-0".to_owned(), 1),
            ("Overseer-Creature-0".to_owned(), 2)
        ]
    );
}
