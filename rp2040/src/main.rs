#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use core::convert::Infallible;
use core::pin::{pin as stack_pin, Pin as PtrPin};

mod keymap;
use keymap::{keymap, KeymapT, COLS, ROWS, SIZE};

use lilos::create_list;
use lilos::create_mutex;
use lilos::exec::{run_tasks_with_idle, Notify, ALL_TASKS};
use lilos::mutex::Mutex;
use lilos::time::Timer as _;

mod lilos_support;
use lilos_support::fifo::reset_read_fifo;
use lilos_support::timer::{make_idle_task, now, Instant, Timer};

use rp_pico as bsp;

use bsp::entry;
use bsp::{hal, hal::pac};
use hal::fugit::{self, ExtU64};
use hal::gpio::{bank0::*, DynPinId, FunctionSioInput, FunctionSioOutput, Pin, PullDown, PullNone};
use hal::multicore::{Multicore, Stack};
use hal::Clock;
use hal::Sio;
use pac::interrupt;

use embedded_hal::digital::v2::{InputPin, OutputPin};

use hal::usb::UsbBus as Rp2040Usb;
use usb_device::class_prelude::*;
use usb_device::prelude::*;
use usbd_human_interface_device::device::keyboard::{NKROBootKeyboard, NKROBootKeyboardConfig};
use usbd_human_interface_device::prelude::*;

use heapless::Vec;

use rmk_mekk_elek::debounce::SchmittDebouncer;
use rmk_mekk_elek::matrix::decode;

use panic_probe as _;

static mut CORE1_STACK: Stack<4096> = Stack::new();

type KeyboardDev<'a> = frunk::HCons<NKROBootKeyboard<'a, Rp2040Usb>, frunk::HNil>;
static USB_EVT: Notify = Notify::new();

defmt::timestamp!("{:us}", { now() });

#[entry]
fn core0() -> ! {
    let mut core = pac::CorePeripherals::take().unwrap();
    let mut pac = pac::Peripherals::take().unwrap();
    let mut sio = Sio::new(pac.SIO);

    // Set up the watchdog driver - needed by the clock setup code
    let mut watchdog = hal::watchdog::Watchdog::new(pac.WATCHDOG);
    // Configure the clocks
    let clocks = hal::clocks::init_clocks_and_plls(
        bsp::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();
    let sys_clk = clocks.system_clock.freq();

    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let usb_conn = pins.vbus_detect.into_floating_input();
    defmt::info!("USB input: {}", usb_conn.is_high());

    let mut rows = Vec::<_, ROWS>::new();
    rows.extend([
        pins.gpio16.reconfigure().into_dyn_pin(),
        pins.gpio17.reconfigure().into_dyn_pin(),
        pins.gpio18.reconfigure().into_dyn_pin(),
        pins.gpio19.reconfigure().into_dyn_pin(),
        pins.gpio20.reconfigure().into_dyn_pin(),
        pins.gpio21.reconfigure().into_dyn_pin(),
    ]);

    let mut cols = Vec::<_, COLS>::new();
    cols.extend([
        pins.gpio10.reconfigure().into_dyn_pin(),
        pins.gpio11.reconfigure().into_dyn_pin(),
        pins.gpio12.reconfigure().into_dyn_pin(),
        pins.gpio13.reconfigure().into_dyn_pin(),
        pins.gpio14.reconfigure().into_dyn_pin(),
        pins.gpio15.reconfigure().into_dyn_pin(),
    ]);

    create_mutex!(keymap_mutex, keymap());

    let mut led = pins.led.reconfigure();

    let mut mc = Multicore::new(&mut pac.PSM, &mut pac.PPB, &mut sio.fifo);
    let cores = mc.cores();
    let _task = cores[1].spawn(unsafe { &mut CORE1_STACK.mem }, move || {
        core1(sys_clk, rows, cols, keymap_mutex);
    });

    // USB
    let usb_alloc = UsbBusAllocator::new(hal::usb::UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));

    create_mutex!(
        keyboard_mutex,
        UsbHidClassBuilder::new()
            .add_device(NKROBootKeyboardConfig::default())
            .build(&usb_alloc)
    );

    // https://pid.codes
    let mut usb_device = UsbDeviceBuilder::new(&usb_alloc, UsbVidPid(0x1209, 0x0001))
        .manufacturer("usbd-human-interface-device")
        .product("Keyboard")
        .serial_number("TEST")
        .build();

    create_list!(timer_list, Instant::from_ticks(0));
    let timer_list = timer_list.as_ref();
    let timer = Timer { timer_list };

    // Set up and run the scheduler with a single task.
    run_tasks_with_idle(
        &mut [
            stack_pin!(write_keyboard(&timer, keyboard_mutex, keymap_mutex)),
            stack_pin!(usb_irq(keyboard_mutex, &mut usb_device, &mut led)),
        ],
        ALL_TASKS,
        &timer,
        0,
        // We use `SEV` to signal from the other core that we can send more
        // data. See also the comment above on SEVONPEND
        cortex_m::asm::wfe,
    );
}

fn core1(
    sys_clk: fugit::Rate<u32, 1, 1>,
    rows: Vec<Pin<DynPinId, FunctionSioOutput, PullDown>, ROWS>,
    cols: Vec<Pin<DynPinId, FunctionSioInput, PullDown>, COLS>,
    keymap_mutex: PtrPin<&Mutex<KeymapT>>,
) -> ! {
    // Because both core's peripherals are mapped to the same address, this
    // is not necessary, but serves as a reminder that core 1 has its own
    // core peripherals
    // See also https://github.com/rust-embedded/cortex-m/issues/149
    let mut core = unsafe { pac::CorePeripherals::steal() };
    let pac = unsafe { pac::Peripherals::steal() };
    let mut sio = hal::Sio::new(pac.SIO);

    create_list!(timer_list, Instant::from_ticks(0));
    let timer_list = timer_list.as_ref();
    let timer = Timer { timer_list };
    let idle_task = make_idle_task(&mut core, timer_list, sys_clk.to_MHz());

    reset_read_fifo(&mut sio.fifo);

    run_tasks_with_idle(
        &mut [
            // core::pin::pin!()
        ], // <-- array of tasks
        ALL_TASKS, // <-- which to start initially
        &timer,
        1,
        idle_task,
    )
}

async fn scan<'a>(
    timer: &'a Timer<'a>,
    mut cols: Vec<Pin<DynPinId, FunctionSioInput, PullDown>, COLS>,
    mut rows: Vec<Pin<DynPinId, FunctionSioOutput, PullDown>, ROWS>,
    keymap_mutex: PtrPin<&'a Mutex<KeymapT>>,
) -> Infallible {
    let mut gate = lilos::time::PeriodicGate::new(timer, 1.millis());
    let mut debouncer: SchmittDebouncer<36, 1> = Default::default();

    loop {
        keymap_mutex.lock().await.perform(|keymap| {
            let mut pressed = decode(&mut cols, &mut rows, true)
                .unwrap()
                .into_iter()
                .flatten()
                .collect::<Vec<_, SIZE>>()
                .into_array()
                .unwrap();
            debouncer.debounce(&mut pressed);
            keymap.process(pressed, timer.now().ticks());
        });

        gate.next_time(timer).await;
    }
}

async fn tick<'a>(
    timer: &'a Timer<'a>,
    keyboard: &'a mut UsbHidClass<'a, Rp2040Usb, KeyboardDev<'a>>,
) -> Infallible {
    let mut gate = lilos::time::PeriodicGate::new(timer, 1.millis());

    loop {
        match keyboard.tick() {
            Err(UsbHidError::WouldBlock) => {}
            Ok(_) => {}
            Err(e) => {
                core::panic!("Failed to process keyboard tick: {:?}", e)
            }
        }

        gate.next_time(timer).await;
    }
}

async fn write_keyboard<'a>(
    timer: &'a Timer<'a>,
    keyboard_mutex: PtrPin<&Mutex<UsbHidClass<'a, Rp2040Usb, KeyboardDev<'a>>>>,
    keymap_mutex: PtrPin<&'a Mutex<KeymapT>>,
) -> Infallible {
    let mut gate = lilos::time::PeriodicGate::new(timer, 1.millis());

    loop {
        let keymap_permit = keymap_mutex.lock().await;
        let keyboard_permit = keyboard_mutex.lock().await;

        let report = keymap_permit.perform(|keymap| {
            let keys = keymap.pressed_keys.iter().cloned();
            keyboard_permit.perform(|keyboard| keyboard.device().write_report(keys))
        });

        match report {
            Err(UsbHidError::WouldBlock) => {}
            Err(UsbHidError::Duplicate) => {}
            Ok(_) => {}
            Err(e) => {
                core::panic!("Failed to write keyboard report: {:?}", e)
            }
        }

        gate.next_time(timer).await;
    }
}

async fn usb_irq<'a>(
    keyboard_mutex: PtrPin<&Mutex<UsbHidClass<'a, Rp2040Usb, KeyboardDev<'a>>>>,
    usb_device: &mut UsbDevice<'a, Rp2040Usb>,
    led: &mut Pin<Gpio25, FunctionSioOutput, PullNone>,
) -> Infallible {
    loop {
        unsafe { pac::NVIC::unmask(pac::Interrupt::USBCTRL_IRQ) };
        USB_EVT.until_next().await;
        keyboard_mutex.lock().await.perform(|keyboard| {
            if usb_device.poll(&mut [keyboard]) {
                let interface = keyboard.device();
                match interface.read_report() {
                    Err(UsbError::WouldBlock) => {}
                    Err(e) => {
                        core::panic!("Failed to read keyboard report: {:?}", e)
                    }
                    Ok(leds) => led.set_state(leds.num_lock.into()).unwrap(),
                }
            }
        });
    }
}

#[interrupt]
fn USBCTRL_IRQ() {
    USB_EVT.notify();
    pac::NVIC::mask(pac::Interrupt::USBCTRL_IRQ);
}

/*
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
*/
