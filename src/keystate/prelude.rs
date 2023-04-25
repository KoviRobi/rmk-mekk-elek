//! For ease and shorthand
pub use super::KeyShorthand::*;
pub use super::Keyboard::*;

pub const NOP: super::Keyboard = NoEventIndicated;
pub const ___: super::Keyboard = NoEventIndicated;

pub const K0: super::Keyboard = Keyboard0;
pub const K1: super::Keyboard = Keyboard1;
pub const K2: super::Keyboard = Keyboard2;
pub const K3: super::Keyboard = Keyboard3;
pub const K4: super::Keyboard = Keyboard4;
pub const K5: super::Keyboard = Keyboard5;
pub const K6: super::Keyboard = Keyboard6;
pub const K7: super::Keyboard = Keyboard7;
pub const K8: super::Keyboard = Keyboard8;
pub const K9: super::Keyboard = Keyboard9;

pub const LSFT: super::Keyboard = LeftShift;
pub const LCTL: super::Keyboard = LeftControl;
pub const LALT: super::Keyboard = LeftAlt;
pub const LWIN: super::Keyboard = LeftGUI;
pub const LGUI: super::Keyboard = LeftGUI;
pub const RSFT: super::Keyboard = RightShift;
pub const RCTL: super::Keyboard = RightControl;
pub const RALT: super::Keyboard = RightAlt;
pub const RWIN: super::Keyboard = RightGUI;
pub const RGUI: super::Keyboard = RightGUI;

pub const LEFT: super::Keyboard = LeftArrow;
pub const RIGHT: super::Keyboard = RightArrow;
pub const UP: super::Keyboard = UpArrow;
pub const DOWN: super::Keyboard = DownArrow;

pub const BSL: super::Keyboard = Backslash;
