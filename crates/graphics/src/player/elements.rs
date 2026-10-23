//! Atlas element names the slugcat sprites use (`rainWorld` atlas, base game).
//! Mirrors: - (atlas element name table)

/// `HeadA{i}`: head seen at angle `i * 360 / 34` degrees from upright.
pub const HEAD_A: [&str; 18] = [
    "HeadA0", "HeadA1", "HeadA2", "HeadA3", "HeadA4", "HeadA5", "HeadA6", "HeadA7", "HeadA8",
    "HeadA9", "HeadA10", "HeadA11", "HeadA12", "HeadA13", "HeadA14", "HeadA15", "HeadA16",
    "HeadA17",
];

/// `FaceA{i}`: open eyes looking in direction `i * 22.5` degrees.
pub const FACE_A: [&str; 9] = [
    "FaceA0", "FaceA1", "FaceA2", "FaceA3", "FaceA4", "FaceA5", "FaceA6", "FaceA7", "FaceA8",
];

/// `FaceB{i}`: blinking.
pub const FACE_B: [&str; 9] = [
    "FaceB0", "FaceB1", "FaceB2", "FaceB3", "FaceB4", "FaceB5", "FaceB6", "FaceB7", "FaceB8",
];

pub const FACE_DEAD: &str = "FaceDead";
pub const FACE_STUNNED: &str = "FaceStunned";

/// `LegsA{frame}`: walking.
pub const LEGS_A: [&str; 7] = [
    "LegsA0", "LegsA1", "LegsA2", "LegsA3", "LegsA4", "LegsA5", "LegsA6",
];

/// `LegsACrawling{frame / 2}`.
pub const LEGS_A_CRAWLING: [&str; 6] = [
    "LegsACrawling0",
    "LegsACrawling1",
    "LegsACrawling2",
    "LegsACrawling3",
    "LegsACrawling4",
    "LegsACrawling5",
];

/// `LegsAOnPole{frame}`: walking on a horizontal beam.
pub const LEGS_A_ON_POLE: [&str; 7] = [
    "LegsAOnPole0",
    "LegsAOnPole1",
    "LegsAOnPole2",
    "LegsAOnPole3",
    "LegsAOnPole4",
    "LegsAOnPole5",
    "LegsAOnPole6",
];

/// `LegsAClimbing{n}`: corridor climbing.
pub const LEGS_A_CLIMBING: [&str; 7] = [
    "LegsAClimbing0",
    "LegsAClimbing1",
    "LegsAClimbing2",
    "LegsAClimbing3",
    "LegsAClimbing4",
    "LegsAClimbing5",
    "LegsAClimbing6",
];

pub const LEGS_A_AIR: &str = "LegsAAir0";
pub const LEGS_A_WALL: &str = "LegsAWall";
/// Standing on a pole tip.
pub const LEGS_A_POLE: &str = "LegsAPole";
/// Climbing a vertical pole.
pub const LEGS_A_VERTICAL_POLE: &str = "LegsAVerticalPole";

/// `PlayerArm{i}`: an arm about `2 * i` px long.
pub const PLAYER_ARM: [&str; 13] = [
    "PlayerArm0",
    "PlayerArm1",
    "PlayerArm2",
    "PlayerArm3",
    "PlayerArm4",
    "PlayerArm5",
    "PlayerArm6",
    "PlayerArm7",
    "PlayerArm8",
    "PlayerArm9",
    "PlayerArm10",
    "PlayerArm11",
    "PlayerArm12",
];

pub const BODY_A: &str = "BodyA";
pub const HIPS_A: &str = "HipsA";
/// Hand drawn over a beam it grips.
pub const ON_TOP_OF_TERRAIN_HAND: &str = "OnTopOfTerrainHand";
/// Hand over a beam it hangs from.
pub const ON_TOP_OF_TERRAIN_HAND_2: &str = "OnTopOfTerrainHand2";
/// Futile's runtime 16x16 white texture (not in any atlas file); the tail mesh samples it.
pub const FUTILE_WHITE: &str = "Futile_White";

/// `LizardArm_NN` (1 to 54), by `NN - 1`.
pub const LIZARD_ARM: [&str; 54] = [
    "LizardArm_01",
    "LizardArm_02",
    "LizardArm_03",
    "LizardArm_04",
    "LizardArm_05",
    "LizardArm_06",
    "LizardArm_07",
    "LizardArm_08",
    "LizardArm_09",
    "LizardArm_10",
    "LizardArm_11",
    "LizardArm_12",
    "LizardArm_13",
    "LizardArm_14",
    "LizardArm_15",
    "LizardArm_16",
    "LizardArm_17",
    "LizardArm_18",
    "LizardArm_19",
    "LizardArm_20",
    "LizardArm_21",
    "LizardArm_22",
    "LizardArm_23",
    "LizardArm_24",
    "LizardArm_25",
    "LizardArm_26",
    "LizardArm_27",
    "LizardArm_28",
    "LizardArm_29",
    "LizardArm_30",
    "LizardArm_31",
    "LizardArm_32",
    "LizardArm_33",
    "LizardArm_34",
    "LizardArm_35",
    "LizardArm_36",
    "LizardArm_37",
    "LizardArm_38",
    "LizardArm_39",
    "LizardArm_40",
    "LizardArm_41",
    "LizardArm_42",
    "LizardArm_43",
    "LizardArm_44",
    "LizardArm_45",
    "LizardArm_46",
    "LizardArm_47",
    "LizardArm_48",
    "LizardArm_49",
    "LizardArm_50",
    "LizardArm_51",
    "LizardArm_52",
    "LizardArm_53",
    "LizardArm_54",
];

/// `LizardArmColor_NN` (1 to 54), by `NN - 1`.
pub const LIZARD_ARM_COLOR: [&str; 54] = [
    "LizardArmColor_01",
    "LizardArmColor_02",
    "LizardArmColor_03",
    "LizardArmColor_04",
    "LizardArmColor_05",
    "LizardArmColor_06",
    "LizardArmColor_07",
    "LizardArmColor_08",
    "LizardArmColor_09",
    "LizardArmColor_10",
    "LizardArmColor_11",
    "LizardArmColor_12",
    "LizardArmColor_13",
    "LizardArmColor_14",
    "LizardArmColor_15",
    "LizardArmColor_16",
    "LizardArmColor_17",
    "LizardArmColor_18",
    "LizardArmColor_19",
    "LizardArmColor_20",
    "LizardArmColor_21",
    "LizardArmColor_22",
    "LizardArmColor_23",
    "LizardArmColor_24",
    "LizardArmColor_25",
    "LizardArmColor_26",
    "LizardArmColor_27",
    "LizardArmColor_28",
    "LizardArmColor_29",
    "LizardArmColor_30",
    "LizardArmColor_31",
    "LizardArmColor_32",
    "LizardArmColor_33",
    "LizardArmColor_34",
    "LizardArmColor_35",
    "LizardArmColor_36",
    "LizardArmColor_37",
    "LizardArmColor_38",
    "LizardArmColor_39",
    "LizardArmColor_40",
    "LizardArmColor_41",
    "LizardArmColor_42",
    "LizardArmColor_43",
    "LizardArmColor_44",
    "LizardArmColor_45",
    "LizardArmColor_46",
    "LizardArmColor_47",
    "LizardArmColor_48",
    "LizardArmColor_49",
    "LizardArmColor_50",
    "LizardArmColor_51",
    "LizardArmColor_52",
    "LizardArmColor_53",
    "LizardArmColor_54",
];

/// The lizard jaw, `LizardJaw{turn}.{graphic}`, by turn (0 to 3) and graphic (0 to 9).
pub const LIZARD_JAW: [[&str; 10]; 4] = [
    [
        "LizardJaw0.0",
        "LizardJaw0.1",
        "LizardJaw0.2",
        "LizardJaw0.3",
        "LizardJaw0.4",
        "LizardJaw0.5",
        "LizardJaw0.6",
        "LizardJaw0.7",
        "LizardJaw0.8",
        "LizardJaw0.9",
    ],
    [
        "LizardJaw1.0",
        "LizardJaw1.1",
        "LizardJaw1.2",
        "LizardJaw1.3",
        "LizardJaw1.4",
        "LizardJaw1.5",
        "LizardJaw1.6",
        "LizardJaw1.7",
        "LizardJaw1.8",
        "LizardJaw1.9",
    ],
    [
        "LizardJaw2.0",
        "LizardJaw2.1",
        "LizardJaw2.2",
        "LizardJaw2.3",
        "LizardJaw2.4",
        "LizardJaw2.5",
        "LizardJaw2.6",
        "LizardJaw2.7",
        "LizardJaw2.8",
        "LizardJaw2.9",
    ],
    [
        "LizardJaw3.0",
        "LizardJaw3.1",
        "LizardJaw3.2",
        "LizardJaw3.3",
        "LizardJaw3.4",
        "LizardJaw3.5",
        "LizardJaw3.6",
        "LizardJaw3.7",
        "LizardJaw3.8",
        "LizardJaw3.9",
    ],
];

/// The lizard lower teeth, `LizardLowerTeeth{turn}.{graphic}`, by turn (0 to 3) and graphic (0 to 9).
pub const LIZARD_LOWER_TEETH: [[&str; 10]; 4] = [
    [
        "LizardLowerTeeth0.0",
        "LizardLowerTeeth0.1",
        "LizardLowerTeeth0.2",
        "LizardLowerTeeth0.3",
        "LizardLowerTeeth0.4",
        "LizardLowerTeeth0.5",
        "LizardLowerTeeth0.6",
        "LizardLowerTeeth0.7",
        "LizardLowerTeeth0.8",
        "LizardLowerTeeth0.9",
    ],
    [
        "LizardLowerTeeth1.0",
        "LizardLowerTeeth1.1",
        "LizardLowerTeeth1.2",
        "LizardLowerTeeth1.3",
        "LizardLowerTeeth1.4",
        "LizardLowerTeeth1.5",
        "LizardLowerTeeth1.6",
        "LizardLowerTeeth1.7",
        "LizardLowerTeeth1.8",
        "LizardLowerTeeth1.9",
    ],
    [
        "LizardLowerTeeth2.0",
        "LizardLowerTeeth2.1",
        "LizardLowerTeeth2.2",
        "LizardLowerTeeth2.3",
        "LizardLowerTeeth2.4",
        "LizardLowerTeeth2.5",
        "LizardLowerTeeth2.6",
        "LizardLowerTeeth2.7",
        "LizardLowerTeeth2.8",
        "LizardLowerTeeth2.9",
    ],
    [
        "LizardLowerTeeth3.0",
        "LizardLowerTeeth3.1",
        "LizardLowerTeeth3.2",
        "LizardLowerTeeth3.3",
        "LizardLowerTeeth3.4",
        "LizardLowerTeeth3.5",
        "LizardLowerTeeth3.6",
        "LizardLowerTeeth3.7",
        "LizardLowerTeeth3.8",
        "LizardLowerTeeth3.9",
    ],
];

/// The lizard upper teeth, `LizardUpperTeeth{turn}.{graphic}`, by turn (0 to 3) and graphic (0 to 9).
pub const LIZARD_UPPER_TEETH: [[&str; 10]; 4] = [
    [
        "LizardUpperTeeth0.0",
        "LizardUpperTeeth0.1",
        "LizardUpperTeeth0.2",
        "LizardUpperTeeth0.3",
        "LizardUpperTeeth0.4",
        "LizardUpperTeeth0.5",
        "LizardUpperTeeth0.6",
        "LizardUpperTeeth0.7",
        "LizardUpperTeeth0.8",
        "LizardUpperTeeth0.9",
    ],
    [
        "LizardUpperTeeth1.0",
        "LizardUpperTeeth1.1",
        "LizardUpperTeeth1.2",
        "LizardUpperTeeth1.3",
        "LizardUpperTeeth1.4",
        "LizardUpperTeeth1.5",
        "LizardUpperTeeth1.6",
        "LizardUpperTeeth1.7",
        "LizardUpperTeeth1.8",
        "LizardUpperTeeth1.9",
    ],
    [
        "LizardUpperTeeth2.0",
        "LizardUpperTeeth2.1",
        "LizardUpperTeeth2.2",
        "LizardUpperTeeth2.3",
        "LizardUpperTeeth2.4",
        "LizardUpperTeeth2.5",
        "LizardUpperTeeth2.6",
        "LizardUpperTeeth2.7",
        "LizardUpperTeeth2.8",
        "LizardUpperTeeth2.9",
    ],
    [
        "LizardUpperTeeth3.0",
        "LizardUpperTeeth3.1",
        "LizardUpperTeeth3.2",
        "LizardUpperTeeth3.3",
        "LizardUpperTeeth3.4",
        "LizardUpperTeeth3.5",
        "LizardUpperTeeth3.6",
        "LizardUpperTeeth3.7",
        "LizardUpperTeeth3.8",
        "LizardUpperTeeth3.9",
    ],
];

/// The lizard head, `LizardHead{turn}.{graphic}`, by turn (0 to 3) and graphic (0 to 9).
pub const LIZARD_HEAD: [[&str; 10]; 4] = [
    [
        "LizardHead0.0",
        "LizardHead0.1",
        "LizardHead0.2",
        "LizardHead0.3",
        "LizardHead0.4",
        "LizardHead0.5",
        "LizardHead0.6",
        "LizardHead0.7",
        "LizardHead0.8",
        "LizardHead0.9",
    ],
    [
        "LizardHead1.0",
        "LizardHead1.1",
        "LizardHead1.2",
        "LizardHead1.3",
        "LizardHead1.4",
        "LizardHead1.5",
        "LizardHead1.6",
        "LizardHead1.7",
        "LizardHead1.8",
        "LizardHead1.9",
    ],
    [
        "LizardHead2.0",
        "LizardHead2.1",
        "LizardHead2.2",
        "LizardHead2.3",
        "LizardHead2.4",
        "LizardHead2.5",
        "LizardHead2.6",
        "LizardHead2.7",
        "LizardHead2.8",
        "LizardHead2.9",
    ],
    [
        "LizardHead3.0",
        "LizardHead3.1",
        "LizardHead3.2",
        "LizardHead3.3",
        "LizardHead3.4",
        "LizardHead3.5",
        "LizardHead3.6",
        "LizardHead3.7",
        "LizardHead3.8",
        "LizardHead3.9",
    ],
];

/// The lizard eyes, `LizardEyes{turn}.{graphic}`, by turn (0 to 3) and graphic (0 to 9).
pub const LIZARD_EYES: [[&str; 10]; 4] = [
    [
        "LizardEyes0.0",
        "LizardEyes0.1",
        "LizardEyes0.2",
        "LizardEyes0.3",
        "LizardEyes0.4",
        "LizardEyes0.5",
        "LizardEyes0.6",
        "LizardEyes0.7",
        "LizardEyes0.8",
        "LizardEyes0.9",
    ],
    [
        "LizardEyes1.0",
        "LizardEyes1.1",
        "LizardEyes1.2",
        "LizardEyes1.3",
        "LizardEyes1.4",
        "LizardEyes1.5",
        "LizardEyes1.6",
        "LizardEyes1.7",
        "LizardEyes1.8",
        "LizardEyes1.9",
    ],
    [
        "LizardEyes2.0",
        "LizardEyes2.1",
        "LizardEyes2.2",
        "LizardEyes2.3",
        "LizardEyes2.4",
        "LizardEyes2.5",
        "LizardEyes2.6",
        "LizardEyes2.7",
        "LizardEyes2.8",
        "LizardEyes2.9",
    ],
    [
        "LizardEyes3.0",
        "LizardEyes3.1",
        "LizardEyes3.2",
        "LizardEyes3.3",
        "LizardEyes3.4",
        "LizardEyes3.5",
        "LizardEyes3.6",
        "LizardEyes3.7",
        "LizardEyes3.8",
        "LizardEyes3.9",
    ],
];

/// `"LizardScaleA" + graphic`: a scale or spike's dark part.
pub const LIZARD_SCALE_A: [&str; 7] = [
    "LizardScaleA0",
    "LizardScaleA1",
    "LizardScaleA2",
    "LizardScaleA3",
    "LizardScaleA4",
    "LizardScaleA5",
    "LizardScaleA6",
];

/// `"LizardScaleB" + graphic`: its coloured part.
pub const LIZARD_SCALE_B: [&str; 7] = [
    "LizardScaleB0",
    "LizardScaleB1",
    "LizardScaleB2",
    "LizardScaleB3",
    "LizardScaleB4",
    "LizardScaleB5",
    "LizardScaleB6",
];

pub const CIRCLE_20: &str = "Circle20";
pub const PIXEL: &str = "pixel";

/// The `LizardScaleA` elements' `sourcePixelSize.y` in the `rainWorld` atlas.
pub const LIZARD_SCALE_A_HEIGHT: [f32; 7] = [18.0, 14.0, 10.0, 18.0, 37.0, 29.0, 17.0];
