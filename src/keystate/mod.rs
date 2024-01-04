use heapless::Vec;
pub use usbd_human_interface_device::page::Keyboard;

pub mod button;
pub mod layer;
pub mod modtap;
/// Shorthand for `use keystate::Key::*` and for using Kb, La, MT to create a keymap
pub mod prelude;

/// Shared state
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct Shared;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct KeyState<State> {
    state: State,
    shared: Shared,
}

/// Something which is like a key (button, layer, mod-tap etc)
trait Keyish {
    /// Whether this can move on to the current layer, or the MCU go to sleep (if all keys are
    /// finished)
    fn is_finished(&self) -> bool;
}

type Layer = u8;
type Duration = u64;
type Instant = u64;

/// Shorthand for `use keystate::Key::*` and for using K, L, MT to create a keymap
#[derive(Debug, Clone, Copy)]
pub enum KeyShorthand {
    Kb(Keyboard),
    La(Layer),
    MT(Keyboard, Keyboard),
}

/// Actual keys containing key-state
#[derive(Debug, PartialEq, Eq)]
enum Key {
    Button(button::ButtonState),
    Layer(layer::LayerState),
    ModTap(modtap::ModTapState<Keyboard, Keyboard>),
}
impl Key {
    fn new(key: KeyShorthand) -> Self {
        match key {
            KeyShorthand::Kb(key) => Key::Button(button::ButtonState::new(key)),
            KeyShorthand::La(layer) => Key::Layer(layer::LayerState::new(layer)),
            KeyShorthand::MT(mod_, tap) => Key::ModTap(modtap::ModTapState::new(mod_, tap)),
        }
    }
}
impl Keyish for Key {
    fn is_finished(&self) -> bool {
        match self {
            Key::Button(button) => button.is_finished(),
            Key::Layer(layer) => layer.is_finished(),
            Key::ModTap(mod_tap) => mod_tap.is_finished(),
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
struct Keys<const LAYERS: usize> {
    current: Layer,
    layers: [Key; LAYERS],
}

#[derive(Debug, Default)]
pub struct KeymapFlags {
    pub rollover: bool,
}
#[derive(Debug)]
pub struct Keymap<const SIZE: usize, const LAYERS: usize> {
    modtap_config: modtap::ModTapConfig,
    layers: Vec<Layer, LAYERS>,
    keys: [Keys<LAYERS>; SIZE],
    pub flags: KeymapFlags,
}

impl<const SIZE: usize, const LAYERS: usize> Keymap<SIZE, LAYERS> {
    pub fn new(
        keymap: [[KeyShorthand; SIZE]; LAYERS],
        mod_timeout: Duration,
        tap_release: Duration,
        tap_repeat: Duration,
    ) -> Self {
        let keys: [Keys<LAYERS>; SIZE] = core::array::from_fn(|key| Keys {
            current: 0,
            layers: core::array::from_fn(|layer| Key::new(keymap[layer][key])),
        });
        Keymap {
            modtap_config: modtap::ModTapConfig {
                mod_timeout,
                tap_release,
                tap_repeat,
            },
            keys,
            layers: Default::default(),
            flags: Default::default(),
        }
    }

    pub fn process<const ROLLOVER: usize>(
        &mut self,
        keypresses: &[bool; SIZE],
        keys: &mut Vec<Keyboard, ROLLOVER>,
        now: Instant,
    ) {
        for (key, pressed) in self.keys.iter_mut().zip(keypresses) {
            if key.layers[key.current as usize].is_finished() {
                key.current = self.layers.last().copied().unwrap_or(0)
            };
            match &mut key.layers[key.current as usize] {
                Key::Button(state) => {
                    state.key_transition(*pressed);
                    if let Some(key) = state.get_key() {
                        if keys.push(key).is_err() {
                            self.flags.rollover = true;
                        }
                    }
                }
                Key::Layer(state) => state.layer_transition(*pressed, &mut self.layers),
                Key::ModTap(state) => {
                    state.modtap_transition(*pressed, now, &self.modtap_config);
                    if let Some(key) = state.get_key() {
                        if keys.push(key).is_err() {
                            self.flags.rollover = true;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::prelude::*;
    use super::Key;
    use super::Keymap;
    use super::Keys;

    #[test]
    fn simple_keyboard() {
        let mut keymap: Keymap<3, 2> = Keymap::new(
            [[Kb(A), La(1), MT(M, T)], [Kb(B), Kb(___), Kb(___)]],
            2,
            4,
            6,
        );
        let mut keys: heapless::Vec<_, 2> = Default::default();
        assert_eq!(keys, []);

        keys.clear();
        keymap.process(&[false, false, false], &mut keys, 1);
        assert_eq!(keys, []);
        assert_eq!(
            keymap.keys[0],
            Keys {
                current: 0,
                layers: [Key::new(Kb(A)), Key::new(Kb(B))]
            }
        );

        keys.clear();
        keymap.process(&[true, false, false], &mut keys, 2);
        assert_eq!(keys, [A]);

        keys.clear();
        keymap.process(&[true, true, false], &mut keys, 3);
        assert_eq!(keys, [A]);

        keys.clear();
        keymap.process(&[false, true, false], &mut keys, 4);
        assert_eq!(keys, []);
        assert_eq!(
            keymap.keys[0],
            Keys {
                current: 0,
                layers: [Key::new(Kb(A)), Key::new(Kb(B))]
            }
        );

        keys.clear();
        keymap.process(&[true, true, false], &mut keys, 5);
        assert_eq!(keys, [B]);
        assert_eq!(
            keymap.keys[0],
            Keys {
                current: 1,
                layers: [
                    Key::new(Kb(A)),
                    Key::Button(super::button::ButtonState::Pressed(super::KeyState {
                        state: super::button::Pressed { key: B },
                        shared: super::Shared
                    }))
                ]
            }
        );

        keys.clear();
        keymap.process(&[true, false, true], &mut keys, 6);
        assert_eq!(keys, [B]);

        keys.clear();
        keymap.process(&[true, false, true], &mut keys, 7);
        assert_eq!(keys, [B]);

        keys.clear();
        keymap.process(&[true, false, true], &mut keys, 8);
        assert_eq!(keys, [B, M]);
    }
}
