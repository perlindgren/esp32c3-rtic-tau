//! Blinks an LED
//!
//! This assumes that a LED is connected to the pin assigned to `led`. (GPIO7 for the ESP32c3-RUST DK)
//!
//! Run on target:
//!
//! cargo embed --example rtic_blinky
//!

#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

// bring in panic handler
use panic_rtt_target as _;

#[rtic::app(device = esp32c3)]
mod app {
    use esp32c3_hal::{clock::ClockControl, peripherals::Peripherals, prelude::*, Delay, IO};

    use rtt_target::{rprintln, rtt_init_print};

    #[shared]
    struct Shared {}

    #[local]
    struct Local {}

    #[init]
    fn init(_cx: init::Context) -> (Shared, Local) {
        rtt_init_print!();
        rprintln!("rtic_blinky");
        let peripherals = Peripherals::take();

        let system = peripherals.SYSTEM.split();
        let clocks = ClockControl::max(system.clock_control).freeze();

        // Set GPIO7 as an output, and set its state high initially.
        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
        let mut led = io.pins.gpio7.into_push_pull_output();

        led.set_high().unwrap();

        // Initialize the Delay peripheral, and use it to toggle the LED state in a loop.
        let mut delay = Delay::new(&clocks);

        let mut i = 0;

        loop {
            i = (i + 1) % 10;
            rprintln!("blink {}", i);

            led.toggle().unwrap();
            delay.delay_ms(500u32);
        }
    }
}
