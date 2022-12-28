//! Decodes a keyboard matrix

use embedded_hal::digital::v2::{InputPin, OutputPin};
use heapless::Vec;

pub fn decode<E, const INPUTS: usize, const OUTPUTS: usize>(
    inputs: &mut Vec<&mut dyn InputPin<Error = E>, INPUTS>,
    outputs: &mut Vec<&mut dyn OutputPin<Error = E>, OUTPUTS>,
    output_active: bool,
) -> Result<Vec<Vec<bool, INPUTS>, OUTPUTS>, E> {
    for output in outputs.iter_mut() {
        output.set_state((!output_active).into())?;
    }

    outputs
        .iter_mut()
        .map(move |output| {
            output.set_state(output_active.into())?;

            let result = inputs
                .iter_mut()
                .map(move |input| Ok(input.is_high()? == output_active))
                .collect();

            output.set_state((!output_active).into())?;

            result
        })
        .collect()
}
