//! The user's real save slots, read-only: each is imported into a temporary directory
//! and only that copy is read and written. Skipped without the game or its saves. Set
//! `RW_DIR` for a game outside the default Steam location.

use std::path::{Path, PathBuf};

use rw_assets::SlugcatName;
use rw_save::user_data::UserDataFile;
use rw_save::{CampaignStart, ChecksumSalt, Platform, PlayerProgression, SaveSlot, import_slot};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

fn game_dir() -> PathBuf {
    std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from)
}

fn salt(game: &Path) -> ChecksumSalt {
    ChecksumSalt::from_assembly(&game.join("RainWorld_Data/Managed/Assembly-CSharp.dll"))
        .expect("salt from the assembly")
}

#[test]
fn imported_slots_read_and_round_trip() {
    let game = game_dir();
    let Some(saves) = Platform::current().game_save_dir_here(&game) else {
        eprintln!("skipping: no game save directory on this platform");
        return;
    };
    if !saves.join("sav").is_file() {
        eprintln!("skipping: no save in {}", saves.display());
        return;
    }
    let salt = salt(&game);
    let port = tempfile::tempdir().unwrap();
    let mut white_states = 0;
    for slot in SaveSlot::ALL {
        if !saves.join(slot.file_name()).is_file() {
            continue;
        }
        let report = import_slot(&saves, port.path(), slot).expect("imports");
        assert!(report.destination.starts_with(port.path()));
        // The container round-trips byte for byte, but for the NUL padding.
        let bytes = std::fs::read(&report.destination).unwrap();
        let end = bytes.iter().rposition(|&b| b != 0).unwrap() + 1;
        let file = UserDataFile::parse(&bytes).expect("parses");
        assert_eq!(file.to_bytes(), &bytes[..end], "{}", slot.file_name());

        let mut p = PlayerProgression::load(&report.destination, Some(salt.clone())).unwrap();
        assert!(!p.prog_lines().is_empty());
        assert!(!p.game_tinkered_with, "checksum of {}", slot.file_name());
        for slugcat in [SlugcatName::White, SlugcatName::Yellow, SlugcatName::Red] {
            let start = CampaignStart {
                slugcat,
                den: "SU_C04",
                vanilla: true,
            };
            let Some(state) = p.peek_save_state(start).expect("reads") else {
                continue;
            };
            white_states += usize::from(slugcat == SlugcatName::White);
            let d = &state.death_persistent;
            eprintln!(
                "{} {slugcat}: {} cycle {} food {} karma {}/{}",
                slot.file_name(),
                state.den_position,
                state.cycle_number,
                state.food,
                d.karma,
                d.karma_cap
            );
            assert!(state.den_position.contains('_'));
            assert!(state.cycle_number >= 0);
            assert!((0..=9).contains(&state.food));
            assert!((4..=9).contains(&d.karma_cap) && (0..=d.karma_cap).contains(&d.karma));
            assert!(d.deaths >= 0 && d.quits >= 0);
        }
        // A death save on the copy changes only that state's line.
        if p.is_there_a_saved_game(SlugcatName::White) {
            let before = p.prog_lines();
            let start = CampaignStart {
                slugcat: SlugcatName::White,
                den: "SU_C04",
                vanilla: true,
            };
            p.get_or_initiate_save_state(start, 0, true, true).unwrap();
            let after = p.prog_lines();
            let changed: Vec<&String> = after.iter().filter(|l| !before.contains(l)).collect();
            assert_eq!(changed.len(), 1);
            assert!(changed[0].starts_with("SAVE STATE<progDivB>SAV STATE NUMBER<svB>White"));
            assert!(changed[0].ends_with("<svA><svA>"));
            assert!(
                !PlayerProgression::load(&report.destination, Some(salt.clone()))
                    .unwrap()
                    .game_tinkered_with
            );
        }
    }
    assert!(white_states > 0, "no Survivor save to check");
}
