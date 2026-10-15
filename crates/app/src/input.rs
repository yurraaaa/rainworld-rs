//! Keyboard to per-tick input.
//!
//! Key events update [`InputState`] whenever they arrive; once per simulation tick
//! [`InputState::take_tick_input`] produces an [`Input`]. A key pressed and released
//! between two ticks still counts for the next tick, so short taps are never lost.

use rw_menu::KeyboardKey;
use rw_sim::Input;
use winit::keyboard::KeyCode;

/// The physical key behind a Rewired keyboard key (menu input).
pub trait WinitKey: Sized {
    fn from_winit(code: KeyCode) -> Option<Self>;
}

impl WinitKey for KeyboardKey {
    fn from_winit(code: KeyCode) -> Option<Self> {
        Some(match code {
            KeyCode::Enter | KeyCode::NumpadEnter => Self::Return,
            KeyCode::Escape => Self::Escape,
            KeyCode::Space => Self::Space,
            KeyCode::KeyC => Self::C,
            KeyCode::KeyS => Self::S,
            KeyCode::KeyX => Self::X,
            KeyCode::KeyZ => Self::Z,
            KeyCode::ArrowUp => Self::UpArrow,
            KeyCode::ArrowDown => Self::DownArrow,
            KeyCode::ArrowRight => Self::RightArrow,
            KeyCode::ArrowLeft => Self::LeftArrow,
            KeyCode::ShiftLeft => Self::LeftShift,
            _ => return None,
        })
    }
}

/// A held game action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Left,
    Right,
    Up,
    Down,
    Jump,
    /// The map button: shows the HUD's meters and stands the slugcat still.
    Map,
    /// Take: picks up, drops and eats.
    PickUp,
    Throw,
}

impl Action {
    const COUNT: usize = 8;

    const fn index(self) -> usize {
        self as usize
    }
}

/// App commands, acted on immediately rather than per tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    Respawn,
    ToggleView,
    ToggleGrid,
    Screenshot,
    Quit,
}

impl Action {
    /// What a physical key does in Rewired's default gameplay keyboard map
    /// (`rainworld-re/notes/menu.md` "Input"): arrows move, Z jumps, X throws, left Shift
    /// picks up, Space is the map button. C (special) and Esc (pause) have no use in the
    /// port yet.
    #[must_use]
    pub fn from_key(key: KeyCode) -> Option<Self> {
        Some(match key {
            KeyCode::ArrowLeft => Action::Left,
            KeyCode::ArrowRight => Action::Right,
            KeyCode::ArrowUp => Action::Up,
            KeyCode::ArrowDown => Action::Down,
            KeyCode::KeyZ => Action::Jump,
            KeyCode::KeyX => Action::Throw,
            KeyCode::ShiftLeft => Action::PickUp,
            KeyCode::Space => Action::Map,
            _ => return None,
        })
    }
}

impl Action {
    /// The gameplay action a Rewired keyboard key drives in the default map.
    #[must_use]
    pub fn from_rewired(key: KeyboardKey) -> Option<Self> {
        Some(match key {
            KeyboardKey::LeftArrow => Action::Left,
            KeyboardKey::RightArrow => Action::Right,
            KeyboardKey::UpArrow => Action::Up,
            KeyboardKey::DownArrow => Action::Down,
            KeyboardKey::Z => Action::Jump,
            KeyboardKey::X => Action::Throw,
            KeyboardKey::LeftShift => Action::PickUp,
            KeyboardKey::Space => Action::Map,
            _ => return None,
        })
    }
}

impl Command {
    /// R respawns, V switches level graphic / debug view, G toggles the tile grid,
    /// F12 saves a screenshot, Esc quits (in the game only; there is no pause menu yet).
    #[must_use]
    pub fn from_key(key: KeyCode) -> Option<Self> {
        Some(match key {
            KeyCode::KeyR => Command::Respawn,
            KeyCode::KeyV => Command::ToggleView,
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
            map: self.is_on(Action::Map),
            pickup: self.is_on(Action::PickUp),
            throw: self.is_on(Action::Throw),
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

/// A fixed input sequence for `--screenshot` runs: `(input, ticks)` steps played in
/// order instead of the keyboard (for capturing poses without key presses).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Script {
    steps: Vec<(Input, u32)>,
    tick: u32,
}

impl Script {
    /// Parses `X,Y,J[,M]:TICKS;...` (`X`, `Y` in -1..=1, `J` and the map button `M` 0
    /// or 1), e.g. `0,1,0:1;1,0,0:30;0,0,0,1:40`.
    ///
    /// # Errors
    /// A message naming the bad step.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut steps = Vec::new();
        for step in text.split(';').filter(|s| !s.is_empty()) {
            steps.push(
                Self::step(step)
                    .ok_or_else(|| format!("bad script step {step:?} (want X,Y,J[,M]:TICKS)"))?,
            );
        }
        Ok(Self { steps, tick: 0 })
    }

    /// One `X,Y,J[,M]:TICKS` step.
    fn step(step: &str) -> Option<(Input, u32)> {
        let (keys, ticks) = step.split_once(':')?;
        let ticks = ticks.trim().parse().ok()?;
        let v: Vec<i8> = keys
            .split(',')
            .map(|k| k.trim().parse::<i8>())
            .collect::<Result<_, _>>()
            .ok()?;
        let (x, y, j, m) = match v[..] {
            [x, y, j] => (x, y, j, 0),
            [x, y, j, m] => (x, y, j, m),
            _ => return None,
        };
        let valid = (-1..=1).contains(&x)
            && (-1..=1).contains(&y)
            && (0..=1).contains(&j)
            && (0..=1).contains(&m);
        let input = Input::new(x, y, j == 1).with_map(m == 1);
        valid.then_some((input, ticks))
    }

    /// Ticks the whole script lasts.
    #[must_use]
    pub fn len(&self) -> u32 {
        self.steps.iter().map(|(_, t)| t).sum()
    }

    /// The next tick's input, `None` once the script has run out.
    pub fn next_input(&mut self) -> Option<Input> {
        let mut t = self.tick;
        for (input, ticks) in &self.steps {
            if t < *ticks {
                self.tick += 1;
                return Some(*input);
            }
            t -= ticks;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewired_default_gameplay_keys() {
        assert_eq!(Action::from_key(KeyCode::ArrowLeft), Some(Action::Left));
        assert_eq!(Action::from_key(KeyCode::ArrowRight), Some(Action::Right));
        assert_eq!(Action::from_key(KeyCode::ArrowUp), Some(Action::Up));
        assert_eq!(Action::from_key(KeyCode::ArrowDown), Some(Action::Down));
        assert_eq!(Action::from_key(KeyCode::KeyZ), Some(Action::Jump));
        assert_eq!(Action::from_key(KeyCode::Space), Some(Action::Map));
        assert_eq!(Action::from_key(KeyCode::KeyX), Some(Action::Throw));
        assert_eq!(Action::from_key(KeyCode::ShiftLeft), Some(Action::PickUp));
        // Not in the default map: WASD.
        for key in [KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD] {
            assert_eq!(Action::from_key(key), None);
        }
    }

    #[test]
    fn menu_keys() {
        assert_eq!(
            KeyboardKey::from_winit(KeyCode::Enter),
            Some(KeyboardKey::Return)
        );
        assert_eq!(
            KeyboardKey::from_winit(KeyCode::ArrowUp),
            Some(KeyboardKey::UpArrow)
        );
        // WASD are not in Rewired's default UI map.
        assert_eq!(KeyboardKey::from_winit(KeyCode::KeyW), None);
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
        assert_eq!(i, Input::new(1, -1, false));
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
