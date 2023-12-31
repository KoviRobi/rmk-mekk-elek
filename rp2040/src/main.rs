#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

mod keymap;
use keymap::{keymap, KeymapT, COLS, ROWS, SIZE};

#[rtic::app(device = rp_pico::hal::pac, peripherals = true, dispatchers = [XIP_IRQ])]
mod app {

    use rp_pico as bsp;

    use rmk_mekk_elek::debounce::SchmittDebouncer;
    use rmk_mekk_elek::matrix::decode;

    use super::*;

    use bsp::{
        hal::gpio::bank0::*,
        hal::gpio::{DynPinId, FunctionSio, Pin, PullDown, SioInput, SioOutput},
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
    use usbd_human_interface_device::device::keyboard::{NKROBootKeyboard, NKROBootKeyboardConfig};
    use usbd_human_interface_device::prelude::*;

    #[monotonic(binds = TIMER_IRQ_0, default = true)]
    type AppMonotonic = Rp2040Monotonic;
    type Instant = <Rp2040Monotonic as rtic::Monotonic>::Instant;

    #[shared]
    struct Shared {
        keyboard: UsbHidClass<
            'static,
            hal::usb::UsbBus,
            HList!(NKROBootKeyboard<'static, hal::usb::UsbBus>),
        >,
        usb_device: UsbDevice<'static, hal::usb::UsbBus>,
    }

    #[local]
    struct Local {
        led: Pin<Gpio25, FunctionSio<SioOutput>, PullDown>,
        rows: Vec<Pin<DynPinId, FunctionSio<SioOutput>, PullDown>, ROWS>,
        cols: Vec<Pin<DynPinId, FunctionSio<SioInput>, PullDown>, COLS>,
        keymap: KeymapT,
        debouncer: SchmittDebouncer<SIZE, 10>,
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
            pins.gpio16.into_push_pull_output().into_dyn_pin(),
            pins.gpio17.into_push_pull_output().into_dyn_pin(),
            pins.gpio18.into_push_pull_output().into_dyn_pin(),
            pins.gpio19.into_push_pull_output().into_dyn_pin(),
            pins.gpio20.into_push_pull_output().into_dyn_pin(),
            pins.gpio21.into_push_pull_output().into_dyn_pin(),
        ]);

        let mut cols = Vec::<_, COLS>::new();
        cols.extend([
            pins.gpio10.into_pull_down_input().into_dyn_pin(),
            pins.gpio11.into_pull_down_input().into_dyn_pin(),
            pins.gpio12.into_pull_down_input().into_dyn_pin(),
            pins.gpio13.into_pull_down_input().into_dyn_pin(),
            pins.gpio14.into_pull_down_input().into_dyn_pin(),
            pins.gpio15.into_pull_down_input().into_dyn_pin(),
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
            .add_device(NKROBootKeyboardConfig::default())
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

        (
            Shared {
                keyboard,
                usb_device,
            },
            Local {
                led,
                rows,
                cols,
                keymap: keymap(),
                debouncer: Default::default(),
            },
            init::Monotonics(mono),
        )
    }

    #[task(
        shared = [keyboard],
    )]
    fn tick(mut cx: tick::Context, scheduled: Instant) {
        cx.shared.keyboard.lock(|k| match k.tick() {
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
        local = [rows, cols, keymap, debouncer],
    )]
    fn write_keyboard(mut cx: write_keyboard::Context, scheduled: Instant) {
        cx.shared.keyboard.lock(|k| {
            let mut pressed = decode(cx.local.cols, cx.local.rows, true)
                .unwrap()
                .into_iter()
                .flatten()
                .collect::<Vec<_, SIZE>>()
                .into_array()
                .unwrap();
            cx.local.debouncer.debounce(&mut pressed);
            cx.local.keymap.process(pressed, scheduled.ticks());
            match k
                .device()
                .write_report(cx.local.keymap.pressed_keys.iter().cloned())
            {
                Err(UsbHidError::WouldBlock) => {}
                Err(UsbHidError::Duplicate) => {}
                Ok(_) => {}
                Err(e) => {
                    core::panic!("Failed to write keyboard report: {:?}", e)
                }
            }
        });

        let next = scheduled + 1.millis();
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
                let interface = keyboard.device();
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
