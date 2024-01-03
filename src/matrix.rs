//! Decodes a keyboard matrix

use embedded_hal::digital::v2::{InputPin, OutputPin};
use heapless::Vec;

pub fn decode<
    E,
    InputPinT: InputPin<Error = E>,
    OutputPinT: OutputPin<Error = E>,
    const INPUTS: usize,
    const OUTPUTS: usize,
    const SIZE: usize,
>(
    inputs: &mut Vec<InputPinT, INPUTS>,
    outputs: &mut Vec<OutputPinT, OUTPUTS>,
    keys: &mut [bool; SIZE],
    output_active: bool,
) -> Result<(), E> {
    for output in outputs.iter_mut() {
        output.set_state((!output_active).into())?;
    }

    for (o, output) in outputs.iter_mut().enumerate() {
        output.set_state(output_active.into())?;
        for (i, input) in inputs.iter_mut().enumerate() {
            keys[o * OUTPUTS + i] = input.is_high()? == output_active;
        }
        output.set_state((!output_active).into())?;
    }
    Ok(())
}
