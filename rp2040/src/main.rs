#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

#[rtic::app(device = rp_pico::hal::pac, peripherals = true, dispatchers = [XIP_IRQ])]
mod app {
    use rp_pico as bsp;

    use rmk_mekk_elek::keymap::make_keymap;
    use rmk_mekk_elek::keymap::Action;
    use rmk_mekk_elek::keymap::Keymap;
    use rmk_mekk_elek::keymap::State;
    use rmk_mekk_elek::matrix::decode;

    const ROWS: usize = 6;
    const COLS: usize = 6;
    const LAYERS: usize = 2;

    // For alignment with `vi]:EasyAlign <C-r>4<CR>*,
    #[rustfmt::skip]
    pub const KEYMAP: [[[Action; COLS]; ROWS]; LAYERS] = make_keymap![
      [
        [Eql,    0,             1,             2,             3,             4],
        [Bsl,    Q,             W,             E,             R,             T],
        [Esc,    (MT LSf A),    (MT LSf S),    (MT LCl D),    (MT LCl F),    G],
        [LSf,    (MT LWn Z),    (MT LWn X),    (MT LAl C),    (MT LAl V),    B],
        [LWn,    Left,          Down,          Up,            Right,         (MT (L 1) Space)],
        [NOP,    NOP,           NOP,           NOP,           NOP,           NOP],
      ],
      [
        [F1,     F2,     F3,     F4,     F5,     F6],
        [NOP,    NOP,    NOP,    NOP,    NOP,    NOP],
        [NOP,    NOP,    NOP,    NOP,    NOP,    NOP],
        [NOP,    NOP,    NOP,    NOP,    NOP,    NOP],
        [NOP,    NOP,    NOP,    NOP,    NOP,    NOP],
        [NOP,    NOP,    NOP,    NOP,    NOP,    NOP],
      ],
    ];

    use bsp::{
        hal::gpio::pin::bank0::*,
        hal::gpio::pin::{FunctionI2C, Pin},
        hal::i2c::peripheral::{I2CEvent, I2CPeripheralEventIterator},
        hal::i2c::I2C,
        hal::{self, clocks::init_clocks_and_plls, watchdog::Watchdog, Sio},
        XOSC_CRYSTAL_FREQ,
    };
    use embedded_hal::blocking::i2c::Read;
    use embedded_hal::blocking::i2c::Write;
    use embedded_hal::digital::v2::*;
    use frunk::HList;
    use fugit::{ExtU64, RateExtU32};
    use heapless::Vec;
    use postcard::to_vec;
    use rp2040_monotonic::Rp2040Monotonic;
    use usb_device::class_prelude::*;
    use usb_device::prelude::*;
    use usbd_human_interface_device::device::keyboard::NKROBootKeyboardInterface;
    use usbd_human_interface_device::prelude::*;

    #[monotonic(binds = TIMER_IRQ_0, default = true)]
    type AppMonotonic = Rp2040Monotonic;
    type Instant = <Rp2040Monotonic as rtic::Monotonic>::Instant;
    type Duration = <Rp2040Monotonic as rtic::Monotonic>::Duration;

    #[shared]
    struct Shared {
        keyboard: UsbHidClass<
            hal::usb::UsbBus,
            HList!(NKROBootKeyboardInterface<'static, hal::usb::UsbBus>),
        >,
        usb_device: UsbDevice<'static, hal::usb::UsbBus>,
    }

    const I2C_PERIPHERAL_ADDRESS: u8 = 0x08;

    cfg_if::cfg_if! {
        if #[cfg(feature = "rhs")] {
            type I2CPins = (Pin<Gpio6, FunctionI2C>, Pin<Gpio7, FunctionI2C>);
        } else {
            type I2CPins = (Pin<Gpio26, FunctionI2C>, Pin<Gpio27, FunctionI2C>);
        }
    }

    #[local]
    struct Local {
        led: Pin<Gpio25, hal::gpio::PushPullOutput>,
        rows: Vec<hal::gpio::DynPin, ROWS>,
        cols: Vec<hal::gpio::DynPin, COLS>,
        keymap: Keymap<Instant, Duration, ROWS, COLS, LAYERS, LAYERS>,
        is_usb_connected: bool,
        i2c_peripheral: Option<I2CPeripheralEventIterator<bsp::pac::I2C1, I2CPins>>,
        i2c_controller: Option<I2C<bsp::pac::I2C1, I2CPins>>,
    }

    #[init(local = [usb_alloc: Option<UsbBusAllocator<hal::usb::UsbBus>> = None])]
    fn init(cx: init::Context) -> (Shared, Local, init::Monotonics) {
        // Soft-reset does not release the hardware spinlocks
        // Release them now to avoid a deadlock after debug or watchdog reset
        unsafe {
            hal::sio::spinlock_reset();
        }

        let mut resets = cx.device.RESETS;
        let mut watchdog = Watchdog::new(cx.device.WATCHDOG);
        let clocks = init_clocks_and_plls(
            XOSC_CRYSTAL_FREQ,
            cx.device.XOSC,
            cx.device.CLOCKS,
            cx.device.PLL_SYS,
            cx.device.PLL_USB,
            &mut resets,
            &mut watchdog,
        )
        .ok()
        .unwrap();

        let sio = Sio::new(cx.device.SIO);
        let pins = rp_pico::Pins::new(
            cx.device.IO_BANK0,
            cx.device.PADS_BANK0,
            sio.gpio_bank0,
            &mut resets,
        );
        let mut led = pins.led.into_push_pull_output();
        led.set_low().unwrap();

        cfg_if::cfg_if! {
            if #[cfg(feature = "rhs")] {
                let mut rows = Vec::<_, ROWS>::new();
                rows.extend([
                            pins.gpio10.into_push_pull_output().into(),
                            pins.gpio11.into_push_pull_output().into(),
                            pins.gpio12.into_push_pull_output().into(),
                            pins.gpio13.into_push_pull_output().into(),
                            pins.gpio14.into_push_pull_output().into(),
                            pins.gpio15.into_push_pull_output().into(),
                ]);

                let mut cols = Vec::<_, COLS>::new();
                cols.extend([
                            pins.gpio16.into_pull_down_input().into(),
                            pins.gpio17.into_pull_down_input().into(),
                            pins.gpio18.into_pull_down_input().into(),
                            pins.gpio19.into_pull_down_input().into(),
                            pins.gpio20.into_pull_down_input().into(),
                            pins.gpio21.into_pull_down_input().into(),
                ]);
            } else {
                let mut rows = Vec::<_, ROWS>::new();
                rows.extend([
                            pins.gpio16.into_push_pull_output().into(),
                            pins.gpio17.into_push_pull_output().into(),
                            pins.gpio18.into_push_pull_output().into(),
                            pins.gpio19.into_push_pull_output().into(),
                            pins.gpio20.into_push_pull_output().into(),
                            pins.gpio21.into_push_pull_output().into(),
                ]);

                let mut cols = Vec::<_, COLS>::new();
                cols.extend([
                            pins.gpio10.into_pull_down_input().into(),
                            pins.gpio11.into_pull_down_input().into(),
                            pins.gpio12.into_pull_down_input().into(),
                            pins.gpio13.into_pull_down_input().into(),
                            pins.gpio14.into_pull_down_input().into(),
                            pins.gpio15.into_pull_down_input().into(),
                ]);
            }
        }

        let mono = Rp2040Monotonic::new(cx.device.TIMER);

        // USB
        let usb_alloc = cx
            .local
            .usb_alloc
            .insert(UsbBusAllocator::new(hal::usb::UsbBus::new(
                cx.device.USBCTRL_REGS,
                cx.device.USBCTRL_DPRAM,
                clocks.usb_clock,
                true,
                &mut resets,
            )));

        let keyboard = UsbHidClassBuilder::new()
            .add_interface(NKROBootKeyboardInterface::default_config())
            .build(usb_alloc);

        // https://pid.codes
        let usb_device = UsbDeviceBuilder::new(usb_alloc, UsbVidPid(0x1209, 0x0001))
            .manufacturer("usbd-human-interface-device")
            .product("Keyboard")
            .serial_number("TEST")
            .build();

        // Enable the USB interrupt
        unsafe {
            bsp::pac::NVIC::unmask(hal::pac::Interrupt::USBCTRL_IRQ);
        };

        let is_usb_connected = pins.vbus_detect.into_floating_input().is_high().unwrap();
        defmt::info!("USB connected?: {}", is_usb_connected);

        let now = monotonics::now();

        if is_usb_connected {
            tick::spawn(now).unwrap();
            write_keyboard::spawn(now).unwrap();

            let keymap = Keymap {
                tap_duration: 200.millis(),
                state: [[State::default(); ROWS]; COLS],
                layers: Vec::new(),
                map: KEYMAP,
            };

            cfg_if::cfg_if! {
                if #[cfg(feature = "rhs")] {
                    let sda_pin = pins.gpio6.into_mode::<FunctionI2C>();
                    let scl_pin = pins.gpio7.into_mode::<FunctionI2C>();
                } else {
                    let sda_pin = pins.gpio26.into_mode::<FunctionI2C>();
                    let scl_pin = pins.gpio27.into_mode::<FunctionI2C>();
                }
            }
            let i2c_peripheral = Some(I2C::new_peripheral_event_iterator(
                cx.device.I2C1,
                sda_pin,
                scl_pin,
                &mut resets,
                I2C_PERIPHERAL_ADDRESS as u16,
            ));

            // Enable the I2C interrupt
            unsafe {
                bsp::pac::NVIC::unmask(bsp::pac::interrupt::I2C1_IRQ);
            };

            (
                Shared {
                    keyboard,
                    usb_device,
                },
                Local {
                    led,
                    rows,
                    cols,
                    keymap,
                    is_usb_connected,
                    i2c_peripheral,
                    i2c_controller: None,
                },
                init::Monotonics(mono),
            )
        } else {
            write_i2c::spawn(now).unwrap();

            let keymap = Keymap {
                tap_duration: 200.millis(),
                state: [[State::default(); ROWS]; COLS],
                layers: Vec::new(),
                map: KEYMAP,
            };

            cfg_if::cfg_if! {
                if #[cfg(feature = "rhs")] {
                    let sda_pin = pins.gpio6.into_mode::<FunctionI2C>();
                    let scl_pin = pins.gpio7.into_mode::<FunctionI2C>();
                } else {
                    let sda_pin = pins.gpio26.into_mode::<FunctionI2C>();
                    let scl_pin = pins.gpio27.into_mode::<FunctionI2C>();
                }
            }
            let i2c_controller = Some(I2C::i2c1(
                cx.device.I2C1,
                sda_pin,
                scl_pin,
                100.kHz(),
                &mut resets,
                &clocks.peripheral_clock,
            ));

            (
                Shared {
                    keyboard,
                    usb_device,
                },
                Local {
                    led,
                    rows,
                    cols,
                    keymap,
                    is_usb_connected,
                    i2c_peripheral: None,
                    i2c_controller,
                },
                init::Monotonics(mono),
            )
        }
    }

    #[task(
        shared = [keyboard],
    )]
    fn tick(mut cx: tick::Context, scheduled: Instant) {
        cx.shared.keyboard.lock(|k| match k.interface().tick() {
            Err(UsbHidError::WouldBlock) => {}
            Ok(_) => {}
            Err(e) => {
                core::panic!("Failed to process keyboard tick: {:?}", e)
            }
        });

        let next = scheduled + 1.millis();
        tick::spawn_at(next, next).unwrap();
    }

    #[task(
        shared = [keyboard],
        local = [i2c_controller, is_usb_connected, rows, cols, keymap]
    )]
    fn write_keyboard(mut cx: write_keyboard::Context, scheduled: Instant) {
        let pressed = decode(cx.local.cols, cx.local.rows, true).unwrap();
        if *cx.local.is_usb_connected {
            let keys = cx.local.keymap.get_keys::<36>(pressed, scheduled);
            cx.shared
                .keyboard
                .lock(|k| match k.interface().write_report(keys.iter()) {
                    Err(UsbHidError::WouldBlock) => {}
                    Err(UsbHidError::Duplicate) => {}
                    Ok(_) => {}
                    Err(e) => {
                        core::panic!("Failed to write keyboard report: {:?}", e)
                    }
                });
        } else if let Some(i2c) = cx.local.i2c_controller {
            let mut data = to_vec(&pressed).unwrap();
            let write = i2c.write(I2C_PERIPHERAL_ADDRESS, data.as_slice());
            match write {
                Ok(val) => defmt::info!("Ok({:?}); data = {:?}", val, data),
                Err(e) => defmt::info!("Err({})", e),
            }
        }
        let next = scheduled + 50.millis();
        write_keyboard::spawn_at(next, next).unwrap();
    }

    #[task(
        binds = USBCTRL_IRQ,
        shared = [keyboard, usb_device],
        local = [led]
    )]
    fn usb_irq(cx: usb_irq::Context) {
        (cx.shared.keyboard, cx.shared.usb_device).lock(|keyboard, usb_device| {
            if usb_device.poll(&mut [keyboard]) {
                let interface = keyboard.interface();
                match interface.read_report() {
                    Err(UsbError::WouldBlock) => {}
                    Err(e) => {
                        core::panic!("Failed to read keyboard report: {:?}", e)
                    }
                    Ok(leds) => cx
                        .local
                        .led
                        .set_state(PinState::from(leds.num_lock))
                        .unwrap(),
                }
            }
        })
    }

    #[task(binds = I2C1_IRQ, local = [
           i2c_peripheral,
           data: [u8; 1] = [0]
    ])]
    fn i2c1_irq(cx: i2c1_irq::Context) {
        if let Some(i2c) = cx.local.i2c_peripheral {
            match i2c.next() {
                Some(I2CEvent::TransferRead) => {
                    defmt::info!("I2CEvent::TransferRead");
                    i2c.write(cx.local.data);
                }
                Some(I2CEvent::TransferWrite) => {
                    defmt::info!("I2CEvent::TransferWrite");
                    i2c.write(cx.local.data);
                }
                Some(I2CEvent::Start) => defmt::info!("I2CEvent::Start"),
                Some(I2CEvent::Restart) => defmt::info!("I2CEvent::Restart"),
                Some(I2CEvent::Stop) => {
                    defmt::info!("I2CEvent::Stop");
                    cx.local.data[0] = cx.local.data[0] + 1;
                }
                None => (),
            }
        }
    }
}
