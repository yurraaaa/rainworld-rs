//! Per-tick controller input (`Player.InputPackage`) and the history ring.

/// Number of past inputs the player keeps (`Player.input`, newest first).
pub const INPUT_HISTORY: usize = 10;

/// One tick of controller input.
///
/// Only the fields the port reads. The game's package also carries the special button
/// (DLC) and the analogue stick (the digital `x`/`y` are derived from it with a 0.5
/// threshold).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Input {
    /// Horizontal direction: -1 left, 0, +1 right. Other values are reduced to their sign.
    pub x: i8,
    /// Vertical direction: -1 down, 0, +1 up. Other values are reduced to their sign.
    pub y: i8,
    /// Jump button held.
    pub jump: bool,
    /// Map button held (`mp`): the HUD's `RevealMap`; the player stands still.
    pub map: bool,
    /// Pick-up button held (`pckp`, Take).
    pub pickup: bool,
    /// Throw button held (`thrw`).
    pub throw: bool,
}

impl Input {
    /// No direction, no buttons.
    pub const NONE: Self = Self {
        x: 0,
        y: 0,
        jump: false,
        map: false,
        pickup: false,
        throw: false,
    };

    /// An input with each axis reduced to -1, 0 or +1.
    #[must_use]
    pub const fn new(x: i8, y: i8, jump: bool) -> Self {
        Self {
            x: x.signum(),
            y: y.signum(),
            jump,
            map: false,
            pickup: false,
            throw: false,
        }
    }

    /// The same input with the map button `map`.
    #[must_use]
    pub const fn with_map(self, map: bool) -> Self {
        Self { map, ..self }
    }

    /// The same input with the pick-up and throw buttons.
    #[must_use]
    pub const fn with_hands(self, pickup: bool, throw: bool) -> Self {
        Self {
            pickup,
            throw,
            ..self
        }
    }

    /// The horizontal direction while down is held, else 0 (`downDiagonal`).
    ///
    /// This is the game's digital (Remix) rule: down plus left or right gives that side.
    #[must_use]
    pub const fn down_diagonal(self) -> i8 {
        if self.y < 0 { self.x } else { 0 }
    }

    /// The same input with both axes reduced to their sign.
    pub(crate) const fn normalized(self) -> Self {
        Self::new(self.x, self.y, self.jump)
            .with_map(self.map)
            .with_hands(self.pickup, self.throw)
    }
}

/// `Player.PlayerController`: when set, the player's input comes from it instead of the
/// player's own controls (Hunter's scripted start would be another).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerController {
    /// `Player.NullController`: nothing pressed.
    Null,
}

impl PlayerController {
    /// `GetInput`.
    #[must_use]
    pub fn input(self) -> Input {
        match self {
            Self::Null => Input::NONE,
        }
    }
}
