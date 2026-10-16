//! What `get_state` and `step_process` report of the music player and the room
//! triggers.

use rw_audio::music::{MusicPlayer, Song};
use rw_save::SongPlayRecord;
use rw_sim::RainWorldGame;
use rw_sim::trigger::{TriggerObject, TriggerObjectKind};
use serde::Serialize;

/// `manager.musicPlayer`.
#[derive(Debug, Clone, Serialize)]
pub struct MusicState {
    pub context: String,
    pub song: Option<SongState>,
    pub next_song: Option<SongState>,
    pub main_song_mix: f32,
    pub drone_goal_mix: f32,
    pub has_played_a_song_this_cycle: bool,
    /// `nextProcedural` (`""`: none asked for).
    pub next_procedural: Option<String>,
    pub threat: Option<ThreatState>,
    /// The save state's `songsPlayRecords`, when a story save is loaded.
    pub play_records: Option<Vec<PlayRecordState>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SongState {
    pub name: String,
    pub priority: f32,
    pub base_volume: f32,
    pub fade_in_time: f32,
    /// Negative when not fading out.
    pub fade_out_time: f32,
    pub room_transitions: i32,
    pub looped: bool,
    pub started_playing: bool,
}

/// `PlayerThreatTracker`.
#[derive(Debug, Clone, Serialize)]
pub struct ThreatState {
    pub room: i32,
    pub room_switches: i32,
    pub room_switch_delay: i32,
    pub region: Option<String>,
    pub recommended_drone_volume: f32,
    pub current_threat: f32,
    pub current_music_agnostic_threat: f32,
    /// `ThreatDetermination`'s tick threat, before smoothing into the current one.
    pub threat: f32,
    pub music_agnostic_threat: f32,
    pub threat_decline_counter: i32,
    pub music_agnostic_threat_decline_counter: i32,
    /// Each realized creature around the player and its `ThreatOfCreature`.
    pub creatures: Vec<CreatureThreatState>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreatureThreatState {
    pub kind: String,
    /// EntityID (spawner, number).
    pub id: [i32; 2],
    pub threat: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayRecordState {
    pub song: String,
    pub cycle_last_played: i32,
}

/// A trigger checker, or an object a trigger's event added.
#[derive(Debug, Clone, Serialize)]
pub struct TriggerState {
    pub room: String,
    /// `checker`, `pickup_instruction` or `text_message`.
    pub kind: &'static str,
    /// The checker's event type.
    pub event: Option<String>,
    pub counter: Option<i32>,
    pub wait: Option<i32>,
}

impl MusicState {
    #[must_use]
    pub fn of(player: &MusicPlayer, records: Option<&[SongPlayRecord]>) -> Self {
        Self {
            context: format!("{:?}", player.music_context),
            song: player.song.as_ref().map(SongState::of),
            next_song: player.next_song.as_ref().map(SongState::of),
            main_song_mix: player.main_song_mix,
            drone_goal_mix: player.drone_goal_mix,
            has_played_a_song_this_cycle: player.has_played_a_song_this_cycle,
            next_procedural: player.next_procedural.clone(),
            threat: player.threat_tracker.as_ref().map(|t| ThreatState {
                room: t.room,
                room_switches: t.room_switches,
                room_switch_delay: t.room_switch_delay,
                region: t.region.clone(),
                recommended_drone_volume: t.recommended_drone_volume,
                current_threat: t.current_threat,
                current_music_agnostic_threat: t.current_music_agnostic_threat,
                threat: t.threat_determine.threat,
                music_agnostic_threat: t.threat_determine.music_agnostic_threat,
                threat_decline_counter: t.threat_determine.threat_decline_counter,
                music_agnostic_threat_decline_counter: t
                    .threat_determine
                    .music_agnostic_threat_decline_counter,
                creatures: t
                    .threat_determine
                    .creatures
                    .iter()
                    .map(|c| CreatureThreatState {
                        kind: c.kind.value().to_owned(),
                        id: [c.id.spawner, c.id.number],
                        threat: c.threat,
                    })
                    .collect(),
            }),
            play_records: records.map(|r| {
                r.iter()
                    .map(|r| PlayRecordState {
                        song: r.song_name.clone(),
                        cycle_last_played: r.cycle_last_played,
                    })
                    .collect()
            }),
        }
    }
}

impl SongState {
    fn of(s: &Song) -> Self {
        Self {
            name: s.name.clone(),
            priority: s.priority,
            base_volume: s.base_volume,
            fade_in_time: s.fade_in_time,
            fade_out_time: s.fade_out_time,
            room_transitions: s.room_transitions,
            looped: s.looped,
            started_playing: s.started_playing,
        }
    }
}

impl TriggerState {
    /// The game's live trigger objects.
    #[must_use]
    pub fn all(sim: &RainWorldGame) -> Vec<Self> {
        sim.trigger_objects()
            .iter()
            .map(|o| Self::of(sim, o))
            .collect()
    }

    fn of(sim: &RainWorldGame, o: &TriggerObject) -> Self {
        let world = sim.world();
        let room = world
            .abstract_room(o.room)
            .map(|r| r.name.clone())
            .unwrap_or_default();
        match &o.kind {
            TriggerObjectKind::Checker(c) => Self {
                event: world
                    .realized(o.room)
                    .and_then(|r| r.settings.triggers.get(c.trigger))
                    .and_then(|t| t.event.as_ref())
                    .map(|e| match e.event_type() {
                        Some(t) => t.value().to_owned(),
                        None => "unknown".to_owned(),
                    }),
                room,
                kind: "checker",
                counter: Some(c.counter),
                wait: Some(c.wait),
            },
            TriggerObjectKind::PickupInstruction(_) => Self {
                room,
                kind: "pickup_instruction",
                event: None,
                counter: None,
                wait: None,
            },
            TriggerObjectKind::SuperJumpInstruction(_) => Self {
                room,
                kind: "super_jump_instruction",
                event: None,
                counter: None,
                wait: None,
            },
            TriggerObjectKind::ScavTradeInstruction(_) => Self {
                room,
                kind: "scav_trade_instruction",
                event: None,
                counter: None,
                wait: None,
            },
            TriggerObjectKind::ReliableIggyDirection(_) => Self {
                room,
                kind: "reliable_iggy_direction",
                event: None,
                counter: None,
                wait: None,
            },
            TriggerObjectKind::TextMessage => Self {
                room,
                kind: "text_message",
                event: None,
                counter: None,
                wait: None,
            },
        }
    }
}
