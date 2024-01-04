// For alignment with `vi]:EasyAlign <C-r>4<CR>*,
use rmk_mekk_elek::keystate::Keymap;

pub const ROWS: usize = 6;
pub const COLS: usize = 6;
pub const SIZE: usize = ROWS * COLS;
pub const LAYERS: usize = 2;

pub type KeymapT = Keymap<SIZE, LAYERS>;

use fugit::ExtU64;
use rp2040_monotonic::Rp2040Monotonic;
type Duration = <Rp2040Monotonic as rtic::Monotonic>::Duration;

pub fn keymap() -> KeymapT {
    use rmk_mekk_elek::keystate::prelude::*;
    let mod_timeout: Duration = 200.millis();
    let tap_release: Duration = 100.millis();
    let tap_repeat: Duration = 500.millis();

    #[rustfmt::skip]
    let ret_statement = Keymap::new([[
            Kb(Equal),   Kb(K0),      Kb(K1),      Kb(K2),      Kb(K3),      Kb(K4),
            Kb(BSL),     Kb(Q),       Kb(W),       Kb(E),       Kb(R),       Kb(T),
            Kb(Escape),  MT(LSFT, A), MT(LSFT, S), MT(LCTL, D), MT(LCTL, F), Kb(G),
            Kb(LSFT),    MT(LWIN, Z), MT(LWIN, X), MT(LALT, C), MT(LALT, V), Kb(B),
            Kb(LWIN),    Kb(LEFT),    Kb(DOWN),    Kb(UP),      Kb(RIGHT),   Kb(Space) /* (MT (L 1) SPACE)*/,
            Kb(___),     Kb(___),     Kb(___),     Kb(___),     Kb(___),     Kb(___),
        ], [
            Kb(F1),     Kb(F2),     Kb(F3),     Kb(F4),     Kb(F5),     Kb(F6),
            Kb(___),    Kb(___),    Kb(___),    Kb(___),    Kb(___),    Kb(___),
            Kb(___),    Kb(___),    Kb(___),    Kb(___),    Kb(___),    Kb(___),
            Kb(___),    Kb(___),    Kb(___),    Kb(___),    Kb(___),    Kb(___),
            Kb(___),    Kb(___),    Kb(___),    Kb(___),    Kb(___),    Kb(___),
            Kb(___),    Kb(___),    Kb(___),    Kb(___),    Kb(___),    Kb(___),
        ]],
        mod_timeout.ticks(), tap_release.ticks(), tap_repeat.ticks());
    ret_statement
}
