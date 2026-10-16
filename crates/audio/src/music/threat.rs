//! `Music.PlayerThreatTracker` (room switches, the drone level, ghost and laugh modes)
//! and its `ThreatDetermination` (the threat of the creatures around the player).
//! Behaviour: `rainworld-re/notes/music.md` section 3.

use rw_assets::CreatureType;
use rw_random::UnityRandom;
use rw_sim::creature::{CreatureDanger, EntityId};
use rw_sim::{FloatExt, INPUT_HISTORY, Input, Vec2};

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
    /// The player's `mainBodyChunk.pos`.
    pub player_pos: Vec2,
    /// `player.input`, newest first.
    pub input: [Input; INPUT_HISTORY],
    /// The other realized creatures of the player's abstract room, in its order.
    pub creatures: Vec<CreatureDanger>,
}

/// A creature's `ThreatOfCreature` in the last update.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreatureThreat {
    pub id: EntityId,
    pub kind: CreatureType,
    pub threat: f32,
}

/// `ThreatDetermination` for player 0. It draws no random numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreatDetermination {
    pub threat_decline_counter: i32,
    pub current_threat: f32,
    pub threat: f32,
    pub music_agnostic_threat_decline_counter: i32,
    pub current_music_agnostic_threat: f32,
    pub music_agnostic_threat: f32,
    pub room: i32,
    pub last_room: i32,
    pub last_last_room: i32,
    /// Each creature's threat in the last update (for inspection).
    pub creatures: Vec<CreatureThreat>,
}

impl Default for ThreatDetermination {
    fn default() -> Self {
        Self {
            threat_decline_counter: 0,
            current_threat: 0.0,
            threat: 0.0,
            music_agnostic_threat_decline_counter: 0,
            current_music_agnostic_threat: 0.0,
            music_agnostic_threat: 0.0,
            room: -1,
            last_room: -1,
            last_last_room: -1,
            creatures: Vec::new(),
        }
    }
}

impl ThreatDetermination {
    /// `Update` while `Players[0]` is a realized player in a room (`view`), with
    /// camera 0's `ghostMode`. No world ghost is ported, so any ghost mode counts as 1.
    pub fn update(&mut self, view: &GameMusicView, ghost_mode: f32) {
        if view.game_over {
            self.current_threat = 0.0;
            self.current_music_agnostic_threat = 0.0;
            return;
        }
        let mut drone = view.bkg_drone_volume;
        if view.rain_mutes_drone {
            drone = 0.0;
        }
        let ghost = if ghost_mode > 0.0 { 1.0 } else { 0.0 };
        if ghost > 0.0 {
            drone = 0.0;
        }
        if !view.single_room_world && view.room != self.room {
            self.last_last_room = self.last_room;
            self.last_room = self.room;
            self.room = view.room;
            if self.room != self.last_last_room {
                self.threat_decline_counter = self.threat_decline_counter.max(120);
                self.music_agnostic_threat_decline_counter =
                    self.music_agnostic_threat_decline_counter.max(120);
            }
        }
        self.threat = 0.0;
        self.music_agnostic_threat = 0.0;
        self.creatures.clear();
        for c in &view.creatures {
            let t = Self::threat_of_creature(c, view.player_pos);
            if drone > 0.0 {
                self.threat = (self.threat + t).lerp(self.threat.max(t), 0.75);
            }
            self.music_agnostic_threat =
                (self.music_agnostic_threat + t).lerp(self.music_agnostic_threat.max(t), 0.75);
            self.creatures.push(CreatureThreat {
                id: c.id,
                kind: c.kind,
                threat: t,
            });
        }
        let power = (Self::input_changes(&view.input) as f32).lerp_map(5.0, 30.0, 1.2, 0.8);
        if drone > 0.0 {
            self.threat = self.threat.clamp(0.0, 1.0).mathf_pow(power);
        }
        self.music_agnostic_threat = self.music_agnostic_threat.clamp(0.0, 1.0).mathf_pow(power);
        if ghost > 0.0 {
            self.music_agnostic_threat = 0.0;
        }
        self.threat = Self::hump(self.threat);
        self.music_agnostic_threat = Self::hump(self.music_agnostic_threat);
        if self.threat_decline_counter > 0 {
            self.threat_decline_counter -= 1;
        }
        if self.music_agnostic_threat_decline_counter > 0 {
            self.music_agnostic_threat_decline_counter -= 1;
        }
        if self.threat < self.current_threat - 0.35 {
            self.threat_decline_counter = self.threat_decline_counter.max(10);
        }
        if self.music_agnostic_threat < self.current_music_agnostic_threat - 0.35 {
            self.music_agnostic_threat_decline_counter =
                self.music_agnostic_threat_decline_counter.max(10);
        }
        self.current_threat = Self::follow(
            self.current_threat,
            self.threat,
            self.threat_decline_counter,
        );
        self.current_music_agnostic_threat = Self::follow(
            self.current_music_agnostic_threat,
            self.music_agnostic_threat,
            self.music_agnostic_threat_decline_counter,
        );
    }

    /// The input changes over the history against the newest input: a direction or the
    /// pick-up button 1, jump or throw 2.
    #[must_use]
    pub fn input_changes(input: &[Input; INPUT_HISTORY]) -> i32 {
        let now = input[0];
        input[1..]
            .iter()
            .map(|i| {
                i32::from(i.x != now.x)
                    + i32::from(i.y != now.y)
                    + 2 * i32::from(i.jump != now.jump)
                    + 2 * i32::from(i.throw != now.throw)
                    + i32::from(i.pickup != now.pickup)
            })
            .sum()
    }

    /// Between 0.25 and 0.75 a threat is drawn toward 0.5.
    fn hump(t: f32) -> f32 {
        if t > 0.25 && t < 0.75 {
            t.lerp(0.5, (t - 0.5).abs().inverse_lerp(0.25, 0.0))
        } else {
            t
        }
    }

    /// The current level rises toward the threat fast and falls slowly, faster while
    /// the decline counter runs.
    fn follow(current: f32, threat: f32, decline: i32) -> f32 {
        if current < threat {
            (current + 1.0 / 280.0.lerp(80.0, threat)).min(1.0)
        } else if decline > 0 {
            (current - 1.0 / 800.0.lerp(4200.0, threat)).max(0.0)
        } else {
            (current - 1.0 / 1600.0.lerp(22000.0, threat.mathf_pow(0.25))).max(0.0)
        }
    }

    /// `ThreatOfCreature`: the template's danger, a third of it when dead; else fading
    /// with distance from 300 to 2400 px (to 0.2 while the creature sees the player),
    /// up to doubled when close and fast, scaled by how well it knows where the player
    /// is and by its aggression. Ripple layers are the Watcher's: every base-game
    /// creature shares the player's.
    #[must_use]
    pub fn threat_of_creature(c: &CreatureDanger, player_pos: Vec2) -> f32 {
        let mut danger = c.dangerous_to_player;
        if danger == 0.0 {
            return 0.0;
        }
        if c.dead {
            return danger / 3.0;
        }
        let (visual, chance) = c.player_rep.map_or((false, 0.0), |r| {
            (r.visual_contact, r.estimated_chance_of_finding)
        });
        let dist = c.danger_pos.distance(player_pos);
        danger *= dist.lerp_map(300.0, 2400.0, 1.0, if visual { 0.2 } else { 0.0 });
        danger *= 1.0
            + dist.inverse_lerp(300.0, 20.0) * c.first_chunk_vel.length().inverse_lerp(2.0, 7.0);
        danger *= (1.0_f32 / 3.0).lerp(1.0, chance.mathf_pow(0.75));
        if let Some(aggression) = c.player_aggression {
            danger *= aggression;
        }
        danger
    }
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
                self.new_region(&bank, rng);
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
        let ghost_mode = tracker.ghost_mode;
        tracker.threat_determine.update(&view, ghost_mode);
        if has_song {
            tracker.threat_determine.current_threat = 0.0;
        }
        tracker.current_threat = tracker.threat_determine.current_threat;
        tracker.current_music_agnostic_threat =
            tracker.threat_determine.current_music_agnostic_threat;
    }
}

#[cfg(test)]
#[path = "threat_tests.rs"]
mod tests;
