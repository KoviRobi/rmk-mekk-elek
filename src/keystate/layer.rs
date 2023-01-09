use super::KeyState;
use super::Layer;

use heapless::Vec;

pub struct Unpressed;
pub struct ShiftedLayer {
    layer: Layer,
}

impl KeyState<Unpressed> {
    fn shift(&self, layer: Layer) -> KeyState<ShiftedLayer> {
        KeyState(ShiftedLayer { layer })
    }
}

impl KeyState<ShiftedLayer> {
    fn release(&self) -> KeyState<Unpressed> {
        KeyState(Unpressed)
    }
}

pub enum LayerState {
    Unpressed(KeyState<Unpressed>),
    Pressed(KeyState<ShiftedLayer>),
}

impl LayerState {
    pub fn new() -> Self {
        Self::Unpressed(KeyState(Unpressed))
    }

    pub fn layer_transition<const N: usize>(
        &mut self,
        pressed: bool,
        layer: Layer,
        layers: &mut Vec<Layer, N>,
    ) {
        match (&self, pressed) {
            (Self::Unpressed(state), true) => {
                layers.retain(|layer2| layer2 != &layer);
                layers.push(layer).ok();
                *self = Self::Pressed(state.shift(layer));
            }
            (Self::Pressed(state @ KeyState(ShiftedLayer { layer })), false) => {
                layers.retain(|layer2| layer2 != layer);
                *self = Self::Unpressed(state.release());
            }
            (_state, _) => (),
        }
    }
}

impl Default for LayerState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn active_layer<const LAYERS: usize, Map: Copy>(
    layers: &Vec<Layer, LAYERS>,
    keymaps: [Map; LAYERS],
) -> Map {
    keymaps[*layers.first().unwrap_or(&0) as usize]
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::super::button;
    use super::super::Keyboard;
    use super::*;

    #[test]
    fn get_keys_layer() {
        let mut button_state = button::ButtonState::default();
        let mut layer_state = LayerState::default();
        let button_maps = [Keyboard::A, Keyboard::B];
        let layer = 1;
        let mut layers = Vec::<Layer, 2>::new();

        assert_eq!(button_state.get_key(), None);

        button_state.key_transition(true, active_layer(&layers, button_maps));
        assert_eq!(button_state.get_key(), Some(Keyboard::A));

        layer_state.layer_transition(true, layer, &mut layers);
        assert_eq!(button_state.get_key(), Some(Keyboard::A));

        button_state.key_transition(false, active_layer(&layers, button_maps));
        assert_eq!(button_state.get_key(), None);

        button_state.key_transition(true, active_layer(&layers, button_maps));
        assert_eq!(button_state.get_key(), Some(Keyboard::B));

        layer_state.layer_transition(false, layer, &mut layers);
        assert_eq!(button_state.get_key(), Some(Keyboard::B));

        button_state.key_transition(false, active_layer(&layers, button_maps));
        assert_eq!(button_state.get_key(), None);
    }

    // TODO: Test layer re-arrangement
    // TODO: Test popping non-topmost layer
}
