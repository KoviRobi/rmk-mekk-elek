#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

mod matrix;

#[rtic::app(device = rp_pico::hal::pac, peripherals = true, dispatchers = [XIP_IRQ])]
mod app {

    use rp_pico as bsp;

    use rmk_mekk_elek::keymap::make_keymap;
    use rmk_mekk_elek::keymap::Action;
    use rmk_mekk_elek::keymap::Keymap;
    use rmk_mekk_elek::keymap::State;

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

    use super::matrix::decode;

    use bsp::{
        hal::{self, clocks::init_clocks_and_plls, watchdog::Watchdog, Sio},
        XOSC_CRYSTAL_FREQ,
    };
    use embedded_hal::digital::v2::*;
    use frunk::HList;
    use fugit::ExtU64;
    use heapless::Vec;
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

    #[local]
    struct Local {
        led: hal::gpio::Pin<hal::gpio::pin::bank0::Gpio25, hal::gpio::PushPullOutput>,
        rows: Vec<hal::gpio::DynPin, ROWS>,
        cols: Vec<hal::gpio::DynPin, COLS>,
        keymap: Keymap<Instant, Duration, 6, 6, 2, 2>,
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
        let usb_conn = pins.vbus_detect.into_floating_input();
        defmt::info!("USB input: {}", usb_conn.is_high());
        led.set_low().unwrap();

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

        let now = monotonics::now();
        tick::spawn(now).unwrap();
        write_keyboard::spawn(now).unwrap();

        let keymap = Keymap {
            tap_duration: 200.millis(),
            state: [[State::default(); ROWS]; COLS],
            layers: Vec::new(),
            map: KEYMAP,
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
            },
            init::Monotonics(mono),
        )
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
        local = [rows, cols, keymap]
    )]
    fn write_keyboard(mut cx: write_keyboard::Context, scheduled: Instant) {
        cx.shared.keyboard.lock(|k| {
            let mut rows: Vec<_, ROWS> = cx
                .local
                .rows
                .into_iter()
                .map(|pin| pin as &mut dyn OutputPin<Error = _>)
                .collect();
            let mut cols: Vec<_, COLS> = cx
                .local
                .cols
                .into_iter()
                .map(|pin| pin as &mut dyn InputPin<Error = _>)
                .collect();
            let pressed = decode(&mut cols, &mut rows, true).unwrap();
            let keys = cx.local.keymap.get_keys::<36>(pressed, scheduled);
            match k.interface().write_report(keys.iter()) {
                Err(UsbHidError::WouldBlock) => {}
                Err(UsbHidError::Duplicate) => {}
                Ok(_) => {}
                Err(e) => {
                    core::panic!("Failed to write keyboard report: {:?}", e)
                }
            }
        });

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
}
