//! Atlas element names the slugcat sprites use (`rainWorld` atlas, base game).

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
