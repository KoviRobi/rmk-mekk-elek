use super::KeyState;
use super::Keyboard;

pub struct Unpressed;
pub struct Pressed {
    key: Keyboard,
}

impl KeyState<Unpressed> {
    fn press(&self, key: Keyboard) -> KeyState<Pressed> {
        KeyState(Pressed { key })
    }
}

impl KeyState<Pressed> {
    fn release(&self) -> KeyState<Unpressed> {
        KeyState(Unpressed)
    }
}

pub enum ButtonState {
    Unpressed(KeyState<Unpressed>),
    Pressed(KeyState<Pressed>),
}

impl ButtonState {
    pub fn new() -> Self {
        Self::Unpressed(KeyState(Unpressed))
    }

    pub fn key_transition(&mut self, pressed: bool, key: Keyboard) {
        *self = match (&self, pressed) {
            (Self::Unpressed(state), true) => Self::Pressed(state.press(key)),
            (Self::Pressed(state), false) => Self::Unpressed(state.release()),
            (_state, _) => return,
        };
    }

    pub fn get_key(&self) -> Option<Keyboard> {
        match self {
            ButtonState::Unpressed(_) => None,
            ButtonState::Pressed(KeyState(Pressed { key })) => Some(*key),
        }
    }
}

impl Default for ButtonState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn get_key_unpressed() {
        let mut state = ButtonState::default();
        assert_eq!(state.get_key(), None);
        state.key_transition(false, Keyboard::A);
        assert_eq!(state.get_key(), None);
    }

    #[test]
    fn get_keys_pressed() {
        let mut state = ButtonState::default();

        assert_eq!(state.get_key(), None);
        state.key_transition(true, Keyboard::A);
        assert_eq!(state.get_key(), Some(Keyboard::A));
    }
}
