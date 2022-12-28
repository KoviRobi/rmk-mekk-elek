//! Associates presses to keymaps

use heapless::Vec;
use no_std_compat::iter::zip;
use usbd_human_interface_device::page::Keyboard;

use paste::paste;

macro_rules! make_keymap {
    // Shorthands
    (Esc) => {Keyboard::Escape};
    (Eql) => {Keyboard::Equal};
    (Bsl) => {Keyboard::Backslash};
    (Bsp) => {Keyboard::DeleteBackspace};
    (Ent) => {Keyboard::ReturnEnter};
    (Spc) => {Keyboard::Space};
    (Min) => {Keyboard::Minus};
    (LBr) => {Keyboard::LeftBrace};
    (RBr) => {Keyboard::RightBrace};
    (NUB) => {Keyboard::NonUSBackslash};
    (NUH) => {Keyboard::NonUSHash};

    (Scol) => {Keyboard::Semicolon};
    (Slash) => {Keyboard::ForwardSlash};
    (Caps) => {Keyboard::CapsLock};

    (LSf) => {Keyboard::LeftShift};
    (LCl) => {Keyboard::LeftControl};
    (LAl) => {Keyboard::LeftAlt};
    (LWn) => {Keyboard::LeftGUI};
    (RSf) => {Keyboard::RightShift};
    (RCl) => {Keyboard::RightControl};
    (RAl) => {Keyboard::RightAlt};
    (RWn) => {Keyboard::RightGUI};

    // Do nothing
    (NOP) => {Keyboard::NoEventIndicated};

    (Left) => {Keyboard::LeftArrow};
    (Down) => {Keyboard::DownArrow};
    (Up) => {Keyboard::UpArrow};
    (Right) => {Keyboard::RightArrow};

    (Ins) => {Keyboard::Insert};
    (Del) => {Keyboard::Delete};
    (PgUp) => {Keyboard::PageUp};
    (PgDn) => {Keyboard::PageDown};

    (KP0) => {Keyboard::Keypad0};
    (KP1) => {Keyboard::Keypad1};
    (KP2) => {Keyboard::Keypad2};
    (KP3) => {Keyboard::Keypad3};
    (KP4) => {Keyboard::Keypad4};
    (KP5) => {Keyboard::Keypad5};
    (KP6) => {Keyboard::Keypad6};
    (KP7) => {Keyboard::Keypad7};
    (KP8) => {Keyboard::Keypad8};
    (KP9) => {Keyboard::Keypad9};
    (KPDot) => {Keyboard::KeypadDot};
    (KPEnt) => {Keyboard::KeypadEnter};
    (KPEql) => {Keyboard::KeypadEqual};

    // Have numbers translate to number keys
    ($n:literal) => {paste! { Keyboard::[<Keyboard $n>] }};

    // Fallback
    ($i:ident) => {Keyboard::$i};

    ([ $( $t:tt ),* $(,)? ]) => {[ $( make_keymap!($t) ),* ]};

    // To allow `make_keymap![...]` be the same as `make_keymap!([...])`
    ( $( $t:tt ),* $(,)? ) => {[ $( make_keymap!($t) ),* ]};
}

const INPUTS: usize = 6;
const OUTPUTS: usize = 6;

// For alignment with `vi[:EasyAlign <C-r>4<CR>*,
#[rustfmt::skip]
pub const KEYMAP: [[Keyboard; INPUTS]; OUTPUTS] = make_keymap![
    [Eql,    0,       1,       2,      3,        4],
    [Bsl,    Q,       W,       E,      R,        T],
    [Esc,    A,       S,       D,      F,        G],
    [LSf,    Z,       X,       C,      V,        B],
    [LWn,    Left,    Down,    Up,     Right,    Ins],
    [NOP,    NOP,     NOP,     NOP,    NOP,      NOP],
];

pub fn associate<const ROWS: usize, const COLUMNS: usize, const OUTPUT: usize>(
    presses: Vec<Vec<bool, ROWS>, COLUMNS>,
    keymap: [[Keyboard; ROWS]; COLUMNS],
) -> Vec<Keyboard, OUTPUT> {
    zip(presses, keymap)
        .flat_map(|(row, rowmap)| {
            zip(row, rowmap).filter_map(|(pressed, key)| {
                ((key != Keyboard::NoEventIndicated) && pressed).then_some(key)
            })
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
        let keymap = make_keymap![[A, B], [C, D]];
        let expected = Vec::<Keyboard, 4>::new();
        assert_eq!(associate::<2, 2, 4>(pressed, keymap), expected);
    }

    #[test]
    fn associate_one_pressed() {
        let pressed: Vec<Vec<bool, 2>, 2> = [[false, true], [false, false]]
            .into_iter()
            .map(|row| row.into_iter().collect())
            .collect();
        let keymap = make_keymap![[A, B], [C, D]];
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
        let keymap = make_keymap![[A, B], [C, D]];
        let mut expected = Vec::<Keyboard, 4>::new();
        expected.push(Keyboard::A).unwrap();
        expected.push(Keyboard::B).unwrap();
        expected.push(Keyboard::C).unwrap();
        expected.push(Keyboard::D).unwrap();
        assert_eq!(associate::<2, 2, 4>(pressed, keymap), expected);
    }
}
