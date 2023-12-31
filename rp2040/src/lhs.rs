use rp_pico as bsp;

use bsp::hal::gpio::pin::bank0::*;
use bsp::hal::gpio::{DynPin, FunctionI2C, Pin};
use bsp::hal::i2c::peripheral::I2CPeripheralEventIterator;
use bsp::hal::i2c::I2C;
use bsp::pac::Peripherals;
use bsp::Pins;
use heapless::Vec;

const I2C_PERIPHERAL_ADDRESS: u8 = 0x08;

pub type I2CPeripheral = I2CPeripheralEventIterator<
    bsp::pac::I2C1,
    (Pin<Gpio26, FunctionI2C>, Pin<Gpio27, FunctionI2C>),
>;

pub fn pins<const COLS: usize, const ROWS: usize>(
    pins: Pins,
    mut peripherals: Peripherals,
    rows: &mut Vec<DynPin, ROWS>,
    cols: &mut Vec<DynPin, COLS>,
    i2c_peripheral: &mut Option<I2CPeripheral>,
) {
    rows.extend([
        pins.gpio16.into_push_pull_output().into(),
        pins.gpio17.into_push_pull_output().into(),
        pins.gpio18.into_push_pull_output().into(),
        pins.gpio19.into_push_pull_output().into(),
        pins.gpio20.into_push_pull_output().into(),
        pins.gpio21.into_push_pull_output().into(),
    ]);

    cols.extend([
        pins.gpio10.into_pull_down_input().into(),
        pins.gpio11.into_pull_down_input().into(),
        pins.gpio12.into_pull_down_input().into(),
        pins.gpio13.into_pull_down_input().into(),
        pins.gpio14.into_pull_down_input().into(),
        pins.gpio15.into_pull_down_input().into(),
    ]);

    let sda_pin = pins.gpio26.into_mode::<FunctionI2C>();
    let scl_pin = pins.gpio27.into_mode::<FunctionI2C>();
    i2c_peripheral.replace(I2C::new_peripheral_event_iterator(
        peripherals.I2C1,
        sda_pin,
        scl_pin,
        &mut peripherals.RESETS,
        I2C_PERIPHERAL_ADDRESS as u16,
    ));
}
