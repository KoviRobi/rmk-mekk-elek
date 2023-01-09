//! Associates presses to keymaps

use core::ops::{Add, Sub};
use heapless::Vec;

// Export as pub so that it can be used from macro
#[doc(hidden)]
pub use paste::paste;
#[doc(hidden)]
pub use usbd_human_interface_device::page::Keyboard;

#[derive(Debug, Clone, Copy)]
pub enum ModTapMode {
    Default,
    Permissive,
    HoldOnOtherPress,
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    ModTap {
        hold: Button,
        tap: Button,
        mode: ModTapMode,
    },
    Button(Button),
}

impl Default for Action {
    fn default() -> Self {
        Self::Button(Button::default())
    }
}

type Layer = u8;

#[derive(Debug, Clone, Copy)]
pub enum Button {
    Keyboard(Keyboard),
    Layer(Layer),
}

impl Default for Button {
    fn default() -> Self {
        Self::Keyboard(Keyboard::default())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct State<
    Instant: Ord
        + Copy
        + Add<Duration, Output = Instant>
        + Sub<Duration, Output = Instant>
        + Sub<Instant, Output = Duration>,
    Duration: Copy,
> {
    last_pressed: Option<Instant>,
    active: Option<Button>,
}

impl<
        Instant: Ord
            + Copy
            + Add<Duration, Output = Instant>
            + Sub<Duration, Output = Instant>
            + Sub<Instant, Output = Duration>,
        Duration: Copy,
    > Default for State<Instant, Duration>
{
    fn default() -> Self {
        Self {
            active: None,
            last_pressed: None,
        }
    }
}

impl<
        Instant: Ord
            + Copy
            + Add<Duration, Output = Instant>
            + Sub<Duration, Output = Instant>
            + Sub<Instant, Output = Duration>,
        Duration: Copy,
    > State<Instant, Duration>
{
    fn process_action<const MAX_ACTIVE_LAYERS: usize, const OUTPUT: usize>(
        &mut self,
        output: &mut Vec<Keyboard, OUTPUT>,
        layers: &mut Vec<Layer, MAX_ACTIVE_LAYERS>,
        pressed: bool,
        map: &Action,
        now: Instant,
        tap_duration: Duration,
    ) {
        match map {
            Action::Button(button) => self.process_button(output, layers, pressed, button, now),

            Action::ModTap { hold, tap, mode: _ } => {
                if pressed {
                    self.last_pressed.get_or_insert(now);

                    if self.last_pressed.unwrap() + tap_duration < now {
                        self.process_button(output, layers, pressed, hold, now);
                    }
                } else if self
                    .last_pressed
                    .map_or(false, |last| last + tap_duration > now)
                {
                    self.process_button(output, layers, !pressed, tap, now);
                    self.last_pressed.take();
                }
            }
        }
    }

    fn process_button<const MAX_ACTIVE_LAYERS: usize, const OUTPUT: usize>(
        &mut self,
        output: &mut Vec<Keyboard, OUTPUT>,
        layers: &mut Vec<Layer, MAX_ACTIVE_LAYERS>,
        pressed: bool,
        button: &Button,
        now: Instant,
    ) {
        match (pressed, &self.active, button) {
            (true, _, Button::Keyboard(Keyboard::NoEventIndicated)) => (),

            (true, None, Button::Keyboard(key)) => {
                if let None = self.active {
                    output.push(*key).unwrap();
                    self.active.get_or_insert(Button::Keyboard(*key));
                    self.last_pressed.get_or_insert(now);
                }
            }

            (true, None, Button::Layer(new_layer)) => {
                if let None = self.active {
                    layers.push(*new_layer).unwrap();
                    self.active
                        .get_or_insert(Button::Layer((layers.len() - 1) as Layer));
                    self.last_pressed.get_or_insert(now);
                }
            }

            (true, Some(Button::Keyboard(key)), _) => {
                output.push(*key).unwrap();
            }

            (true, Some(Button::Layer(_layer)), _) => {}

            (false, Some(Button::Keyboard(_key)), _) => {
                self.active.take();
            }

            (false, Some(Button::Layer(layer)), _) => {
                layers.remove(*layer as usize);
                self.active.take();
            }

            _ => (),
        }
    }
}

pub struct Keymap<
    Instant: Ord
        + Copy
        + Add<Duration, Output = Instant>
        + Sub<Duration, Output = Instant>
        + Sub<Instant, Output = Duration>,
    Duration: Copy,
    const ROWS: usize,
    const COLS: usize,
    const LAYERS: usize,
    const MAX_ACTIVE_LAYERS: usize,
> {
    pub tap_duration: Duration,
    pub layers: Vec<Layer, MAX_ACTIVE_LAYERS>,
    pub state: [[State<Instant, Duration>; ROWS]; COLS],
    pub map: [[[Action; ROWS]; COLS]; LAYERS],
}

impl<
        Instant: Ord
            + Copy
            + Add<Duration, Output = Instant>
            + Sub<Duration, Output = Instant>
            + Sub<Instant, Output = Duration>,
        Duration: Copy,
        const ROWS: usize,
        const COLS: usize,
        const LAYERS: usize,
        const MAX_ACTIVE_LAYERS: usize,
    > Keymap<Instant, Duration, ROWS, COLS, LAYERS, MAX_ACTIVE_LAYERS>
{
    pub fn get_keys<const OUTPUT: usize>(
        &mut self,
        presses: Vec<Vec<bool, ROWS>, COLS>,
        now: Instant,
    ) -> Vec<Keyboard, OUTPUT> {
        let mut output = Vec::new();
        for col in 0..COLS {
            for row in 0..ROWS {
                let active_layer = *self.layers.last().unwrap_or(&0);
                self.state[col][row].process_action(
                    &mut output,
                    &mut self.layers,
                    presses[col][row],
                    &self.map[active_layer as usize][col][row],
                    now,
                    self.tap_duration,
                );
            }
        }
        output
    }
}

#[macro_export]
macro_rules! make_keymap {
    // To allow `make_keymap![...]` be the same as `make_keymap!([...])`
    ( $( $t:tt ),* $(,)? ) => {[ $( $crate::keymap::make_action!($t) ),* ]};
}

#[rustfmt::skip]
#[macro_export]
macro_rules! make_button {
    // Shorthands
    (Esc) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Escape) };
    (Eql) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Equal) };
    (Bsl) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Backslash) };
    (Bsp) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::DeleteBackspace) };
    (Ent) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::ReturnEnter) };
    (Spc) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Space) };
    (Min) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Minus) };
    (LBr) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::LeftBrace) };
    (RBr) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::RightBrace) };
    (NUB) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::NonUSBackslash) };
    (NUH) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::NonUSHash) };

    (Scol) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Semicolon) };
    (Slash) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::ForwardSlash) };
    (Caps) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::CapsLock) };

    (LSf) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::LeftShift) };
    (LCl) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::LeftControl) };
    (LAl) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::LeftAlt) };
    (LWn) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::LeftGUI) };
    (RSf) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::RightShift) };
    (RCl) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::RightControl) };
    (RAl) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::RightAlt) };
    (RWn) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::RightGUI) };

    // Do nothing
    (NOP) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::NoEventIndicated) };

    (Left) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::LeftArrow) };
    (Down) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::DownArrow) };
    (Up) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::UpArrow) };
    (Right) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::RightArrow) };

    (Ins) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Insert) };
    (Del) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Delete) };
    (PgUp) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::PageUp) };
    (PgDn) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::PageDown) };

    (KP0) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Keypad0) };
    (KP1) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Keypad1) };
    (KP2) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Keypad2) };
    (KP3) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Keypad3) };
    (KP4) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Keypad4) };
    (KP5) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Keypad5) };
    (KP6) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Keypad6) };
    (KP7) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Keypad7) };
    (KP8) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Keypad8) };
    (KP9) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::Keypad9) };
    (KPDot) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::KeypadDot) };
    (KPEnt) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::KeypadEnter) };
    (KPEql) => { $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::KeypadEqual) };

    // Have numbers translate to number keys
    ($n:literal) => {
        $crate::keymap::Button::Keyboard(
            $crate::keymap::paste! { $crate::keymap::Keyboard::[<Keyboard $n>] },
        )
    };

    // Fallback
    ($i:ident) => {
        $crate::keymap::Button::Keyboard($crate::keymap::Keyboard::$i)
    };

    ((L $layer:literal)) => { $crate::keymap::Button::Layer($layer) };
}
#[macro_export]
macro_rules! make_action {
    ((MT $hold:tt $tap:tt)) => {
        Action::ModTap{
            hold: $crate::keymap::make_button!($hold),
            tap: $crate::keymap::make_button!($tap),
            mode: $crate::keymap::ModTapMode::Default
        }
    };
    ([ $( $t:tt ),* $(,)? ]) => {[ $( $crate::keymap::make_action!($t) ),* ]};
    ($tt:tt) => { Action::Button( $crate::keymap::make_button!($tt) ) };
}

// Export the macros
pub use make_action;
pub use make_button;
pub use make_keymap;

const INPUTS: usize = 6;
const OUTPUTS: usize = 6;

// For alignment with `vi]:EasyAlign <C-r>4<CR>*,
#[rustfmt::skip]
pub const KEYMAP: [[Action; INPUTS]; OUTPUTS] = make_keymap![
    [Eql,    0,             1,             2,             3,             4],
    [Bsl,    Q,             W,             E,             R,             T],
    [Esc,    (MT LSf A),    (MT LSf S),    (MT LCl D),    (MT LCl F),    G],
    [LSf,    (MT LWn Z),    (MT LWn X),    (MT LAl C),    (MT LAl V),    B],
    [LWn,    Left,          Down,          Up,            Right,         Ins],
    [NOP,    NOP,           NOP,           NOP,           NOP,           NOP],
];

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn get_keys_all_unpressed() -> Result<(), ()> {
        let pressed: Vec<Vec<bool, 2>, 2> = Vec::from_slice(&[
            Vec::from_slice(&[false, false])?,
            Vec::from_slice(&[false, false])?,
        ])?;
        let mut keymap = Keymap {
            tap_duration: 0,
            state: [[State::default(); 2]; 2],
            layers: Vec::<Layer, 1>::new(),
            map: [make_keymap![[A, B], [C, D]]; 1],
        };
        let expected: Vec<Keyboard, 4> = Vec::from_slice(&[])?;
        assert_eq!(keymap.get_keys::<4>(pressed, 0), expected);
        Ok(())
    }

    #[test]
    fn get_keys_one_pressed() -> Result<(), ()> {
        let pressed: Vec<Vec<bool, 2>, 2> = Vec::from_slice(&[
            Vec::from_slice(&[false, true])?,
            Vec::from_slice(&[false, false])?,
        ])?;
        let mut keymap = Keymap {
            tap_duration: 0,
            state: [[State::default(); 2]; 2],
            layers: Vec::<Layer, 1>::new(),
            map: [make_keymap![[A, B], [C, D]]; 1],
        };
        let expected: Vec<Keyboard, 4> = Vec::from_slice(&[Keyboard::B])?;
        assert_eq!(keymap.get_keys::<4>(pressed, 0), expected);
        Ok(())
    }

    #[test]
    fn get_keys_all_pressed() -> Result<(), ()> {
        let pressed: Vec<Vec<bool, 2>, 2> = Vec::from_slice(&[
            Vec::from_slice(&[true, true])?,
            Vec::from_slice(&[true, true])?,
        ])?;
        let mut keymap = Keymap {
            tap_duration: 0,
            state: [[State::default(); 2]; 2],
            layers: Vec::<Layer, 1>::new(),
            map: [make_keymap![[A, B], [C, D]]; 1],
        };
        let expected: Vec<Keyboard, 4> =
            Vec::from_slice(&[Keyboard::A, Keyboard::B, Keyboard::C, Keyboard::D])?;
        assert_eq!(keymap.get_keys::<4>(pressed, 0), expected);
        Ok(())
    }

    #[test]
    fn get_keys_layer() -> Result<(), ()> {
        let pressed: Vec<Vec<bool, 2>, 1> = Vec::from_slice(&[Vec::from_slice(&[true, false])?])?;
        let mut keymap = Keymap {
            tap_duration: 0,
            state: [[State::default(); 2]; 1],
            layers: Vec::<Layer, 1>::new(),
            #[rustfmt::skip]
            map: make_keymap![
                [[(L 1), A,]],
                [[NOP, B,]],
            ],
        };
        let expected: Vec<Keyboard, 2> = Vec::from_slice(&[])?;
        let layers: Vec<Layer, 2> = Vec::from_slice(&[1])?;
        assert_eq!(keymap.get_keys::<2>(pressed, 0), expected);
        assert_eq!(keymap.layers, layers);

        let pressed: Vec<Vec<bool, 2>, 1> = Vec::from_slice(&[Vec::from_slice(&[true, true])?])?;
        let expected: Vec<Keyboard, 2> = Vec::from_slice(&[Keyboard::B])?;
        assert_eq!(keymap.get_keys::<2>(pressed, 0), expected);
        assert_eq!(keymap.layers, layers);

        let pressed: Vec<Vec<bool, 2>, 1> = Vec::from_slice(&[Vec::from_slice(&[false, true])?])?;
        let expected: Vec<Keyboard, 2> = Vec::from_slice(&[Keyboard::B])?;
        assert_eq!(keymap.get_keys::<2>(pressed, 0), expected);
        assert_eq!(keymap.layers, Vec::<Layer, 1>::new());

        let pressed: Vec<Vec<bool, 2>, 1> = Vec::from_slice(&[Vec::from_slice(&[false, false])?])?;
        let expected: Vec<Keyboard, 2> = Vec::from_slice(&[])?;
        assert_eq!(keymap.get_keys::<2>(pressed, 0), expected);
        assert_eq!(keymap.layers, Vec::<Layer, 1>::new());
        Ok(())
    }

    #[test]
    fn get_keys_mod_tap_tap() -> Result<(), ()> {
        let pressed: Vec<Vec<bool, 1>, 1> = Vec::from_slice(&[Vec::from_slice(&[true])?])?;
        let mut keymap = Keymap {
            tap_duration: 5,
            state: [[State::default(); 1]; 1],
            layers: Vec::<Layer, 1>::new(),
            #[rustfmt::skip]
            map: make_keymap![
                [[(MT A B)]],
            ],
        };
        let expected: Vec<Keyboard, 2> = Vec::from_slice(&[])?;
        assert_eq!(keymap.get_keys::<4>(pressed, 0), expected);

        let pressed: Vec<Vec<bool, 1>, 1> = Vec::from_slice(&[Vec::from_slice(&[false])?])?;
        let expected: Vec<Keyboard, 2> = Vec::from_slice(&[Keyboard::B])?;
        assert_eq!(keymap.get_keys::<4>(pressed, 1), expected);

        let pressed: Vec<Vec<bool, 1>, 1> = Vec::from_slice(&[Vec::from_slice(&[false])?])?;
        let expected: Vec<Keyboard, 2> = Vec::from_slice(&[])?;
        assert_eq!(keymap.get_keys::<4>(pressed, 2), expected);
        Ok(())
    }

    #[test]
    fn get_keys_mod_tap_mod() -> Result<(), ()> {
        let pressed: Vec<Vec<bool, 1>, 1> = Vec::from_slice(&[Vec::from_slice(&[true])?])?;
        let mut keymap = Keymap {
            tap_duration: 2,
            state: [[State::default(); 1]; 1],
            layers: Vec::<Layer, 1>::new(),
            #[rustfmt::skip]
            map: make_keymap![
                [[(MT A B)]],
            ],
        };
        let expected: Vec<Keyboard, 2> = Vec::from_slice(&[])?;
        assert_eq!(keymap.get_keys::<4>(pressed, 0), expected);

        let pressed: Vec<Vec<bool, 1>, 1> = Vec::from_slice(&[Vec::from_slice(&[true])?])?;
        let expected: Vec<Keyboard, 2> = Vec::from_slice(&[])?;
        assert_eq!(keymap.get_keys::<4>(pressed, 1), expected);

        let pressed: Vec<Vec<bool, 1>, 1> = Vec::from_slice(&[Vec::from_slice(&[true])?])?;
        let expected: Vec<Keyboard, 2> = Vec::from_slice(&[Keyboard::A])?;
        assert_eq!(keymap.get_keys::<4>(pressed, 3), expected);

        let pressed: Vec<Vec<bool, 1>, 1> = Vec::from_slice(&[Vec::from_slice(&[false])?])?;
        let expected: Vec<Keyboard, 2> = Vec::from_slice(&[])?;
        assert_eq!(keymap.get_keys::<4>(pressed, 4), expected);
        Ok(())
    }
}
