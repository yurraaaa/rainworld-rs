//! `LizardBreedParams`, as `LizardBreeds.BreedTemplate` fills them per breed. Only the
//! green lizard's are entered so far; a breed without data is not realized (it waits
//! abstract in a realized room, docs/divergences/ D14).

use rw_assets::CreatureType;
use rw_random::UnityRandom;

use crate::color::{Hsl, RandomVariation};
use crate::world::Accessibility;

/// `LizardBreedParams.SpeedMultiplier`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeedMultiplier {
    pub speed: f32,
    pub horizontal: f32,
    pub up: f32,
    pub down: f32,
}

impl SpeedMultiplier {
    const fn new(speed: f32, horizontal: f32, up: f32, down: f32) -> Self {
        Self {
            speed,
            horizontal,
            up,
            down,
        }
    }

    /// `BreedTemplate`'s default for every accessibility class.
    const DEFAULT: Self = Self::new(0.1, 1.0, 1.0, 1.0);
}

impl std::ops::Add for SpeedMultiplier {
    type Output = Self;

    fn add(self, b: Self) -> Self {
        Self::new(
            self.speed + b.speed,
            self.horizontal + b.horizontal,
            self.up + b.up,
            self.down + b.down,
        )
    }
}

impl std::ops::Div<f32> for SpeedMultiplier {
    type Output = Self;

    fn div(self, b: f32) -> Self {
        Self::new(
            self.speed / b,
            self.horizontal / b,
            self.up / b,
            self.down / b,
        )
    }
}

/// `LizardBreedParams`.
#[derive(Debug, Clone, PartialEq)]
pub struct LizardBreedParams {
    pub template: CreatureType,
    pub toughness: f32,
    pub stun_toughness: f32,
    pub bite_damage: f32,
    pub bite_damage_chance: f32,
    pub aggression_curve_exponent: f32,
    pub danger: f32,
    pub bite_delay: i32,
    pub base_speed: f32,
    pub bite_in_front: f32,
    pub bite_rad_bonus: f32,
    pub bite_homing_speed: f32,
    pub bite_chance: f32,
    pub attempt_bite_radius: f32,
    pub get_free_bite_chance: f32,
    pub standard_color: [f32; 3],
    pub regain_footing_counter: i32,
    pub body_mass: f32,
    pub body_size_fac: f32,
    pub body_length_fac: f32,
    pub body_rad_fac: f32,
    pub pull_down_fac: f32,
    pub floor_leverage: f32,
    pub max_muscle_power: f32,
    /// By `AItile.Accessibility` index.
    pub terrain_speeds: [SpeedMultiplier; Accessibility::ALL.len()],
    pub wiggle_speed: f32,
    pub wiggle_delay: i32,
    pub body_stiffnes: f32,
    pub swim_speed: f32,
    pub idle_counter_subtract_when_close_to_idle_pos: i32,
    pub head_shield_angle: f32,
    pub can_exit_lounge: bool,
    pub can_exit_lounge_warm_up: bool,
    pub find_lounge_direction: f32,
    pub lounge_distance: f32,
    pub pre_lounge_crouch: i32,
    pub pre_lounge_crouch_movement: f32,
    pub lounge_speed: f32,
    pub lounge_propulsion_frames: i32,
    pub lounge_maximum_frames: i32,
    pub lounge_jumpyness: f32,
    pub lounge_delay: i32,
    pub risk_of_double_lounge_delay: f32,
    pub post_lounge_stun: i32,
    pub lounge_tendensy: f32,
    pub perfect_vision_angle: f32,
    pub periferal_vision_angle: f32,
    pub shake_prey: i32,
    pub bite_dominance: f32,
    pub limb_size: f32,
    pub limb_thickness: f32,
    pub step_length: f32,
    pub lift_feet: f32,
    pub feet_down: f32,
    pub no_grip_speed: f32,
    pub limb_speed: f32,
    pub limb_quickness: f32,
    pub limb_grip_delay: i32,
    pub smoothen_leg_movement: bool,
    pub leg_pair_displacement: f32,
    pub walk_bob: f32,
    pub tail_segments: i32,
    pub tail_stiffness: f32,
    pub tail_stiffness_decline: f32,
    pub tail_length_factor: f32,
    pub tail_coloration_start: f32,
    pub tail_coloration_exponent: f32,
    pub head_size: f32,
    pub neck_stiffness: f32,
    pub jaw_open_angle: f32,
    pub jaw_open_lower_jaw_fac: f32,
    pub jaw_open_move_jaws_apart: f32,
    pub head_graphics: [i32; 5],
    pub frames_between_look_focus_change: i32,
    pub tongue: bool,
    pub taming_difficulty: f32,
    /// The template's `visualRadius`, `waterVision`, `throughSurfaceVision` and
    /// `movementBasedVision` the breed sets.
    pub vision: crate::creature::CreatureVision,
}

/// The green lizard (`BreedTemplate`'s `GreenLizard` block).
static GREEN: LizardBreedParams = LizardBreedParams::green();

impl LizardBreedParams {
    /// The breed's parameters; `None` for the breeds not entered yet.
    #[must_use]
    pub fn of(kind: CreatureType) -> Option<&'static Self> {
        match kind {
            CreatureType::GreenLizard => Some(&GREEN),
            _ => None,
        }
    }

    const fn green() -> Self {
        let mut terrain_speeds = [SpeedMultiplier::DEFAULT; Accessibility::ALL.len()];
        terrain_speeds[1] = SpeedMultiplier::new(1.0, 1.0, 0.8, 1.7);
        terrain_speeds[3] = SpeedMultiplier::new(0.8, 1.0, 0.8, 1.7);
        // `terrainSpeeds[2]` still has the default speed, so it copies `[1]`.
        terrain_speeds[2] = terrain_speeds[1];
        Self {
            template: CreatureType::GreenLizard,
            toughness: 2.5,
            stun_toughness: 2.5,
            bite_damage: 2.0,
            bite_damage_chance: 0.5,
            aggression_curve_exponent: 0.95,
            danger: 0.45,
            bite_delay: 20,
            base_speed: 6.7,
            bite_in_front: 40.0,
            bite_rad_bonus: 0.0,
            bite_homing_speed: 0.7,
            bite_chance: 1.0 / 3.0,
            attempt_bite_radius: 100.0,
            get_free_bite_chance: 0.45,
            standard_color: [0.2, 1.0, 0.0],
            regain_footing_counter: 10,
            body_mass: 7.5,
            body_size_fac: 1.2,
            body_length_fac: 1.0,
            body_rad_fac: 1.0,
            pull_down_fac: 1.0,
            floor_leverage: 7.0,
            max_muscle_power: 16.0,
            terrain_speeds,
            wiggle_speed: 0.0,
            wiggle_delay: 55,
            body_stiffnes: 0.5,
            swim_speed: 0.6,
            idle_counter_subtract_when_close_to_idle_pos: 0,
            head_shield_angle: 100.0,
            can_exit_lounge: false,
            can_exit_lounge_warm_up: false,
            find_lounge_direction: 0.0,
            lounge_distance: 170.0,
            pre_lounge_crouch: 50,
            pre_lounge_crouch_movement: -0.3,
            lounge_speed: 1.0,
            lounge_propulsion_frames: 40,
            lounge_maximum_frames: 40,
            lounge_jumpyness: 0.0,
            lounge_delay: 150,
            risk_of_double_lounge_delay: 0.3,
            post_lounge_stun: 40,
            lounge_tendensy: 1.0,
            // `Mathf.Lerp(1, -1, 0)` and `Mathf.Lerp(1, -1, 5 / 18)`.
            perfect_vision_angle: 1.0,
            periferal_vision_angle: 1.0 - 2.0 * (5.0 / 18.0),
            shake_prey: 100,
            bite_dominance: 0.8,
            limb_size: 1.4,
            limb_thickness: 1.0,
            step_length: 0.9,
            lift_feet: 0.5,
            feet_down: 1.0,
            no_grip_speed: 0.05,
            limb_speed: 3.0,
            limb_quickness: 0.3,
            limb_grip_delay: 1,
            smoothen_leg_movement: false,
            leg_pair_displacement: 1.0,
            walk_bob: 4.0,
            tail_segments: 7,
            tail_stiffness: 300.0,
            tail_stiffness_decline: 0.6,
            tail_length_factor: 0.9,
            tail_coloration_start: 0.05,
            tail_coloration_exponent: 4.0,
            head_size: 1.0,
            neck_stiffness: 1.0,
            jaw_open_angle: 50.0,
            jaw_open_lower_jaw_fac: 0.5,
            jaw_open_move_jaws_apart: 14.0,
            head_graphics: [1, 1, 1, 1, 1],
            frames_between_look_focus_change: 160,
            tongue: false,
            taming_difficulty: 0.8,
            vision: crate::creature::CreatureVision {
                visual_radius: 850.0,
                water_vision: 0.3,
                through_surface_vision: 0.85,
                movement_based_vision: 0.7,
            },
        }
    }

    /// `TerrainSpeed`.
    #[must_use]
    pub fn terrain_speed(&self, acc: Accessibility) -> SpeedMultiplier {
        self.terrain_speeds[acc.index()]
    }

    /// Body chunk radius: `8 * bodySizeFac * bodyRadFac`.
    #[must_use]
    pub fn chunk_rad(&self) -> f32 {
        8.0 * self.body_size_fac * self.body_rad_fac
    }

    /// Body chunk mass: a third of `bodyMass`.
    #[must_use]
    pub fn chunk_mass(&self) -> f32 {
        self.body_mass / 3.0
    }

    /// The length of the two chunk rods: `17 * bodyLengthFac * (bodySizeFac + 1) / 2`.
    #[must_use]
    pub fn rod_length(&self) -> f32 {
        17.0 * self.body_length_fac * ((self.body_size_fac + 1.0) / 2.0)
    }

    /// The `Lizard` constructor's `effectColor`, drawn under the ID's seed (the caller
    /// saves and restores the stream). The breeds without their own variation take the
    /// standard colour.
    pub fn effect_color(&self, rng: &mut UnityRandom) -> [f32; 3] {
        let (h, dh, l, dl, kl) = match self.template {
            CreatureType::PinkLizard => (0.87, 0.1, 0.5, 0.15, 0.1),
            CreatureType::GreenLizard => (0.32, 0.1, 0.5, 0.15, 0.1),
            CreatureType::BlueLizard => (0.57, 0.08, 0.5, 0.15, 0.1),
            CreatureType::YellowLizard => (0.1, 0.05, 0.5, 0.15, 0.1),
            CreatureType::Salamander => (0.9, 0.15, 0.4, 0.15, 0.2),
            CreatureType::RedLizard => (0.0025, 0.02, 0.5, 0.15, 0.1),
            CreatureType::CyanLizard => (0.49, 0.04, 0.5, 0.15, 0.1),
            _ => return self.standard_color,
        };
        let hue = rng.wrapped_random_variation(h, dh, 0.6);
        let lightness = rng.clamped_random_variation(l, dl, kl);
        Hsl::new(hue, 1.0, lightness).rgb()
    }
}
