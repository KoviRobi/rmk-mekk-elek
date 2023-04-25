use super::Duration;
use super::Instant;
use super::KeyState;
use super::Keyboard;
use super::Keyish;
use super::Shared;

#[derive(Debug, PartialEq, Eq)]
pub struct ModTapConfig {
    /// Time before a held key becomes a `mod` instead of a `tap`
    pub mod_timeout: Duration,
    /// Time during which the `tap` is transmitted after the key is released (it has to be after
    /// the key is released as only then do we know that it isn't a mod)
    pub tap_release: Duration,
    /// Time during which another press counts as a multi-tap (i.e. goes into the tap-state without
    /// waiting for the `mod_timeout`
    pub tap_repeat: Duration,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Unpressed<ModState, TapState> {
    mod_state: ModState,
    tap_state: TapState,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ModTapWait<ModState, TapState> {
    mod_state: ModState,
    tap_state: TapState,
    tap_timeout: Instant,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Mod<ModState, TapState> {
    mod_state: ModState,
    tap_state: TapState,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Tap<ModState, TapState> {
    mod_state: ModState,
    tap_state: TapState,
    release_timeout: Instant,
    again_timeout: Instant,
}
#[derive(Debug, PartialEq, Eq)]
pub struct DoubleTapWait<ModState, TapState> {
    mod_state: ModState,
    tap_state: TapState,
    again_timeout: Instant,
}
#[derive(Debug, PartialEq, Eq)]
pub struct DoubleTap<ModState, TapState> {
    mod_state: ModState,
    tap_state: TapState,
}

impl<ModState: Copy, TapState: Copy> KeyState<Unpressed<ModState, TapState>> {
    fn start(&self, tap_timeout: Instant) -> KeyState<ModTapWait<ModState, TapState>> {
        KeyState {
            state: ModTapWait {
                mod_state: self.state.mod_state,
                tap_state: self.state.tap_state,
                tap_timeout,
            },
            shared: self.shared,
        }
    }
}

impl<ModState: Copy, TapState: Copy> KeyState<ModTapWait<ModState, TapState>> {
    fn mod_press(&self) -> KeyState<Mod<ModState, TapState>> {
        KeyState {
            state: Mod {
                mod_state: self.state.mod_state,
                tap_state: self.state.tap_state,
            },
            shared: self.shared,
        }
    }
}

impl<ModState: Copy, TapState: Copy> KeyState<Mod<ModState, TapState>> {
    fn release(&self) -> KeyState<Unpressed<ModState, TapState>> {
        KeyState {
            state: Unpressed {
                mod_state: self.state.mod_state,
                tap_state: self.state.tap_state,
            },
            shared: self.shared,
        }
    }
}

impl<ModState: Copy, TapState: Copy> KeyState<ModTapWait<ModState, TapState>> {
    fn tap_press(
        &self,
        release_timeout: Instant,
        again_timeout: Instant,
    ) -> KeyState<Tap<ModState, TapState>> {
        KeyState {
            state: Tap {
                mod_state: self.state.mod_state,
                tap_state: self.state.tap_state,
                release_timeout,
                again_timeout,
            },
            shared: self.shared,
        }
    }
}

impl<ModState: Copy, TapState: Copy> KeyState<Tap<ModState, TapState>> {
    fn release(&self) -> KeyState<Unpressed<ModState, TapState>> {
        KeyState {
            state: Unpressed {
                mod_state: self.state.mod_state,
                tap_state: self.state.tap_state,
            },
            shared: self.shared,
        }
    }
}

impl<ModState: Copy, TapState: Copy> KeyState<DoubleTapWait<ModState, TapState>> {
    fn tap_press(&self) -> KeyState<DoubleTap<ModState, TapState>> {
        KeyState {
            state: DoubleTap {
                mod_state: self.state.mod_state,
                tap_state: self.state.tap_state,
            },
            shared: self.shared,
        }
    }
    fn timeout(&self) -> KeyState<Unpressed<ModState, TapState>> {
        KeyState {
            state: Unpressed {
                mod_state: self.state.mod_state,
                tap_state: self.state.tap_state,
            },
            shared: self.shared,
        }
    }
}

impl<ModState: Copy, TapState: Copy> KeyState<Tap<ModState, TapState>> {
    fn new_tap(&self, tap_timeout: Instant) -> KeyState<ModTapWait<ModState, TapState>> {
        KeyState {
            state: ModTapWait {
                mod_state: self.state.mod_state,
                tap_state: self.state.tap_state,
                tap_timeout,
            },
            shared: self.shared,
        }
    }
    fn double_tap(&self) -> KeyState<DoubleTapWait<ModState, TapState>> {
        KeyState {
            state: DoubleTapWait {
                mod_state: self.state.mod_state,
                tap_state: self.state.tap_state,
                again_timeout: self.state.again_timeout,
            },
            shared: self.shared,
        }
    }
}

impl<ModState: Copy, TapState: Copy> KeyState<DoubleTap<ModState, TapState>> {
    fn release(&self, again_timeout: Instant) -> KeyState<DoubleTapWait<ModState, TapState>> {
        KeyState {
            state: DoubleTapWait {
                mod_state: self.state.mod_state,
                tap_state: self.state.tap_state,
                again_timeout,
            },
            shared: self.shared,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ModTapState<ModState, TapState> {
    Unpressed(KeyState<Unpressed<ModState, TapState>>),
    Wait(KeyState<ModTapWait<ModState, TapState>>),
    Mod(KeyState<Mod<ModState, TapState>>),
    Tap(KeyState<Tap<ModState, TapState>>),
    DoubleTapWait(KeyState<DoubleTapWait<ModState, TapState>>),
    DoubleTap(KeyState<DoubleTap<ModState, TapState>>),
}

impl<ModState, TapState> Keyish for ModTapState<ModState, TapState> {
    fn is_finished(&self) -> bool {
        matches!(self, ModTapState::Unpressed(_))
    }
}

impl<ModState: Copy, TapState: Copy> ModTapState<ModState, TapState> {
    pub fn new(mod_state: ModState, tap_state: TapState) -> Self {
        Self::Unpressed(KeyState {
            state: Unpressed {
                mod_state,
                tap_state,
            },
            shared: Shared,
        })
    }

    pub fn modtap_transition(&mut self, pressed: bool, now: Instant, modtap_config: &ModTapConfig) {
        match &self {
            Self::Unpressed(state) if pressed => {
                *self = Self::Wait(state.start(now + modtap_config.mod_timeout))
            }
            Self::Unpressed(_state) => (),

            Self::Wait(state) if pressed && state.state.tap_timeout <= now => {
                *self = Self::Mod(state.mod_press())
            }
            Self::Wait(state) if !pressed => {
                *self = Self::Tap(state.tap_press(
                    now + modtap_config.tap_release,
                    now + modtap_config.tap_repeat,
                ))
            }
            Self::Wait(_state) => (),

            Self::Mod(state) if !pressed => *self = Self::Unpressed(state.release()),
            Self::Mod(_state) => (),

            Self::Tap(state)
                if !pressed
                    && state.state.release_timeout <= now
                    && state.state.again_timeout <= now =>
            {
                *self = Self::Unpressed(state.release())
            }
            Self::Tap(state) if !pressed && state.state.release_timeout <= now => {
                *self = Self::DoubleTapWait(state.double_tap())
            }
            Self::Tap(state) if pressed && state.state.again_timeout <= now => {
                *self = Self::Wait(state.new_tap(now + modtap_config.mod_timeout))
            }
            Self::Tap(state) if pressed => *self = Self::DoubleTapWait(state.double_tap()),
            Self::Tap(_state) => (),

            Self::DoubleTapWait(state) if pressed && state.state.again_timeout > now => {
                *self = Self::DoubleTap(state.tap_press())
            }
            Self::DoubleTapWait(state) if !pressed && state.state.again_timeout <= now => {
                *self = Self::Unpressed(state.timeout())
            }
            Self::DoubleTapWait(_state) => (),

            Self::DoubleTap(state) if !pressed => {
                *self = Self::DoubleTapWait(state.release(now + modtap_config.tap_repeat))
            }
            Self::DoubleTap(_state) => (),
        }
    }
}

impl ModTapState<Keyboard, Keyboard> {
    pub fn get_key(&self) -> Option<Keyboard> {
        match self {
            // None
            Self::Unpressed(_) => None,
            Self::Wait(_) => None,
            Self::DoubleTapWait(_) => None,
            // Some
            Self::Mod(state) => Some(state.state.mod_state),
            Self::Tap(state) => Some(state.state.tap_state),
            Self::DoubleTap(state) => Some(state.state.tap_state),
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn get_keys_modtap_nothing() {
        let mut state = ModTapState::<Keyboard, Keyboard>::new(Keyboard::M, Keyboard::T);
        let modtap_config = ModTapConfig {
            mod_timeout: 1,
            tap_release: 2,
            tap_repeat: 3,
        };
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, 0, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, 1, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, 2, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, 3, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, 4, &modtap_config);
        assert_eq!(state.get_key(), None);
    }

    #[test]
    fn get_keys_modtap_tap() {
        let mut state = ModTapState::<Keyboard, Keyboard>::new(Keyboard::M, Keyboard::T);
        let modtap_config = ModTapConfig {
            mod_timeout: 2,
            tap_release: 4, // Held for 4 ticks
            tap_repeat: 6,
        };
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, 0, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, 1, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, 2, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, 3, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, 4, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, 5, &modtap_config);
        assert_eq!(state.get_key(), None);
        for i in 0..modtap_config.tap_repeat {
            state.modtap_transition(false, 6 + i, &modtap_config);
            assert_eq!(state.get_key(), None);
        }
    }

    #[test]
    fn get_keys_modtap_mod() {
        let mut state = ModTapState::<Keyboard, Keyboard>::new(Keyboard::M, Keyboard::T);
        let modtap_config = ModTapConfig {
            mod_timeout: 2, // Needs to be held for at least 2 ticks
            tap_release: 4,
            tap_repeat: 6,
        };
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, 0, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, 1, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, 2, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, 3, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, 4, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, 5, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, 6, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(false, 7, &modtap_config);
        assert_eq!(state.get_key(), None);
        for i in 0..modtap_config.tap_repeat {
            state.modtap_transition(false, 8 + i, &modtap_config);
            assert_eq!(state.get_key(), None);
        }
    }

    #[test]
    /// Double-tap before tap_release
    fn get_keys_modtap_double_tap_quick() {
        let mut state = ModTapState::<Keyboard, Keyboard>::new(Keyboard::M, Keyboard::T);
        let modtap_config = ModTapConfig {
            mod_timeout: 2,
            tap_release: 4, // Held for 4 ticks
            tap_repeat: 6,
        };
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, 0, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, 1, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(true, 2, &modtap_config);
        // Simulate release of double-tap
        assert_eq!(state.get_key(), None);
        for i in 0..modtap_config.tap_repeat {
            state.modtap_transition(true, 3 + i, &modtap_config);
            assert_eq!(state.get_key(), Some(Keyboard::T));
        }
        for i in modtap_config.tap_repeat..2 * modtap_config.tap_repeat {
            state.modtap_transition(false, 3 + i, &modtap_config);
            assert_eq!(state.get_key(), None);
        }
    }

    #[test]
    /// Double-tap after tap_release before tap_repeat ends
    fn get_keys_modtap_double_tap_slow() {
        let mut state = ModTapState::<Keyboard, Keyboard>::new(Keyboard::M, Keyboard::T);
        let modtap_config = ModTapConfig {
            mod_timeout: 2,
            tap_release: 4, // Held for 4 ticks
            tap_repeat: 6,
        };
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, 0, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(false, 1, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, 2, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, 3, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, 4, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        state.modtap_transition(false, 5, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, 6, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::T));
        for i in 0..modtap_config.tap_repeat {
            state.modtap_transition(true, 7 + i, &modtap_config);
            assert_eq!(state.get_key(), Some(Keyboard::T));
        }
        for i in modtap_config.tap_repeat..2 * modtap_config.tap_repeat {
            state.modtap_transition(false, 7 + i, &modtap_config);
            assert_eq!(state.get_key(), None);
        }
    }

    #[test]
    fn get_keys_modtap_double_mod() {
        let mut state = ModTapState::<Keyboard, Keyboard>::new(Keyboard::M, Keyboard::T);
        let modtap_config = ModTapConfig {
            mod_timeout: 2, // Needs to be held for at least 2 ticks
            tap_release: 4,
            tap_repeat: 6,
        };
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, 0, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, 1, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, 2, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, 3, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, 4, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, 5, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, 6, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(false, 7, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, 8, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, 9, &modtap_config);
        assert_eq!(state.get_key(), None);
        state.modtap_transition(true, 10, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, 11, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, 12, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, 13, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        state.modtap_transition(true, 14, &modtap_config);
        assert_eq!(state.get_key(), Some(Keyboard::M));
        for i in 0..modtap_config.tap_repeat {
            state.modtap_transition(false, 15 + i, &modtap_config);
            assert_eq!(state.get_key(), None);
        }
    }
}
