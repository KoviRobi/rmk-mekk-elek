use crate::keymap::Button;
use heapless::Vec;

pub use usbd_human_interface_device::page::Keyboard;

type Instant = u32;
type Layer = u8;

#[derive(Debug, Clone, Copy)]
pub struct KeyState<State>(State);

pub struct Unpressed {}
pub struct Pressed {
    key: Keyboard,
}
pub struct ShiftedLayer {
    stack_index: usize,
}
pub struct ModTapWait {
    timeout: Instant,
}
pub struct Mod<ModState> {
    mod_state: ModState,
}
pub struct Tap<TapState> {
    again_timeout: Instant,
    tap_state: TapState,
}
pub struct TapHold<TapState> {
    tap_state: TapState,
}

impl KeyState<Unpressed> {
    fn press(&self, key: Keyboard) -> KeyState<Pressed> {
        KeyState(Pressed { key })
    }
}

impl KeyState<Pressed> {
    fn release(&self) -> KeyState<Unpressed> {
        KeyState(Unpressed {})
    }
}

pub enum ButtonState {
    Unpressed(KeyState<Unpressed>),
    Pressed(KeyState<Pressed>),
}

fn key_transition(state: ButtonState, pressed: bool, key: Keyboard) -> ButtonState {
    match (state, pressed) {
        (ButtonState::Unpressed(state), true) => ButtonState::Pressed(state.press(key)),
        (ButtonState::Pressed(state), false) => ButtonState::Unpressed(state.release()),
        (state, _) => state,
    }
}

impl KeyState<Unpressed> {
    fn shift(&self, stack_index: usize) -> KeyState<ShiftedLayer> {
        KeyState(ShiftedLayer { stack_index })
    }
}

impl KeyState<ShiftedLayer> {
    fn release(&self) -> KeyState<Unpressed> {
        KeyState(Unpressed {})
    }
}

pub enum LayerState {
    Unpressed(KeyState<Unpressed>),
    Pressed(KeyState<ShiftedLayer>),
}

fn layer_transition(state: LayerState, pressed: bool, layer: Layer) -> LayerState {
    let mut layers: Vec<Layer, 4> = Vec::new();
    match (state, pressed) {
        (LayerState::Unpressed(state), true) => {
            layers.push(layer).ok();
            LayerState::Pressed(state.shift(layers.len() - 1))
        }
        (LayerState::Pressed(state), false) => LayerState::Unpressed(state.release()),
        (state, _) => state,
    }
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
        tap_state: TapState,
        again_timeout: Instant,
    ) -> KeyState<Tap<TapState>> {
        KeyState(Tap {
            tap_state,
            again_timeout,
        })
    }
}

impl<TapState> KeyState<Tap<TapState>> {
    fn release(&self) -> KeyState<Unpressed> {
        KeyState(Unpressed {})
    }
}

impl<TapState> KeyState<Tap<TapState>> {
    fn tap_press(&self, tap_state: TapState) -> KeyState<TapHold<TapState>> {
        KeyState(TapHold { tap_state })
    }
}

impl<TapState> KeyState<TapHold<TapState>> {
    fn release(&self) -> KeyState<Tap<TapState>> {
        todo!()
    }
}

pub enum ModTapState<ModState, TapState> {
    Unpressed(KeyState<Unpressed>),
    Wait(KeyState<ModTapWait>),
    Mod(KeyState<Mod<ModState>>),
    Tap(KeyState<Tap<TapState>>),
    TapHold(KeyState<TapHold<TapState>>),
}

fn modtap_transition<Mod: Copy, Tap: Copy>(
    state: ModTapState<Mod, Tap>,
    pressed: bool,
    mod_action: Mod,
    tap_action: Tap,
    now: Instant,
    timeout: Instant,
    again_timeout: Instant,
) -> ModTapState<Mod, Tap> {
    match (state, pressed) {
        (ModTapState::Unpressed(state), true) => ModTapState::Wait(state.start(timeout)),
        (ModTapState::Wait(state), true) if state.0.timeout <= now => {
            ModTapState::Mod(state.mod_press(mod_action))
        }
        (ModTapState::Wait(state), false) if state.0.timeout > now => {
            ModTapState::Tap(state.tap_press(tap_action, again_timeout))
        }
        (ModTapState::Mod(state), false) => ModTapState::Unpressed(state.release()),
        (ModTapState::Tap(state), true) => ModTapState::TapHold(state.tap_press(state.0.tap_state)),
        (ModTapState::Tap(state), false) if state.0.again_timeout <= now => {
            ModTapState::Unpressed(state.release())
        }
        (ModTapState::TapHold(state), false) => ModTapState::Tap(state.release()),
        (state, _) => state,
    }
}
