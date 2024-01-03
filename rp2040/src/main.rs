#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use core::convert::Infallible;
use core::pin::{pin as stack_pin, Pin as PtrPin};

mod keymap;
use keymap::{keymap, KeymapT, COLS, ROWS, SIZE};

use lilos::exec::{run_tasks_with_idle, Notify, ALL_TASKS};
use lilos::mutex::Mutex;
use lilos::time::Timer as _;
use lilos::{create_list, create_mutex, create_static_mutex};

mod lilos_support;
use lilos_support::fifo::reset_read_fifo;
use lilos_support::timer::{make_idle_task, now, Instant, Timer};

use rp_pico as bsp;

use bsp::entry;
use bsp::{hal, hal::pac};
use hal::fugit::{self, ExtU64};
use hal::gpio::{
    bank0::*, DynPinId, FunctionSioInput, FunctionSioOutput, Pin as GpioPin, PinId, PullDown,
    PullNone,
};
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

defmt::timestamp!("{} {:us}", Sio::core(), now());

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

    // TODO: We need to use FIFO to have a mutex-free way so we could debug from
    // the other core
    let keymap_mutex = create_static_mutex!(KeymapT, keymap());

    let mut led = pins.led.reconfigure();
    let mut core0_idle_pin = pins.gpio2.reconfigure();
    let mut core1_idle_pin = pins.gpio3.reconfigure();

    let mut mc = Multicore::new(&mut pac.PSM, &mut pac.PPB, &mut sio.fifo);
    let cores = mc.cores();
    let _task = cores[1].spawn(unsafe { &mut CORE1_STACK.mem }, move || {
        core1(
            sys_clk,
            &mut rows,
            &mut cols,
            keymap_mutex,
            Some(&mut core1_idle_pin),
        );
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
    let idle_task = make_idle_task(
        &mut core.SYST,
        &mut core.SCB,
        timer_list,
        sys_clk.to_MHz(),
        Some(&mut core0_idle_pin),
    );

    // Set up and run the scheduler with a single task.
    run_tasks_with_idle(
        &mut [
            stack_pin!(tick(&timer, keyboard_mutex)),
            stack_pin!(write_keyboard(&timer, keyboard_mutex, keymap_mutex)),
            stack_pin!(usb_irq(keyboard_mutex, &mut usb_device, &mut led)),
        ],
        ALL_TASKS,
        &timer,
        0,
        // We use `SEV` to signal from the other core that we can send more
        // data. See also the comment above on SEVONPEND
        idle_task,
    );
}

fn core1<'a, P: PinId>(
    sys_clk: fugit::Rate<u32, 1, 1>,
    rows: &'a mut Vec<GpioPin<DynPinId, FunctionSioOutput, PullDown>, ROWS>,
    cols: &'a mut Vec<GpioPin<DynPinId, FunctionSioInput, PullDown>, COLS>,
    keymap_mutex: PtrPin<&Mutex<KeymapT>>,
    core1_idle_pin: Option<&'a mut GpioPin<P, FunctionSioOutput, PullNone>>,
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
    let idle_task = make_idle_task(
        &mut core.SYST,
        &mut core.SCB,
        timer_list,
        sys_clk.to_MHz(),
        core1_idle_pin,
    );

    reset_read_fifo(&mut sio.fifo);

    run_tasks_with_idle(
        &mut [core::pin::pin!(scan(&timer, cols, rows, keymap_mutex))],
        ALL_TASKS,
        &timer,
        1,
        idle_task,
    );
}

async fn scan<'a>(
    timer: &'a Timer<'a>,
    cols: &'a mut Vec<GpioPin<DynPinId, FunctionSioInput, PullDown>, COLS>,
    rows: &'a mut Vec<GpioPin<DynPinId, FunctionSioOutput, PullDown>, ROWS>,
    keymap_mutex: PtrPin<&'a Mutex<KeymapT>>,
) -> Infallible {
    let mut gate = lilos::time::PeriodicGate::new(timer, 1.millis());
    let mut debouncer: SchmittDebouncer<36, 1> = Default::default();

    loop {
        keymap_mutex.lock().await.perform(|keymap| {
            let mut pressed = decode(cols, rows, true)
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
    keyboard_mutex: PtrPin<&Mutex<UsbHidClass<'a, Rp2040Usb, KeyboardDev<'a>>>>,
) -> Infallible {
    let mut gate = lilos::time::PeriodicGate::new(timer, 1.millis());

    loop {
        match keyboard_mutex
            .lock()
            .await
            .perform(|keyboard| keyboard.tick())
        {
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
    led: &mut GpioPin<Gpio25, FunctionSioOutput, PullNone>,
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
