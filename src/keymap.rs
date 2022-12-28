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

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn associate_all_unpressed() {
        let pressed: Vec<Vec<bool, 2>, 2> = [[false, false], [false, false]]
            .into_iter()
            .map(|row| row.into_iter().collect())
            .collect();
        let keymap = [[Keyboard::A, Keyboard::B], [Keyboard::C, Keyboard::D]];
        let expected = Vec::<Keyboard, 4>::new();
        assert_eq!(associate::<2, 2, 4>(pressed, keymap), expected);
    }

    #[test]
    fn associate_one_pressed() {
        let pressed: Vec<Vec<bool, 2>, 2> = [[false, true], [false, false]]
            .into_iter()
            .map(|row| row.into_iter().collect())
            .collect();
        let keymap = [[Keyboard::A, Keyboard::B], [Keyboard::C, Keyboard::D]];
        let mut expected = Vec::<Keyboard, 4>::new();
        expected.push(Keyboard::B).unwrap();
        assert_eq!(associate::<2, 2, 4>(pressed, keymap), expected);
    }

    #[test]
    fn associate_all_pressed() {
        let pressed: Vec<Vec<bool, 2>, 2> = [[true, true], [true, true]]
            .into_iter()
            .map(|row| row.into_iter().collect())
            .collect();
        let keymap = [[Keyboard::A, Keyboard::B], [Keyboard::C, Keyboard::D]];
        let mut expected = Vec::<Keyboard, 4>::new();
        expected.push(Keyboard::A).unwrap();
        expected.push(Keyboard::B).unwrap();
        expected.push(Keyboard::C).unwrap();
        expected.push(Keyboard::D).unwrap();
        assert_eq!(associate::<2, 2, 4>(pressed, keymap), expected);
    }
}
