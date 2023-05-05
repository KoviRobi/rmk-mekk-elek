#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

const ROWS: usize = 6;
const COLS: usize = 6;
const SIZE: usize = ROWS * COLS;
const LAYERS: usize = 2;
const ROLLOVER: usize = 32;
use rmk_mekk_elek::keystate::Keymap;
type KeymapT = Keymap<SIZE, LAYERS, ROLLOVER>;

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

#[rtic::app(device = rp_pico::hal::pac, peripherals = true, dispatchers = [XIP_IRQ])]
mod app {

    use rp_pico as bsp;

    use rmk_mekk_elek::matrix::decode;

    use super::*;

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
        keymap: KeymapT,
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
            let pressed = decode(cx.local.cols, cx.local.rows, true)
                .unwrap()
                .into_iter()
                .flatten()
                .collect::<Vec<_, SIZE>>()
                .into_array()
                .unwrap();
            cx.local.keymap.process(pressed, scheduled.ticks());
            match k
                .interface()
                .write_report(cx.local.keymap.pressed_keys.iter())
            {
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
