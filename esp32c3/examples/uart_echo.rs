//! uart_echo
//!
//! Run on target:
//!
//! cargo embed --example uart_echo
//!
//! Run on host:
//!
//! minicom -b 115200 -D /dev/ttyACM1
//!
//! or
//!
//! moserial -p moserial_acm1.cfg
//!
//! Echoes incoming data + 1 (a->b, etc.)
//!
//! This assumes we have usb<->serial adepter appearing as /dev/ACM1
//! - Target TX = GPIO0, connect to RX on adapter
//! - Target RX = GPIO1, connect to TX on adapter
//!

#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

// bring in panic handler
use panic_rtt_target as _;

#[rtic::app(device = esp32c3)]
mod app {
    use core::fmt::Write;
    use esp32c3_hal::{
        clock::ClockControl,
        peripherals::{Peripherals, TIMG0, UART0},
        prelude::*,
        timer::{Timer, Timer0, TimerGroup},
        uart::{
            config::{Config, DataBits, Parity, StopBits},
            TxRxPins,
        },
        Uart, IO,
    };
    use nb::block;
    use rtt_target::{rprint, rprintln, rtt_init_print};

    #[shared]
    struct Shared {
        uart0: Uart<'static, UART0>,
    }

    #[local]
    struct Local {
        timer0: Timer<Timer0<TIMG0>>,
    }

    #[init]
    fn init(_: init::Context) -> (Shared, Local) {
        rtt_init_print!();
        rprintln!("uart_echo");

        let peripherals = Peripherals::take();
        let mut system = peripherals.SYSTEM.split();
        let clocks = ClockControl::max(system.clock_control).freeze();

        let timer_group0 = TimerGroup::new(
            peripherals.TIMG0,
            &clocks,
            &mut system.peripheral_clock_control,
        );
        let mut timer0 = timer_group0.timer0;

        let config = Config {
            baudrate: 115200,
            data_bits: DataBits::DataBits8,
            parity: Parity::ParityNone,
            stop_bits: StopBits::STOP1,
        };

        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
        let pins = TxRxPins::new_tx_rx(
            io.pins.gpio0.into_push_pull_output(),
            io.pins.gpio1.into_floating_input(),
        );

        let mut uart0 = Uart::new_with_config(
            peripherals.UART0,
            config,
            Some(pins),
            &clocks,
            &mut system.peripheral_clock_control,
        );

        // This is stupid!
        // TODO, can we have interrupts after timeout even if threshold > 1?
        uart0.set_rx_fifo_full_threshold(1).unwrap();
        uart0.listen_rx_fifo_full();

        timer0.start(1u64.secs());

        (Shared { uart0 }, Local { timer0 })
    }

    #[idle(local = [timer0], shared = [uart0])]
    fn idle(mut cx: idle::Context) -> ! {
        loop {
            cx.shared.uart0.lock(|uart0| {
                writeln!(uart0, "Hello to Finland from Esp32C3!").unwrap();
            });
            block!(cx.local.timer0.wait()).unwrap();
        }
    }

    #[task(binds = UART0, priority=1, shared=[uart0])]
    fn uart0(mut cx: uart0::Context) {
        rprint!("Interrupt Received: ");
        cx.shared.uart0.lock(|uart0| {
            while let nb::Result::Ok(c) = uart0.read() {
                writeln!(uart0, "Read byte: {:02x}", c).unwrap();
                rprint!("{}", c as char);
            }
            uart0.reset_rx_fifo_full_interrupt()
        });
    }
}
