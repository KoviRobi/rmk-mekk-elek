use super::KeyState;
use super::Keyish;
use super::Layer;
use super::Shared;

use heapless::Vec;

#[derive(Debug, PartialEq, Eq)]
pub struct Unpressed {
    layer: Layer,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ShiftedLayer {
    layer: Layer,
}

impl KeyState<Unpressed> {
    fn shift(&self) -> KeyState<ShiftedLayer> {
        KeyState {
            state: ShiftedLayer {
                layer: self.state.layer,
            },
            shared: self.shared,
        }
    }
}

impl KeyState<ShiftedLayer> {
    fn release(&self) -> KeyState<Unpressed> {
        KeyState {
            state: Unpressed {
                layer: self.state.layer,
            },
            shared: self.shared,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum LayerState {
    Unpressed(KeyState<Unpressed>),
    Pressed(KeyState<ShiftedLayer>),
}

impl Keyish for LayerState {
    fn is_finished(&self) -> bool {
        matches!(self, LayerState::Unpressed(_))
    }
}

impl LayerState {
    pub fn new(layer: Layer) -> Self {
        Self::Unpressed(KeyState {
            state: Unpressed { layer },
            shared: Shared,
        })
    }

    pub fn layer_transition<const N: usize>(&mut self, pressed: bool, layers: &mut Vec<Layer, N>) {
        match &self {
            Self::Unpressed(state) if pressed => {
                layers.retain(|layer2| *layer2 != state.state.layer);
                layers.push(state.state.layer).ok();
                *self = Self::Pressed(state.shift());
            }
            Self::Pressed(state) if !pressed => {
                layers.retain(|layer2| *layer2 != state.state.layer);
                *self = Self::Unpressed(state.release());
            }
            _state => (),
        }
    }
}

pub fn active_layer<const LAYERS: usize, Map: Copy>(
    layers: &Vec<Layer, LAYERS>,
    keymaps: [Map; LAYERS],
) -> Map {
    keymaps[layers.last().copied().unwrap_or(0) as usize]
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn get_keys_layer() {
        let mut layer1 = LayerState::new(1);
        let mut layers = Vec::<Layer, 2>::new();

        assert_eq!(layers, []);
        assert!(layer1.is_finished());
        layer1.layer_transition(false, &mut layers);
        assert_eq!(layers, []);
        assert!(layer1.is_finished());
        layer1.layer_transition(true, &mut layers);
        assert_eq!(layers, [1]);
        assert!(!layer1.is_finished());
        layer1.layer_transition(false, &mut layers);
        assert_eq!(layers, []);
        assert!(layer1.is_finished());
    }

    #[test]
    fn get_keys_pop_bottom() {
        let mut layer1 = LayerState::new(1);
        let mut layer2 = LayerState::new(2);
        let mut layers = Vec::<Layer, 2>::new();

        assert_eq!(layers, []);
        layer1.layer_transition(true, &mut layers);
        assert_eq!(layers, [1]);
        layer2.layer_transition(true, &mut layers);
        assert_eq!(layers, [1, 2]);
        layer1.layer_transition(false, &mut layers);
        assert_eq!(layers, [2]);
        layer2.layer_transition(false, &mut layers);
        assert_eq!(layers, []);
    }

    #[test]
    fn get_keys_rearrange() {
        let mut layer1 = LayerState::new(1);
        let mut layer2 = LayerState::new(2);
        let mut layer1_ = LayerState::new(1);
        let mut layers = Vec::<Layer, 2>::new();

        assert_eq!(layers, []);
        layer1.layer_transition(true, &mut layers);
        assert_eq!(layers, [1]);
        layer2.layer_transition(true, &mut layers);
        assert_eq!(layers, [1, 2]);
        layer1_.layer_transition(true, &mut layers);
        assert_eq!(layers, [2, 1]);
        layer1.layer_transition(false, &mut layers);
        assert_eq!(layers, [2]);
        layer2.layer_transition(false, &mut layers);
        assert_eq!(layers, []);
        layer1_.layer_transition(false, &mut layers);
        assert_eq!(layers, []);
    }
}
