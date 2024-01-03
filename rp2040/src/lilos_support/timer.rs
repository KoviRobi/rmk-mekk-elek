/// Support for using the RP2040 timer in lilos
use core::pin::Pin;
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m_rt::exception;
use embedded_hal::digital::v2::OutputPin;
use lilos::list::List;
use rp2040_hal::fugit;
use rp2040_hal::gpio::{FunctionSioOutput, Pin as GpioPin, PinId, PullNone};
use rp2040_hal::pac;

pub type Instant = fugit::Instant<u64, 1, 1_000_000>;
pub type Duration = fugit::Duration<u64, 1, 1_000_000>;

/// We mostly just need to not enter an infinite loop, which is what the
/// `cortex_m_rt` does in `DefaultHandler`. But turning systick off until it's
/// needed can save some energy, especially if the reload value is small.
#[exception]
fn SysTick() {
    // Disable the counter, we enable it again when necessary
    // Safety: We are in the SysTick interrupt handler, having been woken up by
    // it, so shouldn't receive another systick interrupt here.
    unsafe {
        let syst = &*cortex_m::peripheral::SYST::PTR;
        const SYST_CSR_TICKINT: u32 = 1 << 1;
        syst.csr.modify(|v| v & !SYST_CSR_TICKINT);
    }
}

pub fn now() -> Instant {
    let timer = unsafe { &*pac::TIMER::ptr() };
    Instant::from_ticks(loop {
        let e = timer.timerawh.read().bits();
        let t = timer.timerawl.read().bits();
        let e2 = timer.timerawh.read().bits();
        if e == e2 {
            break ((e as u64) << 32) | (t as u64);
        }
    })
}

pub fn make_idle_task<'init, 'closure, P: PinId>(
    syst: &'closure mut cortex_m::peripheral::SYST,
    scb: &'init mut cortex_m::peripheral::SCB,
    timer_list: Pin<&'closure List<Instant>>,
    cycles_per_us: u32,
    mut idle_pin: Option<&'closure mut GpioPin<P, FunctionSioOutput, PullNone>>,
) -> impl FnMut() + 'closure {
    // Make it so that `wfe` waits for masked interrupts as well as events --
    // the problem is that the idle-task is called with interrupts disabled (to
    // not have an interrupt fire before we call the idle task but after we
    // check that we should sleep -- for `wfi` it would just wake up).
    // See
    // https://www.embedded.com/the-definitive-guide-to-arm-cortex-m0-m0-wake-up-operation/
    const SEVONPEND: u32 = 1 << 4;
    unsafe {
        scb.scr.modify(|scr| scr | SEVONPEND);
    }

    // 24-bit timer
    let max_sleep_us = ((1 << 24) - 1) / cycles_per_us;
    syst.set_clock_source(SystClkSource::Core);

    move || {
        match timer_list.peek() {
            Some(wake_at) => {
                let now = now();
                if wake_at > now {
                    let wake_in_us = u64::min(max_sleep_us as u64, (wake_at - now).to_micros());
                    let wake_in_ticks = wake_in_us as u32 * cycles_per_us;
                    // Setting zero to the reload register disables systick --
                    // systick is non-zero due to `wake_at > now`
                    syst.set_reload(wake_in_ticks);
                    syst.clear_current();
                    syst.enable_interrupt();
                    syst.enable_counter();
                    idle_pin.as_mut().map(|pin| pin.set_low());
                    // We use `SEV` to signal from the other core that we can
                    // send more data. See also the comment above on SEVONPEND
                    cortex_m::asm::wfe();
                    idle_pin.as_mut().map(|pin| pin.set_high());
                } else {
                    // We just missed a timer, don't idle
                }
            }
            None => {
                idle_pin.as_mut().map(|pin| pin.set_low());
                // We use `SEV` to signal from the other core that we can send
                // more data. See also the comment above on SEVONPEND
                cortex_m::asm::wfe();
                idle_pin.as_mut().map(|pin| pin.set_high());
            }
        }
    }
}

pub struct Timer<'a> {
    pub timer_list: Pin<&'a List<Instant>>,
}

impl<'a> lilos::time::Timer for Timer<'a> {
    type Instant = Instant;
    fn timer_list(&self) -> Pin<&'a List<Self::Instant>> {
        self.timer_list
    }

    fn now(&self) -> Self::Instant {
        now()
    }
}
