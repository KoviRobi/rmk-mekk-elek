use super::KeyState;
use super::Keyboard;
use super::Keyish;
use super::Shared;

#[derive(Debug, PartialEq, Eq)]
pub struct Unpressed {
    pub(super) key: Keyboard,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Pressed {
    pub(super) key: Keyboard,
}

impl KeyState<Unpressed> {
    fn press(&self) -> KeyState<Pressed> {
        KeyState {
            state: Pressed {
                key: self.state.key,
            },
            shared: self.shared,
        }
    }
}

impl KeyState<Pressed> {
    fn release(&self) -> KeyState<Unpressed> {
        KeyState {
            state: Unpressed {
                key: self.state.key,
            },
            shared: self.shared,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ButtonState {
    Unpressed(KeyState<Unpressed>),
    Pressed(KeyState<Pressed>),
}

impl Keyish for ButtonState {
    fn is_finished(&self) -> bool {
        matches!(self, ButtonState::Unpressed(_))
    }
}

impl ButtonState {
    pub fn new(key: Keyboard) -> Self {
        Self::Unpressed(KeyState {
            state: Unpressed { key },
            shared: Shared,
        })
    }

    pub fn key_transition(&mut self, pressed: bool) {
        match &self {
            Self::Unpressed(state) if pressed => *self = Self::Pressed(state.press()),
            Self::Pressed(state) if !pressed => *self = Self::Unpressed(state.release()),
            _ => (),
        };
    }

    pub fn get_key(&self) -> Option<Keyboard> {
        match self {
            ButtonState::Unpressed(_) => None,
            ButtonState::Pressed(KeyState { state, .. }) => Some(state.key),
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn get_key_unpressed() {
        let mut state = ButtonState::new(Keyboard::A);
        assert_eq!(state.get_key(), None);
        assert!(state.is_finished());
        state.key_transition(false);
        assert_eq!(state.get_key(), None);
        assert!(state.is_finished());
    }

    #[test]
    fn get_keys_pressed() {
        let mut state = ButtonState::new(Keyboard::A);

        assert_eq!(state.get_key(), None);
        assert!(state.is_finished());
        state.key_transition(true);
        assert_eq!(state.get_key(), Some(Keyboard::A));
        assert!(!state.is_finished());
        state.key_transition(false);
        assert_eq!(state.get_key(), None);
        assert!(state.is_finished());
    }
}
