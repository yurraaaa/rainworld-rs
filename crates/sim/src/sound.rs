//! The game's `Room.PlaySound` layer as engine-agnostic events, with the random draws
//! `VirtualMicrophone.PlaySound` makes (`rainworld-re/notes/sound.md` sections 2.1, 2.8).
//!
//! The sim decides what plays and draws from the shared `UnityRandom` exactly as the
//! game does, whether or not anything is listening: `silentChance` (only when set), the
//! sample pick, the four volume and pitch values, the variation check and the variation
//! that plays, and the random start when asked. An audio backend turns the
//! [`SoundEvent`]s into sound; the sim never reads back from it.
//!
//! Mirrors: Room.cs (PlaySound, PlayDisembodiedLoop, PlayRectSound), VirtualMicrophone.cs (PlaySound, PlayDisembodiedLoop), RectSoundEmitter.cs, SoundLoader.cs

use std::sync::Arc;

use rw_assets::{SoundTable, SoundTrigger};
use rw_random::UnityRandom;

use crate::Vec2;
use crate::float_ext::FloatExt;

macro_rules! sound_ids {
    ($($variant:ident = $name:literal,)*) => {
        /// The `SoundID`s the port plays; [`SoundId::name`] is the game's name.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum SoundId {
            $($variant,)*
        }

        impl SoundId {
            /// Every ported ID.
            pub const ALL: &'static [SoundId] = &[$(SoundId::$variant,)*];

            /// The `SoundID` enum name, as `sounds.txt` names it.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(SoundId::$variant => $name,)*
                }
            }

            /// The ported ID the game spells `name`.
            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|id| id.name() == name)
            }
        }
    };
}

sound_ids! {
    SlugcatStepA = "Slugcat_Step_A",
    SlugcatStepB = "Slugcat_Step_B",
    SlugcatCrawlingStep = "Slugcat_Crawling_Step",
    SlugcatStandUp = "Slugcat_Stand_Up",
    SlugcatDownOnFours = "Slugcat_Down_On_Fours",
    SlugcatInCorridorStep = "Slugcat_In_Corridor_Step",
    SlugcatTurnInCorridor = "Slugcat_Turn_In_Corridor",
    SlugcatCorridorHorizontalSlideSuccess = "Slugcat_Corridor_Horizontal_Slide_Success",
    SlugcatCorridorHorizontalSlideFail = "Slugcat_Corridor_Horizontal_Slide_Fail",
    SlugcatVerticalChuteJumpSuccess = "Slugcat_Vertical_Chute_Jump_Success",
    SlugcatVerticalChuteJumpFail = "Slugcat_Vertical_Chute_Jump_Fail",
    SlugcatSkidOnGroundInit = "Slugcat_Skid_On_Ground_Init",
    SlugcatNormalJump = "Slugcat_Normal_Jump",
    SlugcatFlipJump = "Slugcat_Flip_Jump",
    SlugcatCrouchJump = "Slugcat_Crouch_Jump",
    SlugcatSuperJump = "Slugcat_Super_Jump",
    SlugcatWallJump = "Slugcat_Wall_Jump",
    SlugcatRocketJump = "Slugcat_Rocket_Jump",
    SlugcatFromVerticalPoleJump = "Slugcat_From_Vertical_Pole_Jump",
    SlugcatFromHorizontalPoleJump = "Slugcat_From_Horizontal_Pole_Jump",
    SlugcatBellySlideInit = "Slugcat_Belly_Slide_Init",
    SlugcatBellySlideFinishSuccess = "Slugcat_Belly_Slide_Finish_Success",
    SlugcatBellySlideFinishFail = "Slugcat_Belly_Slide_Finish_Fail",
    SlugcatSectretSuperWallJump = "Slugcat_Sectret_Super_Wall_Jump",
    SlugcatTerrainImpactDeath = "Slugcat_Terrain_Impact_Death",
    SlugcatTerrainImpactHard = "Slugcat_Terrain_Impact_Hard",
    SlugcatTerrainImpactMedium = "Slugcat_Terrain_Impact_Medium",
    SlugcatTerrainImpactLight = "Slugcat_Terrain_Impact_Light",
    SlugcatFloorImpactStandard = "Slugcat_Floor_Impact_Standard",
    SlugcatFloorImpactStealthy = "Slugcat_Floor_Impact_Stealthy",
    SlugcatRollInit = "Slugcat_Roll_Init",
    SlugcatRollFinish = "Slugcat_Roll_Finish",
    SlugcatGrabBeam = "Slugcat_Grab_Beam",
    SlugcatClimbUpVerticalBeam = "Slugcat_Climb_Up_Vertical_Beam",
    SlugcatClimbAlongHorizontalBeam = "Slugcat_Climb_Along_Horizontal_Beam",
    SlugcatWalkOnHorizontalBeam = "Slugcat_Walk_On_Horizontal_Beam",
    SlugcatGetUpOnHorizontalBeam = "Slugcat_Get_Up_On_Horizontal_Beam",
    SlugcatGetUpOnTopOfVerticalBeamTip = "Slugcat_Get_Up_On_Top_Of_Vertical_Beam_Tip",
    SlugcatRegainFooting = "Slugcat_Regain_Footing",
    SlugcatJumpOnCreature = "Slugcat_Jump_On_Creature",
    SlugcatEnterWallSlide = "Slugcat_Enter_Wall_Slide",
    UiSlugcatStunnedLoop = "UI_Slugcat_Stunned_LOOP",
    SlugcatSlideInNarrowCorridorLoop = "Slugcat_Slide_In_Narrow_Corridor_LOOP",
    SlugcatSkidOnGroundLoop = "Slugcat_Skid_On_Ground_LOOP",
    SlugcatRollLoop = "Slugcat_Roll_LOOP",
    SlugcatSlideDownVerticalBeamLoop = "Slugcat_Slide_Down_Vertical_Beam_LOOP",
    SlugcatBellySlideLoop = "Slugcat_Belly_Slide_LOOP",
    SlugcatWallSlideLoop = "Slugcat_Wall_Slide_LOOP",
    UiSlugcatStunnedInit = "UI_Slugcat_Stunned_Init",
    UiSlugcatExitStun = "UI_Slugcat_Exit_Stun",
    UiSlugcatDie = "UI_Slugcat_Die",
    PlayerEnterShortcut = "Player_Enter_Shortcut",
    PlayerExitShortcut = "Player_Exit_Shortcut",
    PlayerTickAlongInShortcut = "Player_Tick_Along_In_Shortcut",
    SmallObjectIntoWaterFast = "Small_Object_Into_Water_Fast",
    SmallObjectIntoWaterSlow = "Small_Object_Into_Water_Slow",
    MediumObjectIntoWaterFast = "Medium_Object_Into_Water_Fast",
    MediumObjectIntoWaterSlow = "Medium_Object_Into_Water_Slow",
    LargeObjectIntoWaterFast = "Large_Object_Into_Water_Fast",
    LargeObjectIntoWaterSlow = "Large_Object_Into_Water_Slow",
    SplashingWaterIntoTerrain = "Splashing_Water_Into_Terrain",
    SplashingWaterIntoWaterSurface = "Splashing_Water_Into_Water_Surface",
    HudFoodMeterFillPlopA = "HUD_Food_Meter_Fill_Plop_A",
    HudFoodMeterFillPlopB = "HUD_Food_Meter_Fill_Plop_B",
    HudFoodMeterFillQuarterPlop = "HUD_Food_Meter_Fill_Quarter_Plop",
    HudFoodMeterFillFadeCircle = "HUD_Food_Meter_Fill_Fade_Circle",
    HudFoodMeterDepletePlopA = "HUD_Food_Meter_Deplete_Plop_A",
    HudFoodMeterDepletePlopB = "HUD_Food_Meter_Deplete_Plop_B",
    HudKarmaReinforceFlicker = "HUD_Karma_Reinforce_Flicker",
    HudKarmaReinforceSmallCircle = "HUD_Karma_Reinforce_Small_Circle",
    HudKarmaReinforceContract = "HUD_Karma_Reinforce_Contract",
    HudKarmaReinforceBump = "HUD_Karma_Reinforce_Bump",
    HudGameOverPrompt = "HUD_Game_Over_Prompt",
    OverseerWithdraw = "Overseer_Withdraw",
    OverseerEmerge = "Overseer_Emerge",
    OverseerDeath = "Overseer_Death",
    OverseerImageLoop = "Overseer_Image_LOOP",
    OverseerImageSmallFlicker = "Overseer_Image_Small_Flicker",
    OverseerImageBigFlicker = "Overseer_Image_Big_Flicker",
    BatAfraidFlyingSounds = "Bat_Afraid_Flying_Sounds",
    BatIdleFlyingSounds = "Bat_Idle_Flying_Sounds",
    FlyWingFlap = "Fly_Wing_Flap",
    BatHoverFlyLoop = "Bat_Hover_Fly_LOOP",
    BatDiveIntoGrass = "Bat_Dive_Into_Grass",
    BatEmergeFromGrass = "Bat_Emerge_From_Grass",
    BatDetatchFromChain = "Bat_Detatch_From_Chain",
    BatAttatchToChain = "Bat_Attatch_To_Chain",
    BatStartled = "Bat_Startled",
    SmallNpcEnterShortcut = "Small_NPC_Enter_Shortcut",
    SmallNpcTickAlongInShortcut = "Small_NPC_Tick_Along_In_Shortcut",
    SmallNpcExitShortcut = "Small_NPC_Exit_Shortcut",
    MediumNpcEnterShortcut = "Medium_NPC_Enter_Shortcut",
    MediumNpcTickAlongInShortcut = "Medium_NPC_Tick_Along_In_Shortcut",
    MediumNpcExitShortcut = "Medium_NPC_Exit_Shortcut",
    LargeNpcEnterShortcut = "Large_NPC_Enter_Shortcut",
    LargeNpcTickAlongInShortcut = "Large_NPC_Tick_Along_In_Shortcut",
    LargeNpcExitShortcut = "Large_NPC_Exit_Shortcut",
    FlyCaught = "Fly_Caught",
    FlyCaughtDead = "Fly_Caught_Dead",
    SlugcatBiteFly = "Slugcat_Bite_Fly",
    SlugcatFinalBiteFly = "Slugcat_Final_Bite_Fly",
    SlugcatEatMeatA = "Slugcat_Eat_Meat_A",
    SlugcatEatMeatB = "Slugcat_Eat_Meat_B",
    SlugcatLoseGrasp = "Slugcat_Lose_Grasp",
    SlugcatPickUpCreature = "Slugcat_Pick_Up_Creature",
    SlugcatThrowCreature = "Slugcat_Throw_Creature",
    SlugcatLayDownCreature = "Slugcat_Lay_Down_Creature",
    SlugcatSwitchHandsInit = "Slugcat_Switch_Hands_Init",
    SlugcatSwitchHandsComplete = "Slugcat_Switch_Hands_Complete",
    SlugcatPickUpRock = "Slugcat_Pick_Up_Rock",
    SlugcatPickUpSpear = "Slugcat_Pick_Up_Spear",
    SlugcatStashSpearOnBack = "Slugcat_Stash_Spear_On_Back",
    SlugcatPickUpMiscInanimate = "Slugcat_Pick_Up_Misc_Inanimate",
    SlugcatThrowMiscInanimate = "Slugcat_Throw_Misc_Inanimate",
    SlugcatLayDownObject = "Slugcat_Lay_Down_Object",
    SlugcatSwallowItem = "Slugcat_Swallow_Item",
    SlugcatRegurgitateItem = "Slugcat_Regurgitate_Item",
    UiWeaponInRangeToPickUp = "UI_Weapon_In_Range_To_Pick_Up",
    HudPauseGame = "HUD_Pause_Game",
    HudUnpauseGame = "HUD_Unpause_Game",
    HudExitGame = "HUD_Exit_Game",
    MenuButtonStandardButtonPressed = "MENU_Button_Standard_Button_Pressed",
    MenuButtonSelectGamepadOrKeyboard = "MENU_Button_Select_Gamepad_Or_Keyboard",
    MenuButtonSelectMouse = "MENU_Button_Select_Mouse",
    MenuGreyedOutButtonSelectGamepadOrKeyboard = "MENU_Greyed_Out_Button_Select_Gamepad_Or_Keyboard",
    MenuGreyedOutButtonSelectMouse = "MENU_Greyed_Out_Button_Select_Mouse",
    MenuGreyedOutButtonClicked = "MENU_Greyed_Out_Button_Clicked",
    MenuButtonPressInit = "MENU_Button_Press_Init",
    ShelterBoltOpen = "Shelter_Bolt_Open",
    ShelterBoltClose = "Shelter_Bolt_Close",
    ShelterLittleHatchOpen = "Shelter_Little_Hatch_Open",
    ShelterSegmentPairCollide = "Shelter_Segment_Pair_Collide",
    ShelterSegmentPairMoveIn = "Shelter_Segment_Pair_Move_In",
    ShelterSegmentPairMoveOut = "Shelter_Segment_Pair_Move_Out",
    ShelterPistonInHard = "Shelter_Piston_In_Hard",
    ShelterPistonInSoft = "Shelter_Piston_In_Soft",
    ShelterPistonOut = "Shelter_Piston_Out",
    ShelterProtectiveCoverMoveOut = "Shelter_Protective_Cover_Move_Out",
    ShelterProtectiveCoverMoveIn = "Shelter_Protective_Cover_Move_In",
    ShelterProtectiveCoverClickIntoPlace = "Shelter_Protective_Cover_Click_Into_Place",
    ShelterGasketsSeal = "Shelter_Gaskets_Seal",
    ShelterGasketsUnseal = "Shelter_Gaskets_Unseal",
    ShelterWorkingBackgroundLoop = "Shelter_Working_Background_Loop",
    ShelterGasketMoverLoop = "Shelter_Gasket_Mover_LOOP",
    SpearStickInGround = "Spear_Stick_In_Ground",
    RockSkiddingOnGroundLoop = "Rock_Skidding_On_Ground_LOOP",
    RockThroughAirLoop = "Rock_Through_Air_LOOP",
    SpearSpinningThroughAirLoop = "Spear_Spinning_Through_Air_LOOP",
    SpearThrownThroughAirLoop = "Spear_Thrown_Through_Air_LOOP",
    SlugcatThrowRock = "Slugcat_Throw_Rock",
    SlugcatThrowSpear = "Slugcat_Throw_Spear",
    SpearStickInWall = "Spear_Stick_In_Wall",
    SpearBounceOffWall = "Spear_Bounce_Off_Wall",
    RockHitWall = "Rock_Hit_Wall",
    WeaponSkid = "Weapon_Skid",
    SpearBounceOffCreautureShell = "Spear_Bounce_Off_Creauture_Shell",
    SpearStickInCreature = "Spear_Stick_In_Creature",
    SpearDislodgedFromCreature = "Spear_Dislodged_From_Creature",
    RockHitCreature = "Rock_Hit_Creature",
    LizardLightTerrainImpact = "Lizard_Light_Terrain_Impact",
    LizardHeavyTerrainImpact = "Lizard_Heavy_Terrain_Impact",
    LizardHeadShieldDeflect = "Lizard_Head_Shield_Deflect",
    LizardJawsShutMissCreature = "Lizard_Jaws_Shut_Miss_Creature",
    LizardJawsGrabNpc = "Lizard_Jaws_Grab_NPC",
    LizardJawsGrabPlayer = "Lizard_Jaws_Grab_Player",
    LizardGrabPole = "Lizard_Grab_Pole",
    LizardPrepareToShootTongue = "Lizard_Prepare_To_Shoot_Tongue",
    LizardPrepareLungeAttackInit = "Lizard_Prepare_Lunge_Attack_Init",
    LizardLungeAttackInit = "Lizard_Lunge_Attack_Init",
    LizardBellyDragLoop = "Lizard_Belly_Drag_LOOP",
    LizardLoungeAttackLoop = "Lizard_Lounge_Attack_LOOP",
    LizardGreenFootGrab = "Lizard_Green_Foot_Grab",
    LizardGreenFootRelease = "Lizard_Green_Foot_Release",
    LizardBlueWhiteFootGrab = "Lizard_BlueWhite_Foot_Grab",
    LizardBlueWhiteFootRelease = "Lizard_BlueWhite_Foot_Release",
    LizardPinkYellowRedFootGrab = "Lizard_PinkYellowRed_Foot_Grab",
    LizardPinkYellowRedFootRelease = "Lizard_PinkYellowRed_Foot_Release",
    LizardVoicePinkA = "Lizard_Voice_Pink_A",
    LizardVoicePinkB = "Lizard_Voice_Pink_B",
    LizardVoicePinkC = "Lizard_Voice_Pink_C",
    LizardVoicePinkD = "Lizard_Voice_Pink_D",
    LizardVoicePinkE = "Lizard_Voice_Pink_E",
    LizardVoiceGreenA = "Lizard_Voice_Green_A",
    LizardVoiceBlueA = "Lizard_Voice_Blue_A",
    CycleStartDrips = "Cycle_Start_Drips",
    WaterSurfaceCalmLoop = "Water_Surface_Calm_LOOP",
    Thunder = "Thunder",
    ThunderClose = "Thunder_Close",
    ElectricityLoop = "Electricity_Loop",
    // No `sounds.txt` line: it never plays.
    SpearHitSmallCreature = "Spear_Hit_Small_Creature",
}

/// A `ChunkSoundEmitter` handle: follows a body chunk; loops are updated and stopped
/// through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EmitterId(pub u32);

/// Where a sound is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SoundSource {
    /// `DisembodiedSound`: no position; `pan` in -1..=1; `group` is the microphone's
    /// volume group (1 for a menu's sounds in the game, which the pause menu leaves
    /// loud).
    Disembodied { pan: f32, group: usize },
    /// `StaticPositionSound`: a fixed room position.
    Position(Vec2),
    /// `ObjectSound` on a `ChunkSoundEmitter`: chunk `chunk` of the player.
    Chunk { emitter: EmitterId, chunk: usize },
    /// `ObjectSound` on a `ChunkSoundEmitter` of another body, at `pos` when made;
    /// [`SoundEvent::Moved`] follows it.
    Object { emitter: EmitterId, pos: Vec2 },
    /// `DisembodiedLoop` on a `DisembodiedLoopEmitter`: no position, `pan` in -1..=1.
    DisembodiedLoop { emitter: EmitterId, pan: f32 },
    /// `RectangularSound` on a `RectSoundEmitter`: heard from the point of `rect`
    /// nearest the listener.
    Rect { emitter: EmitterId, rect: SoundRect },
}

/// A `FloatRect` a `RectSoundEmitter` covers, room pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoundRect {
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
    pub top: f32,
}

impl SoundRect {
    /// `Custom.RestrictInRect`: `Mathf.Clamp` per axis (the lower bound first, so an
    /// inverted rect gives its left or bottom edge).
    #[must_use]
    pub fn restrict(&self, v: Vec2) -> Vec2 {
        Vec2::new(
            Self::clamp(v.x, self.left, self.right),
            Self::clamp(v.y, self.bottom, self.top),
        )
    }

    /// `Mathf.Clamp`.
    fn clamp(v: f32, min: f32, max: f32) -> f32 {
        if v < min {
            min
        } else if v > max {
            max
        } else {
            v
        }
    }
}

/// One sound object the game would create, with the outcome of its random draws
/// (`SoundLoader.SoundData` plus the emitter's arguments).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayedSound {
    pub id: SoundId,
    /// Index into [`SoundTable::clips`].
    pub clip: usize,
    /// The variation that plays (`GetAudioClip` in the `SoundObject` constructor).
    pub variation: usize,
    /// `soundData.vol` and `soundData.pitch`.
    pub volume: f32,
    pub pitch: f32,
    /// `rangeFac` and `dopplerFac` of the trigger.
    pub range: f32,
    pub doppler_fac: f32,
    pub ignore_effects: f32,
    /// Start at this fraction of the clip (`startAtRandomTime`).
    pub start_at: Option<f32>,
    pub source: SoundSource,
    pub looped: bool,
    /// The `vol` and `pitch` arguments of `PlaySound` (the emitter's for chunk sounds).
    pub emitter_volume: f32,
    pub emitter_pitch: f32,
}

/// What happened to sound this tick, in call order.
#[derive(Debug, Clone, PartialEq)]
pub enum SoundEvent {
    Play(PlayedSound),
    /// An emitter's volume and pitch were set and it was kept alive this tick (loops
    /// with `requireActiveUpkeep` die without this).
    Emitter {
        id: EmitterId,
        volume: f32,
        pitch: f32,
    },
    /// `emitter.alive = false`.
    Stop(EmitterId),
    /// A `DisembodiedLoopEmitter`'s `pan` was set.
    EmitterPan {
        id: EmitterId,
        pan: f32,
    },
    /// An object emitter's `ChunkSoundEmitter.Update` copied its chunk's position.
    Moved {
        id: EmitterId,
        pos: Vec2,
    },
    /// The body chunk sounds follow left the room for good (destroyed or replaced):
    /// every chunk emitter dies.
    OwnerRemoved,
}

/// `StaticSoundLoop`: a looping sound at a fixed position whose emitter is made while
/// its volume is above 0 and let go when it falls to 0.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticSoundLoop {
    pub id: SoundId,
    pub pos: Vec2,
    pub volume: f32,
    pub pitch: f32,
    emitter: Option<EmitterId>,
    /// Emitters let go (`alive = false`), still in the room until their next update.
    released: Vec<EmitterId>,
}

impl StaticSoundLoop {
    #[must_use]
    pub fn new(id: SoundId, pos: Vec2, volume: f32, pitch: f32) -> Self {
        Self {
            id,
            pos,
            volume,
            pitch,
            emitter: None,
            released: Vec::new(),
        }
    }

    #[must_use]
    pub fn emitter(&self) -> Option<EmitterId> {
        self.emitter
    }

    /// `SoundEmitter.Update` of the emitters let go: unkept, each is destroyed and the
    /// microphone cuts its sound. The owner calls it where the emitters update, which
    /// for an owner that was in the room first is before its own update.
    pub fn update_released(&mut self, sounds: &mut Sounds) {
        for e in self.released.drain(..) {
            sounds.stop_emitter(e);
        }
    }

    /// `Update`: keeps the emitter alive with the current volume and pitch, lets it go
    /// at volume 0, or starts the loop (`Room.PlaySound(id, emitter, loop: true, ...)`).
    ///
    /// Unheard (the camera is in another room), the game's emitter dies two ticks on
    /// (`soundStillPlaying`) and is remade without a sound, so it starts again on return.
    pub fn update(&mut self, sounds: &mut Sounds, rng: &mut UnityRandom) {
        if !sounds.heard {
            self.emitter = None;
        } else if let Some(emitter) = self.emitter {
            if self.volume == 0.0 {
                self.released.push(emitter);
                self.emitter = None;
            } else {
                sounds.update_emitter(emitter, self.volume, self.pitch);
            }
        } else if self.volume > 0.0 {
            self.emitter =
                Some(sounds.play_on_object(self.id, self.pos, true, self.volume, self.pitch, rng));
        }
    }
}

/// A looping `ChunkSoundEmitter` (`requireActiveUpkeep`), a room object of its own: a
/// fly's hover loop, an item's `ChunkDynamicSoundLoop`. `still_playing` follows the
/// microphone with one frame per tick (INFERRED: the game counts rendered frames).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopEmitter {
    pub sound: EmitterId,
    pub room: i32,
    pub update_order: u64,
    pub alive: bool,
    pub slated: bool,
    pub still_playing: bool,
    pub last_playing_tick: u64,
    /// The camera's microphone made a sound object for it.
    pub playing: bool,
}

/// Where a body's new emitter joins: its room's update list (the world's counter) and
/// what the microphone's refresh reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EmitterPlace {
    pub room: i32,
    pub camera_room: i32,
    pub tick: u64,
    pub update_seq: u64,
}

impl LoopEmitter {
    /// A new emitter joining the room at `place`; `played`: the microphone made a sound
    /// for it (heard only in the camera's room).
    pub fn join(sound: EmitterId, played: bool, place: &mut EmitterPlace) -> Self {
        place.update_seq += 1;
        Self {
            sound,
            room: place.room,
            update_order: place.update_seq,
            alive: true,
            slated: false,
            still_playing: true,
            last_playing_tick: place.tick,
            playing: played && place.room == place.camera_room,
        }
    }

    /// `ChunkSoundEmitter.Update` with the microphone's refresh before it.
    pub fn update(&mut self, owner_room: Option<i32>, camera_room: i32, tick: u64) {
        if self.playing && camera_room == self.room {
            self.last_playing_tick = tick;
            self.still_playing = true;
        }
        if owner_room != Some(self.room) {
            self.alive = false;
        }
        if self.alive {
            self.alive = false;
        } else {
            self.slated = true;
        }
        if self.still_playing {
            if self.last_playing_tick + 1 < tick {
                self.still_playing = false;
            }
        } else {
            self.slated = true;
        }
    }
}

/// The room's sound calls: the `sounds.txt` table they resolve against, the events
/// of the current tick and the emitter handle counter.
#[derive(Debug, Clone, Default)]
pub struct Sounds {
    table: Arc<SoundTable>,
    events: Vec<SoundEvent>,
    next_emitter: u32,
    /// The camera's microphone hears these calls (false for the other rooms' sink).
    heard: bool,
}

impl Sounds {
    #[must_use]
    pub fn new(table: Arc<SoundTable>) -> Self {
        Self {
            table,
            events: Vec::new(),
            next_emitter: 0,
            heard: true,
        }
    }

    #[must_use]
    pub fn table(&self) -> &Arc<SoundTable> {
        &self.table
    }

    /// The events since the last call.
    pub fn take_events(&mut self) -> Vec<SoundEvent> {
        std::mem::take(&mut self.events)
    }

    #[must_use]
    pub fn events(&self) -> &[SoundEvent] {
        &self.events
    }

    /// `Room.PlaySound(id, pan, vol, pitch)`.
    pub fn play_disembodied(
        &mut self,
        id: SoundId,
        pan: f32,
        volume: f32,
        pitch: f32,
        rng: &mut UnityRandom,
    ) {
        self.play_disembodied_in_group(id, pan, volume, pitch, 0, rng);
    }

    /// `VirtualMicrophone.PlaySound(id, pan, vol, pitch, volumeGroup)`.
    pub fn play_disembodied_in_group(
        &mut self,
        id: SoundId,
        pan: f32,
        volume: f32,
        pitch: f32,
        group: usize,
        rng: &mut UnityRandom,
    ) {
        let source = SoundSource::Disembodied { pan, group };
        self.play(id, source, false, [volume, pitch], false, rng);
    }

    /// `Room.PlaySound(id, pos, vol, pitch)`.
    pub fn play_at(
        &mut self,
        id: SoundId,
        pos: Vec2,
        volume: f32,
        pitch: f32,
        rng: &mut UnityRandom,
    ) {
        self.play(
            id,
            SoundSource::Position(pos),
            false,
            [volume, pitch],
            false,
            rng,
        );
    }

    /// `Room.PlaySound(id, chunk, loop, vol, pitch)`: the emitter exists even when
    /// nothing plays.
    pub fn play_on_chunk(
        &mut self,
        id: SoundId,
        chunk: usize,
        looped: bool,
        volume: f32,
        pitch: f32,
        rng: &mut UnityRandom,
    ) -> EmitterId {
        let emitter = EmitterId(self.next_emitter);
        self.next_emitter = self.next_emitter.wrapping_add(1);
        let source = SoundSource::Chunk { emitter, chunk };
        self.play(id, source, looped, [volume, pitch], false, rng);
        emitter
    }

    /// `Room.PlaySound(id, chunk, loop, vol, pitch)` on a body other than the player's,
    /// whose chunk is at `pos`.
    pub fn play_on_object(
        &mut self,
        id: SoundId,
        pos: Vec2,
        looped: bool,
        volume: f32,
        pitch: f32,
        rng: &mut UnityRandom,
    ) -> EmitterId {
        let emitter = EmitterId(self.next_emitter);
        self.next_emitter = self.next_emitter.wrapping_add(1);
        let source = SoundSource::Object { emitter, pos };
        self.play(id, source, looped, [volume, pitch], false, rng);
        emitter
    }

    /// `Room.PlayDisembodiedLoop(id, vol, pitch, pan)`: the emitter exists even when
    /// nothing plays.
    pub fn play_disembodied_loop(
        &mut self,
        id: SoundId,
        pan: f32,
        volume: f32,
        pitch: f32,
        rng: &mut UnityRandom,
    ) -> EmitterId {
        let emitter = EmitterId(self.next_emitter);
        self.next_emitter = self.next_emitter.wrapping_add(1);
        let source = SoundSource::DisembodiedLoop { emitter, pan };
        self.play(id, source, true, [volume, pitch], false, rng);
        emitter
    }

    /// `Room.PlayRectSound(id, rect, loop, vol, pitch)`: the emitter exists even when
    /// nothing plays.
    pub fn play_rect(
        &mut self,
        id: SoundId,
        rect: SoundRect,
        looped: bool,
        volume: f32,
        pitch: f32,
        rng: &mut UnityRandom,
    ) -> EmitterId {
        let emitter = EmitterId(self.next_emitter);
        self.next_emitter = self.next_emitter.wrapping_add(1);
        let source = SoundSource::Rect { emitter, rect };
        self.play(id, source, looped, [volume, pitch], false, rng);
        emitter
    }

    /// An object emitter follows its chunk to `pos`.
    pub fn move_emitter(&mut self, id: EmitterId, pos: Vec2) {
        self.events.push(SoundEvent::Moved { id, pos });
    }

    /// Sets a loop emitter's volume and pitch and keeps it alive this tick.
    pub fn update_emitter(&mut self, id: EmitterId, volume: f32, pitch: f32) {
        self.events.push(SoundEvent::Emitter { id, volume, pitch });
    }

    /// Sets a `DisembodiedLoopEmitter`'s pan (`DisembodiedDynamicSoundLoop.Pan`).
    pub fn set_emitter_pan(&mut self, id: EmitterId, pan: f32) {
        self.events.push(SoundEvent::EmitterPan { id, pan });
    }

    pub fn stop_emitter(&mut self, id: EmitterId) {
        self.events.push(SoundEvent::Stop(id));
    }

    /// The player's body is gone: its chunk emitters die (`ChunkSoundEmitter.Update`).
    pub fn owner_removed(&mut self) {
        self.events.push(SoundEvent::OwnerRemoved);
    }

    /// `VirtualMicrophone.PlaySound` with its draws. The voice cap of `AllowSound` is
    /// not applied here (it depends on what the audio is still playing;
    /// docs/divergences/ D12).
    fn play(
        &mut self,
        id: SoundId,
        source: SoundSource,
        looped: bool,
        [volume, pitch]: [f32; 2],
        random_start: bool,
        rng: &mut UnityRandom,
    ) {
        let table = Arc::clone(&self.table);
        let Some(trigger) = table.trigger(id.name()) else {
            return;
        };
        if trigger.silent_chance != 0.0 && rng.value() <= trigger.silent_chance {
            return;
        }
        let call = SoundCall {
            id,
            source,
            looped,
            volume,
            pitch,
            random_start,
        };
        if trigger.play_all {
            for sample in 0..trigger.samples.len() {
                self.play_sample(&table, trigger, sample, &call, rng);
            }
        } else {
            let sample = rng.range_i32(0, trigger.samples.len() as i32) as usize;
            self.play_sample(&table, trigger, sample, &call, rng);
        }
    }

    /// `GetSoundData`, `SoundClipReady` and the `SoundObject` constructor for one
    /// sample.
    fn play_sample(
        &mut self,
        table: &SoundTable,
        trigger: &SoundTrigger,
        index: usize,
        call: &SoundCall,
        rng: &mut UnityRandom,
    ) {
        let s = &trigger.samples[index];
        let sample_vol = s.min_vol.lerp(s.max_vol, rng.value());
        let trigger_vol = trigger.min_vol.lerp(trigger.max_vol, rng.value());
        let sample_pitch = s.min_pitch.lerp(s.max_pitch, rng.value());
        let trigger_pitch = trigger.min_pitch.lerp(trigger.max_pitch, rng.value());
        let Some(clip) = table.clip_at(s.clip) else {
            return;
        };
        let count = clip.variations.len() as i32;
        // SoundClipReady loads one random variation; a missing one creates no sound.
        if !clip.variation_exists(rng.range_i32(0, count) as usize) {
            return;
        }
        let variation = rng.range_i32(0, count);
        let start_at =
            (call.random_start && clip.variation_exists(variation as usize)).then(|| rng.value());
        self.events.push(SoundEvent::Play(PlayedSound {
            id: call.id,
            clip: s.clip,
            variation: variation as usize,
            volume: sample_vol * trigger_vol,
            pitch: sample_pitch * trigger_pitch,
            range: trigger.range,
            doppler_fac: trigger.doppler_fac,
            ignore_effects: s.ignore_effects.lerp(1.0, trigger.ignore_effects),
            start_at,
            source: call.source,
            looped: call.looped,
            emitter_volume: call.volume,
            emitter_pitch: call.pitch,
        }));
    }
}

/// The arguments of one `PlaySound` call.
struct SoundCall {
    id: SoundId,
    source: SoundSource,
    looped: bool,
    volume: f32,
    pitch: f32,
    random_start: bool,
}

#[cfg(test)]
mod tests {
    //! Draw order from `rainworld-re/notes/sound.md` 2.8 (`VirtualMicrophone.PlaySound`,
    //! `SoundTrigger.GetSoundData`, `SoundLoader.GetAudioClip`), replayed by hand on a
    //! second generator.

    use std::sync::Arc;

    use rw_assets::{ClipNames, SoundTable};
    use rw_random::UnityRandom;

    use super::{
        EmitterId, PlayedSound, SoundEvent, SoundId, SoundSource, Sounds, StaticSoundLoop,
    };
    use crate::FloatExt;

    const TEXT: &str = "Volume: 6\nVolume exponent: 1.8\n\
        Slugcat_Normal_Jump/minVol=0.5/maxVol=1/minPitch=0.9/maxPitch=1.1/rangeFac=0.5 : \
        clipA/minVol=0.2/maxVol=0.4, clipB/pitch=1.5/ignoreEffects=0.5\n\
        Slugcat_Super_Jump/PLAYALL : clipB, clipA\n\
        Player_Tick_Along_In_Shortcut/silentChance=0.25 : clipB\n\
        Slugcat_Wall_Jump : solo\n\
        UI_Slugcat_Die : missing\n";

    fn sounds() -> Sounds {
        let names = ["clipa_1", "clipa_2", "clipa_3", "clipb", "solo", "solo_2"];
        let names = ClipNames::new(names.map(str::to_owned), None);
        Sounds::new(Arc::new(SoundTable::parse(TEXT, &names).unwrap()))
    }

    /// `rng` after `n` raw steps from `seed`.
    fn advanced(seed: i32, n: usize) -> UnityRandom {
        let mut rng = UnityRandom::new(seed);
        for _ in 0..n {
            rng.next_u32();
        }
        rng
    }

    #[test]
    fn one_shot_draws_pick_four_values_and_two_variation_picks() {
        let mut s = sounds();
        let mut rng = UnityRandom::new(42);
        let id = s.play_on_chunk(SoundId::SlugcatNormalJump, 0, false, 0.8, 1.2, &mut rng);
        assert_eq!(id, EmitterId(0));

        // By hand: Range(0, 2) sample, value x4 (sample vol, trigger vol, sample pitch,
        // trigger pitch), then Range(0, variations) twice (SoundClipReady, SoundObject).
        let mut e = UnityRandom::new(42);
        let sample = e.range_i32(0, 2) as usize;
        let (r1, r2, _r3, r4) = (e.value(), e.value(), e.value(), e.value());
        let variations = if sample == 0 { 3 } else { 1 };
        let _ready = e.range_i32(0, variations);
        let variation = e.range_i32(0, variations) as usize;
        assert_eq!(rng, e);
        assert_eq!(rng, advanced(42, 7), "seven draws");

        let (vol, pitch, clip, ignore) = if sample == 0 {
            (0.2.lerp(0.4, r1), 1.0, 0, 0.0)
        } else {
            (1.0, 1.5, 1, 0.5)
        };
        let want = PlayedSound {
            id: SoundId::SlugcatNormalJump,
            clip,
            variation,
            volume: vol * 0.5.lerp(1.0, r2),
            pitch: pitch * 0.9.lerp(1.1, r4),
            range: 0.5,
            doppler_fac: 1.0,
            ignore_effects: ignore,
            start_at: None,
            source: SoundSource::Chunk {
                emitter: EmitterId(0),
                chunk: 0,
            },
            looped: false,
            emitter_volume: 0.8,
            emitter_pitch: 1.2,
        };
        assert_eq!(s.take_events(), [SoundEvent::Play(want)]);
    }

    #[test]
    fn a_static_loop_restarts_when_heard_again() {
        let mut heard = sounds();
        let mut other_room = Sounds::default();
        let mut rng = UnityRandom::new(1);
        let mut l = StaticSoundLoop::new(SoundId::SlugcatWallJump, crate::Vec2::ZERO, 1.0, 1.0);
        l.update(&mut heard, &mut rng);
        assert_eq!(l.emitter, Some(EmitterId(0)));
        l.update(&mut other_room, &mut rng);
        assert_eq!(l.emitter, None);
        heard.take_events();
        l.update(&mut heard, &mut rng);
        assert_eq!(l.emitter, Some(EmitterId(1)));
        assert!(matches!(heard.take_events()[..], [SoundEvent::Play(_)]));
    }

    #[test]
    fn the_sample_pick_covers_both_samples() {
        let picks: Vec<usize> = (0..20)
            .filter_map(|seed| {
                let mut s = sounds();
                s.play_at(
                    SoundId::SlugcatNormalJump,
                    crate::Vec2::ZERO,
                    1.0,
                    1.0,
                    &mut UnityRandom::new(seed),
                );
                match &s.take_events()[..] {
                    [SoundEvent::Play(p)] => Some(p.clip),
                    _ => None,
                }
            })
            .collect();
        assert_eq!(picks.len(), 20);
        assert!(picks.contains(&0) && picks.contains(&1));
    }

    #[test]
    fn play_all_draws_per_sample_without_a_pick() {
        let mut s = sounds();
        let mut rng = UnityRandom::new(3);
        s.play_disembodied(SoundId::SlugcatSuperJump, -0.5, 1.0, 1.0, &mut rng);
        // Per sample: 4 values + 2 variation picks.
        assert_eq!(rng, advanced(3, 12));
        let clips: Vec<usize> = s
            .take_events()
            .iter()
            .map(|e| match e {
                SoundEvent::Play(p) => {
                    assert_eq!(
                        p.source,
                        SoundSource::Disembodied {
                            pan: -0.5,
                            group: 0
                        }
                    );
                    p.clip
                }
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(clips, [1, 0]);
    }

    #[test]
    fn silent_chance_draws_first_and_may_stop() {
        let (mut played, mut silenced) = (0, 0);
        for seed in 0..40 {
            let mut s = sounds();
            let mut rng = UnityRandom::new(seed);
            let first = UnityRandom::new(seed).value();
            s.play_disembodied(SoundId::PlayerTickAlongInShortcut, 0.0, 1.0, 1.0, &mut rng);
            if first > 0.25 {
                assert_eq!(rng, advanced(seed, 1 + 7), "seed {seed}");
                assert_eq!(s.events().len(), 1);
                played += 1;
            } else {
                assert_eq!(rng, advanced(seed, 1), "seed {seed}");
                assert!(s.events().is_empty());
                silenced += 1;
            }
        }
        assert!(played > 0 && silenced > 0);
    }

    #[test]
    fn dead_and_unknown_ids_draw_nothing() {
        let mut s = sounds();
        let mut rng = UnityRandom::new(9);
        let a = s.play_on_chunk(SoundId::UiSlugcatDie, 0, false, 1.0, 1.0, &mut rng);
        let b = s.play_on_chunk(SoundId::SlugcatStepA, 1, true, 1.0, 1.0, &mut rng);
        assert_eq!(rng, UnityRandom::new(9));
        assert!(s.events().is_empty());
        assert_eq!(
            (a, b),
            (EmitterId(0), EmitterId(1)),
            "emitters exist anyway"
        );
        assert!(Sounds::default().table().ids().next().is_none());
    }

    #[test]
    fn a_missing_variation_creates_no_sound() {
        // `solo` exists bare and as `solo_2`: two variations, `solo_1` and `solo_2`, and
        // only the second loads.
        let (mut played, mut dropped) = (0, 0);
        for seed in 0..40 {
            let mut s = sounds();
            let mut rng = UnityRandom::new(seed);
            let mut e = UnityRandom::new(seed);
            e.range_i32(0, 1);
            for _ in 0..4 {
                e.value();
            }
            let ready = e.range_i32(0, 2);
            s.play_disembodied(SoundId::SlugcatWallJump, 0.0, 1.0, 1.0, &mut rng);
            if ready == 1 {
                let variation = e.range_i32(0, 2) as usize;
                match &s.take_events()[..] {
                    [SoundEvent::Play(p)] => assert_eq!(p.variation, variation),
                    other => panic!("{other:?}"),
                }
                played += 1;
            } else {
                assert!(s.events().is_empty());
                dropped += 1;
            }
            assert_eq!(rng, e, "seed {seed}");
        }
        assert!(played > 0 && dropped > 0);
    }
}
