//! `Music.PlayerThreatTracker` (room switches, the drone level, ghost and laugh modes)
//! and its `ThreatDetermination`, whose threat level is not ported yet. Behaviour:
//! `rainworld-re/notes/music.md` section 3.

use rw_random::UnityRandom;

use super::player::{MusicPlayer, MusicProcess};

/// What `PlayerThreatTracker.Update` reads of the game while `Players[0]` is a realized
/// player in a room (a dead one too).
#[derive(Debug, Clone, PartialEq)]
pub struct GameMusicView {
    /// `player.room.abstractRoom.index`.
    pub room: i32,
    /// `GameOverModeActive || player.redsIllness != null`.
    pub game_over: bool,
    /// The room's `BkgDroneVolume`.
    pub bkg_drone_volume: f32,
    /// `!MusicAllowed` and the room has rain danger: no drone.
    pub rain_mutes_drone: bool,
    pub single_room_world: bool,
    /// The region's `proceduralMusicBank`, else its name.
    pub music_bank: String,
}

/// `ThreatDetermination`: only the state other code reads; `Update` (the threat of the
/// creatures in the player's room) is not ported, so the threat stays 0.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ThreatDetermination {
    pub current_threat: f32,
    pub current_music_agnostic_threat: f32,
    pub threat_decline_counter: i32,
}

/// `PlayerThreatTracker` for player 0.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreatTracker {
    pub threat_determine: ThreatDetermination,
    pub recommended_drone_volume: f32,
    pub current_threat: f32,
    pub current_music_agnostic_threat: f32,
    pub room: i32,
    pub last_room: i32,
    pub last_last_room: i32,
    pub room_switches: i32,
    pub room_switch_delay: i32,
    /// The procedural bank last asked for (`region`).
    pub region: Option<String>,
    /// Echo proximity (`cameras[0].ghostMode`): no echoes are ported, so 0.
    pub ghost_mode: f32,
    /// The Watcher's spinning tops; 0.
    pub spinning_top_laugh_mode: f32,
}

impl ThreatTracker {
    #[must_use]
    pub fn new() -> Self {
        Self {
            threat_determine: ThreatDetermination::default(),
            recommended_drone_volume: 0.3,
            current_threat: 0.0,
            current_music_agnostic_threat: 0.0,
            room: -1,
            last_room: -1,
            last_last_room: -1,
            room_switches: 0,
            room_switch_delay: 200,
            region: None,
            ghost_mode: 0.0,
            spinning_top_laugh_mode: 0.0,
        }
    }

    fn zero(&mut self) {
        self.recommended_drone_volume = 0.0;
        self.current_threat = 0.0;
        self.current_music_agnostic_threat = 0.0;
    }

    /// A new room counts as a switch unless it is the one before last; returns the
    /// bank to ask `NewRegion` for when the switch changes it.
    fn switch_room(&mut self, view: &GameMusicView) -> Option<String> {
        if view.room == self.room {
            return None;
        }
        self.last_last_room = self.last_room;
        self.last_room = self.room;
        self.room = view.room;
        if self.room == self.last_last_room {
            return None;
        }
        self.room_switches += 1;
        if self.region.as_deref() == Some(view.music_bank.as_str()) {
            return None;
        }
        self.region = Some(view.music_bank.clone());
        self.region.clone()
    }

    /// The room-switch delay counts down while switches are pending; at its end one
    /// switch is spent, the next delay drawn, and the songs told (`true`).
    fn count_down_room_switch(&mut self, rng: &mut UnityRandom) -> bool {
        if self.room_switches <= 0 || self.room_switch_delay <= 0 {
            return false;
        }
        self.room_switch_delay -= 1;
        if self.room_switch_delay >= 1 {
            return false;
        }
        self.room_switch_delay = rng.range_i32(80, 400);
        self.room_switches -= 1;
        true
    }
}

impl Default for ThreatTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl MusicPlayer {
    /// `PlayerThreatTracker.Update`, the last step of [`Self::update`]: the drone level
    /// from the player's room, a room switch per new room (not counting a return to the
    /// room before last), and after each switch's delay `PlayerToNewRoom` on the songs
    /// and a new delay of `Random.Range(80, 400)` ticks.
    pub(crate) fn threat_tracker_update(&mut self, rng: &mut UnityRandom) {
        let in_game = self.process.is_some_and(MusicProcess::is_game);
        let view = self.game_view.clone();
        let Some(tracker) = &mut self.threat_tracker else {
            return;
        };
        if !in_game {
            tracker.zero();
            tracker.region = None;
            return;
        }
        let Some(view) = view else {
            return;
        };
        if view.game_over {
            tracker.zero();
            return;
        }
        tracker.recommended_drone_volume = if view.rain_mutes_drone {
            0.0
        } else {
            view.bkg_drone_volume
        };
        tracker.ghost_mode = 0.0;
        tracker.spinning_top_laugh_mode = 0.0;
        if !view.single_room_world {
            if let Some(bank) = tracker.switch_room(&view) {
                self.new_region(&bank);
            }
            if self
                .threat_tracker
                .as_mut()
                .is_some_and(|t| t.count_down_room_switch(rng))
            {
                for song in [&mut self.song, &mut self.next_song].into_iter().flatten() {
                    song.player_to_new_room();
                }
            }
        }
        let has_song = self.song.is_some();
        let Some(tracker) = &mut self.threat_tracker else {
            return;
        };
        if has_song {
            tracker.threat_determine.current_threat = 0.0;
        }
        tracker.current_threat = tracker.threat_determine.current_threat;
        tracker.current_music_agnostic_threat =
            tracker.threat_determine.current_music_agnostic_threat;
    }
}
