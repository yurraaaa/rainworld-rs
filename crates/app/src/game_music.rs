//! The game's side of the music player: the HUD state the trigger checkers read, the
//! sim's song requests (`GameRequestsSong` with the save's play records,
//! `GameRequestsSongStop`, `RainRequestStopSong`), and what `PlayerThreatTracker` reads
//! of the game.
//!
//! Mirrors: Music/MusicPlayer.cs, Music/PlayerThreatTracker.cs, RainWorldGame.cs

use rw_assets::DangerType;
use rw_audio::music::{GameMusicView, HudSubregion, MusicPlayer, ProceduralView, StorySongs};
use rw_menu::MenuShared;
use rw_sim::trigger::{MusicRequest, TriggerHud};

use super::Scene;

impl Scene {
    /// Camera 0's HUD as the checkers read it during the coming tick.
    pub(super) fn link_trigger_hud(&mut self) {
        let hud = self
            .hud
            .as_ref()
            .map_or_else(TriggerHud::default, |h| TriggerHud {
                game_over_mode: h.text_prompt.game_over_mode,
                region_bumps: h
                    .text_prompt
                    .subregion_tracker
                    .as_ref()
                    .map(|t| (t.pre_region_bump(), t.region_bump())),
            });
        self.game.sim_mut().set_trigger_hud(hud);
    }

    /// The tick's requests, in order, to `manager.musicPlayer` (none without one). A
    /// story game's song requests read and write its save state's play records.
    pub(super) fn apply_music_requests(&mut self, shared: &mut MenuShared) {
        let requests = self.game.sim_mut().take_music_requests();
        let story = self.game.story_session();
        let MenuShared { music, saves, .. } = shared;
        let Some(music) = music else {
            return;
        };
        for request in requests {
            match request {
                MusicRequest::Song(event) => {
                    let songs = saves
                        .current_save_state
                        .as_mut()
                        .filter(|_| story)
                        .map(|s| StorySongs {
                            records: &mut s.death_persistent.songs_play_records,
                            cycle_number: s.cycle_number,
                        });
                    music.game_requests_song(&event, songs);
                }
                MusicRequest::Stop(event) => music.game_requests_song_stop(&event),
                MusicRequest::SsSong { amount } => music.ss_music_trigger(amount),
                MusicRequest::RainStop => {
                    if music.song.is_some() || music.next_song.is_some() {
                        music.rain_request_stop_song();
                    }
                }
            }
        }
    }

    /// What the threat tracker reads: `None` unless `Players[0]` is a realized player
    /// in a room (not in a shortcut; a dead body counts).
    #[must_use]
    pub fn music_view(&self) -> Option<GameMusicView> {
        let sim = self.game.sim();
        let player = sim.player().filter(|p| !p.in_shortcut())?;
        let room = sim.player_pos().room;
        let world = sim.world();
        let settings = &world.realized(room)?.settings;
        let game_over = self
            .hud
            .as_ref()
            .is_some_and(|h| h.text_prompt.game_over_mode);
        let bank = world.properties().params.procedural_music_bank();
        let music_bank = if bank.is_empty() {
            world.region_name().unwrap_or("").to_owned()
        } else {
            bank.to_owned()
        };
        Some(GameMusicView {
            room,
            game_over,
            bkg_drone_volume: settings.bkg_drone_volume,
            rain_mutes_drone: !world.music_allowed()
                && settings.danger_type != Some(DangerType::None),
            single_room_world: world.single_room(),
            music_bank,
            player_pos: player.body.chunks[0].pos,
            input: *player.input_history(),
            creatures: sim.creatures_around_player(),
        })
    }

    /// What the procedural piece reads: the region's subregions, camera 0's room and
    /// HUD, and whether the rain has come.
    #[must_use]
    pub fn procedural_view(&self) -> ProceduralView {
        let sim = self.game.sim();
        let world = sim.world();
        let subregions = self.game.subregions();
        let camera_room = world
            .abstract_room(sim.camera_room())
            .and_then(|r| subregions.and_then(|s| s.room_subregion(&r.name)))
            .map(str::to_owned);
        let hud = match &self.hud {
            None => HudSubregion::None,
            Some(h) => match &h.text_prompt.subregion_tracker {
                None => HudSubregion::NoTracker,
                Some(t) => HudSubregion::Tracker {
                    last_shown_region: t.last_shown_region,
                },
            },
        };
        ProceduralView {
            sub_regions: subregions.map(|s| s.names.clone()),
            camera_room: true,
            camera_room_subregion: camera_room,
            hud,
            night: world.rain_cycle.amount_left() < 0.0,
            mushroom_mode: 0.0,
        }
    }

    /// One music tick after the game's: the tracker's and the procedural piece's view
    /// of this game, then `MusicPlayer.Update` drawing from the game's random stream.
    pub fn update_music(
        &mut self,
        music: &mut MusicPlayer,
        sources: &mut dyn rw_audio::music::MusicSources,
    ) {
        music.set_game_view(self.music_view());
        music.set_procedural_view(Some(self.procedural_view()));
        music.update(sources, self.game.sim_mut().rng_mut());
    }
}
