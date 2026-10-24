//! What the menu processes share across processes (`RainWorld`, `ProcessManager` and
//! Futile's globals, and `UnityEngine.Random`).
//!
//! Mirrors: -
//!   (port-side holder of globals the game keeps on RainWorld, ProcessManager and Futile)

use std::sync::Arc;

use rw_assets::{SoundTable, StreamingAssets, UnityResources};
use rw_audio::music::{
    InstalledProcedural, MusicPlayer, MusicProcess, MusicSettings, ProceduralFiles, SourcePlayback,
    TrackId,
};
use rw_random::UnityRandom;
use rw_render::StageTextureId;
use rw_save::{Options, PlayerProgression, SaveSlot};

use crate::atlas::{AtlasLoadError, AtlasManager};
use crate::input::UiInputState;
use crate::process::ProcessId;
use crate::sound::{MenuMic, MenuSoundId};
use crate::translator::Translator;

/// `SlugcatStats.Name`.
pub type Slugcat = rw_assets::SlugcatName;

/// `ProcessManager.MenuSetup.StoryGameInitCondition`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryGameInitCondition {
    Dev,
    New,
    Load,
    RegionSelect,
    FastTravel,
    StartWithFastTravel,
}

impl StoryGameInitCondition {
    /// `FastTravelInitCondition`.
    #[must_use]
    pub fn fast_travel(self) -> bool {
        matches!(self, Self::FastTravel | Self::StartWithFastTravel)
    }

    /// `RainWorldGame.FadeInTime` with no current save state (with one, `Load` uses
    /// `SaveState::slow_fade_in`).
    #[must_use]
    pub fn game_fade_in_time(self) -> f32 {
        match self {
            c if c.fast_travel() => 0.0,
            Self::New => 2.0,
            Self::Dev => 0.45,
            _ => 0.8,
        }
    }

    /// `RainWorldGame.InitialBlackSeconds`; `selected` is the progression's
    /// `currentlySelectedSinglePlayerSlugcat`.
    #[must_use]
    pub fn game_initial_black_seconds(self, selected: Slugcat) -> f32 {
        match self {
            c if c.fast_travel() => 10.0,
            Self::New if selected == Slugcat::Red => 5.5,
            Self::New => 3.0,
            Self::Dev => 0.0,
            _ => 0.75,
        }
    }
}

/// The part of `PlayerProgression.MiscProgressionData` the menus read.
#[derive(Debug, Clone, PartialEq)]
pub struct Progression {
    /// `currentlySelectedSinglePlayerSlugcat` (White in a new progression).
    pub selected_slugcat: Slugcat,
    /// `redUnlocked`.
    pub red_unlocked: bool,
    /// Every arena level and sandbox token collected (the unlock cheat sets them).
    pub all_tokens_collected: bool,
    /// `AreThereAnyDiscoveredShelters`.
    pub any_discovered_shelters: bool,
}

impl Default for Progression {
    fn default() -> Self {
        Self {
            selected_slugcat: Slugcat::White,
            red_unlocked: false,
            all_tokens_collected: false,
            any_discovered_shelters: false,
        }
    }
}

/// `RainWorld.dlcsInstalled`. The game finds it from files ([`Dlcs::check`]); until DLC
/// support exists the port runs with none installed (docs/divergences/ D13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dlcs {
    pub downpour: bool,
    pub watcher: bool,
}

impl Dlcs {
    /// `DLCOwnershipCheckSwitchPC`: Downpour when its main-menu scene and three DLC
    /// mods' illustrations are on disk, Watcher when two of its files are.
    #[must_use]
    pub fn check(assets: &StreamingAssets) -> Self {
        Self {
            downpour: [
                "scenes/main menu - downpour/main menu - downpour - flat.png",
                "mods/moreslugcats/illustrations/safari_ms.png",
                "mods/jollycoop/illustrations/jolly_title.png",
                "mods/expedition/illustrations/expeditiontitle.png",
            ]
            .iter()
            .all(|p| assets.file(p).is_file()),
            watcher: [
                "mods/watcher/illustrations/au_building_0.png",
                "mods/watcher/scenes/main menu - watcher/main menu watcher - flat.png",
            ]
            .iter()
            .all(|p| assets.file(p).is_file()),
        }
    }
}

/// The game files, loaded atlases and fonts, the translator, the menu input and the
/// state the processes hand on to each other.
pub struct MenuShared {
    pub assets: StreamingAssets,
    pub atlases: AtlasManager,
    pub translator: Translator,
    pub input: UiInputState,
    /// `rainWorld.flatIllustrations`: `StreamingAssets/flatmode.txt` exists (the
    /// installed game has none, so its scenes are in depth).
    pub flat_illustrations: bool,
    /// `rainWorld.screenSize`.
    pub screen_size: [f32; 2],
    /// `UnityEngine.Random`: the one stream every process draws from (D1).
    pub random: UnityRandom,
    /// The menu microphone. The game drops `manager.menuMic` on a switch to the game but
    /// leaves the microphone running as a side process, so the port keeps one running
    /// and tracks whether the manager still holds it in `mic_attached`.
    pub mic: MenuMic,
    pub mic_attached: bool,
    /// `manager.menuesMouseMode`.
    pub mouse_mode: bool,
    /// `rainWorld.options`.
    pub options: Options,
    /// Unity's `Screen` and `Cursor` as the code sets them; the window follows.
    pub screen: ScreenState,
    pub progression: Progression,
    /// `rainWorld.progression`: the save slot (in memory until [`Self::set_saves`]).
    pub saves: PlayerProgression,
    pub dlcs: Dlcs,
    /// `rainWorld.inGameSlugCat`.
    pub in_game_slugcat: Option<Slugcat>,
    /// `manager.menuSetup.startGameCondition`; `None` until a menu sets it (a game
    /// started directly, as `Dev`).
    pub start_game_condition: Option<StoryGameInitCondition>,
    /// `manager.nextSlideshow`.
    pub next_slideshow: Option<crate::SlideShowId>,
    /// `_TextGradientTex` (`Palettes/textGradient`), when it could be read.
    pub text_gradient: Option<StageTextureId>,
    /// `_NoiseTex2` (`Palettes/noise2`), when it could be read.
    pub noise2: Option<StageTextureId>,
    /// `manager.musicPlayer`: a side process once created.
    pub music: Option<MusicPlayer>,
    /// `requestPlayMusic`: `setup.playMusic`, on in the distribution build.
    pub request_play_music: bool,
    pub music_settings: MusicSettings,
    /// The procedural music banks the music player builds its threat layers from.
    pub procedural_files: Arc<dyn ProceduralFiles>,
    /// `Application.Quit()` was called.
    pub quit: bool,
    /// Sounds a menu played without a menu microphone, for the game's microphone
    /// (`cameras[0].virtualMicrophone`, volume group 1) to play and draw for.
    pub game_mic_sounds: Vec<(MenuSoundId, [f32; 3])>,
}

impl MenuShared {
    /// `progression.PlayingAsSlugcat`: the current save state's slugcat, else the
    /// selected one.
    #[must_use]
    pub fn playing_as_slugcat(&self) -> Slugcat {
        self.saves
            .current_save_state
            .as_ref()
            .map_or(self.saves.misc.current_slugcat, |s| s.save_state_number)
    }

    /// Loads the English fonts and strings (`RainWorld.Update` once options are ready)
    /// and the atlases and textures the menus draw with (`RainWorld.LoadResources`).
    ///
    /// # Errors
    /// When the font atlas or a font config cannot be read.
    pub fn load(
        assets: StreamingAssets,
        resources: &UnityResources,
        screen_size: [f32; 2],
    ) -> Result<Self, AtlasLoadError> {
        let mut atlases = AtlasManager::new();
        atlases.load_english_fonts(&assets, resources)?;
        for atlas in ["Atlases/rainWorld", "Atlases/uiSprites"] {
            if let Err(e) = atlases.load_atlas(&assets, resources, atlas) {
                log::warn!("no {atlas} atlas ({e}); menu sprites from it are not drawn");
            }
        }
        let text_gradient = atlases
            .load_resource_texture(resources, "Palettes/textGradient")
            .inspect_err(|e| log::warn!("no text gradient ({e}); titles are drawn white"))
            .ok();
        let noise2 = atlases
            .load_resource_texture(resources, "Palettes/noise2")
            .inspect_err(|e| log::warn!("no noise2 texture ({e})"))
            .ok();
        let translator = Translator::load(&assets).unwrap_or_else(|e| {
            log::warn!("no strings ({e}); text is shown untranslated");
            Translator::default()
        });
        let dlcs = Dlcs::default();
        let flat_illustrations = assets.file("flatmode.txt").exists();
        let procedural_files: Arc<dyn ProceduralFiles> =
            Arc::new(InstalledProcedural::open(&assets));
        Ok(Self {
            procedural_files,
            assets,
            atlases,
            translator,
            flat_illustrations,
            input: UiInputState::default(),
            screen_size,
            random: UnityRandom::new(0),
            mic: MenuMic::default(),
            mic_attached: false,
            mouse_mode: false,
            options: Options::default(),
            screen: ScreenState {
                size: screen_size,
                full_screen: true,
                cursor_visible: false,
            },
            progression: Progression::default(),
            saves: PlayerProgression::in_memory(None),
            dlcs,
            in_game_slugcat: None,
            start_game_condition: None,
            next_slideshow: None,
            text_gradient,
            noise2,
            music: None,
            request_play_music: true,
            music_settings: MusicSettings::new(SoundTable::default().volume_exponent),
            quit: false,
            game_mic_sounds: Vec::new(),
        })
    }

    /// Uses `saves` as the progression, as `LoadProgression` leaves it.
    pub fn set_saves(&mut self, saves: PlayerProgression) {
        self.saves = saves;
        self.refresh_progression();
    }

    /// The menus' view of the progression, after it changed.
    pub fn refresh_progression(&mut self) {
        let misc = &self.saves.misc;
        self.progression.selected_slugcat = misc.current_slugcat;
        self.progression.red_unlocked = misc.red_unlocked();
        self.progression.any_discovered_shelters = misc.any_discovered_shelters();
    }

    /// `Options.OnLoadFinished`: the screen takes the options' size and mode, and the
    /// system cursor shows only in a window (a release build).
    pub fn apply_screen_options(&mut self) {
        let size = self.options.screen_size();
        let full = self.options.full_screen();
        self.screen.size = if full && Options::is_steam_deck_resolution(size) {
            [1280.0, 800.0]
        } else {
            size
        };
        self.screen.full_screen = full;
        self.screen_size = size;
        self.screen.cursor_visible = !full;
    }

    /// The options' music volume, for the music player and its settings.
    pub fn apply_music_volume(&mut self) {
        let volume = self.options.music_volume;
        self.music_settings.music_volume = volume;
        if let Some(music) = &mut self.music {
            music.settings.music_volume = volume;
        }
    }

    /// `currentlySelectedSinglePlayerSlugcat` set, in the menus' view and the save's.
    pub fn select_slugcat(&mut self, slugcat: Slugcat) {
        self.progression.selected_slugcat = slugcat;
        self.saves.misc.current_slugcat = slugcat;
    }

    /// `redUnlocked = true`.
    pub fn unlock_red(&mut self) {
        self.progression.red_unlocked = true;
        self.saves.misc.set_red_unlocked(true);
    }

    /// The menu microphone's clips and the music volume exponent come from `sounds.txt`.
    pub fn set_sound_table(&mut self, table: Arc<SoundTable>) {
        self.music_settings = MusicSettings::new(table.volume_exponent);
        self.mic = MenuMic::new(table);
    }

    /// `ProcessManager.Update`'s first step: the music player is made once the options
    /// are ready (at once in the port) if music is on and its volume above 0.
    pub fn create_music_player(&mut self, current: Option<ProcessId>) {
        if self.request_play_music {
            self.request_play_music = false;
            self.music = self.new_music_player(current.map(ProcessId::music_process));
        }
    }

    /// `new MusicPlayer(manager)` (`None` when music is off), reading the procedural
    /// banks.
    #[must_use]
    pub fn new_music_player(&self, process: Option<MusicProcess>) -> Option<MusicPlayer> {
        MusicPlayer::new(self.music_settings, process)
            .map(|m| m.with_procedural_files(Arc::clone(&self.procedural_files)))
    }

    /// `musicPlayer.UpdateMusicContext(currentMainLoop)` after a switch.
    pub fn update_music_context(&mut self, current: ProcessId) {
        if let Some(music) = &mut self.music {
            music.update_music_context(Some(current.music_process()));
        }
    }

    /// `musicPlayer == null || musicPlayer.assetBundlesLoaded`.
    #[must_use]
    pub fn music_ready(&self) -> bool {
        self.music
            .as_ref()
            .is_none_or(MusicPlayer::asset_bundles_loaded)
    }

    /// `musicPlayer.song` when it is an `IntroRollMusic`.
    pub fn intro_roll_song(&mut self) -> Option<&mut rw_audio::music::Song> {
        self.music
            .as_mut()
            .and_then(|m| m.song.as_mut())
            .filter(|s| s.is_intro_roll())
    }

    /// `manager.menuSetup.startGameCondition`, `Dev` when no menu set it.
    #[must_use]
    pub fn game_start_condition(&self) -> StoryGameInitCondition {
        self.start_game_condition
            .unwrap_or(StoryGameInitCondition::Dev)
    }

    /// `PreSwitchMainProcess`'s microphone handling: let go of it before the game,
    /// take one back before anything else.
    /// `Menu.PlaySound(id, pan, vol, pitch)`: through the menu microphone, else (the
    /// game running) queued for the game's.
    pub fn play_menu_sound(&mut self, id: MenuSoundId, args: [f32; 3]) {
        if self.mic_attached {
            self.mic.play_sound(id, args, &mut self.random);
        } else {
            self.game_mic_sounds.push((id, args));
        }
    }

    pub fn pre_switch(&mut self, next: ProcessId) {
        self.mic_attached = next != ProcessId::Game;
    }

    /// `SlugcatSelectMenu.CheckUnlockRed`: `unlockred.txt` in `StreamingAssets`
    /// (`Custom.RootFolderDirectory`).
    #[must_use]
    pub fn unlock_red_file(&self) -> bool {
        self.assets.path().join("unlockred.txt").is_file()
    }
}

/// Unity's `Screen.width`/`height`, `Screen.fullScreen` and `Cursor.visible`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenState {
    pub size: [f32; 2],
    pub full_screen: bool,
    pub cursor_visible: bool,
}

/// An app context the menu processes can run in.
pub trait MenuContext {
    fn menu(&mut self) -> &mut MenuShared;

    /// The music player's source of `track`, where the context's sources can be read.
    fn music_playback(&self, _track: TrackId) -> Option<SourcePlayback> {
        None
    }

    /// `new PlayerProgression(tryLoad: true)` for `options.saveSlot`.
    fn load_save_slot(&mut self) {}

    /// `options.Save()`.
    fn save_options(&mut self) {}

    /// Copies the game's save of `slot` over the port's (backing it up), then reads it.
    ///
    /// # Errors
    /// What went wrong, for the menu to show.
    fn import_save_slot(&mut self, _slot: SaveSlot) -> Result<(), String> {
        Err("saves cannot be imported here".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `RainWorldGame.FadeInTime` / `InitialBlackSeconds` with no save
    /// state; `Dev` falls to `MainLoopProcess`'s 0.45 / 0.
    #[test]
    fn game_fade_per_condition() {
        use StoryGameInitCondition as C;
        for (c, fade, black, red) in [
            (C::Dev, 0.45, 0.0, 0.0),
            (C::New, 2.0, 3.0, 5.5),
            (C::Load, 0.8, 0.75, 0.75),
            (C::RegionSelect, 0.8, 0.75, 0.75),
            (C::FastTravel, 0.0, 10.0, 10.0),
            (C::StartWithFastTravel, 0.0, 10.0, 10.0),
        ] {
            assert_eq!(c.game_fade_in_time(), fade, "{c:?}");
            assert_eq!(c.game_initial_black_seconds(Slugcat::White), black, "{c:?}");
            assert_eq!(
                c.game_initial_black_seconds(Slugcat::Yellow),
                black,
                "{c:?}"
            );
            assert_eq!(c.game_initial_black_seconds(Slugcat::Red), red, "{c:?}");
        }
    }
}
