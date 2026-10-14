//! Keyboard to per-tick input.
//!
//! Key events update [`InputState`] whenever they arrive; once per simulation tick
//! [`InputState::take_tick_input`] produces an [`Input`]. A key pressed and released
//! between two ticks still counts for the next tick, so short taps are never lost.

use rw_sim::Input;
use winit::keyboard::KeyCode;

/// A held game action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Left,
    Right,
    Up,
    Down,
    Jump,
}

impl Action {
    const COUNT: usize = 5;

    const fn index(self) -> usize {
        self as usize
    }
}

/// App commands, acted on immediately rather than per tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    Respawn,
    ToggleGrid,
    Screenshot,
    Quit,
}

impl Action {
    /// What a physical key does: arrows/WASD move, Space/Z jump.
    #[must_use]
    pub fn from_key(key: KeyCode) -> Option<Self> {
        Some(match key {
            KeyCode::ArrowLeft | KeyCode::KeyA => Action::Left,
            KeyCode::ArrowRight | KeyCode::KeyD => Action::Right,
            KeyCode::ArrowUp | KeyCode::KeyW => Action::Up,
            KeyCode::ArrowDown | KeyCode::KeyS => Action::Down,
            KeyCode::Space | KeyCode::KeyZ => Action::Jump,
            _ => return None,
        })
    }
}

impl Command {
    /// R respawns, G toggles the tile grid, F12 saves a screenshot, Esc quits.
    #[must_use]
    pub fn from_key(key: KeyCode) -> Option<Self> {
        Some(match key {
            KeyCode::KeyR => Command::Respawn,
            KeyCode::KeyG => Command::ToggleGrid,
            KeyCode::F12 => Command::Screenshot,
            KeyCode::Escape => Command::Quit,
            _ => return None,
        })
    }
}

/// Held keys plus presses since the last tick.
#[derive(Debug, Clone, Default)]
pub struct InputState {
    held: [bool; Action::COUNT],
    pressed_since_tick: [bool; Action::COUNT],
}

impl InputState {
    /// Records a press (`pressed = true`) or release of `action`.
    pub fn set(&mut self, action: Action, pressed: bool) {
        let i = action.index();
        self.held[i] = pressed;
        if pressed {
            self.pressed_since_tick[i] = true;
        }
    }

    /// Releases everything (e.g. on focus loss).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// The input for one tick: an action counts if held now or pressed since the
    /// previous call. Opposite directions cancel to 0.
    pub fn take_tick_input(&mut self) -> Input {
        let input = Input {
            x: self.axis(Action::Left, Action::Right),
            y: self.axis(Action::Down, Action::Up),
            jump: self.is_on(Action::Jump),
        };
        self.pressed_since_tick = [false; Action::COUNT];
        input
    }

    /// Held now or pressed since the last tick.
    fn is_on(&self, action: Action) -> bool {
        self.held[action.index()] || self.pressed_since_tick[action.index()]
    }

    /// -1, 0 or 1 from a pair of opposite actions.
    fn axis(&self, negative: Action, positive: Action) -> i8 {
        i8::from(self.is_on(positive)) - i8::from(self.is_on(negative))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_and_wasd_map_to_the_same_actions() {
        assert_eq!(Action::from_key(KeyCode::ArrowLeft), Some(Action::Left));
        assert_eq!(Action::from_key(KeyCode::KeyA), Some(Action::Left));
        assert_eq!(Action::from_key(KeyCode::ArrowRight), Some(Action::Right));
        assert_eq!(Action::from_key(KeyCode::KeyD), Some(Action::Right));
        assert_eq!(Action::from_key(KeyCode::ArrowUp), Some(Action::Up));
        assert_eq!(Action::from_key(KeyCode::KeyW), Some(Action::Up));
        assert_eq!(Action::from_key(KeyCode::ArrowDown), Some(Action::Down));
        assert_eq!(Action::from_key(KeyCode::KeyS), Some(Action::Down));
        assert_eq!(Action::from_key(KeyCode::Space), Some(Action::Jump));
        assert_eq!(Action::from_key(KeyCode::KeyZ), Some(Action::Jump));
        assert_eq!(Action::from_key(KeyCode::KeyQ), None);
    }

    #[test]
    fn commands() {
        assert_eq!(Command::from_key(KeyCode::KeyR), Some(Command::Respawn));
        assert_eq!(Command::from_key(KeyCode::KeyG), Some(Command::ToggleGrid));
        assert_eq!(Command::from_key(KeyCode::F12), Some(Command::Screenshot));
        assert_eq!(Command::from_key(KeyCode::Escape), Some(Command::Quit));
        assert_eq!(Command::from_key(KeyCode::Space), None);
    }

    #[test]
    fn held_keys_give_axes() {
        let mut s = InputState::default();
        s.set(Action::Right, true);
        s.set(Action::Down, true);
        let i = s.take_tick_input();
        assert_eq!(
            i,
            Input {
                x: 1,
                y: -1,
                jump: false
            }
        );
        // Still held on the next tick.
        assert_eq!(s.take_tick_input(), i);
    }

    #[test]
    fn opposite_directions_cancel() {
        let mut s = InputState::default();
        s.set(Action::Left, true);
        s.set(Action::Right, true);
        s.set(Action::Up, true);
        s.set(Action::Down, true);
        assert_eq!(s.take_tick_input(), Input::default());
    }

    #[test]
    fn tap_between_ticks_counts_once() {
        let mut s = InputState::default();
        s.set(Action::Jump, true);
        s.set(Action::Jump, false);
        assert!(s.take_tick_input().jump);
        assert!(!s.take_tick_input().jump);
    }

    #[test]
    fn release_takes_effect_next_tick_after_latch() {
        let mut s = InputState::default();
        s.set(Action::Left, true);
        assert_eq!(s.take_tick_input().x, -1);
        s.set(Action::Left, false);
        assert_eq!(s.take_tick_input().x, 0);
    }

    #[test]
    fn clear_releases_everything() {
        let mut s = InputState::default();
        s.set(Action::Jump, true);
        s.clear();
        assert_eq!(s.take_tick_input(), Input::default());
    }
}
