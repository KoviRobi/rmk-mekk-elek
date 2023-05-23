pub struct SchmittDebouncer<
    const KEYS: usize,
    const INCREMENT: u8,
    const LO_TO_HI: u8 = 155,
    const HI_TO_LO: u8 = 100,
> {
    key_value: [u8; KEYS],
    key_state: [bool; KEYS],
}

impl<const KEYS: usize, const INCREMENT: u8, const LO_TO_HI: u8, const HI_TO_LO: u8>
    SchmittDebouncer<KEYS, INCREMENT, LO_TO_HI, HI_TO_LO>
{
    pub fn new() -> Self {
        SchmittDebouncer {
            key_value: [0; KEYS],
            key_state: [false; KEYS],
        }
    }

    /// Returns if any keys had a change
    pub fn debounce(&mut self, presses: &mut [bool; KEYS]) -> bool {
        let mut changed = false;
        for (i, key) in presses.iter_mut().enumerate() {
            if *key {
                self.key_value[i] = self.key_value[i].saturating_add(INCREMENT)
            } else {
                self.key_value[i] = self.key_value[i].saturating_sub(INCREMENT)
            }

            let prev_state = self.key_state[i];

            if self.key_value[i] < HI_TO_LO {
                self.key_state[i] = false;
            } else if self.key_value[i] > LO_TO_HI {
                self.key_state[i] = true;
            }

            changed = changed || (prev_state != self.key_state[i]);

            *key = self.key_state[i];
        }
        changed
    }
}

impl<const KEYS: usize, const INCREMENT: u8, const LO_TO_HI: u8, const HI_TO_LO: u8> Default
    for SchmittDebouncer<KEYS, INCREMENT, LO_TO_HI, HI_TO_LO>
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn test_debouncer_up() {
        let mut debouncer = SchmittDebouncer::<1, 50, 155, 100> {
            key_value: [0],
            key_state: [false],
        };

        let mut presses = [true];
        debouncer.debounce(&mut presses);
        assert_eq!(debouncer.key_value, [50]);
        assert_eq!(presses, [false]);

        presses = [true];
        debouncer.debounce(&mut presses);
        assert_eq!(debouncer.key_value, [100]);
        assert_eq!(presses, [false]);

        presses = [true];
        debouncer.debounce(&mut presses);
        assert_eq!(debouncer.key_value, [150]);
        assert_eq!(presses, [false]);

        presses = [true];
        debouncer.debounce(&mut presses);
        assert_eq!(debouncer.key_value, [200]);
        assert_eq!(presses, [true]);
    }

    #[test]
    fn test_debouncer_down() {
        let mut debouncer = SchmittDebouncer::<1, 50, 155, 100> {
            key_value: [255],
            key_state: [false],
        };

        let mut presses = [false];
        debouncer.debounce(&mut presses);
        assert_eq!(debouncer.key_value, [205]);
        assert_eq!(presses, [true]);

        presses = [false];
        debouncer.debounce(&mut presses);
        assert_eq!(debouncer.key_value, [155]);
        assert_eq!(presses, [true]);

        presses = [false];
        debouncer.debounce(&mut presses);
        assert_eq!(debouncer.key_value, [105]);
        assert_eq!(presses, [true]);

        presses = [false];
        debouncer.debounce(&mut presses);
        assert_eq!(debouncer.key_value, [55]);
        assert_eq!(presses, [false]);
    }
}
