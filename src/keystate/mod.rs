pub use usbd_human_interface_device::page::Keyboard;

pub mod button;
pub mod layer;
pub mod modtap;

/// Shared state
#[derive(Debug, Clone, Copy)]
pub struct KeyState<State>(State);

type Layer = u8;
type Duration = u32;
type Instant = u32;
