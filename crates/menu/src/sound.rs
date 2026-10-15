//! `MenuMicrophone` as the menus see it: what a menu's `PlaySound` and `PlayLoop`
//! start, with the game's random draws (`rainworld-re/notes/sound.md` 2.8, made by
//! `SoundLoader` exactly as for a room sound), and the loops' volume, pitch and fade.
//! An audio backend plays the [`MenuSoundEvent`]s and follows [`MenuMic::loops`]; the
//! menus never read back from it.

use std::sync::Arc;

use rw_assets::{SoundTable, SoundTrigger};
use rw_random::UnityRandom;
use rw_sim::FloatExt;

macro_rules! menu_sound_ids {
    ($($variant:ident = $name:literal,)*) => {
        /// The menu `SoundID`s the port plays; [`MenuSoundId::name`] is the game's name.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum MenuSoundId {
            $($variant,)*
        }

        impl MenuSoundId {
            pub const ALL: &'static [MenuSoundId] = &[$(MenuSoundId::$variant,)*];

            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(MenuSoundId::$variant => $name,)*
                }
            }
        }
    };
}

menu_sound_ids! {
    ButtonSelectGamepadOrKeyboard = "MENU_Button_Select_Gamepad_Or_Keyboard",
    ButtonSelectMouse = "MENU_Button_Select_Mouse",
    GreyedOutButtonSelectGamepadOrKeyboard = "MENU_Greyed_Out_Button_Select_Gamepad_Or_Keyboard",
    GreyedOutButtonSelectMouse = "MENU_Greyed_Out_Button_Select_Mouse",
    GreyedOutButtonClicked = "MENU_Greyed_Out_Button_Clicked",
    ButtonPressInit = "MENU_Button_Press_Init",
    SwitchPageIn = "MENU_Switch_Page_In",
    SwitchPageOut = "MENU_Switch_Page_Out",
    StartNewGame = "MENU_Start_New_Game",
    ContinueGame = "MENU_Continue_Game",
    MainMenuLoop = "MENU_Main_Menu_LOOP",
    NextSlugcat = "MENU_Next_Slugcat",
    SecurityButtonInit = "MENU_Security_Button_Init",
    SecurityButtonRelease = "MENU_Security_Button_Release",
    SecurityButtonLoop = "MENU_Security_Button_LOOP",
    CheckboxCheck = "MENU_Checkbox_Check",
    CheckboxUncheck = "MENU_Checkbox_Uncheck",
    SwitchArenaGametype = "MENU_Switch_Arena_Gametype",
    Thunder = "Thunder",
    ThunderClose = "Thunder_Close",
}

/// One `MenuSoundObject` as created: the drawn `SoundData` and the call's arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuSound {
    pub id: MenuSoundId,
    /// Index into [`SoundTable::clips`].
    pub clip: usize,
    /// The variation that plays.
    pub variation: usize,
    /// `soundData.vol` and `soundData.pitch`.
    pub volume: f32,
    pub pitch: f32,
    /// `audioSource.panStereo`.
    pub pan: f32,
    /// The `vol` and `pitch` arguments.
    pub call_volume: f32,
    pub call_pitch: f32,
}

/// A `MenuSoundLoop` handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LoopId(pub u32);

/// A `MenuSoundLoop`.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuSoundLoop {
    pub handle: LoopId,
    pub sound: MenuSound,
    pub loop_volume: f32,
    pub loop_pitch: f32,
    /// Rises 0.025 per frame while a menu is the main process, else falls.
    pub fade: f32,
    /// The menu's background loop (`mySoundLoopID`), handed on between menus.
    pub is_bkg: bool,
    /// `slatedForDeletion`.
    pub destroyed: bool,
}

impl MenuSoundLoop {
    /// `SetVolume`'s value: `fade * loopVolume`.
    #[must_use]
    pub fn volume_value(&self) -> f32 {
        self.fade * self.loop_volume
    }
}

/// What started this tick or frame, in call order.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuSoundEvent {
    Play(MenuSound),
    StartLoop(LoopId, MenuSound),
    StopLoop(LoopId),
}

/// The menu microphone.
#[derive(Debug, Clone, Default)]
pub struct MenuMic {
    table: Arc<SoundTable>,
    loops: Vec<MenuSoundLoop>,
    events: Vec<MenuSoundEvent>,
    next_loop: u32,
}

impl MenuMic {
    #[must_use]
    pub fn new(table: Arc<SoundTable>) -> Self {
        Self {
            table,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn table(&self) -> &Arc<SoundTable> {
        &self.table
    }

    pub fn take_events(&mut self) -> Vec<MenuSoundEvent> {
        std::mem::take(&mut self.events)
    }

    #[must_use]
    pub fn loops(&self) -> &[MenuSoundLoop] {
        &self.loops
    }

    pub fn loop_mut(&mut self, handle: LoopId) -> Option<&mut MenuSoundLoop> {
        self.loops.iter_mut().find(|l| l.handle == handle)
    }

    #[must_use]
    pub fn get_loop(&self, handle: LoopId) -> Option<&MenuSoundLoop> {
        self.loops.iter().find(|l| l.handle == handle)
    }

    /// `MenuMicrophone.PlaySound(id, pan, vol, pitch)`.
    pub fn play_sound(
        &mut self,
        id: MenuSoundId,
        [pan, vol, pitch]: [f32; 3],
        rng: &mut UnityRandom,
    ) {
        let table = Arc::clone(&self.table);
        let Some(trigger) = Self::should_play(&table, id, rng) else {
            return;
        };
        let samples: Vec<usize> = if trigger.play_all {
            (0..trigger.samples.len()).collect()
        } else {
            vec![rng.range_i32(0, trigger.samples.len() as i32) as usize]
        };
        for sample in samples {
            if let Some(sound) = Self::create(&table, trigger, sample, id, [pan, vol, pitch], rng) {
                self.events.push(MenuSoundEvent::Play(sound));
            }
        }
    }

    /// `MenuMicrophone.PlayLoop`: `None` when the sound does not play.
    pub fn play_loop(
        &mut self,
        id: MenuSoundId,
        [pan, vol, pitch]: [f32; 3],
        is_bkg: bool,
        rng: &mut UnityRandom,
    ) -> Option<LoopId> {
        let table = Arc::clone(&self.table);
        let trigger = Self::should_play(&table, id, rng)?;
        let sample = rng.range_i32(0, trigger.samples.len() as i32) as usize;
        let sound = Self::create(&table, trigger, sample, id, [pan, vol, pitch], rng)?;
        let handle = LoopId(self.next_loop);
        self.next_loop = self.next_loop.wrapping_add(1);
        self.events
            .push(MenuSoundEvent::StartLoop(handle, sound.clone()));
        self.loops.push(MenuSoundLoop {
            handle,
            sound,
            loop_volume: vol,
            loop_pitch: pitch,
            fade: 0.0,
            is_bkg,
            destroyed: false,
        });
        Some(handle)
    }

    /// `MenuSoundObject.Destroy`.
    pub fn destroy_loop(&mut self, handle: LoopId) {
        if let Some(l) = self.loop_mut(handle) {
            l.destroyed = true;
        }
    }

    /// `MenuMicrophone.GrafUpdate`, once per frame: removes destroyed loops, then fades
    /// the others in while `menu_is_main` (the main process is a `Menu`) and out
    /// otherwise, destroying one faded to nothing.
    pub fn graf_update(&mut self, menu_is_main: bool) {
        for i in (0..self.loops.len()).rev() {
            let l = &mut self.loops[i];
            if l.destroyed {
                let handle = l.handle;
                self.loops.remove(i);
                self.events.push(MenuSoundEvent::StopLoop(handle));
                continue;
            }
            if menu_is_main {
                l.fade = (l.fade + 0.025).min(1.0);
            } else {
                l.fade = (l.fade - 0.025).max(0.0);
                if l.fade == 0.0 {
                    l.destroyed = true;
                }
            }
        }
    }

    /// `SoundLoader.ShouldSoundPlay`: the trigger, drawing for `silentChance` if set.
    fn should_play<'t>(
        table: &'t SoundTable,
        id: MenuSoundId,
        rng: &mut UnityRandom,
    ) -> Option<&'t SoundTrigger> {
        let trigger = table.trigger(id.name())?;
        if trigger.silent_chance != 0.0 && rng.value() <= trigger.silent_chance {
            return None;
        }
        Some(trigger)
    }

    /// `GetSoundData`, `SoundClipReady` and the `MenuSoundObject` constructor for one
    /// sample (no random start: the menus never ask for one).
    fn create(
        table: &SoundTable,
        trigger: &SoundTrigger,
        index: usize,
        id: MenuSoundId,
        [pan, vol, pitch]: [f32; 3],
        rng: &mut UnityRandom,
    ) -> Option<MenuSound> {
        let s = &trigger.samples[index];
        let sample_vol = s.min_vol.lerp(s.max_vol, rng.value());
        let trigger_vol = trigger.min_vol.lerp(trigger.max_vol, rng.value());
        let sample_pitch = s.min_pitch.lerp(s.max_pitch, rng.value());
        let trigger_pitch = trigger.min_pitch.lerp(trigger.max_pitch, rng.value());
        let clip = table.clip_at(s.clip)?;
        let count = clip.variations.len() as i32;
        if !clip.variation_exists(rng.range_i32(0, count) as usize) {
            return None;
        }
        let variation = rng.range_i32(0, count) as usize;
        Some(MenuSound {
            id,
            clip: s.clip,
            variation,
            volume: sample_vol * trigger_vol,
            pitch: sample_pitch * trigger_pitch,
            pan,
            call_volume: vol,
            call_pitch: pitch,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Draw order from `rainworld-re/notes/sound.md` 2.8 (`MenuMicrophone.PlaySound` makes
    //! the same `SoundLoader` calls as `VirtualMicrophone.PlaySound`), replayed by hand.

    use super::*;
    use rw_assets::ClipNames;

    const TEXT: &str = "Volume: 6\nVolume exponent: 1.8\n\
        MENU_Button_Select_Gamepad_Or_Keyboard : UIMetal3/vol=0.3\n\
        MENU_Next_Slugcat/PLAYALL : UIMetal1/vol=0.4/pitch=0.9, UIArp/vol=0.2\n\
        MENU_Main_Menu_LOOP/CACHE : MenuLOOP/vol=0.4\n";

    fn mic() -> MenuMic {
        let names = ClipNames::new(
            ["UIMetal3", "UIMetal1", "UIArp", "MenuLOOP"].map(str::to_owned),
            None,
        );
        MenuMic::new(Arc::new(SoundTable::parse(TEXT, &names).unwrap()))
    }

    #[test]
    fn a_one_shot_draws_a_sample_four_values_and_two_variations() {
        let mut m = mic();
        let mut rng = UnityRandom::new(5);
        m.play_sound(
            MenuSoundId::ButtonSelectGamepadOrKeyboard,
            [0.0, 1.0, 1.0],
            &mut rng,
        );
        let mut want = UnityRandom::new(5);
        want.range_i32(0, 1);
        for _ in 0..4 {
            want.value();
        }
        want.range_i32(0, 1);
        want.range_i32(0, 1);
        assert_eq!(rng.state(), want.state());
        let events = m.take_events();
        let [MenuSoundEvent::Play(s)] = &events[..] else {
            panic!("{events:?}");
        };
        assert!((s.volume - 0.3).abs() < 1e-6);
        assert_eq!(s.pitch, 1.0);
    }

    #[test]
    fn play_all_plays_every_sample_without_a_pick() {
        let mut m = mic();
        let mut rng = UnityRandom::new(9);
        m.play_sound(MenuSoundId::NextSlugcat, [0.0, 1.0, 1.0], &mut rng);
        let mut want = UnityRandom::new(9);
        for _ in 0..2 {
            for _ in 0..4 {
                want.value();
            }
            want.range_i32(0, 1);
            want.range_i32(0, 1);
        }
        assert_eq!(rng.state(), want.state());
        let volumes: Vec<f32> = m
            .take_events()
            .iter()
            .map(|e| match e {
                MenuSoundEvent::Play(s) => s.volume,
                _ => panic!(),
            })
            .collect();
        assert_eq!(volumes, [0.4, 0.2]);
    }

    #[test]
    fn loops_fade_in_under_a_menu_and_die_faded_out() {
        let mut m = mic();
        let mut rng = UnityRandom::new(1);
        let h = m
            .play_loop(MenuSoundId::MainMenuLoop, [0.0, 1.0, 1.0], true, &mut rng)
            .unwrap();
        // 40 steps of 0.025 in f32 fall just short of 1; the 41st clamps.
        for _ in 0..41 {
            m.graf_update(true);
        }
        assert_eq!(m.get_loop(h).unwrap().fade, 1.0);
        m.graf_update(false);
        assert!((m.get_loop(h).unwrap().fade - 0.975).abs() < 1e-6);
        for _ in 0..40 {
            m.graf_update(false);
        }
        assert!(m.get_loop(h).unwrap().destroyed);
        m.graf_update(false);
        assert!(m.get_loop(h).is_none());
    }
}
