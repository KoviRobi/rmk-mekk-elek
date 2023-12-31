#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use core::pin::{pin, Pin};

mod keymap;
use keymap::{keymap, KeymapT, COLS, ROWS, SIZE};

mod lilos_support;
use lilos_support::fifo::{reset_read_fifo, AsyncFifo};
use lilos_support::timer::{make_idle_task, Duration, Instant, Timer};

use rp_pico as bsp;

use bsp::entry;
use bsp::{hal, hal::pac};
use hal::fugit::{self, ExtU64};
use hal::multicore::{Multicore, Stack};
use hal::Clock;
use hal::Sio;

use embedded_hal::digital::v2::ToggleableOutputPin;

use panic_probe as _;

static mut CORE1_STACK: Stack<4096> = Stack::new();

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

    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let mut mc = Multicore::new(&mut pac.PSM, &mut pac.PPB, &mut sio.fifo);
    let cores = mc.cores();
    let _task = cores[1].spawn(unsafe { &mut CORE1_STACK.mem }, move || {
        core1(sys_clk, pins);
    });

    let compute_delay = pin!(async {
        /// How much we adjust the LED period every cycle
        const INC: i32 = 2;
        /// The minimum LED toggle interval we allow for.
        const MIN: i32 = 0;
        /// The maximum LED toggle interval period we allow for. Keep it reasonably short so it's easy to see.
        const MAX: i32 = 100;
        loop {
            for period in (MIN..MAX).step_by(INC as usize) {
                sio.fifo.write_async(period as u32).await;
            }
            for period in (MIN..MAX).step_by(INC as usize).rev() {
                sio.fifo.write_async(period as u32).await;
            }
        }
    });

    lilos::create_list!(timer_list, Instant::from_ticks(0));
    let timer_list = timer_list.as_ref();
    let timer = Timer { timer_list };

    // Set up and run the scheduler with a single task.
    lilos::exec::run_tasks_with_idle(
        &mut [compute_delay],   // <-- array of tasks
        lilos::exec::ALL_TASKS, // <-- which to start initially
        &timer,
        0,
        // We use `SEV` to signal from the other core that we can send more
        // data. See also the comment above on SEVONPEND
        cortex_m::asm::wfe,
    )
}

fn core1(sys_clk: fugit::Rate<u32, 1, 1>, pins: hal::gpio::Pins) {
    // Because both core's peripherals are mapped to the same address, this
    // is not necessary, but serves as a reminder that core 1 has its own
    // core peripherals
    // See also https://github.com/rust-embedded/cortex-m/issues/149
    let mut core = unsafe { pac::CorePeripherals::steal() };
    let pac = unsafe { pac::Peripherals::steal() };
    let mut sio = hal::Sio::new(pac.SIO);

    lilos::create_list!(timer_list, Instant::from_ticks(0));
    let timer_list = timer_list.as_ref();
    let timer = Timer { timer_list };
    let idle_task = make_idle_task(&mut core, timer_list, sys_clk.to_MHz());

    reset_read_fifo(&mut sio.fifo);

    let mut led = pins.gpio25.into_push_pull_output();

    // Create a task to blink the LED. You could also write this as an `async
    // fn` but we've inlined it as an `async` block for simplicity.
    let blink = pin!(async {
        // Loop forever, blinking things. Note that this borrows the device
        // peripherals `p` from the enclosing stack frame.
        loop {
            let delay = sio.fifo.read_async().await as u64;
            lilos::time::sleep_for(&timer, delay.millis()).await;
            led.toggle().unwrap();
        }
    });

    lilos::exec::run_tasks_with_idle(
        &mut [blink],           // <-- array of tasks
        lilos::exec::ALL_TASKS, // <-- which to start initially
        &timer,
        1,
        idle_task,
    )
}
