macro_rules! rows {
    ($pins:expr) => {{
        let mut rows = Vec::<_, ROWS>::new();
        rows.extend([
            $pins.gpio16.into_push_pull_output().into(),
            $pins.gpio17.into_push_pull_output().into(),
            $pins.gpio18.into_push_pull_output().into(),
            $pins.gpio19.into_push_pull_output().into(),
            $pins.gpio20.into_push_pull_output().into(),
            $pins.gpio21.into_push_pull_output().into(),
        ]);
        rows
    }};
}

macro_rules! cols {
    ($pins:expr) => {{
        let mut cols = Vec::<_, COLS>::new();
        cols.extend([
            $pins.gpio10.into_pull_down_input().into(),
            $pins.gpio11.into_pull_down_input().into(),
            $pins.gpio12.into_pull_down_input().into(),
            $pins.gpio13.into_pull_down_input().into(),
            $pins.gpio14.into_pull_down_input().into(),
            $pins.gpio15.into_pull_down_input().into(),
        ]);
        cols
    }};
}

macro_rules! i2c {
    ($pins:expr , $resets:expr, $I2C:expr) => {{
        let sda_pin = $pins.gpio6.into_mode::<FunctionI2C>();
        let scl_pin = $pins.gpio7.into_mode::<FunctionI2C>();
        I2CDevice::Peripheral(I2C::new_peripheral_event_iterator(
            $I2C,
            sda_pin,
            scl_pin,
            &mut $resets,
            I2C_PERIPHERAL_ADDRESS as u16,
        ))
    }};
}
