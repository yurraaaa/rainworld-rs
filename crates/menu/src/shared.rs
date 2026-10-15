//! What the menu processes share across processes (`RainWorld`, `ProcessManager` and
//! Futile's globals, and `UnityEngine.Random`).

use std::sync::Arc;

use rw_assets::{SoundTable, StreamingAssets, UnityResources};
use rw_audio::music::{MusicPlayer, MusicSettings};
use rw_random::UnityRandom;
use rw_render::StageTextureId;

use crate::atlas::{AtlasLoadError, AtlasManager};
use crate::input::UiInputState;
use crate::process::ProcessId;
use crate::sound::MenuMic;
use crate::translator::Translator;

/// `SlugcatStats.Name` of the base game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slugcat {
    /// The Survivor.
    White,
    /// The Monk.
    Yellow,
    /// The Hunter.
    Red,
}

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

/// The part of `PlayerProgression.MiscProgressionData` the menus read, in the state of
/// a progression with no save (the port reads no saves yet).
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
/// support exists the port runs with none installed (docs/DIVERGENCES.md D13).
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
    /// `options.fullScreen` (the default options: windowed off).
    pub full_screen: bool,
    pub progression: Progression,
    pub dlcs: Dlcs,
    /// `rainWorld.inGameSlugCat`.
    pub in_game_slugcat: Option<Slugcat>,
    /// `manager.menuSetup.startGameCondition`.
    pub start_game_condition: Option<StoryGameInitCondition>,
    /// `_TextGradientTex` (`Palettes/textGradient`), when it could be read.
    pub text_gradient: Option<StageTextureId>,
    /// `_NoiseTex2` (`Palettes/noise2`), when it could be read.
    pub noise2: Option<StageTextureId>,
    /// `manager.musicPlayer`: a side process once created.
    pub music: Option<MusicPlayer>,
    /// `requestPlayMusic`: `setup.playMusic`, on in the distribution build.
    pub request_play_music: bool,
    pub music_settings: MusicSettings,
    /// `Application.Quit()` was called.
    pub quit: bool,
}

impl MenuShared {
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
        Ok(Self {
            assets,
            atlases,
            translator,
            input: UiInputState::default(),
            screen_size,
            random: UnityRandom::new(0),
            mic: MenuMic::default(),
            mic_attached: false,
            mouse_mode: false,
            full_screen: true,
            progression: Progression::default(),
            dlcs,
            in_game_slugcat: None,
            start_game_condition: None,
            text_gradient,
            noise2,
            music: None,
            request_play_music: true,
            music_settings: MusicSettings::new(SoundTable::default().volume_exponent),
            quit: false,
        })
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
            self.music =
                MusicPlayer::new(self.music_settings, current.map(ProcessId::music_process));
        }
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

    /// `PreSwitchMainProcess`'s microphone handling: let go of it before the game,
    /// take one back before anything else.
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

/// An app context the menu processes can run in.
pub trait MenuContext {
    fn menu(&mut self) -> &mut MenuShared;
}
