//! Associates presses to keymaps

use heapless::Vec;
use no_std_compat::iter::zip;
use usbd_human_interface_device::page::Keyboard;

const INPUTS: usize = 6;
const OUTPUTS: usize = 6;
pub const KEYMAP: [[Keyboard; INPUTS]; OUTPUTS] = [
    [
        Keyboard::Keyboard0,
        Keyboard::Keyboard1,
        Keyboard::Keyboard2,
        Keyboard::Keyboard3,
        Keyboard::Keyboard4,
        Keyboard::Keyboard5,
    ],
    [
        Keyboard::Q,
        Keyboard::W,
        Keyboard::E,
        Keyboard::R,
        Keyboard::T,
        Keyboard::Y,
    ],
    [
        Keyboard::A,
        Keyboard::S,
        Keyboard::D,
        Keyboard::F,
        Keyboard::G,
        Keyboard::H,
    ],
    [
        Keyboard::Z,
        Keyboard::X,
        Keyboard::C,
        Keyboard::V,
        Keyboard::B,
        Keyboard::N,
    ],
    [
        Keyboard::Q,
        Keyboard::W,
        Keyboard::E,
        Keyboard::R,
        Keyboard::T,
        Keyboard::Y,
    ],
    [
        Keyboard::Q,
        Keyboard::W,
        Keyboard::E,
        Keyboard::R,
        Keyboard::T,
        Keyboard::Y,
    ],
];

pub fn associate<const ROWS: usize, const COLUMNS: usize, const OUTPUT: usize>(
    presses: Vec<Vec<bool, ROWS>, COLUMNS>,
    keymap: [[Keyboard; ROWS]; COLUMNS],
) -> Vec<Keyboard, OUTPUT> {
    zip(presses, keymap)
        .flat_map(|(row, rowmap)| {
            zip(row, rowmap).filter_map(|(pressed, key)| pressed.then_some(key))
        })
        .collect()
}
