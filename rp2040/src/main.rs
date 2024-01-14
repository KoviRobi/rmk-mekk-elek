#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use core::convert::Infallible;
use core::pin::{pin as stack_pin, Pin as PtrPin};

mod keymap;
use keymap::{keymap, COLS, ROWS, SIZE};

use lilos::exec::{run_tasks_with_idle, Notify, ALL_TASKS};
use lilos::mutex::Mutex;
use lilos::time::Timer as _;
use lilos::{create_list, create_mutex};

mod lilos_support;
use lilos_support::fifo::{reset_read_fifo, AsyncFifo};
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
use hal::sio::{Sio, SioFifo};
use hal::Clock;
use pac::interrupt;

use embedded_hal::digital::v2::{InputPin, OutputPin};

use hal::usb::UsbBus as Rp2040Usb;
use rp2040_selfdebug::{dap_execute_command, dap_setup, CmsisDap};
use usb_device::class_prelude::*;
use usb_device::prelude::*;
use usbd_human_interface_device::device::keyboard::{NKROBootKeyboard, NKROBootKeyboardConfig};
use usbd_human_interface_device::prelude::*;
use usbd_serial::SerialPort;

use heapless::Vec;

use rmk_mekk_elek::debounce::SchmittDebouncer;
use rmk_mekk_elek::keystate::Keyboard;
use rmk_mekk_elek::matrix::decode;

use panic_probe as _;

static mut CORE1_STACK: Stack<4096> = Stack::new();

type KeyboardDev<'a> = frunk::HCons<NKROBootKeyboard<'a, Rp2040Usb>, frunk::HNil>;
static USB_EVT: Notify = Notify::new();

defmt::timestamp!("{} {:us}", Sio::core(), now());

const ROLLOVER: usize = 36;

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

    let mut led = pins.led.reconfigure();
    let mut core0_idle_pin = pins.gpio2.reconfigure();
    let mut core1_idle_pin = pins.gpio3.reconfigure();
    let mut deb_pin = pins.gpio4.reconfigure();

    let mut mc = Multicore::new(&mut pac.PSM, &mut pac.PPB, &mut sio.fifo);
    let cores = mc.cores();
    let _task = cores[1].spawn(unsafe { &mut CORE1_STACK.mem }, move || {
        core1(
            sys_clk,
            &mut rows,
            &mut cols,
            Some(&mut core1_idle_pin),
            &mut deb_pin,
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

    let mut usb_serial = SerialPort::new(&usb_alloc);

    let mut usb_dap = CmsisDap::new(&usb_alloc);
    dap_setup(&pac.SYSCFG.dbgforce);

    // https://pid.codes
    let mut usb_device = UsbDeviceBuilder::new(&usb_alloc, UsbVidPid(0x1209, 0x0001))
        .manufacturer("usbd-human-interface-device")
        .product("Keyboard CMSIS-DAP")
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

    create_mutex!(keys_mutex, Vec::new());

    // Set up and run the scheduler with a single task.
    run_tasks_with_idle(
        &mut [
            stack_pin!(read_fifo(&mut sio.fifo, keys_mutex)),
            stack_pin!(tick(&timer, keyboard_mutex)),
            stack_pin!(write_keyboard(&timer, keyboard_mutex, keys_mutex)),
            stack_pin!(usb_irq(
                keyboard_mutex,
                &mut usb_device,
                &mut usb_serial,
                &mut usb_dap,
                &mut led
            )),
        ],
        ALL_TASKS,
        &timer,
        0,
        // We use `SEV` to signal from the other core that we can send more
        // data. See also the comment above on SEVONPEND
        idle_task,
    );
}

fn core1<'a, P: PinId, Q: PinId>(
    sys_clk: fugit::Rate<u32, 1, 1>,
    rows: &'a mut Vec<GpioPin<DynPinId, FunctionSioOutput, PullDown>, ROWS>,
    cols: &'a mut Vec<GpioPin<DynPinId, FunctionSioInput, PullDown>, COLS>,
    core1_idle_pin: Option<&'a mut GpioPin<P, FunctionSioOutput, PullNone>>,
    deb_pin: &'a mut GpioPin<Q, FunctionSioOutput, PullNone>,
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
        &mut [core::pin::pin!(scan(
            &timer,
            &mut sio.fifo,
            cols,
            rows,
            deb_pin
        ))],
        ALL_TASKS,
        &timer,
        1,
        idle_task,
    );
}

const KEYS_DONE: u32 = u32::MAX;

async fn scan<'a, P: PinId>(
    timer: &'a Timer<'a>,
    fifo: &'a mut SioFifo,
    cols: &'a mut Vec<GpioPin<DynPinId, FunctionSioInput, PullDown>, COLS>,
    rows: &'a mut Vec<GpioPin<DynPinId, FunctionSioOutput, PullDown>, ROWS>,
    deb_pin: &'a mut GpioPin<P, FunctionSioOutput, PullNone>,
) -> Infallible {
    let mut gate = lilos::time::PeriodicGate::new(timer, 1.millis());
    let mut debouncer: SchmittDebouncer<36, 1> = Default::default();
    let mut pressed = [false; COLS * ROWS];
    let mut keys: heapless::Vec<_, ROLLOVER> = Vec::new();
    let mut keymap = keymap();

    loop {
        decode(cols, rows, &mut pressed, true).unwrap();

        deb_pin.set_high().unwrap();
        debouncer.debounce(&mut pressed);
        deb_pin.set_low().unwrap();

        keymap.process(&pressed, &mut keys, timer.now().ticks());

        for key in &keys {
            fifo.write_async(Into::<u8>::into(*key) as u32).await;
        }
        fifo.write_async(KEYS_DONE).await;

        gate.next_time(timer).await;
    }
}

async fn read_fifo<'a>(
    fifo: &'a mut SioFifo,
    keys_mutex: PtrPin<&'a Mutex<Vec<Keyboard, SIZE>>>,
) -> Infallible {
    let mut new_keys = Vec::new();
    let new_keys_ref = &mut new_keys;

    loop {
        match fifo.read_async().await {
            KEYS_DONE => {
                keys_mutex
                    .lock()
                    .await
                    .perform(|keys| core::mem::swap(keys, new_keys_ref));
            }

            n => {
                assert!(n <= u8::MAX as u32);
                let key = Keyboard::from(n as u8);
                new_keys_ref.push(key).unwrap();
            }
        }
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
    keys_mutex: PtrPin<&'a Mutex<Vec<Keyboard, SIZE>>>,
) -> Infallible {
    let mut gate = lilos::time::PeriodicGate::new(timer, 1.millis());

    loop {
        let keys_permit = keys_mutex.lock().await;
        let keyboard_permit = keyboard_mutex.lock().await;

        let report = keys_permit.perform(|keys| {
            let keys = keys.iter().cloned();
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
    usb_serial: &mut SerialPort<'a, Rp2040Usb>,
    usb_dap: &mut CmsisDap<'a, Rp2040Usb, 64>,
    led: &mut GpioPin<Gpio25, FunctionSioOutput, PullNone>,
) -> Infallible {
    loop {
        unsafe { pac::NVIC::unmask(pac::Interrupt::USBCTRL_IRQ) };
        USB_EVT.until_next().await;

        keyboard_mutex.lock().await.perform(|keyboard| {
            if usb_device.poll(&mut [keyboard, usb_serial, usb_dap]) {
                let interface = keyboard.device();
                match interface.read_report() {
                    Err(UsbError::WouldBlock) => {}
                    Err(e) => {
                        core::panic!("Failed to read keyboard report: {:?}", e)
                    }
                    Ok(leds) => led.set_state(leds.num_lock.into()).unwrap(),
                }

                let mut buf = [0u8; 64];
                match usb_serial.read(&mut buf) {
                    Ok(0) => {}
                    Err(_) => {}
                    Ok(count) => {
                        for b in &buf[..count] {
                            if *b == b'r' {
                                bsp::hal::rom_data::reset_to_usb_boot(1 << 25, 0);
                            }
                        }
                    }
                }

                let mut buf = [0u8; 64];
                match usb_dap.read(&mut buf) {
                    Ok(0) => {}
                    Err(_) => {}
                    Ok(count) => {
                        let mut out = [0; 64];
                        let (_in_size, out_size) = dap_execute_command(&buf, &mut out);
                        let _ = usb_dap.write(&out[..out_size as usize]);
                    }
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
