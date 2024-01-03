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
pub struct Keymap<const SIZE: usize, const LAYERS: usize, const ROLLOVER: usize> {
    modtap_config: modtap::ModTapConfig,
    layers: Vec<Layer, LAYERS>,
    keys: [Keys<LAYERS>; SIZE],
    pub pressed_keys: Vec<Keyboard, ROLLOVER>,
    pub flags: KeymapFlags,
}

impl<const SIZE: usize, const LAYERS: usize, const ROLLOVER: usize> Keymap<SIZE, LAYERS, ROLLOVER> {
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
            pressed_keys: Default::default(),
            flags: Default::default(),
        }
    }

    pub fn process(&mut self, keypresses: &[bool; SIZE], now: Instant) {
        self.pressed_keys.clear();
        for (key, pressed) in self.keys.iter_mut().zip(keypresses) {
            if key.layers[key.current as usize].is_finished() {
                key.current = self.layers.last().copied().unwrap_or(0)
            };
            match &mut key.layers[key.current as usize] {
                Key::Button(state) => {
                    state.key_transition(*pressed);
                    if let Some(key) = state.get_key() {
                        if self.pressed_keys.push(key).is_err() {
                            self.flags.rollover = true;
                        }
                    }
                }
                Key::Layer(state) => state.layer_transition(*pressed, &mut self.layers),
                Key::ModTap(state) => {
                    state.modtap_transition(*pressed, now, &self.modtap_config);
                    if let Some(key) = state.get_key() {
                        if self.pressed_keys.push(key).is_err() {
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
        let mut keymap: Keymap<3, 2, 32> = Keymap::new(
            [[Kb(A), La(1), MT(M, T)], [Kb(B), Kb(___), Kb(___)]],
            2,
            4,
            6,
        );
        assert_eq!(keymap.pressed_keys, []);

        keymap.process(&[false, false, false], 1);
        assert_eq!(keymap.pressed_keys, []);
        assert_eq!(
            keymap.keys[0],
            Keys {
                current: 0,
                layers: [Key::new(Kb(A)), Key::new(Kb(B))]
            }
        );

        keymap.process(&[true, false, false], 2);
        assert_eq!(keymap.pressed_keys, [A]);

        keymap.process(&[true, true, false], 3);
        assert_eq!(keymap.pressed_keys, [A]);

        keymap.process(&[false, true, false], 4);
        assert_eq!(keymap.pressed_keys, []);
        assert_eq!(
            keymap.keys[0],
            Keys {
                current: 0,
                layers: [Key::new(Kb(A)), Key::new(Kb(B))]
            }
        );

        keymap.process(&[true, true, false], 5);
        assert_eq!(keymap.pressed_keys, [B]);
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

        keymap.process(&[true, false, true], 6);
        assert_eq!(keymap.pressed_keys, [B]);

        keymap.process(&[true, false, true], 7);
        assert_eq!(keymap.pressed_keys, [B]);

        keymap.process(&[true, false, true], 8);
        assert_eq!(keymap.pressed_keys, [B, M]);
    }
}
