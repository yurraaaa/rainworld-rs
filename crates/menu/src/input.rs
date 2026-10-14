//! Menu input: `RWInput.PlayerUIInput`, the Rewired "UI" action category read per tick,
//! plus the mouse the menus read directly. Platform code (winit, later touch) feeds a
//! [`UiInputState`] with [`KeyboardKey`] presses and mouse positions in stage pixels.
//!
//! The keyboard bindings are Rewired's shipped defaults for player 0's UI map
//! (`RainWorld_Data/level0`, the "Rewired Input Manager" object;
//! `rainworld-re/notes/menu.md` "Input"). Saved custom bindings are not read.

/// The Rewired `KeyboardKeyCode`s the default maps use (numeric values are Rewired's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyboardKey {
    Return = 13,
    Escape = 27,
    Space = 32,
    C = 99,
    X = 120,
    Z = 122,
    UpArrow = 273,
    DownArrow = 274,
    RightArrow = 275,
    LeftArrow = 276,
    LeftShift = 304,
}

/// Rewired UI-category actions (`RWInput.PlayerInputLogic` category 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiAction {
    /// Axis 6.
    Horizontal = 6,
    /// Axis 7.
    Vertical = 7,
    /// Button 8, read as `jmp`.
    Submit = 8,
    /// Button 9, read as `thrw`.
    Cancel = 9,
    /// Button 10 (not read by `PlayerUIInput`).
    Alternate = 10,
    /// Button 12.
    CheatHoldLeft = 12,
    /// Button 13, read as `mp`.
    CheatHoldRight = 13,
}

/// One element of a keyboard map: the key, its action and, for an axis, the sign.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBinding {
    pub action: UiAction,
    pub key: KeyboardKey,
    /// -1 for a negative axis contribution, else 1.
    pub sign: i8,
}

impl KeyBinding {
    /// Player 0's default keyboard map in map category 1 ("UI"), in the data's order.
    pub const DEFAULT_UI_KEYBOARD: [Self; 9] = [
        Self::new(UiAction::Vertical, KeyboardKey::UpArrow, 1),
        Self::new(UiAction::Vertical, KeyboardKey::DownArrow, -1),
        Self::new(UiAction::Horizontal, KeyboardKey::RightArrow, 1),
        Self::new(UiAction::Horizontal, KeyboardKey::LeftArrow, -1),
        Self::new(UiAction::Submit, KeyboardKey::Return, 1),
        Self::new(UiAction::Cancel, KeyboardKey::Escape, 1),
        Self::new(UiAction::Alternate, KeyboardKey::X, 1),
        Self::new(UiAction::Submit, KeyboardKey::Space, 1),
        Self::new(UiAction::Submit, KeyboardKey::Z, 1),
    ];

    const fn new(action: UiAction, key: KeyboardKey, sign: i8) -> Self {
        Self { action, key, sign }
    }
}

/// `Player.InputPackage` as `PlayerUIInput` fills it from the UI actions.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UiInput {
    pub x: i8,
    pub y: i8,
    /// UI submit.
    pub jmp: bool,
    /// UI cancel.
    pub thrw: bool,
    /// Action 13.
    pub mp: bool,
    pub analogue_dir: [f32; 2],
    pub down_diagonal: i8,
}

impl UiInput {
    /// -1 below `-threshold`, 1 above `threshold`, else 0.
    fn digital(value: f32, threshold: f32) -> i8 {
        if value < -threshold {
            -1
        } else if value > threshold {
            1
        } else {
            0
        }
    }
}

/// The mouse as Unity reports it, in stage pixels (y up).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MouseState {
    pub pos: [f32; 2],
    /// Left, right, middle.
    pub buttons: [bool; 3],
    /// `Input.GetAxis("Mouse ScrollWheel")` accumulated since the last tick.
    pub scroll: f32,
}

/// Held keys and the mouse; keys pressed and released between two ticks still count
/// for the next tick.
#[derive(Debug, Clone, Default)]
pub struct UiInputState {
    held: Vec<KeyboardKey>,
    pressed_since_tick: Vec<KeyboardKey>,
    pub mouse: MouseState,
}

impl UiInputState {
    pub fn set_key(&mut self, key: KeyboardKey, pressed: bool) {
        self.held.retain(|k| *k != key);
        if pressed {
            self.held.push(key);
            if !self.pressed_since_tick.contains(&key) {
                self.pressed_since_tick.push(key);
            }
        }
    }

    /// Releases every key and mouse button (focus lost).
    pub fn clear(&mut self) {
        self.held.clear();
        self.pressed_since_tick.clear();
        self.mouse.buttons = [false; 3];
    }

    fn is_on(&self, key: KeyboardKey) -> bool {
        self.held.contains(&key) || self.pressed_since_tick.contains(&key)
    }

    /// `GetButton(action)`.
    fn button(&self, action: UiAction) -> bool {
        KeyBinding::DEFAULT_UI_KEYBOARD
            .iter()
            .any(|b| b.action == action && self.is_on(b.key))
    }

    /// `GetAxisRaw(action)` from the axis's button elements: the sum of the pressed
    /// keys' signs, clamped to -1..1 (inferred: how Rewired combines opposite keys is
    /// not checked).
    fn axis(&self, action: UiAction) -> f32 {
        let sum: i32 = KeyBinding::DEFAULT_UI_KEYBOARD
            .iter()
            .filter(|b| b.action == action && self.is_on(b.key))
            .map(|b| i32::from(b.sign))
            .sum();
        sum.clamp(-1, 1) as f32
    }

    /// `PlayerInputLogic(1, 0)` for this tick; clears the presses latched since the
    /// last call. Remix's analogue sensitivity and the inversion options are off.
    pub fn take_tick_input(&mut self) -> UiInput {
        let mut dir = [
            self.axis(UiAction::Horizontal),
            self.axis(UiAction::Vertical),
        ];
        let length = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
        if length > 1.0 {
            dir = dir.map(|d| d / length);
        }
        let input = UiInput {
            x: UiInput::digital(dir[0], 0.5),
            y: UiInput::digital(dir[1], 0.5),
            jmp: self.button(UiAction::Submit),
            thrw: self.button(UiAction::Cancel),
            mp: self.button(UiAction::CheatHoldRight),
            analogue_dir: dir,
            down_diagonal: if dir[1] < -0.05 {
                UiInput::digital(dir[0], 0.05)
            } else {
                0
            },
        };
        self.pressed_since_tick.clear();
        input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ui_keys() {
        let mut s = UiInputState::default();
        s.set_key(KeyboardKey::Z, true);
        s.set_key(KeyboardKey::DownArrow, true);
        let i = s.take_tick_input();
        assert_eq!((i.x, i.y, i.jmp, i.thrw), (0, -1, true, false));
        s.set_key(KeyboardKey::Z, false);
        s.set_key(KeyboardKey::DownArrow, false);
        s.set_key(KeyboardKey::Escape, true);
        let i = s.take_tick_input();
        assert_eq!((i.x, i.y, i.jmp, i.thrw), (0, 0, false, true));
        // X is UI "alternate", not cancel.
        s.clear();
        s.set_key(KeyboardKey::X, true);
        assert_eq!(s.take_tick_input(), UiInput::default());
    }

    #[test]
    fn diagonal_is_normalised_but_still_both_directions() {
        let mut s = UiInputState::default();
        s.set_key(KeyboardKey::LeftArrow, true);
        s.set_key(KeyboardKey::DownArrow, true);
        let i = s.take_tick_input();
        assert_eq!((i.x, i.y, i.down_diagonal), (-1, -1, -1));
        assert!((i.analogue_dir[0] + std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    }

    #[test]
    fn tap_between_ticks_counts_once() {
        let mut s = UiInputState::default();
        s.set_key(KeyboardKey::Return, true);
        s.set_key(KeyboardKey::Return, false);
        assert!(s.take_tick_input().jmp);
        assert!(!s.take_tick_input().jmp);
    }
}
