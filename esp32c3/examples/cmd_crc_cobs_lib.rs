//! cmd_crc_cobs_lib
//!
//! Run on target: `cd esp32c3`
//!
//! cargo embed --example cmd_crc_cobs_lib --release
//!
//! Run on host: `cd host`
//! cargo run --example cmd_crc_cobs_lib
//!
//! Demonstrates simple command transactions using ssmarshal + serde + crc + cobs
//!
#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

// bring in panic handler
use panic_rtt_target as _;

#[rtic::app(device = esp32c3, dispatchers = [FROM_CPU_INTR0, FROM_CPU_INTR1])]
mod app {
    // Backend dependencies
    use esp32c3_hal::{
        clock::ClockControl,
        peripherals::{Peripherals, TIMG0, UART0},
        prelude::*,
        timer::{Timer, Timer0, TimerGroup},
        uart::{
            config::{Config, DataBits, Parity, StopBits},
            TxRxPins, UartRx, UartTx,
        },
        Uart, IO,
    };

    // Application dependencies
    use core::mem::size_of;
    use corncobs::{max_encoded_len, ZERO};

    use shared::{deserialize_crc_cobs, serialize_crc_cobs, Command, Response};

    use rtic_sync::{channel::*, make_channel};
    use rtt_target::{rprint, rprintln, rtt_init_print};

    const CAPACITY: usize = 100;

    const IN_SIZE: usize = max_encoded_len(size_of::<Command>() + size_of::<u32>());
    const OUT_SIZE: usize = max_encoded_len(size_of::<Response>() + size_of::<u32>());

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        timer0: Timer<Timer0<TIMG0>>,
        tx: UartTx<'static, UART0>,
        rx: UartRx<'static, UART0>,
        sender: Sender<'static, u8, CAPACITY>,
    }
    #[init]
    fn init(_: init::Context) -> (Shared, Local) {
        rtt_init_print!();
        rprintln!("cmd_crc_cobs_lib");
        let (sender, receiver) = make_channel!(u8, CAPACITY);

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
        // TODO, use at commands with break character
        uart0.set_rx_fifo_full_threshold(1).unwrap();
        uart0.listen_rx_fifo_full();

        timer0.start(1u64.secs());

        let (tx, rx) = uart0.split();

        lowprio::spawn(receiver).unwrap();

        (
            Shared {},
            Local {
                timer0,
                tx,
                rx,
                sender,
            },
        )
    }

    // notice this is not an async task
    #[idle(local = [ timer0 ])]
    fn idle(cx: idle::Context) -> ! {
        loop {
            rprintln!("idle, do some background work if any ...");
            // not async wait
            nb::block!(cx.local.timer0.wait()).unwrap();
        }
    }

    #[task(binds = UART0, priority=2, local = [ rx, sender])]
    fn uart0(cx: uart0::Context) {
        let rx = cx.local.rx;
        let sender = cx.local.sender;

        rprint!("Interrupt Received: ");

        while let nb::Result::Ok(c) = rx.read() {
            rprint!("{}", c as char);
            match sender.try_send(c) {
                Err(_) => {
                    rprintln!("send buffer full");
                }
                _ => {}
            }
        }
        rprintln!(""); // just a new line

        rx.reset_rx_fifo_full_interrupt()
    }

    #[task(priority = 1, local = [ tx ])]
    async fn lowprio(cx: lowprio::Context, mut receiver: Receiver<'static, u8, CAPACITY>) {
        rprintln!("LowPrio started");

        let tx = cx.local.tx;

        let mut index: usize = 0;
        let mut in_buf: [u8; IN_SIZE] = [0u8; IN_SIZE];
        let mut out_buf: [u8; OUT_SIZE] = [0u8; OUT_SIZE];

        // Never ending process
        while let Ok(c) = receiver.recv().await {
            rprint!("Received {} {}", c, index);
            in_buf[index] = c;

            // ensure index in range
            if index < IN_SIZE - 1 {
                index += 1;
            }

            // end of cobs frame
            if c == ZERO {
                rprintln!("\n-- cobs packet received {:?} --", &in_buf[0..index]);
                index = 0;

                match deserialize_crc_cobs::<Command>(&mut in_buf) {
                    Ok(cmd) => {
                        rprintln!("cmd {:?}", cmd);
                        let response = match cmd {
                            Command::Set(_id, _par, _dev) => Response::SetOk,
                            Command::Get(id, par, dev) => Response::Data(id, par, 42, dev),
                        };
                        rprintln!("response {:?}", response);
                        let to_write = serialize_crc_cobs(&response, &mut out_buf);
                        for byte in to_write {
                            nb::block!(tx.write(*byte)).unwrap();
                        }
                    }

                    Err(err) => {
                        rprintln!("ssmarshal err {:?}", err);
                    }
                }
            }
        }
    }
}
