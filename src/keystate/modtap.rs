use super::Duration;
use super::Instant;
use super::KeyState;
use super::Keyboard;

pub struct ModTapConfig {
    pub mod_timeout: Duration,
    pub tap_release: Duration,
    pub tap_repeat: Duration,
}

#[derive(Debug)]
pub struct Unpressed;
#[derive(Debug)]
pub struct ModTapWait {
    timeout: Instant,
}
#[derive(Debug)]
pub struct Mod<ModState> {
    mod_state: ModState,
}
#[derive(Debug)]
pub struct Tap<TapState> {
    tap_state: Option<TapState>,
    release_timeout: Instant,
    again_timeout: Instant,
}
#[derive(Debug)]
pub struct TapHold<TapState> {
    tap_state: TapState,
}

impl KeyState<Unpressed> {
    fn start(&self, timeout: Instant) -> KeyState<ModTapWait> {
        KeyState(ModTapWait { timeout })
    }
}

impl KeyState<ModTapWait> {
    fn mod_press<ModState>(&self, mod_state: ModState) -> KeyState<Mod<ModState>> {
        KeyState(Mod { mod_state })
    }
}

impl<ModState> KeyState<Mod<ModState>> {
    fn release(&self) -> KeyState<Unpressed> {
        KeyState(Unpressed {})
    }
}

impl KeyState<ModTapWait> {
    fn tap_press<TapState>(
        &self,
        tap_state: Option<TapState>,
        release_timeout: Instant,
        again_timeout: Instant,
    ) -> KeyState<Tap<TapState>> {
        KeyState(Tap {
            tap_state,
            release_timeout,
            again_timeout,
        })
    }
}

impl<TapState> KeyState<Tap<TapState>> {
    fn release(&self) -> KeyState<Tap<TapState>> {
        KeyState(Tap {
            tap_state: None,
            ..self.0
        })
    }
}

impl<TapState> KeyState<Tap<TapState>> {
    fn tap_press(&self, tap_state: TapState) -> KeyState<TapHold<TapState>> {
        KeyState(TapHold { tap_state })
    }

    fn new_tap(&self, timeout: Instant) -> KeyState<ModTapWait> {
        KeyState(ModTapWait { timeout })
    }
}

impl<TapState> KeyState<TapHold<TapState>> {
    fn release(
        &self,
        tap_state: Option<TapState>,
        release_timeout: Instant,
        again_timeout: Instant,
    ) -> KeyState<Tap<TapState>> {
        KeyState(Tap {
            tap_state,
            release_timeout,
            again_timeout,
        })
    }
}

#[derive(Debug)]
pub enum ModTapState<ModState, TapState> {
    Unpressed(KeyState<Unpressed>),
    Wait(KeyState<ModTapWait>),
    Mod(KeyState<Mod<ModState>>),
    Tap(KeyState<Tap<TapState>>),
    TapHold(KeyState<TapHold<TapState>>),
}

impl<Mod: Copy, Tap: Copy> ModTapState<Mod, Tap> {
    pub fn new() -> Self {
        Self::Unpressed(KeyState(Unpressed))
    }

    pub fn modtap_transition(
        &mut self,
        pressed: bool,
        mod_action: Mod,
        tap_action: Tap,
        now: Instant,
        modtap_config: &ModTapConfig,
    ) {
        *self = match (&self, pressed) {
            (Self::Unpressed(state), true) =>
                Self::Wait(state.start(now + modtap_config.mod_timeout)),
            (Self::Wait(state), true) if state.0.timeout <= now =>
                Self::Mod(state.mod_press(mod_action)),
            (Self::Wait(state), false) /* if state.0.timeout > now */ =>
                Self::Tap(state.tap_press(
                    Some(tap_action),
                    now + modtap_config.tap_release,
                    now + modtap_config.tap_repeat,
                )),
            (Self::Mod(state), false) => Self::Unpressed(state.release()),
            (Self::Tap(state), true) => if state.0.again_timeout <= now {
                    Self::Wait(state.new_tap(now + modtap_config.mod_timeout))
                } else {
                    Self::TapHold(state.tap_press(tap_action))
                }
            (Self::Tap(state), false) if state.0.release_timeout <= now => {
                Self::Tap(state.release())
            }
            (Self::TapHold(state), false) => Self::Tap(state.release(
                    None,
                    now + modtap_config.tap_release,
                    now + modtap_config.tap_repeat,
            )),
            (_state, _) => return,
        }
    }
}

impl ModTapState<Keyboard, Keyboard> {
    pub fn get_key(&self) -> Option<Keyboard> {
        match self {
            // None
            Self::Unpressed(_) => None,
            Self::Wait(_) => None,
            // Some
            Self::Mod(KeyState(Mod { mod_state })) => Some(*mod_state),
            Self::Tap(KeyState(Tap { tap_state, .. })) => *tap_state,
            Self::TapHold(KeyState(TapHold { tap_state })) => Some(*tap_state),
        }
    }
}

impl<Mod: Copy, Tap: Copy> Default for ModTapState<Mod, Tap> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn get_keys_modtap_nothing() {
        let mut state = ModTapState::<Keyboard, Keyboard>::default();
        let modtap_config = ModTapConfig {
            mod_timeout: 1,
            tap_release: 2,
            tap_repeat: 3,
        };
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 0, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 1, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 2, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 3, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 4, &modtap_config);
        assert_eq!(state.get_key(), None);
    }

    #[test]
    fn get_keys_modtap_tap() {
        let mut state = ModTapState::<Keyboard, Keyboard>::default();
        let modtap_config = ModTapConfig {
            mod_timeout: 2,
            tap_release: 4, // Held for 4 ticks
            tap_repeat: 6,
        };
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 0, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 1, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 2, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 3, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 4, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 5, &modtap_config);
        assert_eq!(state.get_key(), None);
        for i in 0..modtap_config.tap_repeat {
            state.modtap_transition(false, Keyboard::M, Keyboard::T, 6 + i, &modtap_config);
            assert_eq!(state.get_key(), None);
        }
    }

    #[test]
    fn get_keys_modtap_mod() {
        let mut state = ModTapState::<Keyboard, Keyboard>::default();
        let modtap_config = ModTapConfig {
            mod_timeout: 2, // Neds to be held for at least 2 ticks
            tap_release: 4,
            tap_repeat: 6,
        };
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 0, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 1, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 2, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 3, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 4, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 5, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 6, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 7, &modtap_config);
        assert_eq!(state.get_key(), None);
        for i in 0..modtap_config.tap_repeat {
            state.modtap_transition(false, Keyboard::M, Keyboard::T, 8 + i, &modtap_config);
            assert_eq!(state.get_key(), None);
        }
    }

    #[test]
    /// Double-tap before tap_release
    fn get_keys_modtap_double_tap_quick() {
        let mut state = ModTapState::<Keyboard, Keyboard>::default();
        let modtap_config = ModTapConfig {
            mod_timeout: 2,
            tap_release: 4, // Held for 4 ticks
            tap_repeat: 6,
        };
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 0, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 1, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 2, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        for i in 0..modtap_config.tap_repeat {
            state.modtap_transition(true, Keyboard::M, Keyboard::T, 3 + i, &modtap_config);
            assert_eq!(state.get_key(), Some(Keyboard::T));
        }
        for i in modtap_config.tap_repeat..2 * modtap_config.tap_repeat {
            state.modtap_transition(false, Keyboard::M, Keyboard::T, 3 + i, &modtap_config);
            assert_eq!(state.get_key(), None);
        }
    }

    #[test]
    /// Double-tap after tap_release before tap_repeat ends
    fn get_keys_modtap_double_tap_slow() {
        let mut state = ModTapState::<Keyboard, Keyboard>::default();
        let modtap_config = ModTapConfig {
            mod_timeout: 2,
            tap_release: 4, // Held for 4 ticks
            tap_repeat: 6,
        };
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 0, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 1, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 2, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 3, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 4, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 5, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 6, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        for i in 0..modtap_config.tap_repeat {
            state.modtap_transition(true, Keyboard::M, Keyboard::T, 7 + i, &modtap_config);
            assert_eq!(state.get_key(), Some(Keyboard::T));
        }
        for i in modtap_config.tap_repeat..2 * modtap_config.tap_repeat {
            state.modtap_transition(false, Keyboard::M, Keyboard::T, 7 + i, &modtap_config);
            assert_eq!(state.get_key(), None);
        }
    }

    #[test]
    fn get_keys_modtap_double_mod() {
        let mut state = ModTapState::<Keyboard, Keyboard>::default();
        let modtap_config = ModTapConfig {
            mod_timeout: 2, // Neds to be held for at least 2 ticks
            tap_release: 4,
            tap_repeat: 6,
        };
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 0, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 1, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 2, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 3, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 4, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 5, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 6, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(false, Keyboard::M, Keyboard::T, 7, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 8, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 9, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 10, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 11, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 12, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 13, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, Keyboard::M, Keyboard::T, 14, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        for i in 0..modtap_config.tap_repeat {
            state.modtap_transition(false, Keyboard::M, Keyboard::T, 15 + i, &modtap_config);
            assert_eq!(state.get_key(), None);
        }
    }
}
